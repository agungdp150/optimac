#[path = "fs.rs"]
mod fs_ops;
pub mod purge;
use fs_ops::{move_exclusive, operation_lock, sync_dir};
// Conservative, journaled, same-volume cache cleanup. Never permanently deletes.
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub path: PathBuf,
    dev: u64,
    ino: u64,
    size: u64,
    modified: i64,
    #[serde(default)]
    app: bool,
    #[serde(default)]
    personal: bool,
}
impl Candidate {
    /// Candidate came from an explicitly allowlisted old-cache root, not a personal-file scan.
    pub fn is_cache_candidate(&self) -> bool {
        !self.personal && !self.app
    }
    pub fn logical_size(&self) -> u64 {
        self.size
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub items: Vec<Candidate>,
    pub notes: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct Record {
    version: u32,
    candidate: Candidate,
}
fn error(text: &str) -> io::Error {
    io::Error::other(text)
}
fn validate_chain(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(error("Absolute path required"));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, std::path::Component::ParentDir) {
            return Err(error("Parent traversal refused"));
        }
        current.push(part.as_os_str());
        if fs::symlink_metadata(&current)?.is_symlink() {
            return Err(error("Symlink ancestor refused"));
        }
    }
    Ok(())
}
fn cache_roots(home: &Path) -> Vec<PathBuf> {
    [
        ".cache/go-build",
        ".cache/pip",
        ".cache/uv",
        ".npm/_cacache",
        ".cargo/registry/cache",
        "Library/Caches/com.apple.Safari",
        "Library/Caches/Google/Chrome",
        "Library/Caches/Firefox",
        "Library/Caches/BraveSoftware/Brave-Browser",
        "Library/Caches/Microsoft Edge",
    ]
    .iter()
    .map(|p| home.join(p))
    .collect()
}
fn allowed(home: &Path, c: &Candidate) -> bool {
    if c.app {
        c.path.extension().is_some_and(|e| e == "app")
            && (c.path.parent() == Some(home.join("Applications").as_path())
                || c.path.parent() == Some(Path::new("/Applications")))
    } else if c.personal {
        [
            "Documents",
            "Downloads",
            "Desktop",
            "Pictures",
            "Movies",
            "Music",
        ]
        .iter()
        .any(|r| c.path.starts_with(home.join(r)) && c.path != home.join(r))
            && !sensitive(&c.path)
    } else {
        cache_roots(home)
            .iter()
            .any(|r| c.path.starts_with(r) && c.path != *r)
    }
}
fn validate_candidate(home: &Path, c: &Candidate) -> io::Result<()> {
    if !allowed(home, c) {
        return Err(error("Outside allowed cleanup/app roots"));
    }
    validate_chain(&c.path)?;
    let m = fs::symlink_metadata(&c.path)?;
    if (if c.app {
        !m.is_dir()
    } else {
        !m.is_file() || m.nlink() != 1
    }) || m.uid() != unsafe { libc::geteuid() }
        || m.dev() != c.dev
        || m.ino() != c.ino
        || m.len() != c.size
        || m.mtime() != c.modified
    {
        return Err(error(
            "Target changed or is not a single-link owned regular file",
        ));
    }
    Ok(())
}
pub fn preview(home: &Path, cancel: &AtomicBool) -> io::Result<Plan> {
    validate_chain(home)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs() as i64;
    let mut plan=Plan{items:Vec::new(),notes:vec!["Only cache files older than 30 days qualify automatically for review. No browser profiles or system files. Files may still be in use; close developer tools before executing.".into()]};
    let start = std::time::Instant::now();
    let mut visited = 0;
    for root in cache_roots(home) {
        if !root.exists() {
            continue;
        }
        if let Err(e) = validate_chain(&root) {
            plan.notes.push(format!("Skipped {root:?}: {e}"));
            continue;
        }
        let mut stack = vec![(root, 0)];
        while let Some((path, depth)) = stack.pop() {
            if cancel.load(Ordering::Relaxed)
                || visited >= crate::limits::CLEANUP_MAX_ENTRIES
                || start.elapsed() >= crate::limits::CLEANUP_TIMEOUT
            {
                plan.notes.push("Stopped early; partial preview".into());
                return Ok(plan);
            }
            visited += 1;
            let m = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    plan.notes.push(format!("Skipped {path:?}: {e}"));
                    continue;
                }
            };
            if m.is_symlink() {
                continue;
            }
            if m.is_dir() && depth < crate::limits::CLEANUP_MAX_DEPTH {
                match fs::read_dir(&path) {
                    Ok(entries) => {
                        for e in entries.take(
                            crate::limits::CLEANUP_MAX_ENTRIES
                                .saturating_sub(visited + stack.len()),
                        ) {
                            match e {
                                Ok(e) => stack.push((e.path(), depth + 1)),
                                Err(e) => plan.notes.push(format!("Read error: {e}")),
                            }
                        }
                    }
                    Err(e) => plan.notes.push(format!("Skipped {path:?}: {e}")),
                }
            } else if m.is_file()
                && now.saturating_sub(m.mtime()) >= crate::limits::CACHE_MIN_AGE_SECONDS
            {
                let c = Candidate {
                    path,
                    dev: m.dev(),
                    ino: m.ino(),
                    size: m.len(),
                    modified: m.mtime(),
                    app: false,
                    personal: false,
                };
                if validate_candidate(home, &c).is_ok() {
                    plan.items.push(c);
                }
            }
        }
    }
    Ok(plan)
}
#[derive(Debug)]
pub struct Category {
    pub name: &'static str,
    pub count: usize,
    pub bytes: u64,
}
pub fn category(c: &Candidate) -> &'static str {
    crate::categories::classify(&c.path)
}
fn sensitive(path: &Path) -> bool {
    path.components().any(|p| {
        let name = p.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".ssh"
                | ".aws"
                | ".gnupg"
                | ".git"
                | "Keychains"
                | "CloudStorage"
                | "Mobile Documents"
                | ".optimac-rust-recovery"
        ) || name.starts_with(".env")
            || name.ends_with(".app")
            || name.ends_with(".photoslibrary")
    })
}
pub fn select_files(plan: &Plan, paths: &std::collections::BTreeSet<PathBuf>) -> Plan {
    Plan {
        items: plan
            .items
            .iter()
            .filter(|c| paths.contains(&c.path))
            .cloned()
            .collect(),
        notes: plan.notes.clone(),
    }
}
pub fn review_files(home: &Path, scope: &Path, cancel: &AtomicBool) -> io::Result<Plan> {
    let mut plan = preview(home, cancel)?;
    let (paths, note) = crate::actions::review_paths(scope, cancel)?;
    plan.notes.push(format!(
        "Personal files in scope {scope:?}: review only until individually selected. {note}"
    ));
    let mut seen: std::collections::BTreeSet<_> =
        plan.items.iter().map(|c| c.path.clone()).collect();
    for path in paths {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if sensitive(&path) || seen.contains(&path) {
            continue;
        }
        if let Ok(m) = fs::symlink_metadata(&path) {
            let c = Candidate {
                path,
                dev: m.dev(),
                ino: m.ino(),
                size: m.len(),
                modified: m.mtime(),
                app: false,
                personal: true,
            };
            if validate_candidate(home, &c).is_ok() {
                seen.insert(c.path.clone());
                plan.items.push(c);
            }
        }
    }
    plan.notes.push("Categories describe content, not whether it is disposable. Browser scope includes old cache files only; history, passwords, cookies and profiles are excluded. Personal files are limited to your standard Documents, Downloads, Desktop, Pictures, Movies and Music folders within the selected scope.".into());
    Ok(plan)
}

