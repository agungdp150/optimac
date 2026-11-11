mod process;
pub(crate) use process::command;
use process::commands;
// Bounded read-only feature jobs shared by the CLI and menu.
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Inspect,
    CleanPreview,
    Clean,
    LargeFiles,
    Duplicates,
    Artifacts,
    Updates,
    Login,
    Threats,
    Storage,
    Orphans,
    Doctor,
    Apps,
    Memory,
    Status,
    Restore,
    AiTools,
}
impl Action {
    pub fn command(name: &str) -> Option<Self> {
        Some(match name {
            "clean" => Self::CleanPreview,
            "analyze" => Self::LargeFiles,
            "duplicates" => Self::Duplicates,
            "artifacts" => Self::Artifacts,
            "updates" => Self::Updates,
            "login" => Self::Login,
            "space" => Self::Storage,
            "doctor" => Self::Doctor,
            "apps" => Self::Apps,
            "optimize" => Self::Memory,
            "status" => Self::Status,
            _ => return None,
        })
    }
}
pub fn description(index: Action) -> &'static str {
    match index {
        Action::CleanPreview | Action::Clean => {
            "Review Documents, Programming, Browser, Media and other groups. Personal files must be individually selected before confirmation."
        }
        Action::LargeFiles => {
            "Largest regular files in the selected scope; logical sizes, read-only."
        }
        Action::Duplicates => {
            "Compare same-sized local files byte for byte. Bounded scan; no automatic deletion."
        }
        Action::Artifacts => "Find project build and dependency directories in the selected scope.",
        Action::Updates => {
            "List outdated Homebrew packages without updating or installing anything."
        }
        Action::Login => "List user and system launch agent/daemon files. No services are changed.",
        Action::Threats => {
            "Check names against the legacy adware/PUP signatures. Not a malware verdict or antivirus scan."
        }
        Action::Storage => "Inspect disk capacity and local Time Machine snapshots.",
        Action::Orphans => {
            "List Application Support folders for manual review. Absence of an app is not proof that its data is disposable."
        }
        Action::Doctor => "Read FileVault, SIP, Gatekeeper and firewall status.",
        Action::Apps => {
            "Select an owned app bundle, review and confirm moving it to recovery. Personal data is preserved."
        }
        Action::Memory => {
            "Read memory statistics and swap usage. No purge, service restart or process termination."
        }
        Action::Status => "Read macOS, CPU, memory and filesystem status.",
        Action::Restore => {
            "Select a Rust recovery record and restore without overwriting existing files."
        }
        Action::AiTools => "Show local AI-tool directory hints and unavailable usage/quota states.",
        _ => "Inspect metadata in the selected scope with cancellation and scan limits.",
    }
}
/// Read-only hints only: directory existence does not verify a provider or version.
pub fn ai_tools_status(home: &Path) -> String {
    let mut output = String::from(
        "AI tools — local directory hints only; installation and version not verified.\nNo network requests or agent commands run.\n\n",
    );
    for (name, path) in [
        ("Codex", ".codex"),
        ("Claude", ".claude"),
        ("Grok", ".grok"),
        ("Antigravity", ".antigravity"),
    ] {
        output.push_str(&format!("{name}: {}\n  Cache cleanup: unsupported until verified\n  Token usage: unavailable\n  Quota: unavailable\n", if home.join(path).is_dir() { "local directory found (not verified)" } else { "not detected" }));
    }
    output.push_str("\nLocal usage, account usage and quotas require verified provider-specific sources. Missing values are not zero or unlimited.\n");
    output
}
fn home() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME is unavailable"))
}

