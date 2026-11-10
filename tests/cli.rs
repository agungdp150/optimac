use std::process::Command;
#[test]
fn version_matches_manifest() {
    let output = Command::new(env!("CARGO_BIN_EXE_optimac"))
        .arg("version")
        .output()
        .unwrap();
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
        let output = Command::new(env!("CARGO_BIN_EXE_optimac"))
            .args([command, "--execute"])
            .output()
            .unwrap();
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
        let output = Command::new(env!("CARGO_BIN_EXE_optimac"))
            .arg(command)
            .arg(&root)
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
    let root = std::env::temp_dir().join(format!("optimac-size-fixture-{}", std::process::id()));
    std::fs::create_dir_all(root.join("node_modules/nested")).unwrap();
    std::fs::write(root.join("node_modules/a"), vec![0; 1000]).unwrap();
    std::fs::write(root.join("node_modules/nested/b"), vec![0; 1500]).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_optimac"))
        .arg("artifacts")
        .arg(&root)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2.50 KB"));
    std::fs::remove_dir_all(root).unwrap();
}
