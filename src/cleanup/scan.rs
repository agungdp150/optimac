use super::*;

pub(super) fn status(error: &io::Error) -> Status {
    match error.kind() {
        io::ErrorKind::NotFound => Status::Absent,
        io::ErrorKind::PermissionDenied => Status::Denied,
        _ => Status::Blocked,
    }
}
/// Only these native filesystem types are eligible for metadata traversal.
/// Unknown types fail closed; a type name alone does not detect same-device mounts.
const ALLOWED_FILESYSTEMS: &[&str] = &["apfs", "hfs"];

fn allowed_filesystem(name: &str) -> bool {
    ALLOWED_FILESYSTEMS.contains(&name)
}

#[cfg(target_os = "macos")]
fn validate_filesystem(path: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::other("Invalid filesystem path"))?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(name.as_ptr(), info.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    let kind = unsafe { std::ffi::CStr::from_ptr(info.f_fstypename.as_ptr()) }
        .to_str()
        .map_err(|_| io::Error::other("Unknown filesystem type"))?;
    if !allowed_filesystem(kind) {
        return Err(io::Error::other(format!(
            "virtual or mounted filesystem type {kind} excluded"
        )));
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn validate_filesystem(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Refuse existing ancestor symlinks and relative/traversal paths. Inventory only;
/// path-based metadata remains subject to filesystem races, never deletion-safe.
pub(super) fn validate(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::other("Absolute root required"));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if !matches!(part, Component::RootDir | Component::Normal(_)) {
            return Err(io::Error::other("Traversal refused"));
        }
        current.push(part);
        let m = fs::symlink_metadata(&current)?;
        if m.is_symlink() {
            return Err(io::Error::other("Symlink ancestor excluded"));
        }
        if m.is_dir() {
            validate_filesystem(&current)?;
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::macos::fs::MetadataExt;
            if m.st_flags() & 0x40000000 != 0 {
                return Err(io::Error::other("Cloud placeholder excluded"));
            }
        }
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn measure(
    r: ProviderReport,
    cancel: &AtomicBool,
    budget: Budget,
    deadline: Instant,
    seen: &mut HashSet<(u64, u64)>,
    remaining: &mut usize,
    excluded: &dyn Fn(&Path) -> bool,
    progress: &mut dyn FnMut(&ProviderReport),
) -> ProviderReport {
    measure_observed(
        r,
        cancel,
        budget,
        deadline,
        seen,
        remaining,
        excluded,
        progress,
        &mut |_, _, _, _| {},
    )
}

pub(super) type ObservationCallback<'a> =
    dyn FnMut(&ProviderReport, &Path, Option<&fs::Metadata>, Option<&str>) + 'a;

#[allow(clippy::too_many_arguments)]
pub(super) fn measure_observed(
    mut r: ProviderReport,
    cancel: &AtomicBool,
    budget: Budget,
    deadline: Instant,
    seen: &mut HashSet<(u64, u64)>,
    remaining: &mut usize,
    excluded: &dyn Fn(&Path) -> bool,
    progress: &mut dyn FnMut(&ProviderReport),
    observe: &mut ObservationCallback<'_>,
) -> ProviderReport {
    if excluded(&r.root) {
        r.note = "Excluded by policy".into();
        return r;
    }
    if cancel.load(Ordering::Relaxed) {
        r.status = Status::Cancelled;
        return r;
    }
    if Instant::now() >= deadline || *remaining == 0 {
        r.status = Status::Partial;
        r.reason("budget");
        r.note = "Global budget reached; not scanned".into();
        return r;
    }
    if let Err(e) = validate(&r.root) {
        r.status = status(&e);
        r.access = match r.status {
            Status::Denied => "denied",
            Status::Absent => "absent",
            _ => "unknown",
        }
        .into();
        r.note = e.to_string();
        return r;
    }
    let meta = match fs::symlink_metadata(&r.root) {
        Ok(m) => m,
        Err(e) => {
            r.status = status(&e);
            r.access = match r.status {
                Status::Denied => "denied",
                Status::Absent => "absent",
                _ => "unknown",
            }
            .into();
            r.note = e.to_string();
            return r;
        }
    };
    if !meta.is_dir() {
        r.status = Status::Blocked;
        r.note = "Expected directory".into();
        return r;
    }
    let iter = match fs::read_dir(&r.root) {
        Ok(i) => i,
        Err(e) => {
            r.status = status(&e);
            r.access = match r.status {
                Status::Denied => "denied",
                Status::Absent => "absent",
                _ => "unknown",
            }
            .into();
            r.note = e.to_string();
            return r;
        }
    };
    r.access = "readable".into();
    progress(&r);
    let mut last_progress = Instant::now();
    let device = meta.dev();
    let stop = (Instant::now() + budget.per_root).min(deadline);
    let mut stack = vec![iter];
    let mut visited = 0;
    let mut logical = 0u64;
    let mut allocated = 0u64;
    while !stack.is_empty() {
        if last_progress.elapsed() >= Duration::from_millis(100) {
            r.logical_bytes = Some(logical);
            r.allocated_bytes = Some(allocated);
            progress(&r);
            last_progress = Instant::now();
        }
        if cancel.load(Ordering::Relaxed) {
            r.status = Status::Cancelled;
            break;
        }
        if Instant::now() >= stop || visited >= budget.entries || *remaining == 0 {
            r.status = Status::Partial;
            r.reason("budget");
            r.note = "Budget reached; measured bytes are a lower bound".into();
            break;
        }
        let Some(entry) = stack.last_mut().unwrap().next() else {
            stack.pop();
            continue;
        };
        visited += 1;
        *remaining -= 1;
        let entry = match entry {
            Ok(e) => e,
            Err(error) => {
                r.reason(error_reason(&error));
                r.errors += 1;
                continue;
            }
        };
        let path = entry.path();
        if excluded(&path) {
            r.skipped += 1;
            r.reason(
                if super::providers::protected_path(&path)
                    || path.components().any(|p| {
                        p.as_os_str().to_str().is_some_and(|n| {
                            n.starts_with("com.apple.") || n.starts_with("group.com.apple.")
                        })
                    })
                {
                    "protected_name"
                } else {
                    "config_exclusion"
                },
            );
            observe(
                &r,
                &path,
                None,
                Some("excluded by configuration or protected-path policy"),
            );
            continue;
        }
        // Recheck ancestors before each metadata operation; never open file contents.
        if let Err(error) = validate(&path) {
            r.reason(if error.to_string().contains("Symlink") {
                "symlink"
            } else if error.to_string().contains("filesystem type") {
                "mount_boundary"
            } else {
                error_reason(&error)
            });
            observe(&r, &path, None, Some(&error.to_string()));
            if error.kind() == io::ErrorKind::PermissionDenied {
                r.errors += 1;
            } else {
                r.skipped += 1;
            }
            continue;
        }
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(error) => {
                r.reason(error_reason(&error));
                r.errors += 1;
                continue;
            }
        };
        if crosses_device_boundary(device, m.dev()) {
            r.reason("mount_boundary");
            observe(&r, &path, Some(&m), Some("mount or device boundary"));
            r.skipped += 1;
            continue;
        }
        if m.is_file() && m.nlink() > 1 {
            r.reason("hard_link");
        }
        observe(&r, &path, Some(&m), None);
        if m.is_dir() {
            if stack.len() >= 32 {
                r.reason("depth_limit");
                r.skipped += 1;
                continue;
            }
            match fs::read_dir(&path) {
                Ok(i) => stack.push(i),
                Err(error) => {
                    r.reason(error_reason(&error));
                    r.errors += 1;
                }
            }
        } else if m.is_file() {
            if seen.insert((m.dev(), m.ino())) {
                logical = logical.saturating_add(m.len());
                allocated = allocated.saturating_add(m.blocks().saturating_mul(512));
                r.files += 1;
            } else {
                r.skipped += 1;
            }
        } else {
            r.reason("special_file");
            r.skipped += 1;
        }
    }
    if r.status == Status::Complete && r.has_incomplete_reasons() {
        r.status = Status::Partial;
    }
    let reasons = r.reason_summary();
    if !reasons.is_empty() {
        if !r.note.is_empty() {
            r.note.push_str("; ");
        }
        r.note
            .push_str(&format!("skipped: {}; {reasons}", r.skipped));
    }
    r.logical_bytes = Some(logical);
    r.allocated_bytes = Some(allocated);
    progress(&r);
    r
}

/// Returns true when a traversal entry is on a different filesystem device.
/// This alone does not identify same-device virtual mounts or firmlinks.
pub(super) fn crosses_device_boundary(parent_device: u64, child_device: u64) -> bool {
    parent_device != child_device
}

fn error_reason(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "permission_denied",
        io::ErrorKind::NotFound => "vanished",
        _ => "io_error",
    }
}

#[cfg(test)]
mod mount_tests {
    use super::*;
    #[test]
    fn filesystem_allowlist_is_fail_closed() {
        for kind in [
            "nfs", "smbfs", "afpfs", "webdav", "osxfuse", "macfuse", "fuse-t", "unknown",
        ] {
            assert!(!allowed_filesystem(kind), "{kind}");
        }
        assert!(allowed_filesystem("apfs"));
        assert!(allowed_filesystem("hfs"));
    }
    #[test]
    fn device_boundary_comparison_does_not_authorize_crossing() {
        assert!(crosses_device_boundary(17, 18));
        assert!(!crosses_device_boundary(17, 17));
    }
}
