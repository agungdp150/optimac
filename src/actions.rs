//! Bounded read-only feature jobs shared by the CLI and menu.
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const COMMANDS: &[&str] = &[
    "inspect",
    "clean",
    "clean",
    "analyze",
    "duplicates",
    "artifacts",
    "updates",
    "login",
    "threats",
    "space",
    "orphans",
    "doctor",
    "apps",
    "optimize",
    "status",
];
pub fn description(index: usize) -> &'static str {
    match index {
        1 | 2 => {
            "Review conservative developer-cache candidates. Execution remains disabled until recoverable cleanup is ready."
        }
        3 => "Largest regular files in the selected scope; logical sizes, read-only.",
        4 => "Compare same-sized local files byte for byte. Bounded scan; no automatic deletion.",
        5 => "Find project build and dependency directories in the selected scope.",
        6 => "List outdated Homebrew packages without updating or installing anything.",
        7 => "List user and system launch agent/daemon files. No services are changed.",
        8 => {
            "Check names against the legacy adware/PUP signatures. Not a malware verdict or antivirus scan."
        }
        9 => "Inspect disk capacity and local Time Machine snapshots.",
        10 => {
            "List Application Support folders for manual review. Absence of an app is not proof that its data is disposable."
        }
        11 => "Read FileVault, SIP, Gatekeeper and firewall status.",
        12 => {
            "List installed application bundles. Removal remains disabled pending safe recovery support."
        }
        13 => {
            "Read memory statistics and swap usage. No purge, service restart or process termination."
        }
        14 => "Read macOS, CPU, memory and filesystem status.",
        _ => "Inspect metadata in the selected scope with cancellation and scan limits.",
    }
}
fn home() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME is unavailable"))
}
fn safe_text(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '�'
            } else {
                c
            }
        })
        .collect()
}
fn command(program: &str, args: &[&str], cancel: &AtomicBool) -> io::Result<String> {
    use std::os::unix::process::CommandExt;
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::other("Command cancelled"));
    }
    let mut child = Command::new(program)
        .args(args)
        .process_group(0)
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    // Drain continuously to avoid pipe deadlocks, retaining at most 256 KiB.
    fn drain(mut pipe: impl Read) -> Vec<u8> {
        let mut data = Vec::new();
        let mut buf = [0; 4096];
        while let Ok(n) = pipe.read(&mut buf) {
            if n == 0 {
                break;
            }
            let keep = n.min(262144usize.saturating_sub(data.len()));
            data.extend_from_slice(&buf[..keep]);
        }
        data
    }
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = out_tx.send(drain(stdout));
    });
    std::thread::spawn(move || {
        let _ = err_tx.send(drain(stderr));
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if cancel.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(15) {
            // The child owns a separate process group, including normal descendants.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(
                "Command cancelled or timed out; partial output discarded",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // A descendant may retain a pipe even after the direct child exits.
    let remaining = || Duration::from_secs(15).saturating_sub(start.elapsed());
    let output = out_rx.recv_timeout(remaining());
    let error = err_rx.recv_timeout(remaining());
    let (output, error) = match (output, error) {
        (Ok(out), Ok(err)) => (out, err),
        _ => {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            return Err(io::Error::other("Output collection timed out"));
        }
    };
    let text = safe_text(&format!(
        "{}{}",
        String::from_utf8_lossy(&output),
        String::from_utf8_lossy(&error)
    ));
    if !status.success() {
        return Err(io::Error::other(format!(
            "{program} exited {status}: {text}"
        )));
    }
    Ok(text)
}
fn commands(items: &[(&str, &[&str])], cancel: &AtomicBool) -> String {
    let mut result = String::new();
    for (name, args) in items {
        if cancel.load(Ordering::Relaxed) {
            result.push_str("Cancelled\n");
            break;
        }
        result.push_str(&format!("\n{name} {}\n", args.join(" ")));
        match command(name, args, cancel) {
            Ok(s) => result.push_str(&s),
            Err(e) => result.push_str(&format!("Unavailable: {e}\n")),
        }
    }
    result
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
        if count >= 100000 || start.elapsed() > Duration::from_secs(30) {
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
            if depth >= 31 {
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
pub fn run(index: usize, root: &Path, cancel: &AtomicBool) -> io::Result<String> {
    let h = home()?;
    match index {
        1 | 2 => {
            let plan = crate::recovery::preview(&h, cancel)?;
            Ok(format!(
                "Cleanup preview only. Execution remains disabled pending safety validation.\n{}",
                crate::recovery::describe(&plan)
            ))
        }
        3..=5 => {
            let (mut entries, mut s) = walk(root, cancel)?;
            if index == 3 {
                entries.retain(|e| !e.dir);
                entries.sort_by_key(|e| std::cmp::Reverse(e.size));
                for e in entries.iter().take(200) {
                    s.push_str(&format!("{:>12}  {:?}\n", e.size, e.path));
                }
            } else if index == 5 {
                s.push_str("Build/dependency candidates; manual review only:\n");
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
                    s.push_str(&format!("{:?}\n", e.path));
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
        6 => {
            let brew = if Path::new("/opt/homebrew/bin/brew").exists() {
                "/opt/homebrew/bin/brew"
            } else {
                "/usr/local/bin/brew"
            };
            command(brew, &["outdated", "--greedy"], cancel)
        }
        7 => Ok([
            h.join("Library/LaunchAgents"),
            PathBuf::from("/Library/LaunchAgents"),
            PathBuf::from("/Library/LaunchDaemons"),
        ]
        .iter()
        .map(|p| listing(p))
        .collect()),
        8 => {
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
        9 => Ok(commands(
            &[
                ("/bin/df", &["-h"]),
                ("/usr/bin/tmutil", &["listlocalsnapshots", "/"]),
            ],
            cancel,
        )),
        10 => Ok(format!(
            "Manual leftover review — these are NOT confirmed orphans. No deletion is offered.\n{}",
            listing(&h.join("Library/Application Support"))
        )),
        11 => Ok(commands(
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
        12 => Ok(format!(
            "Application inventory. Uninstall is disabled pending safe recovery support.\n{}{}",
            listing(Path::new("/Applications")),
            listing(&h.join("Applications"))
        )),
        13 => Ok(format!(
            "Memory diagnostics — no purge or system changes.\n{}",
            commands(
                &[
                    ("/usr/bin/vm_stat", &[]),
                    ("/usr/sbin/sysctl", &["vm.swapusage"])
                ],
                cancel
            )
        )),
        14 => Ok(commands(
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
