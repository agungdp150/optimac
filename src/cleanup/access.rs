//! Bounded root access observations. There is no reliable global FDA boolean.
use super::*;

pub const GUIDANCE: &str = "Permission denied or partial access: ownership, ACLs, sandboxing or macOS privacy restrictions may be responsible. For applicable macOS privacy denials, review System Settings > Privacy & Security > Full Disk Access for the launching terminal/app, restart that host, then retry. OptiMac cannot infer missing Full Disk Access from a denial alone. Retry: optimac clean retry with the same --config/--deep options; in Storage inventory press r.";

pub(super) fn preflight(
    reports: &mut [ProviderReport],
    cancel: &AtomicBool,
    deadline: Instant,
    progress: &mut dyn FnMut(&ProviderReport),
) {
    for report in reports {
        if report.status != Status::Complete || report.logical_bytes.is_some() {
            continue;
        }
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            report.status = if cancel.load(Ordering::Relaxed) {
                Status::Cancelled
            } else {
                Status::Partial
            };
            report.note = "Access preflight incomplete; root access unknown".into();
            continue;
        }
        // Only enabled/discovered roots, never unrelated privacy sentinels.
        let result = validate(&report.root).and_then(|_| fs::read_dir(&report.root).map(|_| ()));
        match result {
            Ok(()) => report.access = "readable".into(),
            Err(error) => {
                report.status = status(&error);
                report.access = match report.status {
                    Status::Absent => "absent",
                    Status::Denied => "denied",
                    _ => "unknown",
                }
                .into();
                let previous_note = report.note.clone();
                report.note = if report.status == Status::Denied {
                    GUIDANCE.into()
                } else if previous_note.is_empty() {
                    error.to_string()
                } else {
                    format!("{previous_note}; {error}")
                };
            }
        }
        progress(report);
    }
}
