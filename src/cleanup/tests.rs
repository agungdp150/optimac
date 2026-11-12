use super::*;
#[test]
fn inventory_preserves_data_counts_hardlinks_once_and_excludes_apple() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-inventory-{}", std::process::id()));
    fs::create_dir_all(home.join("Library/Caches/example")).unwrap();
    fs::create_dir_all(home.join("Library/Caches/com.apple.fixture")).unwrap();
    let file = home.join("Library/Caches/example/data");
    fs::write(&file, b"hello").unwrap();
    fs::hard_link(&file, home.join("Library/Caches/example/link")).unwrap();
    std::os::unix::fs::symlink(&home, home.join("Library/Caches/example/escape")).unwrap();
    let report = inventory(&home, &AtomicBool::new(false), Budget::quick()).unwrap();
    let r = report
        .providers
        .iter()
        .find(|r| r.provider == "app-user-caches/example")
        .unwrap();
    assert_eq!(r.logical_bytes, Some(5));
    assert_eq!(r.files, 1);
    assert_eq!(r.status, Status::Partial);
    assert!(
        !report
            .providers
            .iter()
            .any(|r| r.provider.contains("com.apple.fixture"))
    );
    assert!(
        report
            .providers
            .iter()
            .any(|r| r.status == Status::Absent && r.logical_bytes.is_none())
    );
    assert_eq!(fs::read(&file).unwrap(), b"hello");
    let cancelled = inventory(&home, &AtomicBool::new(true), Budget::quick()).unwrap();
    assert!(!cancelled.complete);
    assert!(
        cancelled
            .providers
            .iter()
            .any(|r| r.status == Status::Cancelled)
    );
    let limited = inventory(
        &home,
        &AtomicBool::new(false),
        Budget {
            entries: 0,
            ..Budget::quick()
        },
    )
    .unwrap();
    assert!(!limited.complete);
    fs::remove_dir_all(home).unwrap();
}
#[test]
fn specific_macos_caches_are_not_double_counted() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-specific-caches-{}", std::process::id()));
    for name in ["pip", "go-build", "Homebrew"] {
        let dir = home.join("Library/Caches").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("fixture"), b"cache").unwrap();
    }
    let report = inventory(&home, &AtomicBool::new(false), Budget::quick()).unwrap();
    for id in ["pip-cache", "go-build", "homebrew-cache"] {
        assert_eq!(
            report
                .providers
                .iter()
                .find(|r| r.provider == id)
                .unwrap()
                .logical_bytes,
            Some(5)
        );
    }
    for id in [
        "app-user-caches/pip",
        "app-user-caches/go-build",
        "app-user-caches/Homebrew",
    ] {
        assert!(!report.providers.iter().any(|r| r.provider == id));
    }
    fs::remove_dir_all(home).unwrap();
}
#[test]
fn root_validation_refuses_parent_traversal() {
    assert!(validate(Path::new("relative")).is_err());
    assert!(validate(Path::new("/tmp/../etc")).is_err());
}

#[test]
fn configured_scopes_exclude_before_traversal_and_reject_escape() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-configured-{}", std::process::id()));
    let scope = home.join("work");
    for dir in ["keep", "private", "omit.tmp"] {
        fs::create_dir_all(scope.join(dir)).unwrap();
        fs::write(scope.join(dir).join("data"), b"12345").unwrap();
    }
    let text = r#"schema_version=2
