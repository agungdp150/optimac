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
    pub errors: u64,
    pub note: String,
}
#[derive(Debug, Serialize)]
pub struct Inventory {
    pub schema_version: u32,
    pub read_only: bool,
    pub providers: Vec<ProviderReport>,
    pub available_bytes: Option<u64>,
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
impl Inventory {
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
        let summary = self.summary();
        let lower = if summary["lower_bound"] == true {
            " (lower bound)"
        } else {
            ""
        };
        let mut text = format!(
            "Summary\nMeasured logical: {}{}; allocated: {}{}\nComplete: {}; partial: {}; denied: {}; blocked: {}; absent: {}\nHome volume available: {}\nOptions: --deep, --all, --verbose; retry: clean retry\n",
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
        text.push_str(
            "Downloads and Documents are not scanned unless explicitly scoped in configuration.\n",
        );
        let mut rows: Vec<_> = self
            .providers
            .iter()
            .filter(|r| r.allocated_bytes.is_some())
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
                "{} — {:?} — {} — {}\n",
                r.provider,
                r.status,
                crate::format_size(r.allocated_bytes.unwrap()),
                note
            ));
        }
        text.push_str("\nDenied and blocked\n");
        for r in self
            .providers
            .iter()
            .filter(|r| matches!(r.status, Status::Denied | Status::Blocked))
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
            for r in &self.providers {
                text.push_str(&format!(
                    "{} — {:?} — {:?} — logical: {}; allocated: {}; {}\n",
                    r.provider,
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