pub fn categories(plan: &Plan) -> Vec<Category> {
    let mut groups = std::collections::BTreeMap::<&'static str, (usize, u64)>::new();
    for c in &plan.items {
        let group = groups.entry(category(c)).or_default();
        group.0 += 1;
        group.1 = group.1.saturating_add(c.size);
    }
    groups
        .into_iter()
        .map(|(name, (count, bytes))| Category { name, count, bytes })
        .collect()
}
pub fn select_categories(plan: &Plan, excluded: &std::collections::BTreeSet<String>) -> Plan {
    Plan {
        items: plan
            .items
            .iter()
            .filter(|c| !excluded.contains(category(c)))
            .cloned()
            .collect(),
        notes: plan.notes.clone(),
    }
}
pub fn describe(plan: &Plan) -> String {
    if plan.items.iter().any(|c| c.app) {
        return format!(
            "{}\n\nSelected app bundle: {:?}\nBundle size has not been calculated. Personal data stays in place.",
            plan.notes.join("\n"),
            plan.items[0].path
        );
    }
    let mut s = format!(
        "{}\n\n{} items; {} logical size. Moving to recovery does not free disk space.\n",
        plan.notes.join("\n"),
        plan.items.len(),
        crate::format_size(
            plan.items
                .iter()
                .fold(0u64, |sum, c| sum.saturating_add(c.size))
        )
    );
    for group in categories(plan) {
        s.push_str(&format!(
            "\n{} — {} files · {}\n",
            group.name,
            group.count,
            crate::format_size(group.bytes)
        ));
        for c in plan.items.iter().filter(|c| category(c) == group.name) {
            s.push_str(&format!("  {}  {:?}\n", crate::format_size(c.size), c.path));
        }
    }
    s
}
/// Initialize recovery for an explicitly authorized test fixture only.
/// This does not authorize other recovery operations or the production CLI.
pub fn initialize_fixture_recovery(
    home: &Path,
    authorization: Option<&crate::fixture_paths::FixtureRoot>,
) -> io::Result<std::fs::File> {
    authorization
        .ok_or_else(|| error("fixture authorization required"))?
        .initialize_recovery(home)
}