[scope]
file_roots=["~/work"]
exclude_paths=["~/work/private"]
[[rule]]
id="large-old-files"
enabled=true
scope="files"
scan_roots=["~/work"]
exclude_name_patterns=["*.tmp"]
"#;
    let config = config::Config::parse(text).unwrap();
    let result =
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &config).unwrap();
    assert_eq!(result.providers.len(), 1);
    assert_eq!(result.providers[0].logical_bytes, Some(5));
    assert_eq!(result.providers[0].status, Status::Complete);
    let bad = config::Config::parse(
        &text.replace("scan_roots=[\"~/work\"]", "scan_roots=[\"~/outside\"]"),
    )
    .unwrap();
    assert!(configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &bad).is_err());
    let disabled = config::Config::parse(&text.replace("enabled=true", "enabled=false")).unwrap();
    assert!(
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &disabled)
            .unwrap()
            .providers
            .is_empty()
    );
    assert!(scope.join("private/data").exists());
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn configured_budgets_are_applied() {
    let config=config::Config::parse("schema_version=2\n[scan]\nquick_max_entries_per_provider=1\nquick_max_seconds_per_provider=1\nquick_max_seconds_global=5\ndeep_max_entries_global=123\ndeep_max_entries_per_provider=42\ndeep_max_seconds_global=9").unwrap();
    let quick = config.scan_budget(Budget::quick()).unwrap();
    assert_eq!(quick.entries, 1);
    assert_eq!(quick.total.as_secs(), 5);
    let deep = config.scan_budget(Budget::deep()).unwrap();
    assert_eq!(deep.entries, 42);
    assert_eq!(deep.global_entries, 123);
    assert_eq!(deep.total.as_secs(), 9);
    assert!(config::Config::parse("schema_version=2\n[scan]\nquick_max_seconds_global=0").is_err());
}

#[test]
fn developer_environment_roots_are_explicit_and_safe() {
    let home = Path::new("/Users/fixture");
    let env = |key: &str| {
        if key == "CARGO_HOME" {
            Some("/Users/fixture/custom-cargo".into())
        } else {
            None
        }
    };
    assert_eq!(
        registry::resolve(home, "cargo-registry", ".cargo/registry/cache", env).unwrap(),
        PathBuf::from("/Users/fixture/custom-cargo/registry/cache")
    );
    assert!(
        registry::resolve(home, "pip-cache", "Library/Caches/pip", |_| Some(
            "../outside".into()
        ))
        .is_err()
    );
    assert_eq!(
        registry::resolve(home, "pip-cache", "Library/Caches/pip", |_| None).unwrap(),
        home.join("Library/Caches/pip")
    );
}

