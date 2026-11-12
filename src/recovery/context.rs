//! Shared recovery metadata operations. Fixture authority never falls back to production.
use super::*;
use crate::fixture_paths::FixtureRoot;
use std::{
    ffi::CString,
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
};

#[derive(Clone, Copy)]
pub enum RecoveryPolicy<'a> {
    Production,
    Fixture(Option<&'a FixtureRoot>),
}

impl RecoveryPolicy<'_> {
    fn check_build(self) -> io::Result<()> {
        match self {
            Self::Production if cfg!(feature = "fixture-only") => Err(error(
                "production recovery authorization disabled in fixture-only build",
            )),
            Self::Fixture(None) => Err(error("fixture authorization required")),
            _ => Ok(()),
        }
    }

    /// Payload and path-based reconciliation are not fixture-authorized yet.
    pub(super) fn require_payload_authority(self) -> io::Result<()> {
        self.check_build()?;
        match self {
            Self::Production => Ok(()),
            Self::Fixture(_) => Err(error(
                "fixture payload/reconciliation authorization not implemented",
            )),
        }
    }
}

pub struct RecoveryContext {
    pub(super) root: PathBuf,
    pub(super) directory: File,
    fixture: bool,
}

impl RecoveryContext {
    pub fn initialize(home: &Path, policy: RecoveryPolicy<'_>) -> io::Result<Self> {
        policy.check_build()?;
        let (root, directory, fixture) = match policy {
            RecoveryPolicy::Production => {
                let root = production_recovery_root(home)?;
                let directory = fs_ops::open_dir(&root)?;
                (root, directory, false)
            }
            RecoveryPolicy::Fixture(authorization) => {
                let directory = authorization
                    .ok_or_else(|| error("fixture authorization required"))?
                    .initialize_recovery(home)?;
                (home.join(".optimac-rust-recovery"), directory, true)
            }
        };
        let meta = directory.metadata()?;
        if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(error(
                "Recovery directory must be owned by you and mode 0700",
            ));
        }
        Ok(Self {
            root,
            directory,
            fixture,
        })
    }

    pub fn is_fixture(&self) -> bool {
        self.fixture
    }

    /// Same nonblocking advisory lock used by production recovery.
    pub fn lock(&self) -> io::Result<File> {
        fs_ops::lock_at(&self.directory)
    }

    /// Persist the existing version-1 recovery record before moving a payload.
    /// This operation does not authorize or move the candidate itself.
    pub fn create_journal(&self, id: &str, candidate: &Candidate) -> io::Result<File> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
            return Err(error("Invalid recovery ID"));
        }
        let name = CString::new(id)?;
        if unsafe { libc::mkdirat(self.directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let directory = open_child(&self.directory, id, libc::O_RDONLY | libc::O_DIRECTORY)?;
        let meta = directory.metadata()?;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(error("Unsafe journal directory"));
        }
        let mut file = open_child(
            &directory,
            "record.json",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        file.write_all(&serde_json::to_vec(&Record {
            version: 1,
            candidate: candidate.clone(),
        })?)?;
        file.sync_all()?;
        directory.sync_all()?;
        self.directory.sync_all()?;
        Ok(directory)
    }
}

