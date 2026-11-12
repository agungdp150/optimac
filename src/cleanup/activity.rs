//! In-process, read-only file-activity check used before recovery moves.
//!
//! No external command runs and no process is signalled, stopped or killed.
//! macOS APIs (libproc and sysctl), all usable without elevated rights:
//! - `proc_listallpids` enumerates processes;
//! - `proc_pidinfo(PROC_PIDLISTFDS)` + `proc_pidfdinfo(PROC_PIDFDVNODEPATHINFO)`
//!   give each open vnode's device, inode and path;
//! - `proc_pidinfo(PROC_PIDVNODEPATHINFO)` gives working and root directories;
//! - `proc_pidinfo(PROC_PIDREGIONPATHINFO2)` gives memory-mapped files
//!   (executables and libraries loaded from a unit);
//! - `sysctl(KERN_PROC_PID)` identifies the owner of processes libproc refuses.
//!
//! The kernel only lets an unprivileged caller inspect its own user's
//! processes. Processes owned by other users (root and system accounts) are
//! counted and disclosed, not inspected. Any of the caller's own processes that
//! cannot be inspected make the result unknown, never idle.
use std::{
    collections::HashSet,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activity {
    Idle,
    InUse(String),
    Unknown(String),
}

/// Injected into apply so tests never depend on real process state.
pub trait ActivityCheck {
    fn check(&mut self, unit: &Path) -> Activity;
    /// What the last check could not cover, for the review and results.
    fn coverage(&self) -> Option<String> {
        None
    }
}

/// The production check: a fresh process snapshot for every unit.
pub struct ProcessActivity {
    limit: Duration,
    max_entries: usize,
    /// (other users' processes, protected system processes, dead descriptors).
    last: Option<(usize, usize, usize)>,
}
impl Default for ProcessActivity {
    fn default() -> Self {
        Self {
            limit: Duration::from_secs(20),
            max_entries: 1_000_000,
            last: None,
        }
    }
}
impl ActivityCheck for ProcessActivity {
    fn check(&mut self, unit: &Path) -> Activity {
        match snapshot(Instant::now() + self.limit) {
            Ok(s) => {
                self.last = Some((s.other_users, s.protected_system, s.dead_descriptors));
                classify(&s, unit, self.max_entries)
            }
            Err(e) => Activity::Unknown(format!("activity check failed: {e}")),
        }
    }
    fn coverage(&self) -> Option<String> {
        let (others, protected, dead) = self.last?;
        let mut note = if protected > 0 {
            format!(
                "Activity check: inspected your processes except {protected} macOS system services in SIP-protected locations, which refuse inspection"
            )
        } else {
            "Activity check: inspected all of your processes".to_string()
        };
        if dead > 0 {
            note.push_str(&format!(
                "; {dead} open descriptors point to revoked or unmounted files, which cannot belong to a unit"
            ));
        }
        if others > 0 {
            note.push_str(&format!(
                "; {others} processes owned by other users (system services) need administrator rights and were not checked"
            ));
        }
        note.push('.');
        Some(note)
    }
}

#[derive(Debug)]
pub(crate) struct OpenFile {
    pid: i32,
    device: u64,
    inode: u64,
    path: PathBuf,
}
#[derive(Debug, Default)]
pub(crate) struct Snapshot {
    files: Vec<OpenFile>,
    other_users: usize,
    protected_system: usize,
    dead_descriptors: usize,
    uninspected: Option<String>,
}

fn under(path: &Path, unit: &Path) -> bool {
    path.starts_with(unit)
        || path
            .strip_prefix("/System/Volumes/Data")
            .is_ok_and(|rest| Path::new("/").join(rest).starts_with(unit))
}

/// A positive match wins; otherwise any uninspectable own process is unknown.
pub(crate) fn classify(s: &Snapshot, unit: &Path, max_entries: usize) -> Activity {
    let describe = |f: &OpenFile| format!("in use by process {} ({})", f.pid, name(f.pid));
    if let Some(f) = s.files.iter().find(|f| under(&f.path, unit)) {
        return Activity::InUse(describe(f));
    }
    let ids = match identities(unit, max_entries) {
        Ok(ids) => ids,
        Err(e) => return Activity::Unknown(format!("could not list unit contents: {e}")),
    };
    if let Some(f) = s.files.iter().find(|f| ids.contains(&(f.device, f.inode))) {
        return Activity::InUse(describe(f));
    }
    match &s.uninspected {
        Some(reason) => Activity::Unknown(reason.clone()),
        None => Activity::Idle,
    }
}

/// No-follow identities of every entry in the unit (links are not followed).
fn identities(unit: &Path, max_entries: usize) -> io::Result<HashSet<(u64, u64)>> {
    let mut ids = HashSet::new();
    let mut pending = vec![unit.to_path_buf()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path)?;
        ids.insert((meta.dev(), meta.ino()));
        if ids.len() > max_entries {
            return Err(io::Error::other(format!("more than {max_entries} entries")));
        }
        if meta.is_dir() {
            for entry in fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        }
    }
    Ok(ids)
}

