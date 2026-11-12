//! Permanent purge of plan directory-unit recovery payloads (record schema 2).
//!
//! Authorized only for payloads of `plan_directory_unit` records created by
//! `clean apply`. Every other record kind, including all version-1 records,
//! is refused before anything is written. Nothing here follows a link.
use super::{
    RecoveryContext, error, fs_ops,
    unit::{self, StoredRecord, UnitRecord},
};
use crate::{
    cleanup::activity::{Activity, ActivityCheck},
    fixture_paths::FixtureRoot,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString, OsStr},
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
};

pub const WARNING: &str =
    "Permanently deletes this recovery payload. It cannot be restored afterward.";

/// Whether a purge starts on an intact payload or continues an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeStart {
    /// Intact payload; it is renamed to `deleting` before the walk.
    Fresh,
    /// A partially deleted `deleting` tree from an earlier purge of this record.
    Resume,
}

/// Everything the confirmation shows; produced without writing anything.
#[derive(Debug)]
pub struct PurgeReview {
    pub id: String,
    pub record: UnitRecord,
    pub start: PurgeStart,
    pub state: String,
}
impl PurgeReview {
    pub fn describe(&self) -> String {
        format!(
            "Recovery {id}\nOriginal path: {path}\nProvider: {provider}\nPlan: {plan}\nAllocated size: {size} (plan estimate)\nState: {state}\n{WARNING}",
            id = self.id,
            path = self.record.original_path.display(),
            provider = self.record.provider,
            plan = self.record.plan_id,
            size = crate::format_size(self.record.allocated_bytes),
            state = self.state,
        )
    }
}

const JOURNAL: &str = "purge-journal";
const JOURNAL_VERSION: u32 = 1;
const MAX_JOURNAL_BYTES: u64 = 1 << 20;

/// One append-only, fsynced purge journal line.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PurgeEvent {
    version: u32,
    state: String,
    device: u64,
    inode: u64,
    allocated_bytes: u64,
    deleted: usize,
    failed: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    at: u64,
}

/// Folded purge journal: last state and counts.
#[derive(Debug)]
pub(super) struct PurgeJournal {
    pub last: String,
    pub deleted: usize,
    pub failed: usize,
    pub reason: Option<String>,
    pub torn: bool,
}

