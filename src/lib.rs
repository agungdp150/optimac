pub mod actions;
pub mod categories;
pub mod recovery;
// Shared scanning and feature engine.
use std::{
    fs, io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
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
        if started.elapsed() >= Duration::from_secs(30) {
            report.limited = true;
            break;
        }
        if last_progress.elapsed() >= Duration::from_millis(100) {
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
        } else if metadata.is_dir() && stack.len() < 32 {
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

/// Decimal units: 1 KB = 1,000 bytes. Underlying accounting remains exact bytes.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1000.0 && unit < UNITS.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    // Avoid displaying 1000.00 KB at a rounding boundary.
    if (size * 100.0).round() >= 100000.0 && unit < UNITS.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    format!("{size:.2} {}", UNITS[unit])
}

#[cfg(test)]
mod size_tests {
    #[test]
    fn readable_units_preserve_boundaries() {
        for (bytes, expected) in [
            (0, "0 B"),
            (999, "999 B"),
            (1000, "1.00 KB"),
            (1_500_000, "1.50 MB"),
            (6_069_158_351, "6.07 GB"),
            (999_999, "1.00 MB"),
            (1_000_000_000_000, "1.00 TB"),
        ] {
            assert_eq!(super::format_size(bytes), expected);
        }
        assert_eq!(super::format_size(u64::MAX), "18.45 EB");
    }
}
