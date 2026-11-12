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
    lock_at(&open_dir(root)?)
}

pub(super) fn lock_at(root: &File) -> io::Result<File> {
    use std::os::fd::AsRawFd;
    let file = super::context::open_child(root, ".lock", libc::O_RDWR | libc::O_CREAT)?;
    let m = file.metadata()?;
    if !m.is_file() || m.nlink() != 1 || m.uid() != unsafe { libc::geteuid() } {
        return Err(error("Unsafe recovery lock"));
    }
    // A process forked by this one (between fork and exec) briefly shares the
    // lock's open file description, so a just-released lock can look held.
    // Retry for a bounded half second before reporting a real concurrent holder.
    for attempt in 0..50 {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(file);
        }
        let busy = io::Error::last_os_error().raw_os_error() == Some(libc::EWOULDBLOCK);
        if !busy || attempt == 49 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err(error("Another recovery operation is running"))
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

/// No-follow metadata of a child entry relative to a pinned directory.
pub(super) fn stat_at(dir: &File, name: &std::ffi::CStr) -> io::Result<libc::stat> {
    use std::os::fd::AsRawFd;
    let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: valid descriptor, terminated name and a writable stat buffer.
    let rc = unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            name.as_ptr(),
            st.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fstatat succeeded and initialized the buffer.
    Ok(unsafe { st.assume_init() })
}

/// Exclusive same-volume rename between pinned directories; never replaces an
/// existing destination and never falls back to copy/delete.
#[cfg(target_os = "macos")]
pub(super) fn rename_at_excl(
    source: &File,
    source_name: &std::ffi::CStr,
    destination: &File,
    destination_name: &std::ffi::CStr,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: valid directory descriptors and terminated names.
    let rc = unsafe {
        libc::renameatx_np(
            source.as_raw_fd(),
            source_name.as_ptr(),
            destination.as_raw_fd(),
            destination_name.as_ptr(),
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
pub(super) fn rename_at_excl(
    _: &File,
    _: &std::ffi::CStr,
    _: &File,
    _: &std::ffi::CStr,
) -> io::Result<()> {
    Err(error("Recovery mutation is macOS-only"))
}

/// No path lookup outside the already-authorized fixture directory descriptors.
#[cfg(target_os = "macos")]
pub(super) fn fixture_move_at(
    source: &File,
    name: &std::ffi::CStr,
    destination: &File,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let rc = unsafe {
        libc::renameatx_np(
            source.as_raw_fd(),
            name.as_ptr(),
            destination.as_raw_fd(),
            c"payload".as_ptr(),
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
pub(super) fn fixture_move_at(_: &File, _: &std::ffi::CStr, _: &File) -> io::Result<()> {
    Err(error("Fixture recovery mutation is macOS-only"))
}
