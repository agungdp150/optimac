//! Read-only proposals from aggregate inventory, never filesystem mutation targets.
use super::{Inventory, Status};
use serde::Serialize;
use std::path::PathBuf;

const WARNING: &str = "Estimated reclaimable space is not guaranteed disk space freed. This read-only proposal does not authorize cleanup.";

#[derive(Serialize)]
pub struct Plan {
    pub schema_version: u32,
    pub read_only: bool,
    pub warning: &'static str,
    pub actionable_allocated_bytes: u64,
    pub entries: Vec<Entry>,
}
#[derive(Serialize)]
pub struct Entry {
    pub category: &'static str,
    pub provider: String,
    pub path: PathBuf,
    pub class: String,
    pub risk: String,
    pub status: Status,
    pub logical_bytes: Option<u64>,
    pub allocated_bytes: Option<u64>,
    pub actionable: bool,
    pub reason: String,
}

pub fn from_inventory(inventory: &Inventory) -> Plan {
    let mut rows: Vec<_> = inventory
        .providers
        .iter()
        .filter(|r| r.status != Status::Absent)
        .collect();
    rows.sort_by(|a, b| a.root.cmp(&b.root).then(a.provider.cmp(&b.provider)));
    let mut claimed: Vec<PathBuf> = Vec::new();
    let mut entries = Vec::new();
    let mut total = 0u64;
    for row in rows {
        let id = row.provider.split('/').next().unwrap_or("");
        let cap = inventory
            .capabilities
            .iter()
            .find(|c| c.provider == id || super::registry::alias(&c.provider) == id);
        let (class, risk) = cap
            .map(|c| (c.class.as_str(), c.risk.as_str()))
            .unwrap_or(("unknown_content", "unknown"));
        let eligible = matches!(
            id,
            "playwright-browsers"
                | "puppeteer-browsers"
                | "cypress-browsers"
                | "pnpm-store"
                | "npm"
                | "gradle-cache"
                | "cargo-registry"
        );
        let unverified = id == "npm" || row.note.to_ascii_lowercase().contains("not verified");
        let overlap = claimed
            .iter()
            .any(|root| row.root.starts_with(root) || root.starts_with(&row.root));
        let action = eligible
            && class == "rebuildable_cache"
            && risk == "conditional_cache"
            && row.status == Status::Complete
            && row.allocated_bytes.is_some()
            && !unverified
            && !overlap;
        let reason = if action {
            claimed.push(row.root.clone());
            total = total.saturating_add(row.allocated_bytes.unwrap());
            "Rebuildable cache; tools may need to download or regenerate data after cleanup."
                .to_string()
        } else if row.status != Status::Complete {
            format!(
                "Excluded: inventory status {:?}; size may be unknown or incomplete.",
                row.status
            )
        } else if unverified {
            "Excluded: location is default, not verified.".into()
        } else if overlap {
            "Excluded: overlaps another counted root.".into()
        } else if row.allocated_bytes.is_none() {
            "Excluded: allocated size unknown.".into()
        } else {
            "Review only: classification or provider is outside the currently scoped cache candidates.".into()
        };
        entries.push(Entry {
            category: super::category::NAMES[super::category::category(row)],
            provider: row.provider.clone(),
            path: row.root.clone(),
            class: class.into(),
            risk: risk.into(),
            status: row.status,
            logical_bytes: row.logical_bytes,
            allocated_bytes: row.allocated_bytes,
            actionable: action,
            reason,
        });
    }
    entries.sort_by(|a, b| {
        super::category::NAMES
            .iter()
            .position(|n| *n == a.category)
            .cmp(&super::category::NAMES.iter().position(|n| *n == b.category))
            .then(a.provider.cmp(&b.provider))
            .then(a.path.cmp(&b.path))
    });
    Plan {
        schema_version: 1,
        read_only: true,
        warning: WARNING,
        actionable_allocated_bytes: total,
        entries,
    }
}
impl Plan {
    pub fn render(&self) -> String {
        let mut output = format!(
            "Read-only cleanup proposal\nEstimated candidate allocated size: {}\n{}\n",
            crate::format_size(self.actionable_allocated_bytes),
            self.warning
        );
        let mut category = "";
        for e in &self.entries {
            if category != e.category {
                category = e.category;
                output.push_str(&format!("\n{category}\n"));
            }
            let bytes = e
                .allocated_bytes
                .map(crate::format_size)
                .unwrap_or_else(|| "unknown".into());
            output.push_str(&format!(
                "  {} — {} ({}, {}, {:?})\n    {}\n    {}\n",
                e.provider,
                bytes,
                e.class,
                e.risk,
                e.status,
                e.path.display(),
                e.reason
            ));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::{Capability, ProviderReport};

    fn row(id: &str, path: &str, status: Status, size: Option<u64>, note: &str) -> ProviderReport {
        ProviderReport {
            provider: id.into(),
            root: path.into(),
            access: "test".into(),
            status,
            logical_bytes: size,
            allocated_bytes: size,
            files: 1,
            skipped: 0,
            skip_reasons: Default::default(),
            errors: 0,
            note: note.into(),
        }
    }
    fn cap(id: &str, class: &str, risk: &str) -> Capability {
        Capability {
            provider: id.into(),
            class: class.into(),
            risk: risk.into(),
            certification: "inventory_only",
            mutation_supported: false,
            discovery: "metadata",
            tested_versions: vec![],
        }
    }
    #[test]
    fn clean_plan_groups_and_excludes_unknown_denied_partial_unverified_and_overlaps() {
        let inventory = Inventory {
            schema_version: 1,
            read_only: true,
            available_bytes: None,
            home_used_bytes: None,
            complete: false,
            notes: vec![],
            capabilities: vec![
                cap("gradle-cache", "rebuildable_cache", "conditional_cache"),
                cap("npm", "rebuildable_cache", "conditional_cache"),
                cap("cargo-registry", "rebuildable_cache", "conditional_cache"),
                cap("app-user-caches", "unknown_content", "review"),
            ],
            providers: vec![
                row("gradle-cache", "/fixture/a", Status::Complete, Some(40), ""),
                row(
                    "cargo-registry",
                    "/fixture/a/nested",
                    Status::Complete,
                    Some(30),
                    "",
                ),
                row(
                    "npm",
                    "/fixture/b",
                    Status::Denied,
                    None,
                    "permission denied",
                ),
                row("npm", "/fixture/c", Status::Partial, Some(90), "budget"),
                row(
                    "npm",
                    "/fixture/d",
                    Status::Complete,
                    Some(100),
                    "default, not verified",
                ),
                row(
                    "app-user-caches/BraveSoftware",
                    "/fixture/e",
                    Status::Complete,
                    Some(500),
                    "",
                ),
                row(
                    "unclassified",
                    "/fixture/f",
                    Status::Complete,
                    Some(800),
                    "",
                ),
            ],
        };
        let plan = from_inventory(&inventory);
        assert_eq!(plan.actionable_allocated_bytes, 40);
        assert_eq!(plan.entries.iter().filter(|e| e.actionable).count(), 1);
        assert!(plan.entries.iter().any(|e| e.category == "Browser caches"
            && e.class == "unknown_content"
            && !e.actionable));
        assert!(
            plan.entries.iter().any(|e| e.status == Status::Denied
                && e.allocated_bytes.is_none()
                && !e.actionable)
        );
        assert!(
            plan.entries
                .iter()
                .any(|e| e.reason.contains("not verified"))
        );
        assert!(plan.entries.iter().any(|e| e.reason.contains("overlaps")));
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(json["read_only"], true);
        assert_eq!(json["actionable_allocated_bytes"], 40);
        assert!(plan.render().contains("not guaranteed disk space freed"));
    }
}
