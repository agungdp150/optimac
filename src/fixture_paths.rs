//! Descriptor-relative fixture inspection and narrowly scoped recovery initialization.
//! Existing recovery move, restore and purge entry points are not protected by this capability.
use std::{
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path, PathBuf},
};

pub struct FixtureRoot {
    root: File,
    path: PathBuf,
}
impl FixtureRoot {
    /// Open an explicitly supplied private directory beneath the system temp root.
    pub fn open(path: &Path) -> io::Result<Self> {
        // Fixed OS temporary roots, not caller-controlled TMPDIR or HOME.
        #[cfg(target_os = "macos")]
        let roots = [Path::new("/private/tmp"), Path::new("/private/var/folders")];
        #[cfg(not(target_os = "macos"))]
        let roots = [Path::new("/tmp"), Path::new("/var/tmp")];
        let temp = roots
            .into_iter()
            .find(|root| path.starts_with(root))
            .ok_or_else(|| io::Error::other("fixture must be under a fixed temporary root"))?
            .to_path_buf();
        if !path.is_absolute()
            || path == temp
            || !path.starts_with(&temp)
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err(io::Error::other(
                "fixture root must be inside temporary directory",
            ));
        }
        let mut dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)?;
        for c in path
            .strip_prefix(&temp)
            .map_err(|_| io::Error::other("outside temporary directory"))?
            .components()
        {
            let Component::Normal(name) = c else {
                return Err(io::Error::other("invalid fixture component"));
            };
            let name = std::ffi::CString::new(name.as_bytes())?;
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            dir = unsafe { File::from_raw_fd(fd) };
            let meta = dir.metadata()?;
            if (meta.uid() != unsafe { libc::geteuid() } && meta.uid() != 0)
                || meta.mode() & 0o022 != 0
            {
                return Err(io::Error::other("untrusted fixture ancestor"));
            }
        }
        let meta = dir.metadata()?;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "fixture root must be private and user-owned",
            ));
        }
        Ok(Self {
            root: dir,
            path: path.to_path_buf(),
        })
    }
    pub(crate) fn fixture_identity(&self) -> io::Result<(u64, u64)> {
        let meta = self.root.metadata()?;
        Ok((meta.dev(), meta.ino()))
    }

    pub(crate) fn source_parent(&self, path: &Path) -> io::Result<(File, std::ffi::CString)> {
        let relative = path
            .strip_prefix(&self.path)
            .map_err(|_| io::Error::other("outside fixture"))?;
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(io::Error::other("invalid source component"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing parent"))?;
        let directory = if parent == self.path {
            self.root.try_clone()?
        } else {
            self.open_existing(parent)?
        };
        if !directory.metadata()?.is_dir() {
            return Err(io::Error::other("source parent is not directory"));
        }
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::other("missing filename"))?;
        Ok((directory, std::ffi::CString::new(name.as_bytes())?))
    }

    /// Create only the recovery directory beneath this already-open fixture.
    /// The returned descriptor, rather than the original path, pins the result.
    pub(crate) fn initialize_recovery(&self, home: &Path) -> io::Result<File> {
        if home != self.path
            || home
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        {
            return Err(io::Error::other(
                "recovery home is outside authorized fixture",
            ));
        }
        let name = c".optimac-rust-recovery";
        let rc = unsafe { libc::mkdirat(self.root.as_raw_fd(), name.as_ptr(), 0o700) };
        if rc != 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe {
            libc::openat(
                self.root.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let directory = unsafe { File::from_raw_fd(fd) };
        let meta = directory.metadata()?;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(io::Error::other("unsafe recovery directory"));
        }
        Ok(directory)
    }

    /// Inspect an existing descendant without following symlinks. The returned
    /// handle stays pinned even if the directory entry is later replaced.
    pub fn open_existing(&self, path: &Path) -> io::Result<File> {
        let relative = path
            .strip_prefix(&self.path)
            .map_err(|_| io::Error::other("outside fixture"))?;
        let mut dir = self.root.try_clone()?;
        let mut components = relative.components().peekable();
        if components.peek().is_none() {
            return Err(io::Error::other("fixture root is not a target"));
        }
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return Err(io::Error::other("traversal refused"));
            };
            let name = std::ffi::CString::new(name.as_bytes())?;
            let directory = if components.peek().is_some() {
                libc::O_DIRECTORY
            } else {
                0
            };
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY
                        | directory
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC
                        | libc::O_NONBLOCK,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            dir = unsafe { File::from_raw_fd(fd) };
        }
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn fixture_capability_rejects_escape_and_symlink_without_mutation() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-fixture-capability-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let target = root.join("item");
        std::fs::write(&target, b"unchanged").unwrap();
        let link = root.join("escape");
        symlink("/", &link).unwrap();
        let capability = FixtureRoot::open(&root).unwrap();
        assert_eq!(
            capability
                .open_existing(&target)
                .unwrap()
                .metadata()
                .unwrap()
                .len(),
            9
        );
        assert!(capability.open_existing(&root.join("../outside")).is_err());
        assert!(capability.open_existing(&link.join("etc")).is_err());
        assert!(capability.open_existing(Path::new("/Users")).is_err());
        if let Some(home) = std::env::var_os("HOME") {
            let real_home = PathBuf::from(home);
            assert!(capability.open_existing(&real_home).is_err());
            assert!(
                capability
                    .open_existing(&real_home.join(".optimac-rust-recovery"))
                    .is_err()
            );
        }
        assert_eq!(std::fs::read(target).unwrap(), b"unchanged");
        drop(capability);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn fixture_recovery_initialization_requires_authorization() {
        let base = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-init-auth-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        let outside = base.join("outside");
        let fixture = base.join("fixture");
        std::fs::create_dir(&outside).unwrap();
        std::fs::create_dir(&fixture).unwrap();
        std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cap = FixtureRoot::open(&fixture).unwrap();
        assert!(crate::recovery::initialize_fixture_recovery(&fixture, None).is_err());
        assert!(crate::recovery::initialize_fixture_recovery(&outside, Some(&cap)).is_err());
        assert!(
            crate::recovery::initialize_fixture_recovery(&fixture.join("../outside"), Some(&cap))
                .is_err()
        );
        let link = base.join("escape");
        symlink(&outside, &link).unwrap();
        assert!(crate::recovery::initialize_fixture_recovery(&link, Some(&cap)).is_err());
        assert!(!outside.join(".optimac-rust-recovery").exists());
        assert!(!fixture.join(".optimac-rust-recovery").exists());
        let recovery_link = fixture.join(".optimac-rust-recovery");
        symlink(&outside, &recovery_link).unwrap();
        assert!(crate::recovery::initialize_fixture_recovery(&fixture, Some(&cap)).is_err());
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
        std::fs::remove_file(&recovery_link).unwrap();
        let dir = crate::recovery::initialize_fixture_recovery(&fixture, Some(&cap)).unwrap();
        assert!(dir.metadata().unwrap().is_dir());
        assert!(fixture.join(".optimac-rust-recovery").is_dir());
        drop(dir);
        drop(cap);
        std::fs::remove_dir_all(base).unwrap();
    }
}
