#[cfg(not(feature = "fixture-only"))]
compile_error!("CLI integration tests require --features fixture-only");

use std::process::Command;
// Each migrated test owns a private root and explicitly opens its fixture capability.
fn authorized_home() -> (std::path::PathBuf, optimac::fixture_paths::FixtureRoot) {
    use std::os::unix::fs::DirBuilderExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "optimac-cli-authorized-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let capability = optimac::fixture_paths::FixtureRoot::open(&root).unwrap();
    (root, capability)
}

fn is_fixture_cli(executable: &std::path::Path) -> bool {
    Command::new(executable)
        .arg("--fixture-only-test-probe")
        .output()
        .is_ok_and(|output| {
            output.status.success() && output.stdout == b"optimac-fixture-only-v1\n"
        })
}

fn fixture_cli() -> Command {
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_optimac"));
    assert!(
        is_fixture_cli(executable),
        "refusing unavailable or non-fixture CLI"
    );
    Command::new(executable)
}

#[test]
fn fixture_cli_rejects_missing_executable() {
    let (home, _authorization) = authorized_home();
    let fixture = home.join("missing-cli");
    assert!(!is_fixture_cli(&fixture));
    // An executable that cannot answer the fixture probe is refused too.
    std::fs::write(&fixture, b"not an executable").unwrap();
    assert!(!is_fixture_cli(&fixture));
    std::fs::remove_file(fixture).unwrap();
    std::fs::remove_dir(home).unwrap();
}

#[test]
fn version_matches_manifest() {
    let output = fixture_cli().arg("version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        env!("CARGO_PKG_VERSION")
    );
}
#[test]
fn legacy_mutations_fail_explicitly() {
    for command in [
        "clean",
        "uninstall",
        "browser",
        "restore",
        "trash",
        "optimize",
    ] {
        let output = fixture_cli().args([command, "--execute"]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("not yet ported"));
    }
}