#[test]
fn advanced_storage_is_inventory_only() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-advanced-{}", std::process::id()));
    for dir in [
        "Library/Developer/CoreSimulator/Devices",
        "Library/Developer/Xcode/iOS DeviceSupport",
        "Library/Containers/com.docker.docker/Data/vms",
    ] {
        fs::create_dir_all(home.join(dir)).unwrap();
        fs::write(home.join(dir).join("fixture"), b"state").unwrap();
    }
    let report = inventory(&home, &AtomicBool::new(false), Budget::quick()).unwrap();
    for id in [
        "ios-simulator-devices",
        "xcode-device-support",
        "docker-desktop-storage",
    ] {
        let r = report.providers.iter().find(|r| r.provider == id).unwrap();
        assert_eq!(r.logical_bytes, Some(5), "{id}");
    }
    assert!(report.notes.iter().any(|s| s.contains("No Docker daemon")));
    assert!(
        home.join("Library/Developer/CoreSimulator/Devices/fixture")
            .exists()
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn configured_roots_use_environment_and_refuse_invalid_overrides() {
    let config =
        config::Config::parse("schema_version=2\n[[rule]]\nid='pip'\nenabled=true").unwrap();
    let home = Path::new("/Users/fixture");
    let jobs = config
        .jobs_with_env(home, |_| Some("/Users/fixture/custom-pip".into()))
        .unwrap();
    assert_eq!(
        jobs[0].roots,
        vec![PathBuf::from("/Users/fixture/custom-pip")]
    );
    assert!(jobs[0].unsupported.is_none());
    let jobs = config
        .jobs_with_env(home, |_| Some("../escape".into()))
        .unwrap();
    assert!(jobs[0].roots.is_empty());
    assert!(jobs[0].unsupported.is_some());
    let narrowed = config::Config::parse(
        "schema_version=2\n[[rule]]\nid='pip'\nenabled=true\nscan_roots=['~/Library/Caches/pip']",
    )
    .unwrap();
    assert!(
        narrowed
            .jobs_with_env(home, |_| Some("/Users/fixture/custom-pip".into()))
            .is_err()
    );
}

#[test]
fn catalog_never_certifies_proposed_operations() {
    let providers = catalog();
    assert!(providers.len() > 60);
    assert!(
        providers
            .iter()
            .all(|p| !p.mutation_supported && p.tested_versions.is_empty())
    );
    assert!(providers.iter().any(|p| p.provider == "homebrew"));
    assert!(
        providers
            .iter()
            .any(|p| p.provider == "ios-backups" && p.certification == "guidance_only")
    );
}

#[test]
fn fair_shares_reach_later_roots_and_emit_progress() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-fair-{}", std::process::id()));
    for name in ["large", "small"] {
        fs::create_dir_all(home.join(name)).unwrap();
    }
    for n in 0..100 {
        fs::write(home.join("large").join(n.to_string()), b"x").unwrap();
    }
    fs::write(home.join("small/item"), b"small").unwrap();
    let config = config::Config::parse("schema_version=2\n[scope]\nfile_roots=['~/large','~/small']\n[[rule]]\nid='large-old-files'\nenabled=true\nscope='files'").unwrap();
    let mut events = Vec::new();
    let result = configured_inventory_with_progress(
        &home,
        &AtomicBool::new(false),
        Budget {
            global_entries: 10,
            ..Budget::quick()
        },
        &config,
        &mut |r| events.push(r.root.clone()),
    )
    .unwrap();
    assert_eq!(
        result
            .providers
            .iter()
            .find(|r| r.root.ends_with("small"))
            .unwrap()
            .logical_bytes,
        Some(5)
    );
    assert!(!result.complete);
    assert!(events.iter().any(|p| p.ends_with("small")));
    assert!(result.providers.iter().all(|r| r.access == "readable"));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn project_discovery_requires_marker_and_preserves_source() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-projects-{}", std::process::id()));
    fs::create_dir_all(home.join("work/rust/target")).unwrap();
    fs::create_dir_all(home.join("work/unknown/target")).unwrap();
    fs::write(home.join("work/rust/Cargo.toml"), b"marker only").unwrap();
    fs::write(home.join("work/rust/target/output"), b"12345").unwrap();
    fs::write(home.join("work/unknown/target/personal"), b"keep").unwrap();
    let config = config::Config::parse("schema_version=2\n[scope]\nproject_roots=['~/work']\n[[rule]]\nid='project-artifacts'\nenabled=true\nscope='projects'").unwrap();
    let report =
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &config).unwrap();
    assert_eq!(report.providers.len(), 1);
    assert!(report.providers[0].root.ends_with("rust/target"));
    assert_eq!(report.providers[0].logical_bytes, Some(5));
    assert!(home.join("work/unknown/target/personal").exists());
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["read_only"], true);
    assert_eq!(json["providers"][0]["status"], "complete");
    assert!(
        json["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["mutation_supported"] == false)
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn invalid_schema_controls_fail_during_check() {
    for suffix in [
        "[scan]\nquick_max_entries_per_provider=0",
        "[ui]\ninventory_page_size=0",
        "[defaults]\nage_basis='not_applicable'",
        "[[rule]]\nid='docker-build-cache'\n[rule.docker]\ncontext=42\nallow_remote=false",
    ] {
        assert!(
            config::Config::parse(&format!("schema_version=2\n{suffix}")).is_err(),
            "{suffix}"
        );
    }
}

#[test]
fn preflight_reports_absent_and_retry_guidance_without_claiming_fda() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-preflight-{}", std::process::id()));
    fs::create_dir_all(&home).unwrap();
    let config = config::Config::parse("schema_version=2\n[scope]\nfile_roots=['~/missing']\n[[rule]]\nid='large-old-files'\nenabled=true\nscope='files'").unwrap();
    let report =
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &config).unwrap();
    assert_eq!(report.providers[0].status, Status::Absent);
    assert_eq!(report.providers[0].access, "absent");
    assert_eq!(report.providers[0].logical_bytes, None);
    assert!(report.render().contains("clean retry"));
    assert!(access::GUIDANCE.contains("cannot infer missing Full Disk Access"));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn configured_generic_caches_do_not_override_specific_provider() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-generic-{}", std::process::id()));
    for name in ["pip", "org.example.app", "com.apple.secret"] {
        fs::create_dir_all(home.join("Library/Caches").join(name)).unwrap();
        fs::write(
            home.join("Library/Caches").join(name).join("data"),
            b"12345",
        )
        .unwrap();
    }
    let config =
        config::Config::parse("schema_version=2\n[[rule]]\nid='app-user-caches'\nenabled=true")
            .unwrap();
    let report =
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &config).unwrap();
    assert!(
        report
            .providers
            .iter()
            .any(|r| r.provider.ends_with("org.example.app") && r.logical_bytes == Some(5))
    );
    assert!(report.providers.iter().any(|r| r.provider.ends_with("/pip")
        && r.status == Status::Blocked
        && r.logical_bytes.is_none()));
    assert!(
        !report
            .providers
            .iter()
            .any(|r| r.provider.contains("com.apple"))
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn preflight_denial_and_retry_with_disposable_root() {
    use std::os::unix::fs::PermissionsExt;
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-access-retry-{}", std::process::id()));
    let root = home.join("private");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("fixture"), b"data").unwrap();
    let config = config::Config::parse("schema_version=2\n[scope]\nfile_roots=['~/private']\n[[rule]]\nid='large-old-files'\nenabled=true\nscope='files'").unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
    let first =
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &config).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    if unsafe { libc::geteuid() } != 0 {
        assert_eq!(first.providers[0].status, Status::Denied);
        assert_eq!(first.providers[0].logical_bytes, None);
    }
    let retry =
        configured_inventory(&home, &AtomicBool::new(false), Budget::quick(), &config).unwrap();
    assert_eq!(retry.providers[0].status, Status::Complete);
    assert_eq!(retry.providers[0].logical_bytes, Some(4));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn root_state_change_after_preflight_is_rechecked() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-access-change-{}", std::process::id()));
    let root = home.join("changing");
    fs::create_dir_all(&root).unwrap();
    let config = config::Config::parse("schema_version=2\n[scope]\nfile_roots=['~/changing']\n[[rule]]\nid='large-old-files'\nenabled=true\nscope='files'").unwrap();
    let mut changed = false;
    let report = configured_inventory_with_progress(
        &home,
        &AtomicBool::new(false),
        Budget::quick(),
        &config,
        &mut |r| {
            if !changed && r.access == "readable" {
                fs::remove_dir(&root).unwrap();
                changed = true;
            }
        },
    )
    .unwrap();
    assert!(changed);
    assert_eq!(report.providers[0].status, Status::Absent);
    assert_eq!(report.providers[0].logical_bytes, None);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn summary_sorts_and_marks_incomplete_totals() {
    let rows = (0..20)
        .map(|n| ProviderReport {
            provider: format!("item-{n:02}"),
            root: PathBuf::from(format!("/fixture/{n}")),
            access: "readable".into(),
            status: if n == 19 {
                Status::Partial
            } else {
                Status::Complete
            },
            logical_bytes: Some(n),
            allocated_bytes: Some(n),
            files: 1,
            skipped: 0,
            errors: 0,
            note: if n == 19 {
                "Budget reached; measured bytes are a lower bound".into()
            } else {
                String::new()
            },
        })
        .collect();
    let report = Inventory {
        schema_version: 1,
        read_only: true,
        providers: rows,
        available_bytes: Some(50),
        home_used_bytes: Some(1000),
        complete: false,
        notes: vec!["fixture safety".into()],
        capabilities: vec![],
    };
    let compact = report.render_options(false, false);
    assert!(compact.starts_with("Summary\nMeasured logical:"));
    assert!(compact.contains("(lower bound)"));
    assert!(compact.find("item-19").unwrap() < compact.find("item-18").unwrap());
    assert!(!compact.contains("item-00"));
    assert!(report.render_options(true, true).contains("item-00"));
    assert!(report.render_options(true, true).contains("fixture safety"));
    assert_eq!(report.summary()["allocated_bytes"], 190);
    assert!(
        compact.contains(
            "Unaccounted space estimate (home volume used minus measured allocated): 810 B"
        )
    );
    assert!(compact.contains("- Purgeable space is not quantified."));
    assert!(compact.contains("- Time Machine local snapshots are not inventoried."));
}

