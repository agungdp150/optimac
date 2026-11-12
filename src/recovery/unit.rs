//! Versioned plan-directory-unit recovery records (record schema 2).
//!
//! Version-1 records stay readable and are never rewritten. A directory unit is
//! moved only by the existing exclusive same-volume rename: there is no
//! copy/delete fallback, no automatic replay after interruption and no purge.
//! Internal symlinks travel with the directory; they are never read or followed.
use super::{
    Record, RecoveryContext, error,
    fs_ops::{rename_at_excl, stat_at},
};
use crate::fixture_paths::FixtureRoot;
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString},
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Component, Path, PathBuf},
};

pub const UNIT_RECORD_VERSION: u32 = 2;
pub const UNIT_RECORD_KIND: &str = "plan_directory_unit";
const PAYLOAD: &str = "payload";
const MOVE_COMPLETED: &str = "move-completed";
const RESTORE_INTENT: &str = "restore-intent";
const RESTORE_COMPLETED: &str = "restore-completed";
const MAX_RECORD_BYTES: usize = 65536;

/// A durable record for one top-level plan unit moved into recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitRecord {
    /// Record schema version (2). Version-1 readers fail closed on this record.
    pub version: u32,
    pub kind: String,
    pub original_path: PathBuf,
    pub provider: String,
    pub plan_id: String,
    pub device: u64,
    pub inode: u64,
    pub owner: u32,
    pub file_type: String,
    pub allocated_bytes: u64,
    pub logical_bytes: u64,
    pub created_at_unix_seconds: u64,
    /// Payload location relative to the record directory.
    pub payload: String,
}

impl UnitRecord {
    /// `identity` is (device, inode, owner); `bytes` is (allocated, logical).
    pub fn directory(
        original_path: PathBuf,
        provider: &str,
        plan_id: &str,
        identity: (u64, u64, u32),
        bytes: (u64, u64),
        created_at_unix_seconds: u64,
    ) -> Self {
        Self {
            version: UNIT_RECORD_VERSION,
            kind: UNIT_RECORD_KIND.into(),
            original_path,
            provider: provider.into(),
            plan_id: plan_id.into(),
            device: identity.0,
            inode: identity.1,
            owner: identity.2,
            file_type: "directory".into(),
            allocated_bytes: bytes.0,
            logical_bytes: bytes.1,
            created_at_unix_seconds,
            payload: PAYLOAD.into(),
        }
    }

    /// Scope checks shared by move and restore. A record cannot nominate a
    /// destination outside home, inside recovery or under a protected name.
    pub fn validate(&self, home: &Path, recovery: &Path) -> io::Result<()> {
        let path = &self.original_path;
        if self.version != UNIT_RECORD_VERSION
            || self.kind != UNIT_RECORD_KIND
            || self.file_type != "directory"
            || self.payload != PAYLOAD
            || !crate::cleanup::plan_builder::ALLOWED.contains(&self.provider.as_str())
            || self.plan_id.len() != 64
            || !self
                .plan_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(error("Malformed directory-unit record"));
        }
        if !path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
            || !path.starts_with(home)
            || path == home
            || path.parent() == Some(home)
            || path.starts_with(recovery)
            || crate::cleanup::protected_path(path)
        {
            return Err(error("Directory-unit path outside the allowed scope"));
        }
        Ok(())
    }
}

/// Any record a recovery directory may hold.
#[derive(Debug)]
pub(super) enum StoredRecord {
    V1(Record),
    Unit(UnitRecord),
}

/// Dispatch on the record's version before decoding its body.
pub(super) fn parse_record(bytes: &[u8]) -> io::Result<StoredRecord> {
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(error("Oversized recovery record"));
    }
    let header: Header = serde_json::from_slice(bytes)?;
    match header.version {
        1 => Ok(StoredRecord::V1(serde_json::from_slice(bytes)?)),
        UNIT_RECORD_VERSION => {
            let record: UnitRecord = serde_json::from_slice(bytes)?;
            if record.kind != UNIT_RECORD_KIND {
                return Err(error("Unknown recovery record kind"));
            }
            Ok(StoredRecord::Unit(record))
        }
        _ => Err(error("Unsupported recovery version")),
    }
}