#[test]
fn analysis_and_duplicates_use_disposable_fixtures() {
    let (root, _authorization) = authorized_home();
    std::fs::write(root.join("first"), b"same content").unwrap();
    std::fs::write(root.join("second"), b"same content").unwrap();
    for command in ["analyze", "duplicates", "artifacts"] {
        let output = fixture_cli()
            .arg(command)
            .arg(&root)
            .env("HOME", &root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        if command == "duplicates" {
            assert!(text.contains("1 matching pairs"));
        }
        if command == "analyze" {
            assert!(text.contains("first") && text.contains("second"));
            assert!(text.contains("12 B"));
        }
    }
    assert_eq!(std::fs::read(root.join("first")).unwrap(), b"same content");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn artifact_sizes_sum_scanned_files() {
    let (root, _authorization) = authorized_home();
    std::fs::create_dir_all(root.join("node_modules/nested")).unwrap();
    std::fs::write(root.join("node_modules/a"), vec![0; 1000]).unwrap();
    std::fs::write(root.join("node_modules/nested/b"), vec![0; 1500]).unwrap();
    let output = fixture_cli()
        .arg("artifacts")
        .arg(&root)
        .env("HOME", &root)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2.50 KB"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn retired_commands_explain_why() {
    for command in ["threats", "orphans"] {
        let output = fixture_cli().arg(command).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("retired"));
    }
}

#[test]
fn cleanup_inventory_json_is_read_only_and_reports_unknown_roots() {
    let (home, _authorization) = authorized_home();
    std::fs::create_dir_all(home.join("Library/Caches/fixture")).unwrap();
    std::fs::create_dir_all(home.join("pip-fixture")).unwrap();
    let file = home.join("Library/Caches/fixture/data");
    std::fs::write(&file, b"unchanged").unwrap();
    let output = fixture_cli()
        .args(["clean", "scan", "--json"])
        .env("HOME", &home)
        .env("PIP_CACHE_DIR", home.join("pip-fixture"))
        .env("GOCACHE", home.join("pip-fixture"))
        .env("GOMODCACHE", home.join("pip-fixture"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["read_only"], true);
    assert!(value["summary"]["logical_bytes"].is_number());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Scanning"));
    assert!(
        value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["provider"] == "app-user-caches/fixture" && r["logical_bytes"] == 9)
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"unchanged");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn configured_inventory_applies_scopes_and_exclusions() {
    let (home, _authorization) = authorized_home();
    for dir in [
        "files/keep",
        "files/private",
        "files/local",
        "files/cache.tmp",
    ] {
        std::fs::create_dir_all(home.join(dir)).unwrap();
        std::fs::write(home.join(dir).join("item"), b"12345").unwrap();
    }
    let config = home.join("rules.toml");
    let text = r#"schema_version = 2
[scope]
file_roots = ["~/files"]
exclude_paths = ["~/files/private"]
[[rule]]
id = "large-old-files"
enabled = true
scope = "files"
exclude_paths = ["~/files/local"]
exclude_name_patterns = ["*.tmp"]
"#;
    std::fs::write(&config, text).unwrap();
    let run = || {
        fixture_cli()
            .args(["clean", "scan", "--json", "--config"])
            .arg(&config)
            .env("HOME", &home)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let categorized = fixture_cli()
        .args(["clean", "scan", "--json", "--by-category", "--config"])
        .arg(&config)
        .env("HOME", &home)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&categorized.stdout).unwrap();
    assert_eq!(json["categories"].as_array().unwrap().len(), 10);
    assert_eq!(
        json["categories"][9]["allocated_bytes"],
        report["providers"][0]["allocated_bytes"]
    );
    assert_eq!(report["providers"][0]["logical_bytes"], 5);
    assert_eq!(report["providers"].as_array().unwrap().len(), 1);
    std::fs::write(
        &config,
        text.replace(
            "scope = \"files\"",
            "scope = \"files\"\nscan_roots = [\"~/outside\"]",
        ),
    )
    .unwrap();
    let result = run();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("outside approved scope"));
    for dir in ["files/private", "files/local", "files/cache.tmp"] {
        assert_eq!(
            std::fs::read(home.join(dir).join("item")).unwrap(),
            b"12345"
        );
    }
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn invalid_class_is_usage_error_before_scanning() {
    let result = fixture_cli()
        .args(["clean", "scan", "--class", "invalid"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
}

#[test]
fn fixture_paths_are_used_by_cli_history() {
    let (fixture, _authorization) = authorized_home();
    let paths = optimac::paths::AppPaths::from_home(Some(fixture.as_os_str().to_owned())).unwrap();
    let expected = optimac::recovery::history(paths.home()).unwrap();
    let output = fixture_cli()
        .arg("history")
        .env("HOME", &fixture)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        expected.trim()
    );
    assert!(!paths.recovery().exists());
    std::fs::remove_dir(fixture).unwrap();
}

/// This must exercise a separately compiled binary, not cfg(test) in the library.
#[cfg(feature = "fixture-only")]
#[test]
fn fixture_recovery_cli_cannot_select_production_policy() {
    let (root, _authorization) = authorized_home();
    let output = fixture_cli()
        .env("HOME", &root)
        .args(["recovery", "inspect", "123"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("production recovery authorization disabled in fixture-only build")
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn clean_plan_cli_respects_config_exclusions_and_keeps_files_unchanged() {
    let (home, _authorization) = authorized_home();
    let keep = home.join("Library/Caches/ms-playwright/keep");
    let excluded = home.join("Library/Caches/ms-playwright/private");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::create_dir_all(&excluded).unwrap();
    std::fs::write(keep.join("item"), b"keep").unwrap();
    std::fs::write(excluded.join("secret"), b"unchanged").unwrap();
    for path in [keep.join("item"), keep.clone()] {
        std::fs::File::open(path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(10),
            ))
            .unwrap();
    }
    let config = home.join("rules.toml");
    std::fs::write(&config, "schema_version = 2\n[scope]\nexclude_paths = ['~/Library/Caches/ms-playwright/private']\n[[rule]]\nid = 'playwright-browsers'\nenabled = true\n").unwrap();
    let output = fixture_cli()
        .args(["clean", "plan", "--json", "--config"])
        .arg(&config)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["read_only"], true);
    assert_eq!(json["estimated_logical_bytes"], 4);
    assert!(
        json["content"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["provider"] == "playwright-browsers")
    );
    let text = fixture_cli()
        .args(["clean", "plan", "--config"])
        .arg(&config)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("Read-only cleanup plan"));
    assert!(text.contains("not guaranteed disk space freed"));
    assert!(text.contains("playwright-browsers"));
    let entry = json["content"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["provider"] == "playwright-browsers")
        .unwrap();
    assert_eq!(
        entry["targets"][0]["logical_bytes"], 4,
        "excluded files must not contribute"
    );
    assert_eq!(std::fs::read(keep.join("item")).unwrap(), b"keep");
    assert_eq!(
        std::fs::read(excluded.join("secret")).unwrap(),
        b"unchanged"
    );
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn immutable_plan_cli_show_list_json_and_nonempty_fixture() {
    use std::{
        io::Write,
        os::unix::fs::symlink,
        time::{Duration, SystemTime},
    };
    let (home, _authorization) = authorized_home();
    let playwright = home.join("Library/Caches/ms-playwright");
    let cypress = home.join("Library/Caches/Cypress");
    let cargo = home.join(".cargo/registry/cache");
    for root in [&playwright, &cypress, &cargo] {
        std::fs::create_dir_all(root).unwrap();
    }
    std::fs::write(playwright.join("old"), vec![b'x'; 4096]).unwrap();
    std::fs::File::open(playwright.join("old"))
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
        )
        .unwrap();
    std::fs::write(playwright.join("recent"), b"recent").unwrap();
    symlink(home.join("outside"), cypress.join("link")).unwrap();
    for n in 0..4 {
        std::fs::write(cargo.join(format!("entry-{n}")), b"fixture").unwrap();
    }
    let config = home.join("rules.toml");
    std::fs::write(&config,"schema_version=2\n[scan]\nquick_max_entries_per_provider=3\n[[rule]]\nid='playwright-browsers'\nenabled=true\n[[rule]]\nid='cypress-browsers'\nenabled=true\n[[rule]]\nid='cargo-registry'\nenabled=true\n").unwrap();
    let output = fixture_cli()
        .args(["clean", "plan", "--json", "--config"])
        .arg(&config)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["read_only"], true);
    assert_eq!(json["lower_bound"], true);
    assert_eq!(json["estimated_logical_bytes"], 4096);
    let providers = json["content"]["providers"].as_array().unwrap();
    assert!(providers.iter().any(|p| p["provider"] == "cargo-registry"
        && p["completeness"] == "partial"
        && p["targets"].as_array().unwrap().is_empty()));
    assert!(providers.iter().any(|p| {
        p["excluded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["reason"].as_str().unwrap().contains("symlink"))
    }));
    assert!(providers.iter().any(|p| {
        p["excluded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["reason"].as_str().unwrap().contains("24 hours"))
    }));
    let id = json["plan_id"].as_str().unwrap();
    let shown = fixture_cli()
        .args(["clean", "plan", "--show", id, "--json"])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(shown.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&shown.stdout).unwrap(),
        json
    );
    let list = fixture_cli()
        .args(["clean", "plan", "--list", "--json"])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(list.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&list.stdout).unwrap()["plans"][0],
        json
    );
    let text = fixture_cli()
        .args(["clean", "plan", "--show", id])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("This plan changes nothing."));
    assert!(text.contains("lower bound"));
    assert!(text.contains("scan hit its budget; run with --deep"));
    let refused = fixture_cli()
        .args(["clean", "plan", "--provider", "local-trash"])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let invalid = fixture_cli()
        .args(["clean", "plan", "--list", "--deep"])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert_eq!(
        std::fs::read(playwright.join("old")).unwrap(),
        vec![b'x'; 4096]
    );
    assert!(
        std::fs::symlink_metadata(cypress.join("link"))
            .unwrap()
            .is_symlink()
    );
    // Send the real CLI fixture output to the sandbox harness log, with only the
    // random fixture prefix redacted. This provides a reproducible report example.
    let example = format!(
        "\nPLAN FIXTURE (temporary home shown as <fixture>)\n{text}\nPLAN JSON FIXTURE\n{}\n",
        String::from_utf8(output.stdout).unwrap()
    )
    .replace(home.to_str().unwrap(), "<fixture>");
    std::io::stdout().write_all(example.as_bytes()).unwrap();
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn plan_internal_links_and_denied_provider_have_honest_cli_messages() {
    use std::{
        io::Write,
        os::unix::fs::{PermissionsExt, symlink},
        time::{Duration, SystemTime},
    };
    let (home, _authorization) = authorized_home();
    let root = home.join("Library/Caches/ms-playwright");
    let unit = root.join("chromium-fixture");
    let framework = unit.join("Chromium.app/Frameworks/Example.framework");
    std::fs::create_dir_all(framework.join("Versions/A")).unwrap();
    std::fs::write(framework.join("Versions/A/binary"), vec![0u8; 4096]).unwrap();
    let outside = home.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("must-not-be-measured"), vec![0u8; 16384]).unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o000)).unwrap();
    for (target, name) in [
        ("A", "Versions/Current"),
        ("Versions/Current/Helpers", "Helpers"),
        ("Versions/Current/Libraries", "Libraries"),
        ("Versions/Current/Resources", "Resources"),
    ] {
        symlink(target, framework.join(name)).unwrap();
    }
    symlink(&outside, framework.join("external")).unwrap();
    fn age(path: &std::path::Path) {
        let m = std::fs::symlink_metadata(path).unwrap();
        if m.is_symlink() {
            return;
        }
        if m.is_dir() {
            for e in std::fs::read_dir(path).unwrap() {
                age(&e.unwrap().path());
            }
        }
        std::fs::File::open(path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
            )
            .unwrap();
    }
    age(&unit);
    let denied = home.join("Library/Caches/Cypress/denied");
    std::fs::create_dir_all(&denied).unwrap();
    std::fs::write(denied.join("data"), b"unreadable fixture").unwrap();
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).unwrap();
    let config = home.join("rules.toml");
    std::fs::write(&config,"schema_version=2\n[[rule]]\nid='playwright-browsers'\nenabled=true\n[[rule]]\nid='cypress-browsers'\nenabled=true").unwrap();
    let invoke = |args: &[&str]| {
        fixture_cli()
            .args(args)
            .arg("--config")
            .arg(&config)
            .env("HOME", &home)
            .output()
            .unwrap()
    };
    let scan = invoke(&["clean", "scan", "--json"]);
    // Partial scan retains its existing nonzero status while still emitting JSON.
    let scan_json: serde_json::Value = serde_json::from_slice(&scan.stdout).unwrap();
    let rows = scan_json["providers"].as_array().unwrap();
    let browser = rows
        .iter()
        .find(|r| r["provider"] == "playwright-browsers")
        .unwrap();
    assert_eq!(browser["status"], "complete");
    assert_eq!(browser["skipped"], 5);
    assert_eq!(browser["skip_reasons"]["symlink"], 5);
    let denied_row = rows
        .iter()
        .find(|r| r["provider"] == "cypress-browsers")
        .unwrap();
    assert_eq!(denied_row["status"], "partial");
    assert_eq!(denied_row["skip_reasons"]["permission_denied"], 1);
    let plan = invoke(&["clean", "plan", "--deep", "--json"]);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    let providers = json["content"]["providers"].as_array().unwrap();
    assert_eq!(
        providers
            .iter()
            .map(|p| p["targets"].as_array().unwrap().len())
            .sum::<usize>(),
        1
    );
    assert_eq!(
        providers
            .iter()
            .find(|p| p["provider"] == "playwright-browsers")
            .unwrap()["targets"][0]["path"],
        unit.to_str().unwrap()
    );
    assert!(!String::from_utf8_lossy(&plan.stdout).contains("must-not-be-measured"));
    let text = invoke(&["clean", "plan", "--deep"]);
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("not plannable: permission denied"));
    assert!(!text.contains("run with --deep"));
    let scan_text = String::from_utf8(invoke(&["clean", "scan", "--all"]).stdout).unwrap();
    let rows = scan_text
        .lines()
        .filter(|l| l.starts_with("playwright-browsers —") || l.starts_with("cypress-browsers —"))
        .collect::<Vec<_>>()
        .join("\n");
    let output = format!("\nSLICE 1C FIXTURE SCAN ROWS\n{rows}\nSLICE 1C FIXTURE PLAN\n{text}\n")
        .replace(home.to_str().unwrap(), "<fixture>");
    std::io::stdout().write_all(output.as_bytes()).unwrap();
    // Both budget modes are tested through the actual command parser.
    std::fs::write(&config,"schema_version=2\n[scan]\nquick_max_entries_per_provider=1\ndeep_max_entries_per_provider=1\n[[rule]]\nid='playwright-browsers'\nenabled=true").unwrap();
    let quick = String::from_utf8(invoke(&["clean", "plan"]).stdout).unwrap();
    assert!(quick.contains("scan hit its budget; run with --deep"));
    let deep = String::from_utf8(invoke(&["clean", "plan", "--deep"]).stdout).unwrap();
    assert!(deep.contains("scan hit its budget even with --deep"));
    assert!(!deep.contains("run with --deep"));
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn clean_apply_cli_is_interactive_only_and_changes_nothing() {
    let (home, _authorization) = authorized_home();
    let id = "0".repeat(64);
    for args in [
        vec!["clean", "apply"],
        vec!["clean", "apply", id.as_str(), "--yes"],
    ] {
        let output = fixture_cli()
            .args(&args)
            .env("HOME", &home)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: optimac clean apply"));
    }
    let output = fixture_cli()
        .args(["clean", "apply", id.as_str()])
        .env("HOME", &home)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires an interactive terminal; there is no noninteractive mode")
    );
    let inspect = fixture_cli()
        .args(["recovery", "inspect", id.as_str()])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(!inspect.status.success());
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn recovery_purge_cli_is_interactive_only_and_changes_nothing() {
    let (home, _authorization) = authorized_home();
    for args in [
        vec!["recovery", "purge"],
        vec!["recovery", "purge", "1", "--yes"],
    ] {
        let output = fixture_cli()
            .args(&args)
            .env("HOME", &home)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: optimac recovery purge"));
    }
    let output = fixture_cli()
        .args(["recovery", "purge", "1"])
        .env("HOME", &home)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires an interactive terminal"));
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn recovery_restore_cli_uses_production_policy_only() {
    let (home, _authorization) = authorized_home();
    let output = fixture_cli()
        .args(["recovery", "restore", "1"])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("production recovery authorization disabled in fixture-only build")
    );
    let usage = fixture_cli()
        .args(["recovery", "restore"])
        .env("HOME", &home)
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("restore <id>"));
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
    std::fs::remove_dir_all(home).unwrap();
}
