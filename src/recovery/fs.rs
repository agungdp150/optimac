use super::*;
use std::{fs::File, os::unix::ffi::OsStrExt};

pub(super) fn sync_dir(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
#[cfg(target_os = "macos")]
pub(super) fn move_exclusive(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let source_dir = open_dir(
        source
            .parent()
            .ok_or_else(|| error("Missing source parent"))?,
    )?;
    let target_dir = open_dir(
        target
            .parent()
            .ok_or_else(|| error("Missing target parent"))?,
    )?;
    let a = std::ffi::CString::new(
        source
            .file_name()
            .ok_or_else(|| error("Missing source name"))?
            .as_bytes(),
    )?;
    let b = std::ffi::CString::new(
        target
            .file_name()
            .ok_or_else(|| error("Missing target name"))?
            .as_bytes(),
    )?;
    let rc = unsafe {
        libc::renameatx_np(
            source_dir.as_raw_fd(),
            a.as_ptr(),
            target_dir.as_raw_fd(),
            b.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(target_os = "macos"))]
pub(super) fn move_exclusive(_source: &Path, _target: &Path) -> io::Result<()> {
    Err(error("Recovery mutation is macOS-only"))
}
pub(super) fn operation_lock(root: &Path) -> io::Result<File> {
    use std::os::fd::AsRawFd;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root.join(".lock"))?;
    let m = file.metadata()?;
    if !m.is_file() || m.nlink() != 1 || m.uid() != unsafe { libc::geteuid() } {
        return Err(error("Unsafe recovery lock"));
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(error("Another recovery operation is running"));
    }
    Ok(file)
}

pub(super) fn open_dir(path: &Path) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let mut dir = File::open("/")?;
    for component in path.components() {
        match component {
            std::path::Component::RootDir => continue,
            std::path::Component::Normal(name) => {
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
            }
            _ => return Err(error("Unsafe path component")),
        }
    }
    Ok(dir)
}
