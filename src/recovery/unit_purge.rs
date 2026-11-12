//! Permanent purge of plan directory-unit recovery payloads (record schema 2).
//!
//! Authorized only for payloads of `plan_directory_unit` records created by
//! `clean apply`. Every other record kind, including all version-1 records,
//! is refused before anything is written. Nothing here follows a link.
use super::{
    RecoveryContext, error, fs_ops,
    unit::{self, StoredRecord, UnitRecord},
};
use crate::fixture_paths::FixtureRoot;
use std::{fs::File, io, os::unix::fs::MetadataExt};

pub const WARNING: &str =
    "Permanently deletes this recovery payload. It cannot be restored afterward.";

/// Whether a purge starts on an intact payload or continues an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeStart {
    Fresh,
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

fn not_eligible(id: &str, why: &str) -> io::Error {
    error(&format!("Recovery {id}: {why}; nothing changed"))
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
        let state = unit::state_of(&directory, &record)?;
        let payload = unit::present(&directory, c"payload")?;
        if let Some(st) = &payload
            && st.st_mode & libc::S_IFMT == libc::S_IFLNK
        {
            return Err(not_eligible(id, "payload is a symlink"));
        }
        if let Some(st) = &payload
            && !unit::identity_matches(st, &record)
        {
            return Err(not_eligible(
                id,
                "payload identity (device, inode, owner, type) does not match the record",
            ));
        }
        if !state.starts_with("moved: payload in recovery") {
            return Err(not_eligible(
                id,
                &format!("not in state 'moved: payload in recovery' (state: {state})"),
            ));
        }
        Ok((
            PurgeReview {
                id: id.into(),
                record,
                start: PurgeStart::Fresh,
                state: state.into(),
            },
            directory,
        ))
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
}