pub(super) fn read_journal(directory: &File) -> io::Result<Option<PurgeJournal>> {
    let file = match super::context::open_child(directory, JOURNAL, libc::O_RDONLY) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file() || meta.nlink() != 1 || meta.uid() != unsafe { libc::geteuid() } {
        return Err(error("Unsafe purge journal"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_JOURNAL_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(error("Oversized purge journal"));
    }
    let mut lines: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
    let torn = lines.last().is_some_and(|l| !l.is_empty());
    lines.pop();
    let mut journal: Option<PurgeJournal> = None;
    for line in lines {
        let event: PurgeEvent = serde_json::from_slice(line)
            .map_err(|e| error(&format!("Corrupt purge journal: {e}")))?;
        if event.version != JOURNAL_VERSION
            || !matches!(
                event.state.as_str(),
                "intent" | "deleting" | "completed" | "failed"
            )
        {
            return Err(error("Unexpected purge journal entry"));
        }
        let deleted = journal.as_ref().map_or(0, |j| j.deleted) + event.deleted;
        journal = Some(PurgeJournal {
            last: event.state,
            deleted,
            failed: event.failed,
            reason: event.reason,
            torn,
        });
    }
    if journal.is_none() && torn {
        return Ok(Some(PurgeJournal {
            last: "intent".into(),
            deleted: 0,
            failed: 0,
            reason: None,
            torn,
        }));
    }
    Ok(journal)
}

pub(super) fn entries(n: usize, what: &str) -> String {
    format!("{n} {} {what}", if n == 1 { "entry" } else { "entries" })
}

/// True once a purge has written anything for this record.
pub(super) fn purge_started(directory: &File) -> io::Result<bool> {
    Ok(read_journal(directory)?.is_some() || unit::present(directory, c"deleting")?.is_some())
}

/// Purge lifecycle state for `recovery inspect`, or None if no purge began.
pub(super) fn purge_state(directory: &File, id: &str) -> io::Result<Option<String>> {
    let deleting = unit::present(directory, c"deleting")?.is_some();
    let payload = unit::present(directory, c"payload")?.is_some();
    let Some(j) = read_journal(directory)? else {
        return Ok(deleting.then(|| {
            "needs_reconciliation: a deleting payload exists without a purge journal; cause not inferred".into()
        }));
    };
    let mut state = match (j.last.as_str(), payload, deleting) {
        ("completed", false, false) => {
            "purge_completed: payload permanently deleted; it cannot be restored".to_string()
        }
        ("failed", false, true) => format!(
            "purge_failed: payload partially deleted and cannot be restored; {}{}; fix the cause and re-run optimac recovery purge {id}",
            entries(j.failed, "could not be deleted"),
            j.reason
                .as_ref()
                .map(|r| format!("; stopped: {r}"))
                .unwrap_or_default()
        ),
        ("intent" | "deleting", false, true) => format!(
            "needs_reconciliation: purge interrupted while deleting; payload partially deleted and cannot be restored; re-run optimac recovery purge {id} to continue (never automatic)"
        ),
        ("intent", true, false) => format!(
            "needs_reconciliation: purge confirmed but nothing was deleted; payload intact; re-run optimac recovery purge {id} to continue"
        ),
        _ => "needs_reconciliation: purge journal and payload are inconsistent; cause not inferred"
            .into(),
    };
    if j.torn {
        state.push_str(" (last journal entry cut off)");
    }
    Ok(Some(state))
}

/// Extra inspect line for records with a purge journal.
pub(super) fn journal_summary(directory: &File) -> io::Result<Option<String>> {
    Ok(read_journal(directory)?.map(|j| {
        format!(
            "Purge journal: last state {}; {} entries deleted so far; {} failed in the last run",
            j.last, j.deleted, j.failed
        )
    }))
}

fn append(
    journal: &mut File,
    record: &UnitRecord,
    state: &str,
    report: Option<&WalkReport>,
    reason: Option<String>,
) -> io::Result<()> {
    let mut line = serde_json::to_vec(&PurgeEvent {
        version: JOURNAL_VERSION,
        state: state.into(),
        device: record.device,
        inode: record.inode,
        allocated_bytes: record.allocated_bytes,
        deleted: report.map_or(0, |r| r.deleted),
        failed: report.map_or(0, |r| r.failed),
        reason,
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    })?;
    line.push(b'\n');
    journal.write_all(&line)?;
    journal.sync_all()
}

/// Test-only interruption points; each simulates process death.
#[derive(Clone, Debug, Default)]
pub(crate) struct PurgeFaults {
    pub walk: WalkFaults,
    pub stop_after_intent: bool,
    pub stop_after_rename: bool,
}

/// What one purge run did.
#[derive(Debug)]
pub struct PurgeResult {
    pub report: WalkReport,
    pub completed: bool,
}

fn not_eligible(id: &str, why: &str) -> io::Error {
    error(&format!("Recovery {id}: {why}; nothing changed"))
}

/// Outcome of one deletion walk over a payload.
#[derive(Debug, Default)]
pub struct WalkReport {
    pub deleted: usize,
    pub failed: usize,
    /// First failures (path relative to the payload, reason).
    pub failures: Vec<(PathBuf, String)>,
    /// Set when the unit was stopped (device/mount boundary, identity change).
    pub stopped: Option<String>,
}
const MAX_LISTED_FAILURES: usize = 200;
const MAX_DEPTH: usize = 512;

/// Injection points for boundary, identity and interruption tests.
#[derive(Clone, Debug, Default)]
pub(crate) struct WalkFaults {
    pub foreign_device: Option<PathBuf>,
    pub changed_identity: Option<PathBuf>,
    pub stop_after_entries: Option<usize>,
}

impl WalkReport {
    fn fail(&mut self, path: &Path, error: &io::Error) {
        self.failed += 1;
        if self.failures.len() < MAX_LISTED_FAILURES {
            let reason = match error.raw_os_error() {
                Some(libc::EACCES | libc::EPERM) => format!(
                    "{error}: read-only directory, immutable flag or ACL; permissions were not changed"
                ),
                _ => error.to_string(),
            };
            self.failures.push((path.to_path_buf(), reason));
        }
    }
    fn deleted_one(&mut self, faults: &WalkFaults) -> io::Result<()> {
        self.deleted += 1;
        if faults.stop_after_entries == Some(self.deleted) {
            return Err(error("Injected interruption during deletion"));
        }
        Ok(())
    }
}

fn open_dir_at(parent: &File, name: &CStr) -> io::Result<File> {
    // SAFETY: valid descriptor and terminated name; the result is owned.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn unlink_at(parent: &File, name: &CStr, flags: i32) -> io::Result<()> {
    // SAFETY: valid descriptor and terminated name.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), flags) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Entry names of a pinned directory, read through a duplicate descriptor.
fn names(dir: &File) -> io::Result<Vec<CString>> {
    // SAFETY: dup of a valid descriptor; fdopendir takes ownership of it.
    let fd = unsafe { libc::dup(dir.as_raw_fd()) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }
    struct Stream(*mut libc::DIR);
    impl Drop for Stream {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }
    let stream = Stream(stream);
    unsafe { libc::rewinddir(stream.0) };
    let mut out = Vec::new();
    loop {
        unsafe { *libc::__error() = 0 };
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let errno = unsafe { *libc::__error() };
            if errno != 0 {
                return Err(io::Error::from_raw_os_error(errno));
            }
            return Ok(out);
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if name != c"." && name != c".." {
            out.push(name.to_owned());
        }
    }
}

/// Delete everything inside `dir`. Returns whether the subtree is now empty.
fn clear(
    dir: &File,
    relative: &Path,
    device: u64,
    depth: usize,
    report: &mut WalkReport,
    faults: &WalkFaults,
) -> io::Result<bool> {
    let entries = match names(dir) {
        Ok(entries) => entries,
        Err(e) => {
            report.fail(relative, &e);
            return Ok(false);
        }
    };
    let mut empty = true;
    for name in entries {
        let child = relative.join(OsStr::from_bytes(name.to_bytes()));
        let st = match fs_ops::stat_at(dir, &name) {
            Ok(st) => st,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                report.fail(&child, &e);
                empty = false;
                continue;
            }
        };
        let entry_device = if faults.foreign_device.as_deref() == Some(child.as_path()) {
            device.wrapping_add(1)
        } else {
            st.st_dev as u64
        };
        if entry_device != device {
            report.stopped = Some(format!(
                "{}: on another device or a mount point; unit stopped",
                child.display()
            ));
            return Ok(false);
        }
        if st.st_mode & libc::S_IFMT != libc::S_IFDIR {
            // Files, symlinks (as links, never followed) and special files.
            match unlink_at(dir, &name, 0) {
                Ok(()) => report.deleted_one(faults)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => {
                    report.fail(&child, &e);
                    empty = false;
                }
            }
            continue;
        }
        if depth >= MAX_DEPTH {
            report.stopped = Some(format!(
                "{}: deeper than {MAX_DEPTH} levels",
                child.display()
            ));
            return Ok(false);
        }
        let opened = match open_dir_at(dir, &name) {
            Ok(opened) => opened,
            Err(e) => {
                report.fail(&child, &e);
                empty = false;
                continue;
            }
        };
        let meta = opened.metadata()?;
        let changed = faults.changed_identity.as_deref() == Some(child.as_path());
        if changed || (meta.dev(), meta.ino()) != (st.st_dev as u64, st.st_ino) {
            report.stopped = Some(format!(
                "{}: identity changed during the walk; unit stopped",
                child.display()
            ));
            return Ok(false);
        }
        let cleared = clear(&opened, &child, device, depth + 1, report, faults)?;
        if report.stopped.is_some() {
            return Ok(false);
        }
        if !cleared {
            empty = false;
            continue;
        }
        let again = fs_ops::stat_at(dir, &name)?;
        if (again.st_dev, again.st_ino) != (st.st_dev, st.st_ino) {
            report.stopped = Some(format!(
                "{}: identity changed during the walk; unit stopped",
                child.display()
            ));
            return Ok(false);
        }
        match unlink_at(dir, &name, libc::AT_REMOVEDIR) {
            Ok(()) => report.deleted_one(faults)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                report.fail(&child, &e);
                empty = false;
            }
        }
    }
    Ok(empty)
}

/// Delete the directory `name` inside the pinned `parent`, whose identity
/// must be `expected` (device, inode). Every lookup is relative to a
/// descriptor opened with O_NOFOLLOW; nothing outside the tree is touched.
/// Errors are returned only when the walk could not start or was interrupted;
/// per-entry failures and stops are in the report.
pub(super) fn delete_tree(
    parent: &File,
    name: &CStr,
    expected: (u64, u64),
    faults: &WalkFaults,
) -> io::Result<WalkReport> {
    let top = open_dir_at(parent, name)?;
    let meta = top.metadata()?;
    if (meta.dev(), meta.ino()) != expected {
        return Err(error("Payload identity changed; nothing deleted"));
    }
    let mut report = WalkReport::default();
    let cleared = clear(&top, Path::new(""), expected.0, 1, &mut report, faults)?;
    if cleared && report.stopped.is_none() {
        let again = fs_ops::stat_at(parent, name)?;
        if (again.st_dev as u64, again.st_ino) != expected {
            report.stopped = Some("payload identity changed during the walk; unit stopped".into());
        } else {
            match unlink_at(parent, name, libc::AT_REMOVEDIR) {
                Ok(()) => report.deleted_one(faults)?,
                Err(e) => report.fail(Path::new("."), &e),
            }
        }
    }
    parent.sync_all()?;
    Ok(report)
}

impl RecoveryContext {
    pub(super) fn check_fixture_authority(
        &self,
        authority: Option<&FixtureRoot>,
    ) -> io::Result<()> {
        if self.fixture {
            let cap = authority.ok_or_else(|| error("fixture authorization required"))?;
            if self.fixture_identity != Some(cap.fixture_identity()?) {
                return Err(error("fixture authorization mismatch"));
            }
        }
        Ok(())
    }

    /// The recovery path must still name the pinned directory through real
    /// directories only; the activity check and messages use that path.
    pub(super) fn verify_root_path(&self) -> io::Result<()> {
        let current = fs_ops::open_dir(&self.root)
            .and_then(|d| d.metadata())
            .map_err(|_| {
                error("Recovery directory path changed or is behind a symlink; nothing changed")
            })?;
        let pinned = self.directory.metadata()?;
        if (current.dev(), current.ino()) != (pinned.dev(), pinned.ino()) {
            return Err(error(
                "Recovery directory path changed or is behind a symlink; nothing changed",
            ));
        }
        Ok(())
    }

    /// Read-only eligibility check for `recovery purge`.
    pub fn purge_review(
        &self,
        authority: Option<&FixtureRoot>,
        id: &str,
    ) -> io::Result<PurgeReview> {
        let _lock = self.lock()?;
        self.purge_review_locked(authority, id)
            .map(|(review, _)| review)
    }

    pub(super) fn purge_review_locked(
        &self,
        authority: Option<&FixtureRoot>,
        id: &str,
    ) -> io::Result<(PurgeReview, File)> {
        self.check_fixture_authority(authority)?;
        unit::valid_id(id)?;
        self.verify_root_path()?;
        let directory = self.open_record_dir(id).map_err(|e| {
            if matches!(e.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
                not_eligible(id, "record directory is a symlink or not a directory")
            } else {
                e
            }
        })?;
        let record = match unit::read_record_at(&directory)? {
            StoredRecord::Unit(record) => record,
            StoredRecord::V1(_) => {
                return Err(not_eligible(
                    id,
                    "version-1 record; recovery purge only deletes plan directory-unit payloads (schema 2) created by clean apply",
                ));
            }
        };
        record.validate(&self.home, &self.root)?;
        let payload = unit::present(&directory, c"payload")?;
        let deleting = unit::present(&directory, c"deleting")?;
        for st in [&payload, &deleting].into_iter().flatten() {
            if st.st_mode & libc::S_IFMT == libc::S_IFLNK {
                return Err(not_eligible(id, "payload is a symlink"));
            }
            if !unit::identity_matches(st, &record) {
                return Err(not_eligible(
                    id,
                    "payload identity (device, inode, owner, type) does not match the record",
                ));
            }
        }
        let journal = read_journal(&directory)?;
        let restore_started = unit::present(&directory, c"restore-intent")?.is_some();
        let resumable =
            || -> io::Result<String> { Ok(purge_state(&directory, id)?.unwrap_or_default()) };
        let (start, state) = match (&journal, payload.is_some(), deleting.is_some()) {
            (None, _, false) => {
                let state = unit::state_of(&directory, &record)?;
                if !state.starts_with("moved: payload in recovery") {
                    return Err(not_eligible(
                        id,
                        &format!("not in state 'moved: payload in recovery' (state: {state})"),
                    ));
                }
                (PurgeStart::Fresh, state.to_string())
            }
            (Some(j), true, false) if j.last == "intent" && !restore_started => {
                (PurgeStart::Fresh, resumable()?)
            }
            (Some(j), false, true)
                if matches!(j.last.as_str(), "intent" | "deleting" | "failed") =>
            {
                (PurgeStart::Resume, resumable()?)
            }
            (Some(j), false, false) if j.last == "completed" => {
                return Err(not_eligible(id, "payload was already permanently deleted"));
            }
            _ => {
                return Err(not_eligible(
                    id,
                    "purge journal and payload are inconsistent; inspect the record",
                ));
            }
        };
        Ok((
            PurgeReview {
                id: id.into(),
                record,
                start,
                state,
            },
            directory,
        ))
    }

    /// Permanently delete a reviewed directory-unit payload. Re-checks
    /// eligibility and activity under the lock, syncs the intent, renames the
    /// payload to `deleting`, walks it and records completed or failed.
    pub fn purge_directory_unit(
        &self,
        authority: Option<&FixtureRoot>,
        id: &str,
        activity: &mut dyn ActivityCheck,
    ) -> io::Result<PurgeResult> {
        self.purge_directory_unit_with(authority, id, activity, &PurgeFaults::default())
    }

    pub(crate) fn purge_directory_unit_with(
        &self,
        authority: Option<&FixtureRoot>,
        id: &str,
        activity: &mut dyn ActivityCheck,
        faults: &PurgeFaults,
    ) -> io::Result<PurgeResult> {
        if unsafe { libc::geteuid() } == 0 {
            return Err(error("Do not purge as root"));
        }
        let _lock = self.lock()?;
        let (review, directory) = self.purge_review_locked(authority, id)?;
        let name = match review.start {
            PurgeStart::Fresh => "payload",
            PurgeStart::Resume => "deleting",
        };
        match activity.check(&self.root.join(id).join(name)) {
            Activity::Idle => {}
            Activity::InUse(why) => {
                return Err(error(&format!(
                    "Payload is in use ({why}); nothing deleted"
                )));
            }
            Activity::Unknown(why) => {
                return Err(error(&format!(
                    "Payload activity is unknown ({why}); nothing deleted"
                )));
            }
        }
        let mut journal = super::context::open_child(
            &directory,
            JOURNAL,
            libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND,
        )?;
        let meta = journal.metadata()?;
        if !meta.is_file() || meta.nlink() != 1 || meta.uid() != unsafe { libc::geteuid() } {
            return Err(error("Unsafe purge journal"));
        }
        let record = &review.record;
        match review.start {
            PurgeStart::Fresh => {
                append(&mut journal, record, "intent", None, None)?;
                directory.sync_all()?;
                if faults.stop_after_intent {
                    return Err(error("Injected interruption after intent"));
                }
                fs_ops::rename_at_excl(&directory, c"payload", &directory, c"deleting")?;
                directory.sync_all()?;
                let renamed = fs_ops::stat_at(&directory, c"deleting")?;
                if !unit::identity_matches(&renamed, record) {
                    return Err(error("Payload changed during rename; nothing deleted"));
                }
                append(&mut journal, record, "deleting", None, None)?;
                if faults.stop_after_rename {
                    return Err(error("Injected interruption after rename"));
                }
            }
            PurgeStart::Resume => append(
                &mut journal,
                record,
                "deleting",
                None,
                Some("resumed manually".into()),
            )?,
        }
        let report = delete_tree(
            &directory,
            c"deleting",
            (record.device, record.inode),
            &faults.walk,
        )?;
        let completed = unit::present(&directory, c"deleting")?.is_none();
        if completed {
            append(&mut journal, record, "completed", Some(&report), None)?;
        } else {
            append(
                &mut journal,
                record,
                "failed",
                Some(&report),
                report.stopped.clone(),
            )?;
        }
        directory.sync_all()?;
        Ok(PurgeResult { report, completed })
    }
}

#[cfg(all(test, target_os = "macos"))]
pub(super) mod tests {
    use super::*;
    use crate::recovery::{RecoveryPolicy, unit::Faults};
    use std::{
        fs,
        os::unix::fs::symlink,
        path::{Path, PathBuf},
    };

    pub const PLAN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Bundle-style unit with internal links and one link outside the fixture.
    pub fn bundle(home: &Path, outside: &Path, name: &str) -> PathBuf {
        let unit = home.join("Library/Caches/ms-playwright").join(name);
        let framework = unit.join("Chromium.app/Contents/Frameworks/Chromium Framework.framework");
        let version = framework.join("Versions/A");
        for dir in ["Helpers", "Libraries", "Resources"] {
            fs::create_dir_all(version.join(dir)).unwrap();
        }
        fs::write(
            version.join("Helpers/chrome_crashpad_handler"),
            vec![b'h'; 40_000],
        )
        .unwrap();
        fs::write(version.join("Libraries/libEGL.dylib"), vec![b'l'; 20_000]).unwrap();
        fs::write(version.join("Resources/Info.plist"), b"plist").unwrap();
        symlink("A", framework.join("Versions/Current")).unwrap();
        for n in ["Helpers", "Libraries", "Resources"] {
            symlink(format!("Versions/Current/{n}"), framework.join(n)).unwrap();
        }
        symlink(outside, unit.join("external-profile")).unwrap();
        unit
    }

    pub fn record(unit: &Path) -> UnitRecord {
        let m = fs::symlink_metadata(unit).unwrap();
        UnitRecord::directory(
            unit.to_path_buf(),
            "playwright-browsers",
            PLAN,
            (m.dev(), m.ino(), m.uid()),
            (61_440, 60_005),
            1_000_000,
        )
    }

    /// Metadata of a whole tree without following links or reading contents.
    pub fn metadata(root: &Path) -> Vec<(PathBuf, u32, u64, u64, i64, i64)> {
        let mut out = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let m = fs::symlink_metadata(&path).unwrap();
            if m.is_dir() {
                pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            }
            out.push((path, m.mode(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()));
        }
        out.sort();
        out
    }

    pub struct Setup {
        pub home: PathBuf,
        pub outside: PathBuf,
        pub cap: FixtureRoot,
        pub context: RecoveryContext,
    }
    impl Setup {
        pub fn new() -> Self {
            let (home, cap) = FixtureRoot::create_test_fixture().unwrap();
            let (outside, _) = FixtureRoot::create_test_fixture().unwrap();
            fs::create_dir(outside.join("profile")).unwrap();
            fs::write(outside.join("profile/secret"), b"outside").unwrap();
            let context =
                RecoveryContext::initialize(&home, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
            Self {
                home,
                outside,
                cap,
                context,
            }
        }
        pub fn root(&self) -> PathBuf {
            self.home.join(".optimac-rust-recovery")
        }
        /// Create and move one unit; returns its recovery ID.
        pub fn moved(&self, name: &str, id: &str, faults: Faults) -> io::Result<()> {
            let unit = bundle(&self.home, &self.outside.join("profile"), name);
            let lock = self.context.lock()?;
            self.context.move_directory_unit_with(
                &lock,
                Some(&self.cap),
                id,
                &record(&unit),
                faults,
            )
        }
        /// Version-1 records shaped like the historical recovery entries.
        pub fn historical(&self, count: usize) -> Vec<String> {
            let lock = self.context.lock().unwrap();
            let mut ids = Vec::new();
            for i in 0..count {
                let id = format!("17000000000000000{i:02}-4242-{i}");
                let app = i % 3 == 0;
                let path = if app {
                    self.home.join(format!("Applications/Old{i}.app"))
                } else {
                    self.home.join(format!(".cache/pip/http/{i}"))
                };
                let dir = self
                    .context
                    .create_journal(
                        &id,
                        &serde_json::from_value(serde_json::json!({
                            "path": path, "dev": 1, "ino": i + 1, "size": 7, "modified": 0, "app": app
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                drop(dir);
                let payload = self.root().join(&id).join("payload");
                if app {
                    fs::create_dir_all(payload.join("Contents")).unwrap();
                    fs::write(payload.join("Contents/Info.plist"), b"old app").unwrap();
                } else if i % 3 == 1 {
                    fs::write(&payload, b"old cache").unwrap();
                }
                ids.push(id);
            }
            drop(lock);
            ids
        }
    }
    impl Drop for Setup {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.outside, fs::Permissions::from_mode(0o700));
            let _ = fs::remove_dir_all(&self.home);
            let _ = fs::remove_dir_all(&self.outside);
        }
    }

    fn refusal(s: &Setup, id: &str) -> String {
        s.context
            .purge_review(Some(&s.cap), id)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn purge_review_accepts_only_moved_directory_unit_records() {
        let s = Setup::new();
        s.moved("chromium-1248", "100", Faults::default()).unwrap();
        let review = s.context.purge_review(Some(&s.cap), "100").unwrap();
        assert_eq!(review.start, PurgeStart::Fresh);
        let text = review.describe();
        for needle in [
            "Recovery 100\n",
            "/Library/Caches/ms-playwright/chromium-1248\n",
            "Provider: playwright-browsers\n",
            &format!("Plan: {PLAN}\n"),
            "Allocated size: 61.44 KB (plan estimate)\n",
            "State: moved: payload in recovery; restore available\n",
            WARNING,
        ] {
            assert!(text.contains(needle), "missing {needle:?}\n{text}");
        }
        assert!(s.context.purge_review(None, "100").is_err());

        let historical = s.historical(6);
        s.moved(
            "firefox-1",
            "300",
            Faults {
                stop_after_move: true,
                ..Faults::default()
            },
        )
        .unwrap_err();
        s.moved(
            "firefox-2",
            "301",
            Faults {
                stop_after_record: true,
                ..Faults::default()
            },
        )
        .unwrap_err();
        s.moved("firefox-3", "302", Faults::default()).unwrap();
        s.context
            .restore_directory_unit(Some(&s.cap), "302")
            .unwrap();
        s.moved("webkit-1", "400", Faults::default()).unwrap();
        fs::rename(s.root().join("400/payload"), s.root().join("400/elsewhere")).unwrap();
        fs::create_dir(s.root().join("400/payload")).unwrap();
        s.moved("webkit-2", "401", Faults::default()).unwrap();
        fs::rename(s.root().join("401/payload"), s.root().join("401/real")).unwrap();
        symlink(s.root().join("401/real"), s.root().join("401/payload")).unwrap();
        s.moved("webkit-3", "402", Faults::default()).unwrap();
        fs::rename(s.root().join("402"), s.root().join("402-real")).unwrap();
        symlink(s.root().join("402-real"), s.root().join("402")).unwrap();
        let before = metadata(&s.home);

        for id in &historical {
            assert!(refusal(&s, id).contains("version-1 record"), "{id}");
        }
        for (id, needle) in [
            (
                "300",
                "not in state 'moved: payload in recovery' (state: interrupted_after_move",
            ),
            ("301", "(state: recorded_not_moved"),
            ("302", "(state: restored"),
            (
                "400",
                "payload identity (device, inode, owner, type) does not match",
            ),
            ("401", "payload is a symlink"),
            ("402", "record directory is a symlink"),
            ("999", "No such file"),
            ("../100", "Invalid recovery ID"),
        ] {
            let why = refusal(&s, id);
            assert!(why.contains(needle), "{id}: {why}");
        }
        assert_eq!(metadata(&s.home), before);

        // The recovery path itself behind a symlink is refused.
        let moved_root = s.home.join("recovery-real");
        fs::rename(s.root(), &moved_root).unwrap();
        symlink(&moved_root, s.root()).unwrap();
        assert!(refusal(&s, "100").contains("behind a symlink"));
        fs::remove_file(s.root()).unwrap();
        fs::rename(&moved_root, s.root()).unwrap();
        assert!(s.context.purge_review(Some(&s.cap), "100").is_ok());
    }

    fn tree_of(name: &str) -> (Setup, PathBuf, File, (u64, u64)) {
        let s = Setup::new();
        let unit = bundle(&s.home, &s.outside.join("profile"), name);
        let parent = File::open(unit.parent().unwrap()).unwrap();
        let m = fs::symlink_metadata(&unit).unwrap();
        (s, unit, parent, (m.dev(), m.ino()))
    }
    fn count(root: &Path) -> usize {
        metadata(root).len()
    }
    fn cname(path: &Path) -> CString {
        CString::new(path.file_name().unwrap().as_bytes()).unwrap()
    }

    #[test]
    fn deletion_walk_unlinks_links_as_links_and_never_leaves_the_payload() {
        use std::os::unix::fs::PermissionsExt;
        let (s, unit, parent, identity) = tree_of("chromium-1248");
        let outside_before = metadata(&s.outside);
        let entries = count(&unit);
        fs::set_permissions(&s.outside, fs::Permissions::from_mode(0o000)).unwrap();
        let mut wrong = identity;
        wrong.1 += 1;
        assert!(delete_tree(&parent, &cname(&unit), wrong, &WalkFaults::default()).is_err());
        assert_eq!(count(&unit), entries);
        let report = delete_tree(&parent, &cname(&unit), identity, &WalkFaults::default()).unwrap();
        assert_eq!(
            (report.deleted, report.failed, report.stopped),
            (entries, 0, None)
        );
        assert!(fs::symlink_metadata(&unit).is_err());
        fs::set_permissions(&s.outside, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(metadata(&s.outside), outside_before);
        assert_eq!(
            fs::read(s.outside.join("profile/secret")).unwrap(),
            b"outside"
        );
    }

    #[test]
    fn deletion_walk_records_permission_failures_without_chmod_and_resumes() {
        use std::os::unix::fs::PermissionsExt;
        let (_s, unit, parent, identity) = tree_of("chromium-1248");
        let readonly = unit.join("readonly");
        fs::create_dir(&readonly).unwrap();
        fs::write(readonly.join("a"), b"a").unwrap();
        fs::write(readonly.join("b"), b"b").unwrap();
        let locked = unit.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("c"), b"c").unwrap();
        fs::set_permissions(&readonly, fs::Permissions::from_mode(0o500)).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let report = delete_tree(&parent, &cname(&unit), identity, &WalkFaults::default()).unwrap();
        assert_eq!(report.failed, 3, "{:?}", report.failures);
        assert!(report.stopped.is_none());
        let failed: Vec<_> = report.failures.iter().map(|f| f.0.clone()).collect();
        for path in ["readonly/a", "readonly/b", "locked"] {
            assert!(failed.contains(&PathBuf::from(path)), "{failed:?}");
        }
        assert!(
            report
                .failures
                .iter()
                .all(|f| f.1.contains("permissions were not changed"))
        );
        // No chmod: modes are exactly as the fixture left them.
        assert_eq!(
            fs::symlink_metadata(&readonly).unwrap().mode() & 0o777,
            0o500
        );
        assert_eq!(fs::symlink_metadata(&locked).unwrap().mode() & 0o777, 0o000);
        assert!(!unit.join("Chromium.app").exists());
        assert!(readonly.join("a").exists() && unit.exists());
        // The fixture (not the walk) fixes permissions; a manual re-run finishes.
        fs::set_permissions(&readonly, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        let report = delete_tree(&parent, &cname(&unit), identity, &WalkFaults::default()).unwrap();
        assert_eq!((report.deleted, report.failed), (6, 0));
        assert!(!unit.exists());
    }

    #[test]
    fn deletion_walk_stops_at_device_boundaries_identity_changes_and_interruptions() {
        let (_s, unit, parent, identity) = tree_of("chromium-1248");
        let mount = PathBuf::from("Chromium.app/Contents/Frameworks");
        fs::write(
            unit.join("Chromium.app/Contents/Frameworks/marker"),
            b"keep",
        )
        .unwrap();
        let faults = WalkFaults {
            foreign_device: Some(mount.clone()),
            ..WalkFaults::default()
        };
        let report = delete_tree(&parent, &cname(&unit), identity, &faults).unwrap();
        let stop = report.stopped.unwrap();
        assert!(stop.contains("another device or a mount point"), "{stop}");
        assert_eq!(fs::read(unit.join(&mount).join("marker")).unwrap(), b"keep");
        let faults = WalkFaults {
            changed_identity: Some(mount.clone()),
            ..WalkFaults::default()
        };
        let report = delete_tree(&parent, &cname(&unit), identity, &faults).unwrap();
        assert!(report.stopped.unwrap().contains("identity changed"));
        assert!(unit.join(&mount).join("marker").exists());
        let before = count(&unit);
        let faults = WalkFaults {
            stop_after_entries: Some(2),
            ..WalkFaults::default()
        };
        assert!(delete_tree(&parent, &cname(&unit), identity, &faults).is_err());
        assert_eq!(count(&unit), before - 2);
        let report = delete_tree(&parent, &cname(&unit), identity, &WalkFaults::default()).unwrap();
        assert_eq!((report.deleted, report.failed), (before - 2, 0));
        assert!(!unit.exists());
    }

    pub struct Fixed(pub Activity, pub Vec<PathBuf>);
    impl ActivityCheck for Fixed {
        fn check(&mut self, unit: &Path) -> Activity {
            self.1.push(unit.to_path_buf());
            self.0.clone()
        }
    }
    pub fn idle() -> Fixed {
        Fixed(Activity::Idle, Vec::new())
    }
    fn inspect(s: &Setup, id: &str) -> String {
        s.context.inspect_directory_unit(id).unwrap()
    }
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn purge_deletes_moved_unit_payload_and_records_completion() {
        use std::os::unix::fs::PermissionsExt;
        let s = Setup::new();
        s.moved("chromium-1248", "100", Faults::default()).unwrap();
        let entries = metadata(&s.root().join("100/payload")).len();
        let outside = metadata(&s.outside);
        fs::set_permissions(&s.outside, fs::Permissions::from_mode(0o000)).unwrap();
        let mut activity = idle();
        let result = s
            .context
            .purge_directory_unit(Some(&s.cap), "100", &mut activity)
            .unwrap();
        assert!(result.completed);
        assert_eq!((result.report.deleted, result.report.failed), (entries, 0));
        assert_eq!(activity.1, [s.root().join("100/payload")]);
        assert_eq!(
            names_in(&s.root().join("100")),
            ["move-completed", "purge-journal", "record.json"]
        );
        let text = inspect(&s, "100");
        println!("INSPECT_AFTER_PURGE\n{text}");
        assert!(text.contains("State: purge_completed: payload permanently deleted"));
        assert!(text.contains(&format!(
            "Purge journal: last state completed; {entries} entries deleted so far; 0 failed"
        )));
        let again = s.context.purge_review(Some(&s.cap), "100").unwrap_err();
        assert!(again.to_string().contains("already permanently deleted"));
        let restore = s
            .context
            .restore_directory_unit(Some(&s.cap), "100")
            .unwrap_err();
        assert!(restore.to_string().contains("purge was started"));
        fs::set_permissions(&s.outside, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(metadata(&s.outside), outside);
    }

    #[test]
    fn purge_refuses_in_use_or_unknown_activity_without_writing() {
        let s = Setup::new();
        s.moved("chromium-1248", "100", Faults::default()).unwrap();
        let before = metadata(&s.home);
        for (state, needle) in [
            (
                Activity::InUse("in use by process 7 (Chromium)".into()),
                "in use",
            ),
            (
                Activity::Unknown("your process 8 could not be inspected".into()),
                "unknown",
            ),
        ] {
            let err = s
                .context
                .purge_directory_unit(Some(&s.cap), "100", &mut Fixed(state, Vec::new()))
                .unwrap_err()
                .to_string();
            assert!(
                err.contains(needle) && err.contains("nothing deleted"),
                "{err}"
            );
            assert_eq!(metadata(&s.home), before);
        }
        assert!(inspect(&s, "100").contains("State: moved"));
    }

    #[test]
    fn purge_interruptions_are_readable_never_replayed_and_resumable() {
        let s = Setup::new();
        s.moved("chromium-1248", "100", Faults::default()).unwrap();
        let entries = metadata(&s.root().join("100/payload")).len();
        let faults = PurgeFaults {
            walk: WalkFaults {
                stop_after_entries: Some(3),
                ..WalkFaults::default()
            },
            ..PurgeFaults::default()
        };
        assert!(
            s.context
                .purge_directory_unit_with(Some(&s.cap), "100", &mut idle(), &faults)
                .is_err()
        );
        let record_dir = s.root().join("100");
        let frozen = metadata(&record_dir);
        for _ in 0..2 {
            let text = inspect(&s, "100");
            assert!(
                text.contains("State: needs_reconciliation: purge interrupted while deleting; payload partially deleted and cannot be restored; re-run optimac recovery purge 100 to continue (never automatic)"),
                "{text}"
            );
            assert!(
                text.contains("Purge journal: last state deleting"),
                "{text}"
            );
        }
        assert!(
            s.context
                .restore_directory_unit(Some(&s.cap), "100")
                .is_err()
        );
        assert_eq!(
            metadata(&record_dir),
            frozen,
            "inspect/restore replayed or changed something"
        );
        let review = s.context.purge_review(Some(&s.cap), "100").unwrap();
        assert_eq!(review.start, PurgeStart::Resume);
        let mut activity = idle();
        let result = s
            .context
            .purge_directory_unit(Some(&s.cap), "100", &mut activity)
            .unwrap();
        assert!(result.completed);
        assert_eq!(result.report.deleted, entries - 3);
        assert_eq!(activity.1, [s.root().join("100/deleting")]);
        assert!(
            inspect(&s, "100")
                .contains("purge_completed: payload permanently deleted; it cannot be restored\n")
        );

        s.moved("firefox-1", "200", Faults::default()).unwrap();
        let faults = PurgeFaults {
            stop_after_intent: true,
            ..PurgeFaults::default()
        };
        assert!(
            s.context
                .purge_directory_unit_with(Some(&s.cap), "200", &mut idle(), &faults)
                .is_err()
        );
        assert!(
            inspect(&s, "200").contains("purge confirmed but nothing was deleted; payload intact")
        );
        assert!(s.root().join("200/payload").is_dir());
        let faults = PurgeFaults {
            stop_after_rename: true,
            ..PurgeFaults::default()
        };
        assert!(
            s.context
                .purge_directory_unit_with(Some(&s.cap), "200", &mut idle(), &faults)
                .is_err()
        );
        assert!(inspect(&s, "200").contains("purge interrupted while deleting"));
        assert!(
            s.context
                .purge_directory_unit(Some(&s.cap), "200", &mut idle())
                .unwrap()
                .completed
        );
        // A torn final line is reported, never guessed past.
        let journal = s.root().join("200/purge-journal");
        let mut bytes = fs::read(&journal).unwrap();
        bytes.extend_from_slice(br#"{"version":1,"sta"#);
        fs::write(&journal, &bytes).unwrap();
        assert!(inspect(&s, "200").contains("(last journal entry cut off)"));
    }

    #[test]
    fn purge_failures_and_boundaries_are_recorded_and_resumable_manually() {
        use std::os::unix::fs::PermissionsExt;
        let s = Setup::new();
        s.moved("chromium-1248", "100", Faults::default()).unwrap();
        let readonly = s.root().join("100/payload/Chromium.app/readonly");
        fs::create_dir(&readonly).unwrap();
        fs::write(readonly.join("a"), b"a").unwrap();
        fs::set_permissions(&readonly, fs::Permissions::from_mode(0o500)).unwrap();
        let result = s
            .context
            .purge_directory_unit(Some(&s.cap), "100", &mut idle())
            .unwrap();
        assert!(!result.completed);
        assert_eq!(result.report.failed, 1);
        assert_eq!(
            result.report.failures[0].0,
            PathBuf::from("Chromium.app/readonly/a")
        );
        let readonly = s.root().join("100/deleting/Chromium.app/readonly");
        assert_eq!(
            fs::symlink_metadata(&readonly).unwrap().mode() & 0o777,
            0o500
        );
        let text = inspect(&s, "100");
        println!("INSPECT_PURGE_FAILED\n{text}");
        assert!(text.contains("State: purge_failed: payload partially deleted and cannot be restored; 1 entry could not be deleted; fix the cause and re-run optimac recovery purge 100"), "{text}");
        fs::set_permissions(&readonly, fs::Permissions::from_mode(0o700)).unwrap();
        let result = s
            .context
            .purge_directory_unit(Some(&s.cap), "100", &mut idle())
            .unwrap();
        assert!(result.completed && result.report.failed == 0);
        assert!(inspect(&s, "100").contains("purge_completed"));

        s.moved("webkit-1", "200", Faults::default()).unwrap();
        let faults = PurgeFaults {
            walk: WalkFaults {
                foreign_device: Some(PathBuf::from("Chromium.app/Contents")),
                ..WalkFaults::default()
            },
            ..PurgeFaults::default()
        };
        let result = s
            .context
            .purge_directory_unit_with(Some(&s.cap), "200", &mut idle(), &faults)
            .unwrap();
        assert!(!result.completed);
        assert!(
            result
                .report
                .stopped
                .as_ref()
                .unwrap()
                .contains("mount point")
        );
        assert!(s.root().join("200/deleting/Chromium.app/Contents").is_dir());
        assert!(
            inspect(&s, "200").contains("purge_failed")
                && inspect(&s, "200").contains("mount point")
        );
    }
}
