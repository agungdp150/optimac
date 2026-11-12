//! Exhaustive inventory category mapping; exactly one category per provider.
use super::*;
pub const NAMES: [&str; 10] = [
    "Browser caches",
    "Developer caches",
    "App caches (generic)",
    "App data (Application Support)",
    "Simulators and Xcode",
    "Docker/OrbStack",
    "Logs and crash reports",
    "Trash",
    "Project output",
    "Other",
];
pub const PROVIDERS: &[(&str, usize)] = &[
    ("homebrew", 1),
    ("npm", 1),
    ("yarn", 1),
    ("pip", 1),
    ("poetry", 1),
    ("go-build", 1),
    ("cocoapods", 1),
    ("composer", 1),
    ("cargo-registry", 1),
    ("uv-cache", 1),
    ("xcode-derived-data", 4),
    ("safari-cache", 0),
    ("chrome-cache", 0),
    ("firefox-cache", 0),
    ("arc-cache", 0),
    ("slack-cache", 2),
    ("discord-cache", 2),
    ("vscode-cache", 2),
    ("spotify-cache", 2),
    ("notion-cache", 2),
    ("swiftpm-cache", 1),
    ("go-modules", 1),
    ("gradle-cache", 1),
    ("maven-repository", 1),
    ("conda-packages", 1),
    ("conda-archives", 1),
    ("crash-reports", 6),
    ("app-logs", 6),
    ("project-artifacts", 8),
    ("app-leftovers", 9),
    ("download-installers", 9),
    ("ios-backups", 9),
    ("xcode-archives", 4),
    ("xcode-device-support", 4),
    ("unavailable-simulators", 4),
    ("simulator-reset", 4),
    ("simulator-runtimes", 4),
    ("docker-dangling-images", 5),
    ("docker-stopped-containers", 5),
    ("docker-build-cache", 5),
    ("adobe-media-cache", 2),
    ("dropbox-cache", 2),
    ("android-cache", 2),
    ("steam-download-cache", 2),
    ("teams-cache", 2),
    ("local-trash", 7),
    ("optimac-recovery", 9),
    ("large-old-files", 9),
    ("duplicates", 9),
    ("quicklook-cache", 9),
    ("mail-attachments", 9),
    ("time-machine-snapshots", 9),
    ("minecraft-data", 9),
    ("system-logs", 6),
    ("system-caches", 9),
    ("app-bundle-resources", 9),
    ("personal-databases", 9),
    ("app-user-caches", 2),
    ("sandbox-app-caches", 2),
    ("group-container-caches", 2),
    ("pnpm-store", 1),
    ("bun-cache", 1),
    ("deno-cache", 1),
    ("bundler-cache", 1),
    ("rubygems-old-versions", 1),
    ("pyenv-cache", 1),
    ("pyenv-versions", 1),
    ("dart-pub-cache", 1),
    ("terraform-plugin-cache", 1),
    ("playwright-browsers", 1),
    ("puppeteer-browsers", 1),
    ("cypress-browsers", 1),
    ("jetbrains-caches", 1),
    ("jetbrains-logs", 6),
    ("wget-state", 9),
    ("huggingface-cache", 9),
    ("pytorch-hub", 9),
    ("ollama-models", 9),
    ("cargo-git", 1),
    ("cargo-installed", 1),
    ("docker-tagged-images", 5),
    ("photos-library", 9),
    ("messages-database", 9),
    ("keychain", 9),
    ("mail-database", 9),
    ("notes-database", 9),
    ("credentials", 9),
    ("sip-managed-locations", 9),
    ("application-support", 3),
    ("user-logs", 6),
    ("orbstack-storage", 5),
    ("pip-cache", 1),
    ("homebrew-cache", 1),
    ("poetry-cache", 1),
    ("cocoapods-cache", 1),
    ("composer-cache", 1),
    ("docker-desktop-storage", 5),
    ("ios-simulator-devices", 4),
];
fn category(row: &ProviderReport) -> usize {
    let mut parts = row.provider.split('/');
    let id = parts.next().unwrap_or("");
    if matches!(
        id,
        "app-user-caches" | "sandbox-app-caches" | "group-container-caches"
    ) {
        let owner = parts.next().unwrap_or("").to_ascii_lowercase();
        if owner == "bravesoftware"
            || owner == "firefox"
            || owner == "zen"
            || owner.starts_with("com.brave.")
            || owner.starts_with("org.mozilla.firefox")
            || owner.starts_with("app.zen-browser.")
            || owner.starts_with("com.zen-browser.")
        {
            return 0;
        }
    }
    PROVIDERS
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, index)| *index)
        .unwrap_or(9)
}
#[derive(serde::Serialize)]
pub struct Category {
    pub name: &'static str,
    pub allocated_bytes: Option<u64>,
    pub lower_bound: bool,
    pub denied: bool,
}
pub fn totals(rows: &[ProviderReport]) -> Vec<Category> {
    let mut output: Vec<_> = NAMES
        .iter()
        .map(|&name| Category {
            name,
            allocated_bytes: None,
            lower_bound: false,
            denied: false,
        })
        .collect();
    for row in rows {
        let group = &mut output[category(row)];
        if let Some(bytes) = row.allocated_bytes {
            group.allocated_bytes = Some(group.allocated_bytes.unwrap_or(0).saturating_add(bytes));
        }
        group.lower_bound |= !matches!(row.status, Status::Complete | Status::Absent);
        group.denied |= row.status == Status::Denied || row.errors > 0;
    }
    output
}
pub fn render(rows: &[ProviderReport]) -> String {
    let mut text = String::from("Category inventory (allocated estimates; not reclaimable)\n");
    for c in totals(rows) {
        let amount = if c.name == "Trash" && c.denied {
            "unknown (denied)".into()
        } else {
            c.allocated_bytes
                .map(crate::format_size)
                .unwrap_or_else(|| "unknown".into())
        };
        text.push_str(&format!(
            "{}: {}{}\n",
            c.name,
            amount,
            if c.lower_bound && !(c.name == "Trash" && c.denied) {
                " (lower bound)"
            } else {
                ""
            }
        ));
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_specific_mapping_partial_and_denied_trash() {
        let make = |provider: &str, status, bytes| ProviderReport {
            provider: provider.into(),
            root: PathBuf::from("/fixture"),
            access: "unknown".into(),
            status,
            logical_bytes: bytes,
            allocated_bytes: bytes,
            files: 0,
            skipped: 0,
            errors: 0,
            note: String::new(),
        };
        let rows = vec![
            make(
                "app-user-caches/com.brave.Browser",
                Status::Partial,
                Some(10),
            ),
            make("app-user-caches/other", Status::Complete, Some(20)),
            make("pip-cache", Status::Complete, Some(30)),
            make("application-support/example", Status::Complete, Some(40)),
            make("project-artifacts", Status::Complete, Some(50)),
            make("local-trash", Status::Denied, None),
        ];
        let totals = totals(&rows);
        assert_eq!(totals[0].allocated_bytes, Some(10));
        assert!(totals[0].lower_bound);
        assert_eq!(totals[2].allocated_bytes, Some(20));
        assert!(render(&rows).contains("Trash: unknown (denied)"));
        assert!(render(&rows).contains("Browser caches: 10 B (lower bound)"));
    }
    #[test]
    fn every_shipped_provider_is_explicitly_mapped() {
        for c in super::super::catalog() {
            assert!(
                PROVIDERS.iter().any(|(id, _)| *id == c.provider),
                "{}",
                c.provider
            );
        }
        for (id, _) in super::super::registry::ROOTS {
            assert!(PROVIDERS.iter().any(|(key, _)| key == id), "{id}");
        }
    }
}
