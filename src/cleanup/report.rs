use super::*;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Complete,
    Partial,
    Absent,
    Denied,
    Blocked,
    Cancelled,
    Unsupported,
    Guidance,
    Skipped,
}
#[derive(Debug, Serialize)]
pub struct ProviderReport {
    pub provider: String,
    pub root: PathBuf,
    pub access: String,
    pub status: Status,
    pub logical_bytes: Option<u64>,
    pub allocated_bytes: Option<u64>,
    pub files: u64,
    pub skipped: u64,
    pub skip_reasons: std::collections::BTreeMap<String, u64>,
    pub errors: u64,
    pub note: String,
}
impl ProviderReport {
    pub(super) fn reason(&mut self, reason: &str) {
        *self.skip_reasons.entry(reason.into()).or_default() += 1;
    }
    pub fn reason_summary(&self) -> String {
        self.skip_reasons
            .iter()
            .map(|(reason, count)| {
                let label = match reason.as_str() {
                    "symlink" => "symlinks not followed",
                    "hard_link" => "hard-linked files",
                    "protected_name" => "Apple-owned or protected names",
                    "config_exclusion" => "configuration exclusions",
                    "permission_denied" => "permission denied",
                    "io_error" => "I/O errors",
                    "vanished" => "vanished while scanning",
                    "mount_boundary" => "mount boundaries",
                    "budget" => "budget limits reached",
                    "depth_limit" => "depth limits reached",
                    _ => reason,
                };
                format!("{count} {label}")
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub schema_version: u32,
    pub read_only: bool,
    pub providers: Vec<ProviderReport>,
    pub available_bytes: Option<u64>,
    pub home_used_bytes: Option<u64>,
    pub complete: bool,
    pub notes: Vec<String>,
    pub capabilities: Vec<super::Capability>,
}
#[derive(Clone, Copy)]
pub struct Budget {
    pub entries: usize,
    pub global_entries: usize,
    pub per_root: Duration,
    pub total: Duration,
}
impl Budget {
    /// Reserve a fair share for every remaining root; unused shares flow forward.
    pub(super) fn fair_share(
        self,
        roots_left: usize,
        entries_left: usize,
        deadline: Instant,
    ) -> Self {
        let count = roots_left.max(1);
        Self {
            entries: self.entries.min(entries_left / count),
            per_root: self.per_root.min(
                deadline.saturating_duration_since(Instant::now())
                    / count.min(u32::MAX as usize) as u32,
            ),
            ..self
        }
    }

    pub fn quick() -> Self {
        Self {
            entries: 10_000,
            global_entries: 100_000,
            per_root: Duration::from_secs(2),
            total: Duration::from_secs(30),
        }
    }
    pub fn deep() -> Self {
        Self {
            entries: 250_000,
            global_entries: 1_000_000,
            per_root: Duration::from_secs(10),
            total: Duration::from_secs(120),
        }
    }
}
#[derive(Default, serde::Serialize)]
pub struct GroupTotal {
    pub allocated_bytes: u64,
    pub lower_bound: bool,
    pub providers: usize,
}

impl Inventory {
    fn group_index(&self, row: &ProviderReport) -> usize {
        let id = row.provider.split('/').next().unwrap_or("");
        let capability = self
            .capabilities
            .iter()
            .find(|c| c.provider == id || super::registry::alias(&c.provider) == id);
        match capability.map(|c| (c.class.as_str(), c.risk.as_str())) {
            Some(("generated_output", _)) => 1,
            Some(("rebuildable_cache", "conditional_cache")) => 0,
            Some(("user_state" | "holding_area" | "logs", _))
            | Some(("rebuildable_cache", "review")) => 2,
            _ if matches!(
                id,
                "docker-desktop-storage"
                    | "ios-simulator-devices"
                    | "orbstack-storage"
                    | "application-support"
                    | "user-logs"
                    | "crash-reports"
            ) =>
            {
                2
            }
            _ => 3,
        }
    }
    pub fn grouped_totals(&self) -> [GroupTotal; 4] {
        let mut groups: [GroupTotal; 4] = Default::default();
        for row in &self.providers {
            let g = &mut groups[self.group_index(row)];
            g.providers += 1;
            g.allocated_bytes = g
                .allocated_bytes
                .saturating_add(row.allocated_bytes.unwrap_or(0));
            g.lower_bound |= !matches!(row.status, Status::Complete | Status::Absent);
        }
        groups
    }

    pub fn summary(&self) -> serde_json::Value {
        let sum = |f: fn(&ProviderReport) -> Option<u64>| {
            self.providers
                .iter()
                .filter_map(f)
                .fold(0u64, u64::saturating_add)
        };
        let count = |status| self.providers.iter().filter(|r| r.status == status).count();
        serde_json::json!({"logical_bytes": sum(|r| r.logical_bytes), "allocated_bytes": sum(|r| r.allocated_bytes),
            "lower_bound": self.providers.iter().any(|r| !matches!(r.status, Status::Complete | Status::Absent)),
            "complete": count(Status::Complete), "partial": count(Status::Partial), "denied": count(Status::Denied), "blocked": count(Status::Blocked), "absent": count(Status::Absent), "home_available_bytes": self.available_bytes})
    }
    pub fn render(&self) -> String {
        self.render_options(true, true)
    }
    pub fn render_options(&self, all: bool, verbose: bool) -> String {
        self.render_filtered(all, verbose, "all")
    }
    fn class_label(&self, row: &ProviderReport) -> &str {
        let id = row.provider.split('/').next().unwrap_or("");
        self.capabilities
            .iter()
            .find(|c| c.provider == id || super::registry::alias(&c.provider) == id)
            .map(|c| c.class.as_str())
            .unwrap_or(match id {
                "docker-desktop-storage" | "ios-simulator-devices" | "orbstack-storage" => {
                    "user_state"
                }
                "crash-reports" => "logs",
                _ => "unknown_content",
            })
    }
    pub fn render_filtered(&self, all: bool, verbose: bool, filter: &str) -> String {
        let included = |r: &ProviderReport| match filter {
            "cache" => self.group_index(r) == 0,
            "review" => self.group_index(r) == 2,
            _ => true,
        };
        let summary = self.summary();
        let lower = if summary["lower_bound"] == true {
            " (lower bound)"
        } else {
            ""
        };
        let mut text = format!(
            "Summary\nMeasured (all classes, not reclaimable) logical: {}{}; allocated: {}{}\nComplete: {}; partial: {}; denied: {}; blocked: {}; absent: {}\nHome volume available: {}\nOptions: --deep, --all, --verbose; retry: clean retry\n",
            crate::format_size(summary["logical_bytes"].as_u64().unwrap()),
            lower,
            crate::format_size(summary["allocated_bytes"].as_u64().unwrap()),
            lower,
            summary["complete"],
            summary["partial"],
            summary["denied"],
            summary["blocked"],
            summary["absent"],
            self.available_bytes
                .map(crate::format_size)
                .unwrap_or_else(|| "unknown".into())
        );
        let groups = self.grouped_totals();
        let mut group_lines = String::new();
        for (index, label) in [
            "Rebuildable cache",
            "Generated project output",
            "Review (user state, backups, simulators, app data)",
            "Unknown / not classified",
        ]
        .iter()
        .enumerate()
        {
            let g = &groups[index];
            if index == 1 && g.providers == 0 {
                continue;
            }
            group_lines.push_str(&format!(
                "{}: {}{}\n",
                label,
                crate::format_size(g.allocated_bytes),
                if g.lower_bound { " (lower bound)" } else { "" }
            ));
        }
        let insert_at = text.find("Complete:").unwrap();
        text.insert_str(insert_at, &group_lines);
        let estimate = self
            .home_used_bytes
            .and_then(|used| used.checked_sub(summary["allocated_bytes"].as_u64().unwrap()));
        text.push_str(&format!("Unaccounted space estimate (home volume used minus measured allocated): {}. Not a reclaimable-space estimate.\n", estimate.map(crate::format_size).unwrap_or_else(|| "unavailable".into())));
        text.push_str("Not covered:\n- Purgeable space is not quantified.\n- Time Machine local snapshots are not inventoried.\n- Other volumes are not covered.\n");
        if self.providers.iter().any(|r| r.status == Status::Skipped) {
            text.push_str("- Virtual or mounted filesystems skipped.\n");
        }
        if self
            .providers
            .iter()
            .any(|r| r.status == Status::Denied || r.errors > 0)
        {
            text.push_str("- Denied or partially inaccessible content has unknown size.\n");
        }
        if self
            .providers
            .iter()
            .any(|r| r.provider == "local-trash" && (r.status == Status::Denied || r.errors > 0))
        {
            text.push_str("- Trash size is unknown and may be large.\n");
        }
        text.push_str(
            "Downloads and Documents are not scanned unless explicitly scoped in configuration.\n",
        );
        let mut rows: Vec<_> = self
            .providers
            .iter()
            .filter(|r| r.allocated_bytes.is_some() && included(r))
            .collect();
        rows.sort_by(|a, b| {
            b.allocated_bytes
                .cmp(&a.allocated_bytes)
                .then(a.provider.cmp(&b.provider))
        });
        text.push_str("\nTop providers by allocated size\n");
        for r in rows.into_iter().take(if all { usize::MAX } else { 15 }) {
            let note = r.note.lines().next().unwrap_or("");
            text.push_str(&format!(
                "{} — {} — {:?} — {} — {}\n",
                r.provider,
                self.class_label(r),
                r.status,
                crate::format_size(r.allocated_bytes.unwrap()),
                note
            ));
        }
        text.push_str("\nDenied and blocked\n");
        for r in self
            .providers
            .iter()
            .filter(|r| matches!(r.status, Status::Denied | Status::Blocked | Status::Skipped))
        {
            let note = r.note.lines().next().unwrap_or("");
            text.push_str(&format!(
                "{} — {:?}: {}\n",
                r.provider,
                r.status,
                if verbose {
                    note.to_string()
                } else {
                    note.chars().take(120).collect()
                }
            ));
        }
        if all {
            text.push_str("\nAll providers\n");
            for r in self.providers.iter().filter(|r| included(r)) {
                text.push_str(&format!(
                    "{} — {} — {:?} — {:?} — logical: {}; allocated: {}; {}\n",
                    r.provider,
                    self.class_label(r),
                    r.status,
                    r.root,
                    r.logical_bytes
                        .map(crate::format_size)
                        .unwrap_or_else(|| "unknown".into()),
                    r.allocated_bytes
                        .map(crate::format_size)
                        .unwrap_or_else(|| "unknown".into()),
                    r.note
                ));
            }
        }
        if verbose {
            text.push_str("\nSafety notes\n");
            text.push_str(&self.notes.join("\n"));
            text.push('\n');
        } else {
            text.push_str("\nSafety notes: use --verbose.\n");
        }
        text
    }
}