#[cfg(target_os = "macos")]
fn name(pid: i32) -> String {
    let mut buffer = [0u8; 256];
    // SAFETY: writable buffer of the stated size.
    let n = unsafe { libc::proc_name(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if n <= 0 {
        return "name unavailable".into();
    }
    String::from_utf8_lossy(&buffer[..n as usize]).into_owned()
}
#[cfg(not(target_os = "macos"))]
fn name(_: i32) -> String {
    "name unavailable".into()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn snapshot(_: Instant) -> io::Result<Snapshot> {
    Err(io::Error::other("activity check is macOS-only"))
}

#[cfg(target_os = "macos")]
pub(crate) use macos::snapshot;

#[cfg(target_os = "macos")]
mod macos {
    use super::{OpenFile, Snapshot};
    use std::{
        ffi::{c_int, c_void},
        io,
        mem::{size_of, zeroed},
        path::PathBuf,
        ptr::null_mut,
        time::Instant,
    };

    const PROC_PIDFDVNODEPATHINFO: c_int = 2;
    const PROC_PIDREGIONPATHINFO2: c_int = 22;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcFileInfo {
        fi_openflags: u32,
        fi_status: u32,
        fi_offset: i64,
        fi_type: i32,
        fi_guardflags: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodeFdInfoWithPath {
        pfi: ProcFileInfo,
        pvip: libc::vnode_info_path,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcRegionInfo {
        pri_protection: u32,
        pri_max_protection: u32,
        pri_inheritance: u32,
        pri_flags: u32,
        pri_offset: u64,
        pri_behavior: u32,
        pri_user_wired_count: u32,
        pri_user_tag: u32,
        pri_pages_resident: u32,
        pri_pages_shared_now_private: u32,
        pri_pages_swapped_out: u32,
        pri_pages_dirtied: u32,
        pri_ref_count: u32,
        pri_shadow_depth: u32,
        pri_share_mode: u32,
        pri_private_pages_resident: u32,
        pri_shared_pages_resident: u32,
        pri_obj_id: u32,
        pri_depth: u32,
        pri_address: u64,
        pri_size: u64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcRegionWithPathInfo {
        prp_prinfo: ProcRegionInfo,
        prp_vip: libc::vnode_info_path,
    }
    // Sizes fixed by <sys/proc_info.h>; a mismatch must not compile.
    const _: () = assert!(size_of::<VnodeFdInfoWithPath>() == 1200);
    const _: () = assert!(size_of::<ProcRegionWithPathInfo>() == 1272);
    const _: () = assert!(size_of::<libc::proc_vnodepathinfo>() == 2352);

    /// Run a libproc call with errno cleared so a zero result is unambiguous.
    fn call(f: impl FnOnce() -> c_int) -> (c_int, c_int) {
        // SAFETY: errno is thread-local storage owned by this thread.
        unsafe { *libc::__error() = 0 };
        let r = f();
        (r, unsafe { *libc::__error() })
    }
    fn os_error(errno: c_int) -> io::Error {
        io::Error::from_raw_os_error(if errno == 0 { libc::EIO } else { errno })
    }

    fn open_file(pid: i32, vip: &libc::vnode_info_path) -> Option<OpenFile> {
        // SAFETY: vip_path is a fixed MAXPATHLEN byte array.
        let raw: &[u8] = unsafe {
            std::slice::from_raw_parts(vip.vip_path.as_ptr().cast(), size_of_val(&vip.vip_path))
        };
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let stat = &vip.vip_vi.vi_stat;
        if end == 0 && stat.vst_ino == 0 {
            return None;
        }
        use std::os::unix::ffi::OsStrExt;
        Some(OpenFile {
            pid,
            // Mirror std's `st_dev as u64` (dev_t is i32) so identities compare.
            device: stat.vst_dev as i32 as u64,
            inode: stat.vst_ino,
            path: PathBuf::from(std::ffi::OsStr::from_bytes(&raw[..end])),
        })
    }

    fn list_pids() -> io::Result<Vec<c_int>> {
        let mut capacity = 4096usize;
        loop {
            let mut pids = vec![0 as c_int; capacity];
            let bytes = (capacity * size_of::<c_int>()) as c_int;
            let (n, e) =
                call(|| unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) });
            if n <= 0 {
                return Err(os_error(e));
            }
            if (n as usize) < capacity {
                pids.truncate(n as usize);
                return Ok(pids);
            }
            capacity *= 2;
            if capacity > 1 << 20 {
                return Err(io::Error::other("process list too large"));
            }
        }
    }

    /// Effective uid from kinfo_proc. Offsets are verified against this
    /// process before use; a mismatch makes refused processes unknown.
    pub(crate) fn owner_of(pid: c_int) -> io::Result<Option<u32>> {
        const SIZE: usize = 648;
        const PID: usize = 40;
        const UID: usize = 420;
        let mut buffer = [0u8; SIZE];
        let mut length = SIZE;
        let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
        // SAFETY: valid mib and writable buffer with its length.
        let rc = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                4,
                buffer.as_mut_ptr().cast(),
                &mut length,
                null_mut(),
                0,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        if length == 0 {
            return Ok(None);
        }
        let read = |at: usize| u32::from_ne_bytes(buffer[at..at + 4].try_into().unwrap());
        if length != SIZE || read(PID) as c_int != pid {
            return Err(io::Error::other("unexpected process record layout"));
        }
        Ok(Some(read(UID)))
    }

    fn inspect(pid: c_int, files: &mut Vec<OpenFile>, dead: &mut usize) -> io::Result<()> {
        let (needed, e) =
            call(|| unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, null_mut(), 0) });
        if needed <= 0 {
            return Err(os_error(e));
        }
        let mut capacity = needed as usize / size_of::<libc::proc_fdinfo>() + 32;
        let fds = loop {
            // SAFETY: plain-data C struct; zero is a valid bit pattern.
            let mut fds = vec![unsafe { zeroed::<libc::proc_fdinfo>() }; capacity];
            let bytes = (capacity * size_of::<libc::proc_fdinfo>()) as c_int;
            let (got, e) = call(|| unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDLISTFDS,
                    0,
                    fds.as_mut_ptr().cast(),
                    bytes,
                )
            });
            if got <= 0 {
                return Err(os_error(e));
            }
            let n = got as usize / size_of::<libc::proc_fdinfo>();
            if n < capacity {
                fds.truncate(n);
                break fds;
            }
            capacity *= 2;
            if capacity > 1 << 20 {
                return Err(io::Error::other("descriptor list too large"));
            }
        };
        for fd in fds
            .iter()
            .filter(|f| f.proc_fdtype == libc::PROX_FDTYPE_VNODE as u32)
        {
            // SAFETY: plain-data C struct; zero is a valid bit pattern.
            let mut info: VnodeFdInfoWithPath = unsafe { zeroed() };
            let size = size_of::<VnodeFdInfoWithPath>() as c_int;
            let (r, e) = call(|| unsafe {
                libc::proc_pidfdinfo(
                    pid,
                    fd.proc_fd,
                    PROC_PIDFDVNODEPATHINFO,
                    (&mut info as *mut VnodeFdInfoWithPath).cast::<c_void>(),
                    size,
                )
            });
            if r == size {
                files.extend(open_file(pid, &info.pvip));
            } else if e == libc::ENOENT {
                // The kernel cannot take a reference on this vnode: it is dead
                // (revoked or from an unmounted volume) and is not part of any
                // live directory tree. Path-free PROC_PIDFDVNODEINFO fails alike.
                *dead += 1;
            } else if e != libc::EBADF {
                // The descriptor closing in between is benign; anything else is not.
                return Err(os_error(e));
            }
        }
        // SAFETY: plain-data C struct; zero is a valid bit pattern.
        let mut dirs: libc::proc_vnodepathinfo = unsafe { zeroed() };
        let size = size_of::<libc::proc_vnodepathinfo>() as c_int;
        let (r, e) = call(|| unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDVNODEPATHINFO,
                0,
                (&mut dirs as *mut libc::proc_vnodepathinfo).cast(),
                size,
            )
        });
        if r != size {
            return Err(os_error(e));
        }
        files.extend(open_file(pid, &dirs.pvi_cdir));
        files.extend(open_file(pid, &dirs.pvi_rdir));
        let mut address = 0u64;
        for _ in 0..1_000_000 {
            // SAFETY: plain-data C struct; zero is a valid bit pattern.
            let mut region: ProcRegionWithPathInfo = unsafe { zeroed() };
            let size = size_of::<ProcRegionWithPathInfo>() as c_int;
            let (r, e) = call(|| unsafe {
                libc::proc_pidinfo(
                    pid,
                    PROC_PIDREGIONPATHINFO2,
                    address,
                    (&mut region as *mut ProcRegionWithPathInfo).cast(),
                    size,
                )
            });
            if r == size {
                files.extend(open_file(pid, &region.prp_vip));
                let next = region
                    .prp_prinfo
                    .pri_address
                    .saturating_add(region.prp_prinfo.pri_size);
                if next <= address {
                    return Ok(());
                }
                address = next;
            } else if r == 0 && (e == 0 || e == libc::EINVAL) {
                // No further file-backed region.
                return Ok(());
            } else {
                return Err(os_error(e));
            }
        }
        Err(io::Error::other("mapped-region list too large"))
    }

    /// True only for executables in SIP-protected system locations, which no
    /// user process can place or replace (/usr/local is excluded).
    pub(super) fn system_executable(pid: c_int) -> bool {
        let mut buffer = vec![0u8; 4096];
        // SAFETY: writable buffer of the stated size.
        let n = unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
        if n <= 0 {
            return false;
        }
        let path = std::path::Path::new(std::ffi::OsStr::from_bytes(&buffer[..n as usize]));
        use std::os::unix::ffi::OsStrExt;
        path.is_absolute()
            && path.components().all(|c| {
                matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
            && (["/System", "/bin", "/sbin"]
                .iter()
                .any(|r| path.starts_with(r))
                || (path.starts_with("/usr") && !path.starts_with("/usr/local")))
    }

    pub(crate) fn snapshot(deadline: Instant) -> io::Result<Snapshot> {
        let me = unsafe { libc::geteuid() };
        let pids = list_pids()?;
        let owners_trusted = owner_of(unsafe { libc::getpid() }).ok().flatten() == Some(me);
        let mut s = Snapshot::default();
        for pid in pids.into_iter().filter(|&p| p > 0) {
            if Instant::now() > deadline {
                return Err(io::Error::other("activity check exceeded its time limit"));
            }
            // A process exiting mid-inspection can fail with errors other than
            // ESRCH (e.g. ENOENT for a reclaimed vnode); retry, then confirm it
            // is gone. A live process that keeps failing stays uninspected.
            let mut attempt = 0;
            let result = loop {
                let mut found = Vec::new();
                let mut dead = 0;
                match inspect(pid, &mut found, &mut dead) {
                    Ok(()) => break Ok((found, dead)),
                    Err(e)
                        if attempt < 3
                            && !matches!(e.raw_os_error(), Some(libc::ESRCH | libc::EPERM)) =>
                    {
                        attempt += 1;
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => break Err(e),
                }
            };
            let refusal = match result {
                Ok((found, dead)) => {
                    s.files.extend(found);
                    s.dead_descriptors += dead;
                    continue;
                }
                // Exited or zombie: holds no files.
                Err(e) if e.raw_os_error() == Some(libc::ESRCH) => continue,
                Err(_) if owners_trusted && matches!(owner_of(pid), Ok(None)) => continue,
                Err(e) if e.raw_os_error() == Some(libc::EPERM) && owners_trusted => {
                    match owner_of(pid) {
                        Ok(None) => continue,
                        Ok(Some(uid)) if uid != me => {
                            s.other_users += 1;
                            continue;
                        }
                        Ok(Some(_)) if system_executable(pid) => {
                            s.protected_system += 1;
                            continue;
                        }
                        Ok(Some(_)) => format!("your process {pid} could not be inspected"),
                        Err(e) => format!("owner of process {pid} unknown: {e}"),
                    }
                }
                Err(e) => format!("process {pid} could not be inspected: {e}"),
            };
            s.uninspected.get_or_insert(refusal);
        }
        Ok(s)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn activity_check_uses_libproc_in_process_and_reports_in_use_idle_unknown() {
        let (home, _cap) = crate::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
        assert_eq!(
            macos::owner_of(unsafe { libc::getpid() }).unwrap(),
            Some(unsafe { libc::geteuid() })
        );
        // The test binary lives under target/, never a SIP-protected location.
        assert!(!macos::system_executable(unsafe { libc::getpid() }));
        let unit = home.join("unit");
        fs::create_dir_all(unit.join("Versions/A")).unwrap();
        symlink("A", unit.join("Versions/Current")).unwrap();
        let held_path = unit.join("Versions/A/held");
        fs::write(&held_path, b"held").unwrap();
        let sibling = home.join("unit-sibling");
        fs::create_dir(&sibling).unwrap();
        let mut check = ProcessActivity::default();
        let held = fs::File::open(&held_path).unwrap();
        match check.check(&unit) {
            Activity::InUse(why) => assert!(
                why.contains(&format!("process {}", std::process::id())),
                "{why}"
            ),
            other => panic!("open descriptor not detected: {other:?}"),
        }
        // A path that merely shares a string prefix is a different unit.
        assert!(!matches!(check.check(&sibling), Activity::InUse(_)));
        drop(held);
        assert_eq!(check.check(&unit), Activity::Idle);
        // Working directories are activity too, matched by identity.
        let snapshot = Snapshot {
            files: vec![OpenFile {
                pid: 1,
                device: fs::metadata(&held_path).unwrap().dev(),
                inode: fs::metadata(&held_path).unwrap().ino(),
                path: PathBuf::from("/unrelated/alias"),
            }],
            ..Snapshot::default()
        };
        assert!(matches!(
            classify(&snapshot, &unit, 1000),
            Activity::InUse(_)
        ));
        assert!(matches!(
            classify(&snapshot, &unit, 2),
            Activity::Unknown(why) if why.contains("more than 2 entries")
        ));
        let firmlinked = Snapshot {
            files: vec![OpenFile {
                pid: 1,
                device: 0,
                inode: 0,
                path: Path::new("/System/Volumes/Data").join(held_path.strip_prefix("/").unwrap()),
            }],
            ..Snapshot::default()
        };
        assert!(matches!(
            classify(&firmlinked, &unit, 1000),
            Activity::InUse(_)
        ));
        let uninspected = Snapshot {
            uninspected: Some("your process 7 could not be inspected".into()),
            ..Snapshot::default()
        };
        assert_eq!(
            classify(&uninspected, &unit, 1000),
            Activity::Unknown("your process 7 could not be inspected".into())
        );
        fs::set_permissions(unit.join("Versions/A"), fs::Permissions::from_mode(0o000)).unwrap();
        assert!(matches!(check.check(&unit), Activity::Unknown(_)));
        fs::set_permissions(unit.join("Versions/A"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(home).unwrap();
    }
}
