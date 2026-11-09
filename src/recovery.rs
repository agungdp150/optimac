//! Conservative, journaled, same-volume cache cleanup. Never permanently deletes.
use serde::{Deserialize, Serialize};
use std::os::unix::{
    ffi::OsStrExt,
    fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub path: PathBuf,
    dev: u64,
    ino: u64,
    size: u64,
    modified: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub items: Vec<Candidate>,
    pub notes: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct Record {
    version: u32,
    candidate: Candidate,
}
fn error(text: &str) -> io::Error {
    io::Error::other(text)
}
fn validate_chain(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(error("Absolute path required"));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, std::path::Component::ParentDir) {
            return Err(error("Parent traversal refused"));
        }
        current.push(part.as_os_str());
        if fs::symlink_metadata(&current)?.is_symlink() {
            return Err(error("Symlink ancestor refused"));
        }
    }
    Ok(())
}
fn cache_roots(home: &Path) -> Vec<PathBuf> {
    [
        ".cache/go-build",
        ".cache/pip",
        ".cache/uv",
        ".npm/_cacache",
        ".cargo/registry/cache",
    ]
    .iter()
    .map(|p| home.join(p))
    .collect()
}
fn validate_candidate(home: &Path, c: &Candidate) -> io::Result<()> {
    if !cache_roots(home)
        .iter()
        .any(|root| c.path.starts_with(root) && c.path != *root)
    {
        return Err(error("Outside permitted developer cache roots"));
    }
    validate_chain(&c.path)?;
    let m = fs::symlink_metadata(&c.path)?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.dev() != c.dev
        || m.ino() != c.ino
        || m.len() != c.size
        || m.mtime() != c.modified
    {
        return Err(error(
            "Target changed or is not a single-link owned regular file",
        ));
    }
    Ok(())
}
pub fn preview(home: &Path, cancel: &AtomicBool) -> io::Result<Plan> {
    validate_chain(home)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs() as i64;
    let mut plan=Plan{items:Vec::new(),notes:vec!["Only regular developer-cache files older than 30 days. No app data, archives, directories or system files. Files may still be in use; close developer tools before executing.".into()]};
    let start = std::time::Instant::now();
    let mut visited = 0;
    for root in cache_roots(home) {
        if !root.exists() {
            continue;
        }
        if let Err(e) = validate_chain(&root) {
            plan.notes.push(format!("Skipped {root:?}: {e}"));
            continue;
        }
        let mut stack = vec![(root, 0)];
        while let Some((path, depth)) = stack.pop() {
            if cancel.load(Ordering::Relaxed) || visited >= 10000 || start.elapsed().as_secs() >= 15
            {
                plan.notes.push("Stopped early; partial preview".into());
                return Ok(plan);
            }
            visited += 1;
            let m = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    plan.notes.push(format!("Skipped {path:?}: {e}"));
                    continue;
                }
            };
            if m.is_symlink() {
                continue;
            }
            if m.is_dir() && depth < 16 {
                match fs::read_dir(&path) {
                    Ok(entries) => {
                        for e in entries.take(10000usize.saturating_sub(visited + stack.len())) {
                            match e {
                                Ok(e) => stack.push((e.path(), depth + 1)),
                                Err(e) => plan.notes.push(format!("Read error: {e}")),
                            }
                        }
                    }
                    Err(e) => plan.notes.push(format!("Skipped {path:?}: {e}")),
                }
            } else if m.is_file() && now.saturating_sub(m.mtime()) >= 30 * 86400 {
                let c = Candidate {
                    path,
                    dev: m.dev(),
                    ino: m.ino(),
                    size: m.len(),
                    modified: m.mtime(),
                };
                if validate_candidate(home, &c).is_ok() {
                    plan.items.push(c);
                }
            }
        }
    }
    Ok(plan)
}
pub fn describe(plan: &Plan) -> String {
    let mut s = format!(
        "{}\n\n{} files; {} logical bytes. Moving to recovery does not free disk space.\n",
        plan.notes.join("\n"),
        plan.items.len(),
        plan.items.iter().map(|c| c.size).sum::<u64>()
    );
    for c in &plan.items {
        s.push_str(&format!("{}  {:?}\n", c.size, c.path));
    }
    s
}
fn recovery_root(home: &Path) -> io::Result<PathBuf> {
    let p = home.join(".optimac-rust-recovery");
    if !p.exists() {
        fs::DirBuilder::new().create(&p)?;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700))?;
    }
    validate_chain(&p)?;
    let m = fs::metadata(&p)?;
    if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(error(
            "Recovery directory must be owned by you and mode 0700",
        ));
    }
    Ok(p)
}
fn sync_dir(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
#[cfg(target_os = "macos")]
fn move_exclusive(source: &Path, target: &Path) -> io::Result<()> {
    let a = std::ffi::CString::new(source.as_os_str().as_bytes())?;
    let b = std::ffi::CString::new(target.as_os_str().as_bytes())?;
    // RENAME_EXCL prevents overwriting an existing destination, including a symlink.
    let rc = unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_EXCL) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(target_os = "macos"))]
