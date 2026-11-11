//! Bounded, read-only metadata scanning.
use crate::limits;
use std::{
    fs, io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub files: u64,
    /// Logical file lengths, not allocated or reclaimable space.
    pub logical_bytes: u64,
    pub skipped: u64,
    pub errors: u64,
    pub cancelled: bool,
    pub limited: bool,
}

/// Single-worker metadata scan with bounded depth and entry count.
/// Symlinks are skipped. This is not a mutation authorization boundary.
pub fn scan(root: &Path, cancel: &AtomicBool, max_entries: u64) -> io::Result<Report> {
    scan_progress(root, cancel, max_entries, |_| {})
}

pub fn scan_progress(
    root: &Path,
    cancel: &AtomicBool,
    max_entries: u64,
    mut progress: impl FnMut(&Report),
) -> io::Result<Report> {
    let started = Instant::now();
    let mut last_progress = Instant::now();
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "root must be a directory, not a symlink",
        ));
    }
    #[cfg(unix)]
    let device = {
        use std::os::unix::fs::MetadataExt;
        metadata.dev()
    };
    let mut stack = vec![fs::read_dir(root)?];
    let mut report = Report::default();
    let mut visited = 0;
    while !stack.is_empty() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        if started.elapsed() >= limits::SCAN_TIMEOUT {
            report.limited = true;
            break;
        }
        if last_progress.elapsed() >= limits::PROGRESS_INTERVAL {
            progress(&report);
            last_progress = Instant::now();
        }
        let Some(entry) = stack.last_mut().unwrap().next() else {
            stack.pop();
            continue;
        };
        if visited >= max_entries {
            report.limited = true;
            break;
        }
        visited += 1;
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                report.errors += 1;
                continue;
            }
        };
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(
                ".ssh"
                    | ".aws"
                    | "Keychains"
                    | "CloudStorage"
                    | "Mobile Documents"
                    | ".git"
                    | "target"
                    | ".Spotlight-V100"
                    | ".fseventsd"
            )
        ) {
            report.skipped += 1;
            continue;
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => {
                report.errors += 1;
                continue;
            }
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != device {
                report.skipped += 1;
                continue;
            }
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::macos::fs::MetadataExt;
            if metadata.st_flags() & 0x40000000 != 0 {
                report.skipped += 1;
                continue;
            }
        }
        if metadata.is_file() {
            report.files += 1;
            report.logical_bytes = report.logical_bytes.saturating_add(metadata.len());
        } else if metadata.is_dir() && stack.len() < limits::SCAN_MAX_DEPTH {
            match fs::read_dir(entry.path()) {
                Ok(dir) => stack.push(dir),
                Err(_) => report.errors += 1,
            }
        } else {
            report.skipped += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_metadata_without_following_links_and_honors_limits() {
        let root = std::env::temp_dir().join(format!("optimac-rust-test-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("file"), b"hello").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let cancel = AtomicBool::new(false);
        let report = scan(&root, &cancel, 100).unwrap();
        assert_eq!(report.files, 1);
        assert_eq!(report.logical_bytes, 5);
        assert_eq!(fs::read(root.join("file")).unwrap(), b"hello");
        assert!(scan(&root, &cancel, 0).unwrap().limited);
        cancel.store(true, Ordering::Relaxed);
        assert!(scan(&root, &cancel, 100).unwrap().cancelled);
        fs::remove_dir_all(root).unwrap();
    }
}
