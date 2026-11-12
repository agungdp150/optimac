use super::*;

pub(super) fn status(error: &io::Error) -> Status {
    match error.kind() {
        io::ErrorKind::NotFound => Status::Absent,
        io::ErrorKind::PermissionDenied => Status::Denied,
        _ => Status::Blocked,
    }
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
    mut r: ProviderReport,
    cancel: &AtomicBool,
    budget: Budget,
    deadline: Instant,
    seen: &mut HashSet<(u64, u64)>,
    remaining: &mut usize,
    excluded: &dyn Fn(&Path) -> bool,
    progress: &mut dyn FnMut(&ProviderReport),
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
            Err(_) => {
                r.errors += 1;
                continue;
            }
        };
        let path = entry.path();
        if excluded(&path) {
            continue;
        }
        // Recheck ancestors before each metadata operation; never open file contents.
        if let Err(error) = validate(&path) {
            if error.kind() == io::ErrorKind::PermissionDenied {
                r.errors += 1;
            } else {
                r.skipped += 1;
            }
            continue;
        }
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                r.errors += 1;
                continue;
            }
        };
        if m.dev() != device {
            r.skipped += 1;
            continue;
        }
        if m.is_dir() {
            if stack.len() >= 32 {
                r.skipped += 1;
                continue;
            }
            match fs::read_dir(&path) {
                Ok(i) => stack.push(i),
                Err(_) => r.errors += 1,
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
            r.skipped += 1;
        }
    }
    if r.status == Status::Complete && (r.errors > 0 || r.skipped > 0) {
        r.status = Status::Partial;
    }
    r.logical_bytes = Some(logical);
    r.allocated_bytes = Some(allocated);
    progress(&r);
    r
}