fn move_exclusive(_source: &Path, _target: &Path) -> io::Result<()> {
    Err(error("Recovery mutation is macOS-only"))
}
#[allow(dead_code)] // Not exposed until mutation race and active-file gates pass.
pub(crate) fn execute(home: &Path, plan: &Plan, cancel: &AtomicBool) -> io::Result<String> {
    if unsafe { libc::geteuid() } == 0 {
        return Err(error("Do not run cleanup as root"));
    }
    let root = recovery_root(home)?;
    let mut report = String::new();
    for (i, c) in plan.items.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.push_str("Cancelled; completed moves remain recoverable\n");
            break;
        }
        if let Err(e) = validate_candidate(home, c) {
            report.push_str(&format!("Skipped {:?}: {e}\n", c.path));
            continue;
        }
        let id = format!(
            "{}-{}-{i}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos(),
            std::process::id()
        );
        let dir = root.join(&id);
        fs::create_dir(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.join("record.json"))?;
        file.write_all(&serde_json::to_vec(&Record {
            version: 1,
            candidate: c.clone(),
        })?)?;
        file.sync_all()?;
        sync_dir(&dir)?;
        sync_dir(&root)?;
        validate_candidate(home, c)?;
        match move_exclusive(&c.path, &dir.join("payload")) {
            Ok(()) => {
                sync_dir(c.path.parent().unwrap())?;
                sync_dir(&dir)?;
                report.push_str(&format!("Moved {:?}; restore ID {id}\n", c.path));
            }
            Err(e) => report.push_str(&format!(
                "Not moved {:?}: {e}; no copy/delete fallback\n",
                c.path
            )),
        }
    }
    Ok(report
        + "Recovery records: optimac log. Restore: optimac restore <ID>. No disk-space reclamation claimed.\n")
}
pub fn history(home: &Path) -> io::Result<String> {
    let root = home.join(".optimac-rust-recovery");
    if !root.exists() {
        return Ok("No Rust recovery records. Legacy ~/.optimac data is untouched.\n".into());
    }
    validate_chain(&root)?;
    let mut s = String::new();
    for e in fs::read_dir(root)?.take(10000) {
        let e = e?;
        s.push_str(&format!(
            "{}: payload {}\n",
            e.file_name().to_string_lossy(),
            e.path().join("payload").exists()
        ));
    }
    Ok(s)
}
#[allow(dead_code)] // Fixture validation only; not yet exposed to callers.
pub(crate) fn restore(home: &Path, id: &str) -> io::Result<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(error("Invalid recovery ID"));
    }
    let root = recovery_root(home)?;
    let dir = root.join(id);
    validate_chain(&dir)?;
    let record_path = dir.join("record.json");
    validate_chain(&record_path)?;
    if fs::metadata(&record_path)?.len() > 65536 {
        return Err(error("Invalid recovery record size"));
    }
    let record: Record = serde_json::from_slice(&fs::read(record_path)?)?;
    if record.version != 1 {
        return Err(error("Unsupported recovery version"));
    }
    let c = record.candidate;
    if !cache_roots(home)
        .iter()
        .any(|r| c.path.starts_with(r) && c.path != *r)
    {
        return Err(error("Recovery destination outside allowed roots"));
    }
    validate_chain(
        c.path
            .parent()
            .ok_or_else(|| error("Invalid destination"))?,
    )?;
    let payload = dir.join("payload");
    validate_chain(&payload)?;
    let m = fs::symlink_metadata(&payload)?;
    if !m.is_file() || m.dev() != c.dev || m.ino() != c.ino {
        return Err(error("Payload identity changed"));
    }
    move_exclusive(&payload, &c.path)?;
    sync_dir(c.path.parent().unwrap())?;
    sync_dir(&dir)?;
    Ok(format!("Restored {:?}; journal retained", c.path))
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn preview_refuses_symlinks_and_recent_files() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-preview-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let cache = root.join(".cache/pip");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("recent"), b"keep").unwrap();
        std::os::unix::fs::symlink(&root, cache.join("loop")).unwrap();
        let plan = preview(&root, &AtomicBool::new(false)).unwrap();
        assert!(plan.items.is_empty());
        assert!(restore(&root, "../outside").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recover_without_overwriting() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-recover-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let cache = root.join(".cache/pip");
        fs::create_dir_all(&cache).unwrap();
        let p = cache.join("test");
        fs::write(&p, b"keep").unwrap();
        let m = fs::metadata(&p).unwrap();
        let c = Candidate {
            path: p.clone(),
            dev: m.dev(),
            ino: m.ino(),
            size: m.len(),
            modified: m.mtime(),
        };
        let plan = Plan {
            items: vec![c],
            notes: vec![],
        };
        let result = execute(&root, &plan, &AtomicBool::new(false)).unwrap();
        assert!(result.contains("Moved"));
        assert!(!p.exists());
        let id = fs::read_dir(root.join(".optimac-rust-recovery"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .into_owned();
        fs::write(&p, b"new").unwrap();
        assert!(restore(&root, &id).is_err());
        assert_eq!(fs::read(&p).unwrap(), b"new");
        fs::remove_file(&p).unwrap();
        restore(&root, &id).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
