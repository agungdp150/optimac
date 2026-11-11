//! Read-only status snapshot. Missing metrics remain unavailable.
use super::command;
use std::{sync::atomic::AtomicBool, time::SystemTime};

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub collected: Option<SystemTime>,
    pub os: Option<String>,
    pub cpu: Option<String>,
    pub load: Option<String>,
    pub memory_total: Option<u64>,
    pub memory_available: Option<u64>,
    pub disk_total: Option<u64>,
    pub disk_available: Option<u64>,
    pub battery: Option<String>,
    pub processes: Vec<String>,
    pub warnings: Vec<String>,
}
fn number(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}
fn kv<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(k, _)| k.trim() == key)
            .map(|(_, v)| v.trim())
    })
}
fn vm_available(text: &str) -> Option<u64> {
    let page_size = text
        .lines()
        .next()?
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    let mut total = 0u64;
    for key in ["Pages free", "Pages inactive", "Pages speculative"] {
        let raw = kv(text, key)?.trim_end_matches('.');
        total = total.checked_add(number(raw)?)?;
    }
    total.checked_mul(page_size)
}
fn disk(text: &str) -> Option<(u64, u64)> {
    let row = text.lines().nth(1)?;
    let cols: Vec<_> = row.split_whitespace().collect();
    Some((
        number(cols.get(1)?)?.checked_mul(1024)?,
        number(cols.get(3)?)?.checked_mul(1024)?,
    ))
}
fn probe(
    name: &str,
    args: &[&str],
    cancel: &AtomicBool,
    warnings: &mut Vec<String>,
) -> Option<String> {
    match command(name, args, cancel) {
        Ok(v) => Some(v),
        Err(e) => {
            warnings.push(format!(
                "{}: {}",
                name.rsplit('/').next().unwrap_or(name),
                e
            ));
            None
        }
    }
}
pub fn collect(cancel: &AtomicBool) -> Snapshot {
    let mut s = Snapshot::default();
    if let Some(out) = probe("/usr/bin/sw_vers", &[], cancel, &mut s.warnings) {
        s.os = kv(&out, "ProductVersion").map(|v| format!("macOS {v}"));
    }
    if let Some(out) = probe(
        "/usr/sbin/sysctl",
        &["-n", "machdep.cpu.brand_string"],
        cancel,
        &mut s.warnings,
    ) {
        s.cpu = Some(out.trim().to_string());
    }
    if let Some(out) = probe(
        "/usr/sbin/sysctl",
        &["-n", "hw.memsize"],
        cancel,
        &mut s.warnings,
    ) {
        s.memory_total = number(&out);
    }
    if let Some(out) = probe("/usr/bin/vm_stat", &[], cancel, &mut s.warnings) {
        s.memory_available = vm_available(&out);
        if s.memory_available.is_none() {
            s.warnings
                .push("Memory estimate unavailable: vm_stat format not recognized".into());
        }
    }
    if let Some(out) = probe("/bin/df", &["-k", "/"], cancel, &mut s.warnings) {
        if let Some((total, free)) = disk(&out) {
            s.disk_total = Some(total);
            s.disk_available = Some(free);
        } else {
            s.warnings
                .push("Disk capacity format not recognized".into());
        }
    }
    if let Some(out) = probe(
        "/usr/sbin/sysctl",
        &["-n", "vm.loadavg"],
        cancel,
        &mut s.warnings,
    ) {
        s.load = Some(
            out.trim()
                .trim_matches(|c| c == '{' || c == '}')
                .trim()
                .to_string(),
        );
    }
    if let Some(out) = probe("/usr/bin/pmset", &["-g", "batt"], cancel, &mut s.warnings) {
        s.battery = out
            .lines()
            .find(|l| l.contains('%'))
            .map(|l| l.trim().to_string());
    }
    if let Some(out) = probe(
        "/bin/ps",
        &["-A", "-o", "%cpu=,rss=,comm=", "-r"],
        cancel,
        &mut s.warnings,
    ) {
        s.processes = out.lines().take(5).map(|l| l.trim().to_string()).collect();
    }
    s.collected = Some(SystemTime::now());
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_measurements() {
        let vm = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free: 100.\nPages inactive: 50.\nPages speculative: 10.\nPages purgeable: 5.\n";
        assert_eq!(vm_available(vm), Some(160 * 16384));
        assert_eq!(vm_available("bad"), None);
        assert_eq!(
            disk(
                "Filesystem 1024-blocks Used Available Capacity Mounted\n/dev/disk 1000 600 400 60% /\n"
            ),
            Some((1024000, 409600))
        );
        assert_eq!(disk("bad"), None);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Detail {
    pub collected: Option<SystemTime>,
    pub sections: Vec<(String, Vec<(String, String)>)>,
}
fn detail_probe(name: &str, args: &[&str], cancel: &AtomicBool) -> String {
    match command(name, args, cancel) {
        Ok(output) if output.trim().is_empty() => "None reported".into(),
        Ok(output) => output.trim().to_string(),
        Err(_) => "Unavailable".into(),
    }
}
fn rows_from_vm(text: &str) -> Vec<(String, String)> {
    let page = text
        .lines()
        .next()
        .and_then(|s| s.split("page size of ").nth(1))
        .and_then(|s| s.split_whitespace().next())
        .and_then(number);
    [
        ("Free", "Pages free"),
        ("Active", "Pages active"),
        ("Inactive", "Pages inactive"),
        ("Speculative", "Pages speculative"),
        ("Wired", "Pages wired down"),
        ("Compressed", "Pages occupied by compressor"),
    ]
    .iter()
    .map(|(label, key)| {
        let value = page
            .and_then(|p| {
                kv(text, key)
                    .and_then(|v| number(v.trim_end_matches('.')))
                    .and_then(|n| n.checked_mul(p))
            })
            .map(crate::format_size)
            .unwrap_or_else(|| "Unavailable".into());
        (label.to_string(), value)
    })
    .collect()
}
pub fn collect_detail(
    action: super::Action,
    home: &std::path::Path,
    cancel: &AtomicBool,
) -> Detail {
    use super::Action;
    let mut detail = Detail::default();
    match action {
        Action::Memory => {
            let vm = detail_probe("/usr/bin/vm_stat", &[], cancel);
            detail
                .sections
                .push(("Memory pages".into(), rows_from_vm(&vm)));
            let swap = detail_probe("/usr/sbin/sysctl", &["vm.swapusage"], cancel);
            let swap = swap.split_once(':').map(|(_, v)| v.trim()).unwrap_or(&swap);
            detail.sections.push((
                "Swap".into(),
                vec![
                    ("Reported".into(), swap.into()),
                    ("Note".into(), "Page counts are not memory pressure".into()),
                ],
            ));
        }
        Action::Storage => {
            let df = detail_probe("/bin/df", &["-k", "/"], cancel);
            let values = match disk(&df) {
                Some((total, avail)) => vec![
                    ("Total".into(), crate::format_size(total)),
                    ("Available".into(), crate::format_size(avail)),
                    (
                        "Note".into(),
                        "APFS available is not the same as reclaimable".into(),
                    ),
                ],
                None => vec![("Capacity".into(), "Unavailable".into())],
            };
            detail.sections.push(("Startup volume".into(), values));
            let snaps = detail_probe("/usr/bin/tmutil", &["listlocalsnapshots", "/"], cancel);
            let rows: Vec<_> = snaps
                .lines()
                .filter(|s| s.trim().starts_with("com.apple.TimeMachine."))
                .take(100)
                .enumerate()
                .map(|(i, v)| (format!("{}", i + 1), v.trim().to_string()))
                .collect();
            detail.sections.push((
                "Time Machine snapshots".into(),
                if rows.is_empty() {
                    vec![(
                        "Status".into(),
                        if snaps == "Unavailable" {
                            "Unavailable"
                        } else {
                            "None reported"
                        }
                        .into(),
                    )]
                } else {
                    rows
                },
            ));
        }
        Action::Doctor => {
            let checks = [
                ("FileVault", "/usr/bin/fdesetup", &["status"][..]),
                ("SIP", "/usr/bin/csrutil", &["status"][..]),
                ("Gatekeeper", "/usr/sbin/spctl", &["--status"][..]),
                (
                    "Firewall",
                    "/usr/libexec/ApplicationFirewall/socketfilterfw",
                    &["--getglobalstate"][..],
                ),
            ];
            detail.sections.push((
                "Security checks".into(),
                checks
                    .iter()
                    .map(|(label, program, args)| {
                        (
                            label.to_string(),
                            detail_probe(program, args, cancel).replace('\n', " "),
                        )
                    })
                    .collect(),
            ));
        }
        Action::AiTools => {
            let providers = [
                ("Codex", ".codex"),
                ("Claude", ".claude"),
                ("Grok", ".grok"),
                ("Antigravity", ".antigravity"),
            ];
            detail.sections.push((
                "Local directory hints".into(),
                providers
                    .iter()
                    .map(|(name, path)| {
                        (
                            name.to_string(),
                            if home.join(path).is_dir() {
                                "Directory found; installation unverified"
                            } else {
                                "Not detected"
                            }
                            .into(),
                        )
                    })
                    .collect(),
            ));
            detail.sections.push((
                "Usage & quota".into(),
                vec![
                    ("Token usage".into(), "Unavailable".into()),
                    ("Account quota".into(), "Unavailable".into()),
                    ("Cache cleanup".into(), "Unsupported until verified".into()),
                ],
            ));
        }
        _ => {}
    }
    detail.collected = Some(SystemTime::now());
    detail
}