fn listing(root: &Path) -> String {
    match fs::read_dir(root) {
        Ok(entries) => {
            let mut lines: Vec<_> = entries
                .take(2000)
                .map(|e| match e {
                    Ok(e) => format!("{:?}", e.path()),
                    Err(e) => format!("Error: {e}"),
                })
                .collect();
            lines.sort();
            format!(
                "{} (up to 2000 entries)\n{}\n",
                root.display(),
                lines.join("\n")
            )
        }
        Err(e) => format!("{}: {e}\n", root.display()),
    }
}
#[derive(Debug)]
struct Entry {
    path: PathBuf,
    size: u64,
    dir: bool,
}
fn walk(root: &Path, cancel: &AtomicBool) -> io::Result<(Vec<Entry>, String)> {
    let root = root.canonicalize()?;
    let meta = fs::metadata(&root)?;
    #[cfg(unix)]
    let device = {
        use std::os::unix::fs::MetadataExt;
        meta.dev()
    };
    let mut stack = vec![(fs::read_dir(root)?, 0)];
    let mut entries = Vec::new();
    let mut skipped = 0;
    let mut errors = 0;
    let mut count = 0;
    let start = Instant::now();
    let mut reason = "Completed";
    while let Some((iter, depth)) = stack.last_mut() {
        if cancel.load(Ordering::Relaxed) {
            reason = "Cancelled — partial results";
            break;
        }
        if count >= crate::limits::SCAN_MAX_ENTRIES || start.elapsed() > crate::limits::SCAN_TIMEOUT
        {
            reason = "Scan limit — partial results";
            break;
        }
        let depth = *depth;
        let Some(entry) = iter.next() else {
            stack.pop();
            continue;
        };
        count += 1;
        let e = match entry {
            Ok(e) => e,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let path = e.path();
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let name = e.file_name();
        let name = name.to_string_lossy();
        if m.is_symlink()
            || matches!(
                name.as_ref(),
                ".ssh"
                    | ".aws"
                    | ".git"
                    | "Keychains"
                    | "CloudStorage"
                    | "Mobile Documents"
                    | ".Spotlight-V100"
            )
        {
            skipped += 1;
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if m.dev() != device {
                skipped += 1;
                continue;
            }
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::macos::fs::MetadataExt;
            if m.st_flags() & 0x40000000 != 0 {
                skipped += 1;
                continue;
            }
        }
        if m.is_file() || m.is_dir() {
            entries.push(Entry {
                path: path.clone(),
                size: m.len(),
                dir: m.is_dir(),
            });
        }
        if m.is_dir() {
            if depth + 1 >= crate::limits::SCAN_MAX_DEPTH {
                skipped += 1;
                continue;
            }
            match fs::read_dir(path) {
                Ok(d) => stack.push((d, depth + 1)),
                Err(_) => errors += 1,
            }
        }
    }
    Ok((
        entries,
        format!(
            "{reason}; visited {count}; skipped {skipped}; errors {errors}. Logical sizes, not reclaimable space.\n"
        ),
    ))
}
fn equal(a: &Path, b: &Path, cancel: &AtomicBool, deadline: Instant) -> io::Result<bool> {
    let mut a = fs::File::open(a)?;
    let mut b = fs::File::open(b)?;
    let mut x = [0; 65536];
    let mut y = [0; 65536];
    loop {
        if cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
            return Err(io::Error::other("Comparison cancelled or budget reached"));
        }
        let n = a.read(&mut x)?;
        let m = b.read(&mut y)?;
        if n != m || x[..n] != y[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}
pub(crate) fn review_paths(root: &Path, cancel: &AtomicBool) -> io::Result<(Vec<PathBuf>, String)> {
    let (entries, note) = walk(root, cancel)?;
    Ok((
        entries
            .into_iter()
            .filter(|e| !e.dir)
            .map(|e| e.path)
            .collect(),
        note,
    ))
}
pub fn run(index: Action, root: &Path, cancel: &AtomicBool) -> io::Result<String> {
    let h = home()?;
    match index {
        Action::CleanPreview | Action::Clean => {
            let plan = crate::recovery::review_files(&h, root, cancel)?;
            Ok(format!(
                "Grouped file review only. Select individual files in the Run cleanup menu to perform recoverable moves.\n{}",
                crate::recovery::describe(&plan)
            ))
        }
        Action::LargeFiles | Action::Duplicates | Action::Artifacts => {
            let (mut entries, mut s) = walk(root, cancel)?;
            if index == Action::LargeFiles {
                entries.retain(|e| !e.dir);
                entries.sort_by_key(|e| std::cmp::Reverse(e.size));
                for e in entries.iter().take(200) {
                    s.push_str(&format!(
                        "{:>12}  {:?}\n",
                        crate::format_size(e.size),
                        e.path
                    ));
                }
            } else if index == Action::Artifacts {
                s.push_str("Build/dependency candidates; observed file totals (partial if entries were skipped or limits reached); manual review only:\n");
                for e in entries
                    .iter()
                    .filter(|e| {
                        e.dir
                            && matches!(
                                e.path.file_name().and_then(|n| n.to_str()),
                                Some(
                                    "node_modules"
                                        | "target"
                                        | ".next"
                                        | "__pycache__"
                                        | "build"
                                        | "dist"
                                )
                            )
                    })
                    .take(200)
                {
                    let total = entries
                        .iter()
                        .filter(|file| !file.dir && file.path.starts_with(&e.path))
                        .fold(0u64, |sum, file| sum.saturating_add(file.size));
                    s.push_str(&format!(
                        "{:>12}  {:?}\n",
                        crate::format_size(total),
                        e.path
                    ));
                }
            } else {
                s.push_str("Exact byte comparisons at scan time; files may change afterward. Hard links excluded.\n");
                let mut groups: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();
                #[cfg(unix)]
                let mut seen = std::collections::HashSet::new();
                for e in entries.iter().filter(|e| !e.dir) {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        if let Ok(m) = fs::metadata(&e.path)
                            && !seen.insert((m.dev(), m.ino()))
                        {
                            continue;
                        }
                    }
                    groups.entry(e.size).or_default().push(e.path.clone());
                }
                let deadline = Instant::now() + Duration::from_secs(15);
                let mut comparisons = 0;
                let mut matches = 0;
                'outer: for paths in groups.values().filter(|p| p.len() > 1) {
                    for i in 0..paths.len() {
                        for j in i + 1..paths.len() {
                            comparisons += 1;
                            if comparisons > 1000 || matches >= 200 {
                                s.push_str("Comparison/output limit reached; partial results\n");
                                break 'outer;
                            }
                            match equal(&paths[i], &paths[j], cancel, deadline) {
                                Ok(true) => {
                                    matches += 1;
                                    s.push_str(&format!("{:?}\n  = {:?}\n", paths[i], paths[j]));
                                }
                                Ok(false) => {}
                                Err(e) => {
                                    s.push_str(&format!("Comparison incomplete: {e}\n"));
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                s.push_str(&format!(
                    "{matches} matching pairs; {comparisons} comparisons attempted\n"
                ));
            }
            Ok(s)
        }
        Action::Updates => {
            let brew = if Path::new("/opt/homebrew/bin/brew").exists() {
                "/opt/homebrew/bin/brew"
            } else {
                "/usr/local/bin/brew"
            };
            command(brew, &["outdated", "--greedy"], cancel)
        }
        Action::Login => Ok([
            h.join("Library/LaunchAgents"),
            PathBuf::from("/Library/LaunchAgents"),
            PathBuf::from("/Library/LaunchDaemons"),
        ]
        .iter()
        .map(|p| listing(p))
        .collect()),
        Action::Threats => {
            let mut s = String::from(
                "Legacy filename signature review only. Matches are not proof of malware; no matches is not a clean bill of health.\n",
            );
            let signatures = [
                "genieo",
                "vsearch",
                "conduit",
                "spigot",
                "pirrit",
                "mackeeper",
                "advancedmaccleaner",
                "mac auto fixer",
                "installmac",
                "myshopcoupon",
                "trovi",
                "searchmine",
                "searchpulse",
                "weknow",
                "chillsearch",
                "bundlore",
                "adload",
                "shlayer",
                "crossrider",
            ];
            for p in [
                h.join("Library/LaunchAgents"),
                h.join("Library/Application Support"),
                h.join("Applications"),
                PathBuf::from("/Applications"),
                PathBuf::from("/Library/LaunchAgents"),
                PathBuf::from("/Library/LaunchDaemons"),
            ] {
                match fs::read_dir(&p) {
                    Ok(es) => {
                        for e in es.take(5000) {
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(s + "Cancelled\n");
                            }
                            match e {
                                Ok(e) => {
                                    if signatures.iter().any(|sig| {
                                        e.file_name().to_string_lossy().to_lowercase().contains(sig)
                                    }) {
                                        s.push_str(&format!("Review {:?}\n", e.path()));
                                    }
                                }
                                Err(e) => s.push_str(&format!("Read error: {e}\n")),
                            }
                        }
                    }
                    Err(e) => s.push_str(&format!("{}: {e}\n", p.display())),
                }
            }
            Ok(s)
        }
        Action::Storage => Ok(commands(
            &[
                ("/bin/df", &["-h"]),
                ("/usr/bin/tmutil", &["listlocalsnapshots", "/"]),
            ],
            cancel,
        )),
        Action::Orphans => Ok(format!(
            "Manual leftover review — these are NOT confirmed orphans. No deletion is offered.\n{}",
            listing(&h.join("Library/Application Support"))
        )),
        Action::Doctor => Ok(commands(
            &[
                ("/usr/bin/fdesetup", &["status"]),
                ("/usr/bin/csrutil", &["status"]),
                ("/usr/sbin/spctl", &["--status"]),
                (
                    "/usr/libexec/ApplicationFirewall/socketfilterfw",
                    &["--getglobalstate"],
                ),
            ],
            cancel,
        )),
        Action::Apps => Ok(format!(
            "Application inventory. Uninstall is disabled pending safe recovery support.\n{}{}",
            listing(Path::new("/Applications")),
            listing(&h.join("Applications"))
        )),
        Action::Memory => Ok(format!(
            "Memory diagnostics — no purge or system changes.\n{}",
            commands(
                &[
                    ("/usr/bin/vm_stat", &[]),
                    ("/usr/sbin/sysctl", &["vm.swapusage"])
                ],
                cancel
            )
        )),
        Action::Status => Ok(commands(
            &[
                ("/usr/bin/sw_vers", &[]),
                ("/usr/sbin/sysctl", &["hw.memsize", "hw.ncpu"]),
                ("/usr/bin/vm_stat", &[]),
                ("/bin/df", &["-h"]),
            ],
            cancel,
        )),
        _ => Err(io::Error::other("Unknown action")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_use_readable_headings_and_report_failures() {
        let cancel = AtomicBool::new(false);
        let result = commands(
            &[
                ("/usr/bin/printf", &["example\n"]),
                ("/not-a-real-optimac-command", &[]),
            ],
            &cancel,
        );
        assert!(result.contains("Details\n  example"));
        assert!(result.contains("Unavailable:"));
    }
    #[test]
    fn subprocess_cancellation_and_failure_are_reported() {
        let cancel = AtomicBool::new(true);
        assert!(command("/bin/sleep", &["10"], &cancel).is_err());
        cancel.store(false, Ordering::Relaxed);
        assert!(command("/usr/bin/false", &[], &cancel).is_err());
        assert_eq!(
            command("/usr/bin/printf", &["hello"], &cancel).unwrap(),
            "hello"
        );
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                cancel.store(true, Ordering::Relaxed);
            });
            let start = Instant::now();
            assert!(command("/bin/sleep", &["10"], &cancel).is_err());
            assert!(start.elapsed() < Duration::from_secs(3));
        });
    }
    #[test]
    fn byte_comparison_is_exact_and_cancellable() {
        let d = std::env::temp_dir().join(format!("optimac-compare-{}", std::process::id()));
        fs::create_dir(&d).unwrap();
        let a = d.join("a");
        let b = d.join("b");
        fs::write(&a, b"abc").unwrap();
        fs::write(&b, b"abd").unwrap();
        let c = AtomicBool::new(false);
        assert!(!equal(&a, &b, &c, Instant::now() + Duration::from_secs(1)).unwrap());
        fs::write(&b, b"abc").unwrap();
        assert!(equal(&a, &b, &c, Instant::now() + Duration::from_secs(1)).unwrap());
        c.store(true, Ordering::Relaxed);
        assert!(equal(&a, &b, &c, Instant::now() + Duration::from_secs(1)).is_err());
        fs::remove_dir_all(d).unwrap();
    }
}
