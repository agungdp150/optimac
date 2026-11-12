//! Explicit permanent removal of reviewed regular-file recovery payloads.
use super::*;
use std::io::Read;

#[derive(Debug)]
pub struct PurgePlan {
    id: String,
    device: u64,
    inode: u64,
    size: u64,
    modified: i64,
    modified_ns: i64,
    created: std::time::Instant,
}
impl PurgePlan {
    pub fn describe(&self) -> String {
        format!(
            "Recovery ID: {}\nRegular-file payload: {} logical bytes\nPermanent deletion; restoration will no longer be possible.",
            self.id, self.size
        )
    }
}
fn checked_payload(
    policy: RecoveryPolicy<'_>,
    home: &Path,
    id: &str,
) -> io::Result<(PathBuf, fs::Metadata)> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(error("Invalid recovery ID"));
    }
    let root = recovery_root(policy, home)?;
    let dir = root.join(id);
    validate_chain(&dir)?;
    let dm = fs::metadata(&dir)?;
    if dm.uid() != unsafe { libc::geteuid() } || dm.mode() & 0o077 != 0 {
        return Err(error("Unsafe recovery record directory"));
    }
    let path = dir.join("record.json");
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let mut data = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(65537)
        .read_to_end(&mut data)?;
    if data.len() > 65536 {
        return Err(error("Oversized recovery record"));
    }
    let record = match super::unit::parse_record(&data)? {
        super::unit::StoredRecord::V1(record) => record,
        super::unit::StoredRecord::Unit(unit) => {
            return Err(error(&format!(
                "Recovery {id} is a directory-unit record ({}); directory purge is not implemented",
                unit.original_path.display()
            )));
        }
    };
    let payload = dir.join("payload");
    let m = fs::symlink_metadata(&payload)?;
    if record.version != 1
        || record.candidate.app
        || !allowed(home, &record.candidate)
        || !m.is_file()
        || m.is_symlink()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.dev() != record.candidate.dev
        || m.ino() != record.candidate.ino
    {
        return Err(error("Payload not eligible for regular-file purge"));
    }
    Ok((dir, m))
}
pub fn plan(policy: RecoveryPolicy<'_>, home: &Path, id: &str) -> io::Result<PurgePlan> {
    let (_, m) = checked_payload(policy, home, id)?;
    Ok(PurgePlan {
        id: id.into(),
        device: m.dev(),
        inode: m.ino(),
        size: m.len(),
        modified: m.mtime(),
        modified_ns: m.mtime_nsec(),
        created: std::time::Instant::now(),
    })
}
pub fn execute(
    policy: RecoveryPolicy<'_>,
    home: &Path,
    plan: &PurgePlan,
    cancel: &AtomicBool,
) -> io::Result<String> {
    use std::os::fd::AsRawFd;
    if unsafe { libc::geteuid() } == 0 {
        return Err(error("Do not purge as root"));
    }
    if cancel.load(std::sync::atomic::Ordering::Relaxed) || plan.created.elapsed().as_secs() >= 600
    {
        return Err(error("Cancelled or expired plan"));
    }
    let root = recovery_root(policy, home)?;
    let _lock = operation_lock(&root)?;
    let (dir, m) = checked_payload(policy, home, &plan.id)?;
    if (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
        != (
            plan.device,
            plan.inode,
            plan.size,
            plan.modified,
            plan.modified_ns,
        )
    {
        return Err(error("Payload changed; review again"));
    }
    let active = crate::actions::command(
        "/usr/sbin/lsof",
        &[
            "-nP",
            "-t",
            "--",
            dir.join("payload")
                .to_str()
                .ok_or_else(|| error("Invalid path"))?,
        ],
        cancel,
    )?;
    if !active.trim().is_empty() {
        return Err(error("Payload is open"));
    }
    let directory = fs_ops::open_dir(&dir)?;
    let mut journal = open_child(
        &directory,
        c"purge-intent.json",
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
    )?;
    journal.write_all(&serde_json::to_vec(&serde_json::json!({"version":1,"state":"intent","id":plan.id,"device":plan.device,"inode":plan.inode,"logical_bytes":plan.size}))?)?;
    journal.sync_all()?;
    directory.sync_all()?;
    let payload = open_child(&directory, c"payload", libc::O_RDONLY)?;
    let m = payload.metadata()?;
    if (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
        != (
            plan.device,
            plan.inode,
            plan.size,
            plan.modified,
            plan.modified_ns,
        )
        || cancel.load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(error(
            "Payload changed or cancelled; intent retained for inspection",
        ));
    }
    // SAFETY: directory is pinned and the name is a constant terminated string.
    if unsafe { libc::unlinkat(directory.as_raw_fd(), c"payload".as_ptr(), 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    drop(payload);
    directory.sync_all()?;
    let mut done = open_child(
        &directory,
        c"purge-completed",
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
    )?;
    done.write_all(b"Permanent removal completed\n")?;
    done.sync_all()?;
    directory.sync_all()?;
    Ok(format!(
        "Permanently removed recovery payload {}; journal retained. Logical bytes removed: {}. Physical space reclaimed is not inferred from this size.",
        plan.id, plan.size
    ))
}

fn open_child(dir: &fs::File, name: &std::ffi::CStr, flags: i32) -> io::Result<fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    // SAFETY: valid directory descriptor, terminated name; returned descriptor is owned.
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { fs::File::from_raw_fd(fd) };
    let m = file.metadata()?;
    if !m.is_file() || m.nlink() != 1 || m.uid() != unsafe { libc::geteuid() } {
        return Err(error("Unsafe recovery child"));
    }
    Ok(file)
}

/// Read-only reconciliation: never retries deletion or changes the journal.
pub fn inspect(policy: RecoveryPolicy<'_>, home: &Path, id: &str) -> io::Result<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(error("Invalid recovery ID"));
    }
    let root = recovery_root(policy, home)?;
    let _lock = operation_lock(&root)?;
    let dir = fs_ops::open_dir(&root.join(id))?;
    let probe = |name: &std::ffi::CStr| -> io::Result<bool> {
        match open_child(&dir, name, libc::O_RDONLY) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    };
    let payload = probe(c"payload")?;
    let intent = probe(c"purge-intent.json")?;
    let done = probe(c"purge-completed")?;
    if intent {
        let mut bytes = Vec::new();
        open_child(&dir, c"purge-intent.json", libc::O_RDONLY)?
            .take(65537)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err(error("Oversized purge intent"));
        }
        let v: serde_json::Value = serde_json::from_slice(&bytes)?;
        if v["version"] != 1 || v["id"] != id || v["state"] != "intent" {
            return Err(error("Invalid purge intent"));
        }
    }
    let state = match (payload, intent, done) {
        (true, false, false) => "payload_present",
        (true, true, false) => {
            "interrupted_payload_present: deletion not replayed; restore or inspect manually"
        }
        (false, true, true) => "purge_completed: payload absent",
        (false, true, false) => {
            "needs_reconciliation: payload absent without completion; cause not inferred"
        }
        (false, false, false) => "payload_absent: may have been restored; inspect original record",
        _ => "needs_reconciliation: inconsistent journal and payload",
    };
    Ok(format!("Recovery {id}: {state}"))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires production recovery policy; run make test-recovery-isolated under OS sandbox"]
    fn purge_removes_only_reviewed_recovery_payload() {
        let home = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-purge-{}", std::process::id()));
        fs::create_dir_all(home.join("Documents")).unwrap();
        let path = home.join("Documents/item");
        fs::write(&path, b"fixture").unwrap();
        let m = fs::metadata(&path).unwrap();
        let candidate = Candidate {
            path,
            dev: m.dev(),
            ino: m.ino(),
            size: m.len(),
            modified: m.mtime(),
            app: false,
            personal: true,
        };
        let cancel = AtomicBool::new(false);
        super::super::execute(
            RecoveryPolicy::Production,
            &home,
            &Plan {
                items: vec![candidate],
                notes: vec![],
            },
            &cancel,
        )
        .unwrap();
        let ids = recovery_ids(&home);
        assert_eq!(ids.len(), 1);
        assert!(
            inspect(RecoveryPolicy::Production, &home, &ids[0])
                .unwrap()
                .contains("payload_present")
        );
        let stale = plan(RecoveryPolicy::Production, &home, &ids[0]).unwrap();
        let payload = home
            .join(".optimac-rust-recovery")
            .join(&ids[0])
            .join("payload");
        fs::write(&payload, b"changed fixture").unwrap();
        assert!(execute(RecoveryPolicy::Production, &home, &stale, &cancel).is_err());
        assert_eq!(fs::read(&payload).unwrap(), b"changed fixture");
        let p = plan(RecoveryPolicy::Production, &home, &ids[0]).unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(execute(RecoveryPolicy::Production, &home, &p, &cancelled).is_err());
        assert!(payload.exists());
        assert!(
            execute(RecoveryPolicy::Production, &home, &p, &cancel)
                .unwrap()
                .contains("Permanently removed")
        );
        assert!(
            inspect(RecoveryPolicy::Production, &home, &ids[0])
                .unwrap()
                .contains("purge_completed")
        );
        assert!(recovery_ids(&home).is_empty());
        assert!(execute(RecoveryPolicy::Production, &home, &p, &cancel).is_err());
        assert!(
            home.join(".optimac-rust-recovery")
                .join(&ids[0])
                .join("purge-completed")
                .exists()
        );
        let record_dir = home.join(".optimac-rust-recovery").join(&ids[0]);
        fs::remove_file(record_dir.join("purge-completed")).unwrap();
        assert!(
            inspect(RecoveryPolicy::Production, &home, &ids[0])
                .unwrap()
                .contains("needs_reconciliation")
        );
        fs::write(record_dir.join("purge-intent.json"), b"invalid").unwrap();
        assert!(inspect(RecoveryPolicy::Production, &home, &ids[0]).is_err());
        fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(test)]
mod authorization_tests {
    use super::*;
    #[test]
    fn fixture_recovery_purge_execute_rejects_before_metadata_writes() {
        let home = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "optimac-purge-authority-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&home).unwrap();
        let cap = crate::fixture_paths::FixtureRoot::open(&home).unwrap();
        let plan = PurgePlan {
            id: "123".into(),
            device: 0,
            inode: 0,
            size: 0,
            modified: 0,
            modified_ns: 0,
            created: std::time::Instant::now(),
        };
        for policy in [
            RecoveryPolicy::Fixture(None),
            RecoveryPolicy::Fixture(Some(&cap)),
        ] {
            let err = execute(policy, &home, &plan, &AtomicBool::new(false)).unwrap_err();
            assert!(err.to_string().contains("fixture"));
            assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
        }
        drop(cap);
        fs::remove_dir(home).unwrap();
    }
}
