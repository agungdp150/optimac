#[cfg(not(feature = "fixture-only"))]
compile_error!("CLI integration tests require --features fixture-only");

use std::process::Command;
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
    let fixture = std::env::temp_dir().join(format!("optimac-missing-cli-{}", std::process::id()));
    assert!(!is_fixture_cli(&fixture));
    // An executable that cannot answer the fixture probe is refused too.
    std::fs::write(&fixture, b"not an executable").unwrap();
    assert!(!is_fixture_cli(&fixture));
    std::fs::remove_file(fixture).unwrap();
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
    let root = std::env::temp_dir().join(format!("optimac-cli-actions-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("first"), b"same content").unwrap();
    std::fs::write(root.join("second"), b"same content").unwrap();
    for command in ["analyze", "duplicates", "artifacts"] {
        let output = fixture_cli().arg(command).arg(&root).output().unwrap();
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
    let root = std::env::temp_dir().join(format!("optimac-size-fixture-{}", std::process::id()));
    std::fs::create_dir_all(root.join("node_modules/nested")).unwrap();
    std::fs::write(root.join("node_modules/a"), vec![0; 1000]).unwrap();
    std::fs::write(root.join("node_modules/nested/b"), vec![0; 1500]).unwrap();
    let output = fixture_cli().arg("artifacts").arg(&root).output().unwrap();
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
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-cli-inventory-{}", std::process::id()));
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
    let home = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-config-cli-{}", std::process::id()));
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
    let fixture = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("optimac-cli-home-paths-{}", std::process::id()));
    std::fs::create_dir(&fixture).unwrap();
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
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "optimac-cli-auth-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
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