/// Read `record.json` relative to a pinned record directory without following links.
pub(super) fn read_record_at(directory: &File) -> io::Result<StoredRecord> {
    let file = super::context::open_child(directory, "record.json", libc::O_RDONLY)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.nlink() != 1 || meta.uid() != unsafe { libc::geteuid() } {
        return Err(error("Unsafe recovery record"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    parse_record(&bytes)
}

/// Fault injection for interruption and cross-volume tests.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Faults {
    pub device: Option<u64>,
    pub stop_after_record: bool,
    pub stop_after_move: bool,
}

fn valid_id(id: &str) -> io::Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(error("Invalid recovery ID"));
    }
    Ok(())
}

fn identity_matches(st: &libc::stat, record: &UnitRecord) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFDIR
        && st.st_dev as u64 == record.device
        && st.st_ino == record.inode
        && st.st_uid == record.owner
        && st.st_uid == unsafe { libc::geteuid() }
}

fn present(directory: &File, name: &CStr) -> io::Result<Option<libc::stat>> {
    match stat_at(directory, name) {
        Ok(st) => Ok(Some(st)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn write_marker(directory: &File, name: &str, text: &[u8]) -> io::Result<()> {
    let mut file = super::context::open_child(
        directory,
        name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
    )?;
    file.write_all(text)?;
    file.sync_all()?;
    directory.sync_all()
}

fn parent_refusal(e: io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(libc::ENOENT) => error("Original parent is missing; restore refused"),
        Some(libc::ELOOP) | Some(libc::ENOTDIR) => {
            error("Original parent is a symlink or not a directory; restore refused")
        }
        _ => e,
    }
}

impl RecoveryContext {
    /// Fixture contexts resolve through their capability; production contexts
    /// walk from `/` with no-follow opens. Either way a symlinked ancestor fails.
    fn unit_parent(
        &self,
        authority: Option<&FixtureRoot>,
        path: &Path,
    ) -> io::Result<(File, CString)> {
        if self.fixture {
            let cap = authority.ok_or_else(|| error("fixture authorization required"))?;
            if self.fixture_identity != Some(cap.fixture_identity()?) {
                return Err(error("fixture authorization mismatch"));
            }
            return cap.source_parent(path);
        }
        let parent =
            super::fs_ops::open_dir(path.parent().ok_or_else(|| error("Missing parent"))?)?;
        let name = path.file_name().ok_or_else(|| error("Missing name"))?;
        Ok((parent, CString::new(name.as_bytes())?))
    }

    fn require_lock(&self, lock: &File) -> io::Result<()> {
        let held = lock.metadata()?;
        let current = stat_at(&self.directory, c".lock")?;
        if held.dev() != current.st_dev as u64 || held.ino() != current.st_ino {
            return Err(error("Recovery lock is not held for this directory"));
        }
        Ok(())
    }

    fn open_record_dir(&self, id: &str) -> io::Result<File> {
        valid_id(id)?;
        let directory =
            super::context::open_child(&self.directory, id, libc::O_RDONLY | libc::O_DIRECTORY)?;
        let meta = directory.metadata()?;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(error("Unsafe recovery record directory"));
        }
        Ok(directory)
    }

    /// Move one plan directory unit into recovery. The caller holds `lock()`.
    /// The record is synced before the rename and the completion marker after.
    pub fn move_directory_unit(
        &self,
        lock: &File,
        authority: Option<&FixtureRoot>,
        id: &str,
        record: &UnitRecord,
    ) -> io::Result<()> {
        self.move_directory_unit_with(lock, authority, id, record, Faults::default())
    }

    pub(crate) fn move_directory_unit_with(
        &self,
        lock: &File,
        authority: Option<&FixtureRoot>,
        id: &str,
        record: &UnitRecord,
        faults: Faults,
    ) -> io::Result<()> {
        valid_id(id)?;
        if unsafe { libc::geteuid() } == 0 {
            return Err(error("Do not run cleanup as root"));
        }
        self.require_lock(lock)?;
        record.validate(&self.home, &self.root)?;
        let (parent, name) = self.unit_parent(authority, &record.original_path)?;
        let unit = stat_at(&parent, &name)?;
        if !identity_matches(&unit, record) {
            return Err(error(
                "Unit identity changed or unit is not an owned directory; not moved",
            ));
        }
        let device = faults.device.unwrap_or(unit.st_dev as u64);
        if device != self.directory.metadata()?.dev() {
            return Err(error("Cross-volume unit refused; no copy/delete fallback"));
        }
        let directory = self.create_journal_bytes(id, &serde_json::to_vec(record)?)?;
        if faults.stop_after_record {
            return Err(error("Injected interruption after record"));
        }
        if !identity_matches(&stat_at(&parent, &name)?, record) {
            return Err(error(
                "Unit changed after its record was written; not moved",
            ));
        }
        rename_at_excl(&parent, &name, &directory, c"payload")?;
        let moved = stat_at(&directory, c"payload")?;
        if !identity_matches(&moved, record) {
            let rollback = rename_at_excl(&directory, c"payload", &parent, &name);
            return Err(error(&format!(
                "Unit changed during move; rollback {rollback:?}. Recovery ID {id}"
            )));
        }
        parent.sync_all()?;
        directory.sync_all()?;
        if faults.stop_after_move {
            return Err(error("Injected interruption after move"));
        }
        write_marker(&directory, MOVE_COMPLETED, b"Move to recovery completed\n")
    }

    /// Move a directory unit back. Never overwrites, never recreates a missing
    /// parent and never resolves a symlinked parent.
    pub fn restore_directory_unit(
        &self,
        authority: Option<&FixtureRoot>,
        id: &str,
    ) -> io::Result<String> {
        let _lock = self.lock()?;
        self.restore_directory_unit_locked(authority, id)
    }

    pub(super) fn restore_directory_unit_locked(
        &self,
        authority: Option<&FixtureRoot>,
        id: &str,
    ) -> io::Result<String> {
        if unsafe { libc::geteuid() } == 0 {
            return Err(error("Do not restore as root"));
        }
        let directory = self.open_record_dir(id)?;
        let StoredRecord::Unit(record) = read_record_at(&directory)? else {
            return Err(error("Not a directory-unit record"));
        };
        record.validate(&self.home, &self.root)?;
        let payload = present(&directory, c"payload")?
            .ok_or_else(|| error("Payload absent; nothing to restore"))?;
        if !identity_matches(&payload, &record) {
            return Err(error("Payload identity changed; restore refused"));
        }
        let (parent, name) = self
            .unit_parent(authority, &record.original_path)
            .map_err(parent_refusal)?;
        let parent_meta = parent.metadata()?;
        if !parent_meta.is_dir() {
            return Err(error(
                "Original parent is a symlink or not a directory; restore refused",
            ));
        }
        if present(&parent, &name)?.is_some() {
            return Err(error("Destination exists; restore never overwrites"));
        }
        if parent_meta.dev() != payload.st_dev as u64 {
            return Err(error(
                "Original parent is on another volume; restore refused",
            ));
        }
        if present(&directory, c"restore-intent")?.is_none() {
            write_marker(&directory, RESTORE_INTENT, b"Restore requested\n")?;
        }
        rename_at_excl(&directory, c"payload", &parent, &name)?;
        let restored = stat_at(&parent, &name)?;
        if !identity_matches(&restored, &record) {
            let rollback = rename_at_excl(&parent, &name, &directory, c"payload");
            return Err(error(&format!(
                "Payload changed during restore; rollback {rollback:?}"
            )));
        }
        parent.sync_all()?;
        directory.sync_all()?;
        write_marker(&directory, RESTORE_COMPLETED, b"Restore completed\n")?;
        Ok(format!(
            "Restored {}; journal retained",
            record.original_path.display()
        ))
    }

    /// Read-only description of a directory-unit record. Nothing is replayed.
    pub fn inspect_directory_unit(&self, id: &str) -> io::Result<String> {
        let _lock = self.lock()?;
        let directory = self.open_record_dir(id)?;
        let StoredRecord::Unit(record) = read_record_at(&directory)? else {
            return Err(error("Not a directory-unit record"));
        };
        describe(&directory, id, &record)
    }
}

/// Shared by the fixture inspector and the existing `recovery inspect` path.
pub(super) fn describe(directory: &File, id: &str, record: &UnitRecord) -> io::Result<String> {
    let payload = present(directory, c"payload")?;
    let moved = present(directory, c"move-completed")?.is_some();
    let intent = present(directory, c"restore-intent")?.is_some();
    let restored = present(directory, c"restore-completed")?.is_some();
    let state = match (&payload, moved, intent, restored) {
        (Some(st), ..) if !identity_matches(st, record) => {
            "needs_reconciliation: payload identity differs from the record; nothing replayed"
        }
        (Some(_), true, false, false) => "moved: payload in recovery; restore available",
        (Some(_), false, false, false) => {
            "interrupted_after_move: payload in recovery without completion marker; not replayed; restore available"
        }
        (None, false, false, false) => {
            "recorded_not_moved: record written but no payload; move not replayed"
        }
        (Some(_), _, true, false) => {
            "interrupted_restore: payload still in recovery; restore may be retried"
        }
        (None, _, true, true) => "restored: payload returned to its original path",
        (None, _, true, false) => {
            "needs_reconciliation: payload absent after restore intent without completion; cause not inferred"
        }
        _ => "needs_reconciliation: inconsistent markers; cause not inferred",
    };
    let original = match fs::symlink_metadata(&record.original_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => "absent".to_string(),
        Err(e) => format!("unreadable ({e})"),
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => "present, not a directory".into(),
        Ok(m) if m.dev() == record.device && m.ino() == record.inode => {
            "present, matches recorded identity".into()
        }
        Ok(_) => "present, different identity".into(),
    };
    Ok(format!(
        "Recovery {id}: {kind} record (schema {version})\n\
         State: {state}\n\
         Original path: {path} ({original})\n\
         Provider: {provider}\n\
         Plan: {plan}\n\
         Recorded identity: device {device}, inode {inode}, owner {owner}, type {kind_of}\n\
         Recorded size: {allocated} allocated, {logical} logical (plan estimate; not re-measured)\n\
         Recorded at: {created} (Unix seconds)\n\
         Payload: {id}/{payload_name} ({payload_state})\n\
         Inspection only; nothing was moved, restored or replayed.",
        kind = record.kind,
        version = record.version,
        path = record.original_path.display(),
        provider = record.provider,
        plan = record.plan_id,
        device = record.device,
        inode = record.inode,
        owner = record.owner,
        kind_of = record.file_type,
        allocated = crate::format_size(record.allocated_bytes),
        logical = crate::format_size(record.logical_bytes),
        created = record.created_at_unix_seconds,
        payload_name = record.payload,
        payload_state = if payload.is_some() {
            "present"
        } else {
            "absent"
        },
    ))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::recovery::{RecoveryPolicy, purge};
    use std::os::unix::fs::{PermissionsExt, symlink};

    const PLAN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Bundle-style unit modelled on ms-playwright/chromium-NNNN: a framework
    /// with Versions/Current, Helpers, Libraries and Resources links, plus one
    /// absolute link to a directory outside the unit and outside the fixture.
    fn bundle(home: &Path, outside: &Path) -> PathBuf {
        let unit = home.join("Library/Caches/ms-playwright/chromium-1248");
        let framework =
            unit.join("chrome-mac/Chromium.app/Contents/Frameworks/Chromium Framework.framework");
        let version = framework.join("Versions/A");
        for dir in ["Helpers", "Libraries", "Resources"] {
            fs::create_dir_all(version.join(dir)).unwrap();
        }
        fs::write(version.join("Helpers/chrome_crashpad_handler"), b"helper").unwrap();
        fs::write(version.join("Libraries/libEGL.dylib"), b"library").unwrap();
        fs::write(version.join("Resources/Info.plist"), b"plist").unwrap();
        fs::write(version.join("Chromium Framework"), b"binary").unwrap();
        symlink("A", framework.join("Versions/Current")).unwrap();
        for name in ["Helpers", "Libraries", "Resources", "Chromium Framework"] {
            symlink(format!("Versions/Current/{name}"), framework.join(name)).unwrap();
        }
        fs::create_dir_all(outside).unwrap();
        fs::write(outside.join("secret"), b"outside").unwrap();
        symlink(outside, unit.join("external-profile")).unwrap();
        unit
    }

    /// No-follow tree snapshot: path, identity, link target or file bytes.
    fn snapshot(root: &Path) -> Vec<(PathBuf, u64, String)> {
        let mut out = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let m = fs::symlink_metadata(&path).unwrap();
            let detail = if m.file_type().is_symlink() {
                format!("link:{}", fs::read_link(&path).unwrap().display())
            } else if m.is_dir() {
                for e in fs::read_dir(&path).unwrap() {
                    pending.push(e.unwrap().path());
                }
                "dir".into()
            } else {
                format!(
                    "file:{}",
                    String::from_utf8_lossy(&fs::read(&path).unwrap())
                )
            };
            out.push((
                path.strip_prefix(root).unwrap().to_path_buf(),
                m.ino(),
                detail,
            ));
        }
        out.sort();
        out
    }

    fn record(unit: &Path) -> UnitRecord {
        let m = fs::symlink_metadata(unit).unwrap();
        UnitRecord::directory(
            unit.to_path_buf(),
            "playwright-browsers",
            PLAN,
            (m.dev(), m.ino(), m.uid()),
            (384_000_000, 380_000_000),
            1_000_000,
        )
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    struct Setup {
        home: PathBuf,
        cap: FixtureRoot,
        outside: PathBuf,
        unit: PathBuf,
        context: RecoveryContext,
    }
    impl Setup {
        fn new() -> Self {
            let (home, cap) = FixtureRoot::create_test_fixture().unwrap();
            let (outside, _) = FixtureRoot::create_test_fixture().unwrap();
            let unit = bundle(&home, &outside.join("profile"));
            let context =
                RecoveryContext::initialize(&home, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
            Self {
                home,
                cap,
                outside,
                unit,
                context,
            }
        }
        fn record_dir(&self, id: &str) -> PathBuf {
            self.home.join(".optimac-rust-recovery").join(id)
        }
    }
    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.outside, fs::Permissions::from_mode(0o700));
            let _ = fs::remove_dir_all(&self.home);
            let _ = fs::remove_dir_all(&self.outside);
        }
    }

    #[test]
    fn directory_unit_move_restore_keeps_internal_links_and_outside_target() {
        let s = Setup::new();
        let before = snapshot(&s.unit);
        let outside_before = fs::symlink_metadata(s.outside.join("profile/secret")).unwrap();
        // Any attempt to read or traverse the outside target would fail.
        fs::set_permissions(&s.outside, fs::Permissions::from_mode(0o000)).unwrap();
        let r = record(&s.unit);
        let lock = s.context.lock().unwrap();
        s.context
            .move_directory_unit(&lock, Some(&s.cap), "1001", &r)
            .unwrap();
        drop(lock);
        assert!(fs::symlink_metadata(&s.unit).is_err());
        assert_eq!(
            listing(&s.record_dir("1001")),
            ["move-completed", "payload", "record.json"]
        );
        let stored: UnitRecord =
            serde_json::from_slice(&fs::read(s.record_dir("1001/record.json")).unwrap()).unwrap();
        assert_eq!(stored, r);
        let moved = s.context.inspect_directory_unit("1001").unwrap();
        println!("INSPECT_BEFORE_RESTORE\n{moved}");
        assert_eq!(
            moved,
            format!(
                "Recovery 1001: plan_directory_unit record (schema 2)\n\
                 State: moved: payload in recovery; restore available\n\
                 Original path: {} (absent)\n\
                 Provider: playwright-browsers\n\
                 Plan: {PLAN}\n\
                 Recorded identity: device {}, inode {}, owner {}, type directory\n\
                 Recorded size: 384.00 MB allocated, 380.00 MB logical (plan estimate; not re-measured)\n\
                 Recorded at: 1000000 (Unix seconds)\n\
                 Payload: 1001/payload (present)\n\
                 Inspection only; nothing was moved, restored or replayed.",
                s.unit.display(),
                r.device,
                r.inode,
                r.owner
            )
        );
        assert_eq!(snapshot(&s.record_dir("1001/payload")), before);
        assert!(
            s.context
                .restore_directory_unit(Some(&s.cap), "1001")
                .unwrap()
                .contains("Restored")
        );
        assert_eq!(snapshot(&s.unit), before);
        let restored = s.context.inspect_directory_unit("1001").unwrap();
        println!("INSPECT_AFTER_RESTORE\n{restored}");
        assert!(restored.contains("State: restored: payload returned to its original path"));
        assert!(restored.contains("(present, matches recorded identity)"));
        assert!(restored.contains("Payload: 1001/payload (absent)"));
        assert!(
            s.context
                .restore_directory_unit(Some(&s.cap), "1001")
                .is_err()
        );
        fs::set_permissions(&s.outside, fs::Permissions::from_mode(0o700)).unwrap();
        let outside_after = fs::symlink_metadata(s.outside.join("profile/secret")).unwrap();
        assert_eq!(
            (
                outside_after.ino(),
                outside_after.mtime(),
                outside_after.mtime_nsec()
            ),
            (
                outside_before.ino(),
                outside_before.mtime(),
                outside_before.mtime_nsec()
            )
        );
        assert_eq!(
            fs::read(s.outside.join("profile/secret")).unwrap(),
            b"outside"
        );
        assert_eq!(
            fs::read_link(s.unit.join("external-profile")).unwrap(),
            s.outside.join("profile")
        );
    }

    #[test]
    fn directory_unit_restore_refuses_existing_destination_missing_or_symlinked_parent() {
        let s = Setup::new();
        let before = snapshot(&s.unit);
        let lock = s.context.lock().unwrap();
        s.context
            .move_directory_unit(&lock, Some(&s.cap), "2001", &record(&s.unit))
            .unwrap();
        drop(lock);
        fs::create_dir(&s.unit).unwrap();
        fs::write(s.unit.join("new"), b"new content").unwrap();
        let err = s
            .context
            .restore_directory_unit(Some(&s.cap), "2001")
            .unwrap_err();
        assert!(err.to_string().contains("Destination exists"), "{err}");
        assert_eq!(fs::read(s.unit.join("new")).unwrap(), b"new content");
        assert!(s.record_dir("2001/payload").is_dir());
        fs::remove_dir_all(&s.unit).unwrap();

        let parent = s.unit.parent().unwrap().to_path_buf();
        let moved_parent = s.home.join("moved-parent");
        fs::rename(&parent, &moved_parent).unwrap();
        let err = s
            .context
            .restore_directory_unit(Some(&s.cap), "2001")
            .unwrap_err();
        assert!(err.to_string().contains("parent is missing"), "{err}");
        assert!(!parent.exists());

        symlink(&moved_parent, &parent).unwrap();
        let err = s
            .context
            .restore_directory_unit(Some(&s.cap), "2001")
            .unwrap_err();
        assert!(err.to_string().contains("symlink"), "{err}");
        assert_eq!(fs::read_dir(&moved_parent).unwrap().count(), 0);
        assert!(s.record_dir("2001/payload").is_dir());
        // Refusals happen before the restore intent is written.
        let inspect = s.context.inspect_directory_unit("2001").unwrap();
        assert!(inspect.contains("State: moved"), "{inspect}");
        assert_eq!(
            listing(&s.record_dir("2001")),
            ["move-completed", "payload", "record.json"]
        );

        fs::remove_file(&parent).unwrap();
        fs::rename(&moved_parent, &parent).unwrap();
        s.context
            .restore_directory_unit(Some(&s.cap), "2001")
            .unwrap();
        assert_eq!(snapshot(&s.unit), before);
    }

    #[test]
    fn directory_unit_move_refuses_cross_volume_and_changed_identity_before_record() {
        let s = Setup::new();
        let before = snapshot(&s.unit);
        let r = record(&s.unit);
        let lock = s.context.lock().unwrap();
        let faults = Faults {
            device: Some(r.device.wrapping_add(1)),
            ..Faults::default()
        };
        let err = s
            .context
            .move_directory_unit_with(&lock, Some(&s.cap), "3001", &r, faults)
            .unwrap_err();
        assert!(err.to_string().contains("Cross-volume"), "{err}");
        assert!(!s.record_dir("3001").exists());
        let mut changed = r.clone();
        changed.inode += 1;
        assert!(
            s.context
                .move_directory_unit(&lock, Some(&s.cap), "3002", &changed)
                .is_err()
        );
        let mut escaped = r.clone();
        escaped.original_path = s.outside.join("profile");
        assert!(
            s.context
                .move_directory_unit(&lock, Some(&s.cap), "3003", &escaped)
                .is_err()
        );
        assert!(
            s.context
                .move_directory_unit(&lock, None, "3004", &r)
                .is_err()
        );
        let other = s.context.lock();
        assert!(other.is_err(), "lock must stay exclusive");
        let stray = fs::File::open(&s.home).unwrap();
        assert!(
            s.context
                .move_directory_unit(&stray, Some(&s.cap), "3005", &r)
                .is_err()
        );
        assert_eq!(listing(&s.home.join(".optimac-rust-recovery")), [".lock"]);
        assert_eq!(snapshot(&s.unit), before);
    }

    #[test]
    fn directory_unit_interruptions_are_readable_and_never_replayed() {
        let s = Setup::new();
        let before = snapshot(&s.unit);
        let r = record(&s.unit);
        let lock = s.context.lock().unwrap();
        let crash = Faults {
            stop_after_record: true,
            ..Faults::default()
        };
        assert!(
            s.context
                .move_directory_unit_with(&lock, Some(&s.cap), "4001", &r, crash)
                .is_err()
        );
        drop(lock);
        assert_eq!(listing(&s.record_dir("4001")), ["record.json"]);
        for _ in 0..2 {
            let text = s.context.inspect_directory_unit("4001").unwrap();
            assert!(
                text.contains("State: recorded_not_moved")
                    && text.contains("(present, matches recorded identity)"),
                "{text}"
            );
        }
        assert!(
            s.context
                .restore_directory_unit(Some(&s.cap), "4001")
                .is_err()
        );
        assert_eq!(listing(&s.record_dir("4001")), ["record.json"]);
        assert_eq!(snapshot(&s.unit), before);

        let lock = s.context.lock().unwrap();
        let crash = Faults {
            stop_after_move: true,
            ..Faults::default()
        };
        assert!(
            s.context
                .move_directory_unit_with(&lock, Some(&s.cap), "4002", &r, crash)
                .is_err()
        );
        drop(lock);
        assert_eq!(listing(&s.record_dir("4002")), ["payload", "record.json"]);
        for _ in 0..2 {
            let text = s.context.inspect_directory_unit("4002").unwrap();
            assert!(text.contains("State: interrupted_after_move"), "{text}");
            assert!(text.contains("(absent)"), "{text}");
        }
        assert_eq!(listing(&s.record_dir("4002")), ["payload", "record.json"]);
        assert!(!s.unit.exists());
        s.context
            .restore_directory_unit(Some(&s.cap), "4002")
            .unwrap();
        assert_eq!(snapshot(&s.unit), before);
        assert!(
            s.context
                .inspect_directory_unit("4002")
                .unwrap()
                .contains("State: restored")
        );
        assert_eq!(listing(&s.record_dir("4001")), ["record.json"]);
    }

    #[test]
    fn recovery_record_versions_load_v1_and_unit_records_fail_closed() {
        let s = Setup::new();
        let lock = s.context.lock().unwrap();
        let legacy = s.home.join(".cache/pip/legacy");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"v1").unwrap();
        let m = fs::metadata(&legacy).unwrap();
        let candidate: crate::recovery::Candidate = serde_json::from_value(serde_json::json!({
            "path": legacy, "dev": m.dev(), "ino": m.ino(), "size": m.len(), "modified": m.mtime()
        }))
        .unwrap();
        let v1 = s.context.create_journal("5001", &candidate).unwrap();
        drop(lock);
        let StoredRecord::V1(old) = read_record_at(&v1).unwrap() else {
            panic!("v1 record must still load as v1");
        };
        assert_eq!(old.version, 1);
        assert_eq!(old.candidate.path, legacy);
        assert!(
            s.context
                .inspect_directory_unit("5001")
                .unwrap_err()
                .to_string()
                .contains("Not a directory-unit")
        );
        assert!(
            s.context
                .restore_directory_unit(Some(&s.cap), "5001")
                .is_err()
        );
        assert_eq!(fs::read(&legacy).unwrap(), b"v1");
        let lock = s.context.lock().unwrap();

        let r = record(&s.unit);
        let bytes = serde_json::to_vec(&r).unwrap();
        assert!(matches!(parse_record(&bytes).unwrap(), StoredRecord::Unit(u) if u == r));
        // A version-1 reader cannot mistake a unit record for a file record.
        assert!(serde_json::from_slice::<Record>(&bytes).is_err());
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["kind"] = "other".into();
        assert!(parse_record(&serde_json::to_vec(&value).unwrap()).is_err());
        value["kind"] = UNIT_RECORD_KIND.into();
        value["unexpected"] = true.into();
        assert!(parse_record(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(parse_record(br#"{"version":3}"#).is_err());
        assert!(parse_record(b"corrupt").is_err());
        for bad in [
            UnitRecord {
                provider: "large-old-files".into(),
                ..r.clone()
            },
            UnitRecord {
                plan_id: "../plan".into(),
                ..r.clone()
            },
            UnitRecord {
                original_path: s.home.join("Library/Caches/ms-playwright/../../Keychains"),
                ..r.clone()
            },
            UnitRecord {
                original_path: s.home.join(".optimac-rust-recovery/5001"),
                ..r.clone()
            },
            UnitRecord {
                original_path: s.home.join("Documents"),
                ..r.clone()
            },
        ] {
            assert!(bad.validate(&s.home, &s.context.root).is_err(), "{bad:?}");
            assert!(
                s.context
                    .move_directory_unit(&lock, Some(&s.cap), "5002", &bad)
                    .is_err()
            );
        }
        assert!(!s.record_dir("5002").exists());
    }

    #[test]
    #[ignore = "Requires production recovery policy; run make test-recovery-isolated under OS sandbox"]
    fn directory_unit_production_restore_inspect_and_purge_refusal() {
        let home = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-unit-production-{}", std::process::id()));
        let outside = home.with_extension("outside");
        fs::create_dir_all(&home).unwrap();
        let unit = bundle(&home, &outside);
        let before = snapshot(&unit);
        let r = record(&unit);
        let context = RecoveryContext::initialize(&home, RecoveryPolicy::Production).unwrap();
        let lock = context.lock().unwrap();
        context
            .move_directory_unit(&lock, None, "6001", &r)
            .unwrap();
        drop(lock);
        let inspect = purge::inspect(RecoveryPolicy::Production, &home, "6001").unwrap();
        assert!(
            inspect.contains("plan_directory_unit record (schema 2)"),
            "{inspect}"
        );
        assert!(inspect.contains("State: moved"), "{inspect}");
        let err = purge::plan(RecoveryPolicy::Production, &home, "6001").unwrap_err();
        assert!(
            err.to_string()
                .contains("directory purge is not implemented"),
            "{err}"
        );
        let payload = home.join(".optimac-rust-recovery/6001/payload");
        assert_eq!(snapshot(&payload), before);
        assert_eq!(crate::recovery::recovery_ids(&home), ["6001"]);
        crate::recovery::restore(RecoveryPolicy::Production, &home, "6001").unwrap();
        assert_eq!(snapshot(&unit), before);
        assert!(
            purge::inspect(RecoveryPolicy::Production, &home, "6001")
                .unwrap()
                .contains("State: restored")
        );
        assert!(purge::plan(RecoveryPolicy::Production, &home, "6001").is_err());
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }
}