fn recovery_root(home: &Path) -> io::Result<PathBuf> {
    let p = crate::paths::AppPaths::from_home(Some(home.as_os_str().to_owned()))?.recovery();
    if !p.exists() {
        fs::DirBuilder::new().create(&p)?;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700))?;
    }
    validate_chain(&p)?;
    let m = fs::metadata(&p)?;
    if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(error(
            "Recovery directory must be owned by you and mode 0700",
        ));
    }
    Ok(p)
}

#[derive(Clone, Debug, Default)]
pub struct CleanupProgress {
    pub total: usize,
    pub processed: usize,
    pub moved: usize,
    pub skipped: usize,
    pub current: Option<PathBuf>,
}

pub fn execute(home: &Path, plan: &Plan, cancel: &AtomicBool) -> io::Result<String> {
    execute_with_progress(home, plan, cancel, |_| {})
}
pub fn execute_with_progress(
    home: &Path,
    plan: &Plan,
    cancel: &AtomicBool,
    mut update: impl FnMut(&CleanupProgress),
) -> io::Result<String> {
    let mut progress = CleanupProgress {
        total: plan.items.len(),
        ..CleanupProgress::default()
    };
    update(&progress);
    if unsafe { libc::geteuid() } == 0 {
        return Err(error("Do not run cleanup as root"));
    }
    let root = recovery_root(home)?;
    let _lock = operation_lock(&root)?;
    let mut report = String::new();
    for (i, c) in plan.items.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.push_str("Cancelled; completed moves remain recoverable\n");
            break;
        }
        progress.current = Some(c.path.clone());
        update(&progress);
        if let Err(e) = validate_candidate(home, c) {
            progress.processed += 1;
            progress.skipped += 1;
            update(&progress);
            report.push_str(&format!("Skipped {:?}: {e}\n", c.path));
            continue;
        }
        let path_text = c
            .path
            .to_str()
            .ok_or_else(|| error("Non-UTF8 mutation path refused"))?;
        let args = if c.app {
            vec!["-nP", "-t", "+D", path_text]
        } else {
            vec!["-nP", "-t", "--", path_text]
        };
        match crate::actions::command("/usr/sbin/lsof", &args, cancel) {
            Ok(open) if open.trim().is_empty() => {}
            Ok(_) => {
                progress.processed += 1;
                progress.skipped += 1;
                update(&progress);
                report.push_str(&format!("Skipped {:?}: in use\n", c.path));
                continue;
            }
            Err(e) => {
                if cancel.load(Ordering::Relaxed) {
                    report.push_str("Cancelled; completed moves remain recoverable\n");
                    break;
                }
                progress.processed += 1;
                progress.skipped += 1;
                update(&progress);
                report.push_str(&format!(
                    "Skipped {:?}: active-file status unknown: {e}\n",
                    c.path
                ));
                continue;
            }
        }
        let id = format!(
            "{}-{}-{i}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos(),
            std::process::id()
        );
        let dir = root.join(&id);
        fs::create_dir(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.join("record.json"))?;
        file.write_all(&serde_json::to_vec(&Record {
            version: 1,
            candidate: c.clone(),
        })?)?;
        file.sync_all()?;
        sync_dir(&dir)?;
        sync_dir(&root)?;
        validate_candidate(home, c)?;
        match move_exclusive(&c.path, &dir.join("payload")) {
            Ok(()) => {
                let moved = fs::symlink_metadata(dir.join("payload"))?;
                if moved.dev() != c.dev || moved.ino() != c.ino {
                    let rollback = move_exclusive(&dir.join("payload"), &c.path);
                    return Err(error(&format!(
                        "Target changed during move; rollback {rollback:?}. Recovery ID {id}"
                    )));
                }
                sync_dir(c.path.parent().unwrap())?;
                sync_dir(&dir)?;
                progress.processed += 1;
                progress.moved += 1;
                update(&progress);
                report.push_str(&format!("Moved {:?}; restore ID {id}\n", c.path));
            }
            Err(e) => {
                progress.processed += 1;
                progress.skipped += 1;
                update(&progress);
                report.push_str(&format!(
                    "Not moved {:?}: {e}; no copy/delete fallback\n",
                    c.path
                ));
            }
        }
    }
    Ok(report
        + "Recovery records: optimac log. Restore from the Restore recovered items menu. No disk-space reclamation claimed.\n")
}
pub fn history(home: &Path) -> io::Result<String> {
    let root = home.join(".optimac-rust-recovery");
    if !root.exists() {
        return Ok("No Rust recovery records. Legacy ~/.optimac data is untouched.\n".into());
    }
    validate_chain(&root)?;
    let mut s = String::new();
    for e in fs::read_dir(root)?.take(10000) {
        let e = e?;
        if !e.path().join("record.json").is_file() {
            continue;
        }
        s.push_str(&format!(
            "{}: payload {}\n",
            e.file_name().to_string_lossy(),
            e.path().join("payload").exists()
        ));
    }
    Ok(s)
}
pub fn restore(home: &Path, id: &str) -> io::Result<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(error("Invalid recovery ID"));
    }
    if unsafe { libc::geteuid() } == 0 {
        return Err(error("Do not restore as root"));
    }
    let root = recovery_root(home)?;
    let _lock = operation_lock(&root)?;
    let dir = root.join(id);
    validate_chain(&dir)?;
    let record_path = dir.join("record.json");
    validate_chain(&record_path)?;
    if fs::metadata(&record_path)?.len() > 65536 {
        return Err(error("Invalid recovery record size"));
    }
    let record: Record = serde_json::from_slice(&fs::read(record_path)?)?;
    if record.version != 1 {
        return Err(error("Unsupported recovery version"));
    }
    let c = record.candidate;
    if !allowed(home, &c) {
        return Err(error("Recovery destination outside allowed roots"));
    }
    validate_chain(
        c.path
            .parent()
            .ok_or_else(|| error("Invalid destination"))?,
    )?;
    let payload = dir.join("payload");
    validate_chain(&payload)?;
    let m = fs::symlink_metadata(&payload)?;
    if (if c.app { !m.is_dir() } else { !m.is_file() }) || m.dev() != c.dev || m.ino() != c.ino {
        return Err(error("Payload identity changed"));
    }
    move_exclusive(&payload, &c.path)?;
    let restored = fs::symlink_metadata(&c.path)?;
    if restored.dev() != c.dev || restored.ino() != c.ino {
        let rollback = move_exclusive(&c.path, &payload);
        return Err(error(&format!(
            "Payload changed during restore; rollback {rollback:?}"
        )));
    }
    sync_dir(c.path.parent().unwrap())?;
    sync_dir(&dir)?;
    Ok(format!("Restored {:?}; journal retained", c.path))
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn personal_selection_moves_only_reviewed_file_and_restores() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-personal-{}", std::process::id()));
        let docs = root.join("Documents");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("selected.pdf"), b"selected").unwrap();
        fs::write(docs.join("keep.jpg"), b"keep").unwrap();
        fs::write(docs.join(".env"), b"sensitive").unwrap();
        let cancel = AtomicBool::new(false);
        let plan = review_files(&root, &docs, &cancel).unwrap();
        assert_eq!(plan.items.len(), 2);
        let selected = select_files(
            &plan,
            &std::collections::BTreeSet::from([docs.join("selected.pdf")]),
        );
        assert_eq!(selected.items.len(), 1);
        assert!(
            execute(&root, &selected, &cancel)
                .unwrap()
                .contains("Moved")
        );
        assert_eq!(fs::read(docs.join("keep.jpg")).unwrap(), b"keep");
        assert_eq!(fs::read(docs.join(".env")).unwrap(), b"sensitive");
        let ids = recovery_ids(&root);
        assert_eq!(ids.len(), 1);
        restore(&root, &ids[0]).unwrap();
        assert_eq!(fs::read(docs.join("selected.pdf")).unwrap(), b"selected");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn category_selection_keeps_exact_files_and_totals() {
        let make = |path: &str, size| Candidate {
            path: path.into(),
            dev: 1,
            ino: 1,
            size,
            modified: 0,
            app: false,
            personal: false,
        };
        let plan = Plan {
            items: vec![
                make("/Users/test/.cache/pip/a", 1000),
                make("/Users/test/.cache/pip/b", 2000),
                make("/Users/test/Documents/c.pdf", 500),
            ],
            notes: vec![],
        };
        let groups = categories(&plan);
        let pip = groups.iter().find(|g| g.name == "Programming").unwrap();
        assert_eq!((pip.count, pip.bytes), (2, 3000));
        let excluded = std::collections::BTreeSet::from(["Programming".into()]);
        let selected = select_categories(&plan, &excluded);
        assert_eq!(selected.items.len(), 1);
        assert_eq!(selected.items[0].path, plan.items[2].path);
        assert!(describe(&selected).contains("Documents"));
        assert!(!describe(&selected).contains("Programming"));
        let excluded = categories(&plan)
            .iter()
            .map(|g| g.name.to_string())
            .collect();
        assert!(select_categories(&plan, &excluded).items.is_empty());
    }
    #[test]
    fn app_bundle_roundtrip_and_changed_candidate_refusal() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-app-roundtrip-{}", std::process::id()));
        let app = root.join("Applications/Fixture.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(app.join("Contents/Info.plist"), b"fixture").unwrap();
        let plan = app_plan(&root, &app).unwrap();
        let report = execute(&root, &plan, &AtomicBool::new(false)).unwrap();
        assert!(report.contains("Moved"), "{report}");
        let ids = recovery_ids(&root);
        assert_eq!(ids.len(), 1);
        restore(&root, &ids[0]).unwrap();
        assert_eq!(
            fs::read(app.join("Contents/Info.plist")).unwrap(),
            b"fixture"
        );
        let mut stale = app_plan(&root, &app).unwrap();
        stale.items[0].ino = 0;
        let mut events = Vec::new();
        assert!(
            execute_with_progress(&root, &stale, &AtomicBool::new(false), |p| events
                .push(p.clone()))
            .unwrap()
            .contains("Skipped")
        );
        assert_eq!(events.last().unwrap().processed, 1);
        assert_eq!(events.last().unwrap().skipped, 1);
        assert_eq!(events.last().unwrap().moved, 0);
        assert!(app.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn preview_refuses_symlinks_and_recent_files() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-preview-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let cache = root.join(".cache/pip");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("recent"), b"keep").unwrap();
        std::os::unix::fs::symlink(&root, cache.join("loop")).unwrap();
        let plan = preview(&root, &AtomicBool::new(false)).unwrap();
        assert!(plan.items.is_empty());
        assert!(restore(&root, "../outside").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recover_without_overwriting() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-recover-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let cache = root.join(".cache/pip");
        fs::create_dir_all(&cache).unwrap();
        let p = cache.join("test");
        fs::write(&p, b"keep").unwrap();
        let m = fs::metadata(&p).unwrap();
        let c = Candidate {
            path: p.clone(),
            dev: m.dev(),
            ino: m.ino(),
            size: m.len(),
            modified: m.mtime(),
            app: false,
            personal: false,
        };
        let plan = Plan {
            items: vec![c],
            notes: vec![],
        };
        let mut events = Vec::new();
        let result = execute_with_progress(&root, &plan, &AtomicBool::new(false), |p| {
            events.push(p.clone())
        })
        .unwrap();
        assert_eq!(events.first().unwrap().total, 1);
        assert_eq!(events.last().unwrap().processed, 1);
        assert_eq!(events.last().unwrap().moved, 1);
        assert!(result.contains("Moved"));
        assert!(!p.exists());
        let id = fs::read_dir(root.join(".optimac-rust-recovery"))
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().join("payload").exists())
            .unwrap()
            .file_name()
            .to_string_lossy()
            .into_owned();
        fs::write(&p, b"new").unwrap();
        assert!(restore(&root, &id).is_err());
        assert_eq!(fs::read(&p).unwrap(), b"new");
        fs::remove_file(&p).unwrap();
        restore(&root, &id).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}

pub fn app_plan(home: &Path, path: &Path) -> io::Result<Plan> {
    validate_chain(path)?;
    let m = fs::symlink_metadata(path)?;
    let c = Candidate {
        path: path.to_path_buf(),
        dev: m.dev(),
        ino: m.ino(),
        size: m.len(),
        modified: m.mtime(),
        app: true,
        personal: false,
    };
    validate_candidate(home, &c)?;
    if !path.join("Contents/Info.plist").is_file() {
        return Err(error("Not a recognized application bundle"));
    }
    Ok(Plan{items:vec![c],notes:vec!["Move only this app bundle to recovery. Quit the app first. Personal data and shared leftovers are preserved. Bundle byte totals are not calculated here.".into()]})
}
pub fn app_paths(home: &Path) -> Vec<PathBuf> {
    [home.join("Applications"), PathBuf::from("/Applications")]
        .iter()
        .filter_map(|r| fs::read_dir(r).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| app_plan(home, p).is_ok())
        .collect()
}
pub fn recovery_ids(home: &Path) -> Vec<String> {
    fs::read_dir(home.join(".optimac-rust-recovery"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.path().join("payload").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}