pub(super) fn open_child(parent: &File, name: &str, flags: i32) -> io::Result<File> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(error("Invalid metadata component"));
    }
    let name = CString::new(name)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "optimac-context-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let mut builder = fs::DirBuilder::new();
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700).create(&path).unwrap();
            Self(path)
        }
        fn candidate(&self) -> Candidate {
            Candidate {
                path: self.0.join("candidate"),
                dev: 0,
                ino: 0,
                size: 42,
                modified: 0,
                app: false,
                personal: false,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn fixture_recovery_context_rejects_missing_outside_and_symlink_authority() {
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let cap = FixtureRoot::open(&fixture.0).unwrap();
        assert!(RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Fixture(None)).is_err());
        for path in [&outside.0, &fixture.0.join("../outside")] {
            assert!(
                RecoveryContext::initialize(path, RecoveryPolicy::Fixture(Some(&cap))).is_err()
            );
        }
        let root = fixture.0.join(".optimac-rust-recovery");
        assert!(!root.exists());
        fs::write(outside.0.join("sentinel"), b"unchanged").unwrap();
        symlink(&outside.0, &root).unwrap();
        assert!(
            RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Fixture(Some(&cap))).is_err()
        );
        assert_eq!(fs::read(outside.0.join("sentinel")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 1);
        fs::remove_file(root).unwrap();
        let context =
            RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
        assert!(context.is_fixture());
        assert!(context.directory.metadata().unwrap().is_dir());
    }

    #[test]
    fn recovery_lock_fixture_is_relative_exclusive_and_refuses_links() {
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let cap = FixtureRoot::open(&fixture.0).unwrap();
        let context =
            RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
        let sentinel = outside.0.join("sentinel");
        fs::write(&sentinel, b"unchanged").unwrap();
        symlink(&sentinel, context.root.join(".lock")).unwrap();
        assert!(context.lock().is_err());
        fs::remove_file(context.root.join(".lock")).unwrap();
        fs::hard_link(&sentinel, context.root.join(".lock")).unwrap();
        assert!(context.lock().is_err());
        fs::remove_file(context.root.join(".lock")).unwrap();
        let lock = context.lock().unwrap();
        assert!(context.lock().is_err());
        assert_eq!(lock.metadata().unwrap().mode() & 0o777, 0o600);
        drop(lock);
        assert!(context.lock().is_ok());
        assert_eq!(fs::read(sentinel).unwrap(), b"unchanged");
    }

    #[test]
    fn recovery_journal_fixture_rejects_escape_and_pins_directory() {
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let cap = FixtureRoot::open(&fixture.0).unwrap();
        let context =
            RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
        let candidate = fixture.candidate();
        for id in ["../escape", "/outside", "", "1/2"] {
            assert!(context.create_journal(id, &candidate).is_err());
        }
        symlink(&outside.0, context.root.join("123")).unwrap();
        assert!(context.create_journal("123", &candidate).is_err());
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
        // Replacing the path after authorization must not redirect either write.
        let pinned = fixture.0.join("pinned");
        fs::rename(&context.root, &pinned).unwrap();
        symlink(&outside.0, &context.root).unwrap();
        let _lock = context.lock().unwrap();
        context.create_journal("456", &candidate).unwrap();
        assert!(context.create_journal("456", &candidate).is_err());
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
        let record: Record =
            serde_json::from_slice(&fs::read(pinned.join("456/record.json")).unwrap()).unwrap();
        assert_eq!(record.version, 1);
        assert_eq!(record.candidate.path, candidate.path);
        assert_eq!(
            fs::metadata(pinned.join("456/record.json")).unwrap().mode() & 0o777,
            0o600
        );
    }

    #[test]
    #[cfg(not(feature = "fixture-only"))]
    fn recovery_journal_production_context_preserves_format_and_validation() {
        // Production policy tested only against a disposable home.
        let fixture = Fixture::new();
        let empty = Plan {
            items: vec![],
            notes: vec![],
        };
        assert!(
            execute(
                RecoveryPolicy::Production,
                &fixture.0,
                &empty,
                &AtomicBool::new(false)
            )
            .is_ok()
        );
        let context = RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Production).unwrap();
        assert!(!context.is_fixture());
        let _lock = context.lock().unwrap();
        context.create_journal("789", &fixture.candidate()).unwrap();
        let record: Record =
            serde_json::from_slice(&fs::read(context.root.join("789/record.json")).unwrap())
                .unwrap();
        assert_eq!(record.version, 1);
        assert_eq!(record.candidate.size, 42);
        fs::set_permissions(&context.root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Production).is_err());
    }
    #[test]
    fn fixture_recovery_entry_points_fail_closed_without_payload_authority() {
        let fixture = Fixture::new();
        let cap = FixtureRoot::open(&fixture.0).unwrap();
        let plan = Plan {
            items: vec![],
            notes: vec![],
        };
        let cancel = AtomicBool::new(false);
        for policy in [
            RecoveryPolicy::Fixture(None),
            RecoveryPolicy::Fixture(Some(&cap)),
        ] {
            assert!(execute(policy, &fixture.0, &plan, &cancel).is_err());
            assert!(restore(policy, &fixture.0, "123").is_err());
            assert!(purge::plan(policy, &fixture.0, "123").is_err());
            assert!(purge::inspect(policy, &fixture.0, "123").is_err());
            assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
        }
        let context =
            RecoveryContext::initialize(&fixture.0, RecoveryPolicy::Fixture(Some(&cap))).unwrap();
        let _lock = context.lock().unwrap();
        context.create_journal("123", &fixture.candidate()).unwrap();
    }

    #[test]
    #[cfg(feature = "fixture-only")]
    fn fixture_recovery_build_refuses_explicit_production_policy() {
        let fixture = Fixture::new();
        let policy = RecoveryPolicy::Production;
        let plan = Plan {
            items: vec![],
            notes: vec![],
        };
        assert!(RecoveryContext::initialize(&fixture.0, policy).is_err());
        assert!(execute(policy, &fixture.0, &plan, &AtomicBool::new(false)).is_err());
        assert!(restore(policy, &fixture.0, "123").is_err());
        assert!(purge::plan(policy, &fixture.0, "123").is_err());
        assert!(purge::inspect(policy, &fixture.0, "123").is_err());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
}
