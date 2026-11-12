//! Built-in default locations; these are inventory capabilities, not deletion rules.
pub const ROOTS: &[(&str, &str)] = &[
    ("orbstack-storage", ".orbstack"),
    ("pip-cache", "Library/Caches/pip"),
    ("go-build", "Library/Caches/go-build"),
    ("homebrew-cache", "Library/Caches/Homebrew"),
    ("npm", ".npm/_cacache"),
    ("cargo-registry", ".cargo/registry/cache"),
    ("cargo-git", ".cargo/git"),
    ("cargo-installed", ".cargo/bin"),
    ("go-modules", "go/pkg/mod"),
    ("bun-cache", ".bun/install/cache"),
    ("maven-repository", ".m2/repository"),
    ("deno-cache", "Library/Caches/deno"),
    ("uv-cache", "Library/Caches/uv"),
    ("poetry-cache", "Library/Caches/pypoetry"),
    ("cocoapods-cache", "Library/Caches/CocoaPods"),
    ("composer-cache", "Library/Caches/composer"),
    ("playwright-browsers", "Library/Caches/ms-playwright"),
    ("puppeteer-browsers", ".cache/puppeteer"),
    ("cypress-browsers", "Library/Caches/Cypress"),
    ("jetbrains-caches", "Library/Caches/JetBrains"),
    ("jetbrains-logs", "Library/Logs/JetBrains"),
    ("swiftpm-cache", "Library/Caches/org.swift.swiftpm"),
    ("gradle-cache", ".gradle/caches"),
    ("dart-pub-cache", ".pub-cache"),
    ("huggingface-cache", ".cache/huggingface/hub"),
    ("pytorch-hub", ".cache/torch/hub"),
    ("ollama-models", ".ollama/models"),
    ("xcode-derived-data", "Library/Developer/Xcode/DerivedData"),
    ("xcode-archives", "Library/Developer/Xcode/Archives"),
    (
        "xcode-device-support",
        "Library/Developer/Xcode/iOS DeviceSupport",
    ),
    (
        "ios-simulator-devices",
        "Library/Developer/CoreSimulator/Devices",
    ),
    (
        "docker-desktop-storage",
        "Library/Containers/com.docker.docker/Data/vms",
    ),
    (
        "ios-backups",
        "Library/Application Support/MobileSync/Backup",
    ),
    ("crash-reports", "Library/Logs/DiagnosticReports"),
    ("local-trash", ".Trash"),
    ("optimac-recovery", ".optimac-rust-recovery"),
];

/// Resolve explicit environment overrides as read-only hints. No command is run.
/// Reject relative and parent-traversal values; callers still validate symlinks.
pub fn resolve(
    home: &std::path::Path,
    id: &str,
    default: &str,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<std::path::PathBuf, &'static str> {
    use std::path::{Component, PathBuf};
    let (key, suffix) = match id {
        "pip-cache" => ("PIP_CACHE_DIR", ""),
        "go-build" => ("GOCACHE", ""),
        "go-modules" => ("GOMODCACHE", ""),
        "homebrew-cache" => ("HOMEBREW_CACHE", ""),
        "cargo-registry" => ("CARGO_HOME", "registry/cache"),
        "cargo-git" => ("CARGO_HOME", "git"),
        "cargo-installed" => ("CARGO_HOME", "bin"),
        "gradle-cache" => ("GRADLE_USER_HOME", "caches"),
        "dart-pub-cache" => ("PUB_CACHE", ""),
        "uv-cache" => ("UV_CACHE_DIR", ""),
        "deno-cache" => ("DENO_DIR", ""),
        "ollama-models" => ("OLLAMA_MODELS", ""),
        _ => return Ok(home.join(default)),
    };
    let Some(value) = env(key) else {
        return Ok(home.join(default));
    };
    let root = PathBuf::from(value);
    if !root.is_absolute()
        || root
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err("Configured cache location is not an absolute safe path");
    }
    Ok(if suffix.is_empty() {
        root
    } else {
        root.join(suffix)
    })
}

/// Catalog policy is shipped with the binary, never taken from the user overlay.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Capability {
    pub provider: String,
    pub class: String,
    pub risk: String,
    pub certification: &'static str,
    pub mutation_supported: bool,
    pub discovery: &'static str,
    pub tested_versions: Vec<String>,
}

pub fn catalog() -> Vec<Capability> {
    let builtin: toml::Value = toml::from_str(include_str!("../../docs/rules.example.toml"))
        .expect("built-in catalog is validated by tests");
    builtin["rule"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| Capability {
            provider: rule["id"].as_str().unwrap().into(),
            class: rule["class"].as_str().unwrap().into(),
            risk: rule["risk"].as_str().unwrap().into(),
            certification: if rule["operation"].as_str() == Some("guidance") {
                "guidance_only"
            } else {
                "inventory_only"
            },
            discovery: if ROOTS
                .iter()
                .any(|(id, _)| *id == alias(rule["id"].as_str().unwrap()))
                || matches!(
                    rule["id"].as_str().unwrap(),
                    "app-user-caches" | "sandbox-app-caches" | "group-container-caches"
                ) {
                "metadata"
            } else if matches!(
                rule["id"].as_str().unwrap(),
                "project-artifacts" | "large-old-files" | "duplicates"
            ) {
                "explicit_scope_metadata"
            } else {
                "unsupported"
            },
            mutation_supported: false,
            tested_versions: Vec::new(),
        })
        .collect()
}

pub fn alias(id: &str) -> &str {
    match id {
        "pip" => "pip-cache",
        "homebrew" => "homebrew-cache",
        "poetry" => "poetry-cache",
        "cocoapods" => "cocoapods-cache",
        "composer" => "composer-cache",
        _ => id,
    }
}
