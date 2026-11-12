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
    pub fn render(&self) -> String {
        let mut text = self.notes.join("\n");
        text.push_str(&format!(
            "\n\nCoverage: {}\nHome volume available: {}\n",
            if self.complete {
                "complete for discovered roots"
            } else {
                "partial; inspect provider statuses"
            },
            self.available_bytes
                .map(crate::format_size)
                .unwrap_or_else(|| "unavailable".into())
        ));
        let readable = self
            .providers
            .iter()
            .filter(|r| r.access == "readable")
            .count();
        let denied = self
            .providers
            .iter()
            .filter(|r| r.access == "denied" || r.errors > 0)
            .count();
        let absent = self
            .providers
            .iter()
            .filter(|r| r.status == Status::Absent)
            .count();
        text.push_str(&format!("Access: {readable} roots readable; {denied} inaccessible or partially inaccessible; {absent} absent.\n"));
        if denied > 0 {
            text.push_str(super::access::GUIDANCE);
            text.push('\n');
        }
        text.push_str("Retry inventory: CLI clean retry (same options); TUI r.\n");
        for r in &self.providers {
            text.push_str(&format!("\n{} — {:?}\n  {:?}\n  logical: {}; allocated estimate: {}; files: {}; skipped: {}; errors: {}\n",r.provider,r.status,r.root,r.logical_bytes.map(crate::format_size).unwrap_or_else(||"unknown".into()),r.allocated_bytes.map(crate::format_size).unwrap_or_else(||"unknown".into()),r.files,r.skipped,r.errors));
            if !r.note.is_empty() {
                text.push_str(&format!("  {:?}\n", r.note));
            }
        }
        text
    }
}