#[test]
fn application_support_logs_and_orbstack_are_review_only() {
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-extra-{}", std::process::id()));
    for path in [
        "Library/Application Support/Example",
        "Library/Application Support/com.apple.private",
        "Library/Application Support/Mail",
        "Library/Logs/Example",
        ".orbstack",
    ] {
        fs::create_dir_all(home.join(path)).unwrap();
        fs::write(home.join(path).join("fixture"), b"example").unwrap();
    }
    let r = inventory(&home, &AtomicBool::new(false), Budget::deep()).unwrap();
    for name in [
        "application-support/Example",
        "user-logs/Example",
        "orbstack-storage",
    ] {
        let p = r.providers.iter().find(|p| p.provider == name).unwrap();
        assert_eq!(p.logical_bytes, Some(7), "{name}");
        assert!(p.note.contains("review"));
    }
    assert!(
        !r.providers
            .iter()
            .any(|p| p.provider.contains("com.apple.private")
                || p.provider == "application-support/Mail")
    );
    assert!(
        r.render_options(true, false)
            .contains("application-support/Example")
    );
    assert!(
        r.render_options(false, false)
            .contains("Downloads and Documents are not scanned")
    );
    assert!(
        home.join("Library/Application Support/Mail/fixture")
            .exists()
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn full_scan_refused_probes_still_measure_default_roots() {
    use std::os::unix::fs::symlink;
    let home = std::env::current_dir()
        .unwrap()
        .join(format!(".optimac-refused-scan-{}", std::process::id()));
    for dir in [
        "Library/Caches/pip",
        "Library/Caches/go-build",
        "go/pkg/mod",
    ] {
        fs::create_dir_all(home.join(dir)).unwrap();
        fs::write(home.join(dir).join("fixture"), b"fixture").unwrap();
    }
    let fake = home.join("fake");
    fs::write(&fake, b"not a program").unwrap();
    let link = home.join("fake-link");
    symlink(&fake, &link).unwrap();
    let paths = [link.to_str().unwrap()];
    let cancel = AtomicBool::new(false);
    let pip = super::pip::resolve_with(&home, None, &paths, &cancel);
    let go = super::go::resolve_with(&home, None, None, &paths, &cancel);
    let report = providers::inventory_with_resolutions(
        &home,
        &cancel,
        Budget::quick(),
        &mut |_| {},
        pip,
        go,
    )
    .unwrap();
    for id in ["pip-cache", "go-build", "go-modules"] {
        let row = report.providers.iter().find(|r| r.provider == id).unwrap();
        assert_eq!(row.logical_bytes, Some(7), "{id}");
        assert!(
            row.note.contains("tool not run: candidate refused"),
            "{}",
            row.note
        );
        assert!(row.note.contains("default, not verified"));
        let rendered = report.render_options(true, false);
        assert!(rendered.contains(id));
        assert!(rendered.contains("tool not run: candidate refused"));
    }
    fs::remove_dir_all(home).unwrap();
}
