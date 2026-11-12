//! Metadata-only project/output relationships. No project code or hooks run.
use super::*;

pub(super) fn discover(
    root: &Path,
    job: &config::Job,
    budget: Budget,
    cancel: &AtomicBool,
    deadline: Instant,
) -> (Vec<PathBuf>, bool) {
    let mut found = Vec::new();
    let device = match fs::symlink_metadata(root) {
        Ok(m) => m.dev(),
        Err(_) => return (found, false),
    };
    let mut queue = std::collections::VecDeque::from([(root.to_path_buf(), 0)]);
    let mut visited = 0;
    while let Some((path, depth)) = queue.pop_front() {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline || visited >= budget.entries
        {
            return (found, false);
        }
        if job.excluded(&path) {
            continue;
        }
        if validate(&path).is_err()
            || fs::symlink_metadata(&path)
                .map(|m| m.dev() != device)
                .unwrap_or(true)
        {
            return (found, false);
        }
        let entries = match fs::read_dir(&path) {
            Ok(v) => v,
            Err(_) => return (found, false),
        };
        let mut children = Vec::new();
        let mut markers = HashSet::new();
        for entry in entries {
            if cancel.load(Ordering::Relaxed)
                || Instant::now() >= deadline
                || visited >= budget.entries
            {
                return (found, false);
            }
            visited += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return (found, false),
            };
            if job.excluded(&entry.path()) {
                continue;
            }
            let kind = match entry.file_type() {
                Ok(k) => k,
                Err(_) => return (found, false),
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_file() {
                markers.insert(entry.file_name());
            }
            if kind.is_dir() {
                children.push(entry.path());
            }
        }
        for child in children {
            let name = child.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let recognized = match name {
                "target" => markers.contains(std::ffi::OsStr::new("Cargo.toml")),
                "node_modules" | ".next" => markers.contains(std::ffi::OsStr::new("package.json")),
                ".build" => markers.contains(std::ffi::OsStr::new("Package.swift")),
                "build" => {
                    markers.contains(std::ffi::OsStr::new("build.gradle"))
                        || markers.contains(std::ffi::OsStr::new("build.gradle.kts"))
                }
                _ => false,
            };
            if recognized {
                found.push(child);
            } else if !matches!(
                name,
                "target" | "node_modules" | ".next" | ".build" | ".venv" | "venv"
            ) {
                if depth >= 16 {
                    return (found, false);
                }
                queue.push_back((child, depth + 1));
            }
        }
    }
    (found, true)
}
