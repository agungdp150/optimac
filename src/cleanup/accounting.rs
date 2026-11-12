use super::*;

#[cfg(target_os = "macos")]
pub(super) fn available(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: pointers refer to a terminated path and writable statfs storage.
    if unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(info.f_bavail.saturating_mul(u64::from(info.f_bsize)))
}
#[cfg(not(target_os = "macos"))]
pub(super) fn available(_: &Path) -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
pub(super) fn used(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    info.f_blocks
        .checked_sub(info.f_bfree)?
        .checked_mul(u64::from(info.f_bsize))
}
#[cfg(not(target_os = "macos"))]
pub(super) fn used(_: &Path) -> Option<u64> {
    None
}
