//! Immutable proposal storage. No candidate path is opened or mutated here.
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path, PathBuf},
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub path: PathBuf,
    pub kind: String,
    pub device: u64,
    pub inode: u64,
    pub owner: u32,
    pub size: u64,
    pub modified_seconds: i64,
    pub modified_nanoseconds: i64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    pub path: PathBuf,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub provider: String,
    pub root: PathBuf,
    pub category: String,
    pub class: String,
    pub risk: String,
    pub completeness: String,
    pub consequence: String,
    pub targets: Vec<Target>,
    pub excluded: Vec<Exclusion>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub created_at_unix_seconds: u64,
    pub tool_version: String,
    pub providers: Vec<Provider>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StoredPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub hash: String,
    pub content: Content,
}
impl StoredPlan {
    /// Stable for identical content, including the supplied creation timestamp.
    pub fn new(mut content: Content) -> io::Result<Self> {
        for p in &mut content.providers {
            p.targets.sort_by(|a, b| a.path.cmp(&b.path));
            p.excluded
                .sort_by(|a, b| a.path.cmp(&b.path).then(a.reason.cmp(&b.reason)));
        }
        content
            .providers
            .sort_by(|a, b| a.provider.cmp(&b.provider).then(a.root.cmp(&b.root)));
        let hash = digest(SCHEMA_VERSION, &content)?;
        let plan = Self {
            schema_version: SCHEMA_VERSION,
            plan_id: hash.clone(),
            hash,
            content,
        };
        plan.validate()?;
        Ok(plan)
    }
    pub fn validate(&self) -> io::Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(io::Error::other("Unsupported plan schema version"));
        }
        if !valid_id(&self.plan_id)
            || self.plan_id != self.hash
            || self.hash != digest(self.schema_version, &self.content)?
        {
            return Err(io::Error::other("Plan ID/hash mismatch"));
        }
        if self.content.tool_version.is_empty() {
            return Err(io::Error::other("Missing tool version"));
        }
        let mut targets = std::collections::HashSet::new();
        for p in &self.content.providers {
            absolute(&p.root)?;
            if p.provider.is_empty() || p.consequence.is_empty() || p.completeness.is_empty() {
                return Err(io::Error::other("Malformed plan provider"));
            }
            if !p.targets.is_empty()
                && (!super::plan_builder::ALLOWED.contains(&p.provider.as_str())
                    || p.class != "rebuildable_cache"
                    || p.risk == "protected"
                    || p.completeness != "complete")
            {
                return Err(io::Error::other(
                    "Ineligible provider targets in stored plan",
                ));
            }
            for t in &p.targets {
                absolute(&t.path)?;
                if t.path.parent() != Some(p.root.as_path())
                    || !matches!(t.kind.as_str(), "file" | "directory")
                    || !(0..1_000_000_000).contains(&t.modified_nanoseconds)
                    || !targets.insert(&t.path)
                {
                    return Err(io::Error::other("Malformed or duplicate top-level target"));
                }
            }
        }
        Ok(())
    }
}
fn digest(version: u32, content: &Content) -> io::Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(version, content))?)
    ))
}
fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn absolute(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(io::Error::other("Absolute path without traversal required"));
    }
    Ok(())
}

/// The caller must supply every scan root before storing. Read-only open never
/// creates directories. The app-owned final directories must be private.
pub struct PlanStore {
    directory: File,
}
impl PlanStore {
    pub fn open(paths: &AppPaths, scan_roots: &[PathBuf], create: bool) -> io::Result<Self> {
        let location = paths.config().join("plans");
        absolute(&location)?;
        if location.starts_with(paths.recovery())
            || scan_roots
                .iter()
                .any(|r| location.starts_with(r) || r.starts_with(&location))
        {
            return Err(io::Error::other(
                "Plan store overlaps a scan or recovery root",
            ));
        }
        let mut directory = File::open("/")?;
        let mut current = PathBuf::from("/");
        for component in location.components() {
            let Component::Normal(name) = component else {
                continue;
            };
            current.push(name);
            let name = CString::new(name.as_bytes())?;
            let mut fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0
                && io::Error::last_os_error().kind() == io::ErrorKind::NotFound
                && create
                && current.starts_with(paths.home())
                && current != paths.home()
            {
                if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
                    return Err(io::Error::last_os_error());
                }
                directory.sync_all()?;
                fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
            }
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            directory = unsafe { File::from_raw_fd(fd) };
            let m = directory.metadata()?;
            if current.starts_with(paths.config())
                && (m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0)
            {
                return Err(io::Error::other(
                    "Plan directory must be user-owned and private (0700)",
                ));
            }
        }
        Ok(Self { directory })
    }
    pub fn load(&self, id: &str) -> io::Result<StoredPlan> {
        if !valid_id(id) {
            return Err(io::Error::other("Invalid plan ID"));
        }
        let name = CString::new(format!("{id}.json"))?;
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let m = file.metadata()?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || m.len() > MAX_BYTES
        {
            return Err(io::Error::other("Unsafe or oversized plan file"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(io::Error::other("Oversized plan file"));
        }
        let plan: StoredPlan = serde_json::from_slice(&bytes)?;
        plan.validate()?;
        if plan.plan_id != id {
            return Err(io::Error::other("Plan filename mismatch"));
        }
        Ok(plan)
    }
    pub fn save(&self, plan: &StoredPlan) -> io::Result<()> {
        plan.validate()?;
        let bytes = serde_json::to_vec_pretty(plan)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(io::Error::other("Oversized plan file"));
        }
        // A unique temporary file; never overwrite an existing plan or temporary file.
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = CString::new(format!(
            ".{}-{}-{seq}.tmp",
            plan.plan_id,
            std::process::id()
        ))?;
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                temp.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(&bytes)?;
        file.sync_all()?;
        let dest = CString::new(format!("{}.json", plan.plan_id))?;
        publish(&self.directory, &temp, &dest)?;
        self.directory.sync_all()
    }
    pub fn list(&self) -> io::Result<Vec<StoredPlan>> {
        // Open an independent directory cursor relative to the pinned directory.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let dir = unsafe { libc::fdopendir(fd) };
        if dir.is_null() {
            unsafe { libc::close(fd) };
            return Err(io::Error::last_os_error());
        }
        struct Dir(*mut libc::DIR);
        impl Drop for Dir {
            fn drop(&mut self) {
                unsafe { libc::closedir(self.0) };
            }
        }
        let dir = Dir(dir);
        let mut ids = Vec::new();
        loop {
            #[cfg(target_os = "macos")]
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(dir.0) };
            if entry.is_null() {
                #[cfg(target_os = "macos")]
                if unsafe { *libc::__error() } != 0 {
                    return Err(io::Error::last_os_error());
                }
                break;
            }
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_str()
                .map_err(io::Error::other)?;
            if let Some(id) = name.strip_suffix(".json") {
                ids.push(id.to_owned());
            }
        }
        ids.sort();
        ids.iter().map(|id| self.load(id)).collect()
    }
}
#[cfg(target_os = "macos")]
fn publish(dir: &File, temp: &std::ffi::CStr, dest: &std::ffi::CStr) -> io::Result<()> {
    if unsafe {
        libc::renameatx_np(
            dir.as_raw_fd(),
            temp.as_ptr(),
            dir.as_raw_fd(),
            dest.as_ptr(),
            libc::RENAME_EXCL,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn publish(_: &File, _: &std::ffi::CStr, _: &std::ffi::CStr) -> io::Result<()> {
    Err(io::Error::other("Plan publication is macOS-only"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn immutable_plan_storage_roundtrip_hash_schema_permissions_and_no_overwrite() {
        let (home, _auth) = crate::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
        let paths = AppPaths::from_home(Some(home.clone().into_os_string())).unwrap();
        assert!(PlanStore::open(&paths, &[], false).is_err());
        assert!(!paths.config().exists());
        let content = Content {
            created_at_unix_seconds: 123,
            tool_version: "fixture".into(),
            providers: vec![],
        };
        let plan = StoredPlan::new(content.clone()).unwrap();
        assert_eq!(plan, StoredPlan::new(content).unwrap());
        assert!(PlanStore::open(&paths, std::slice::from_ref(&home), true).is_err());
        assert!(!paths.config().exists());
        let store = PlanStore::open(&paths, &[], true).unwrap();
        store.save(&plan).unwrap();
        assert_eq!(store.load(&plan.plan_id).unwrap(), plan);
        assert_eq!(store.list().unwrap(), vec![plan.clone()]);
        let path = paths
            .config()
            .join("plans")
            .join(format!("{}.json", plan.plan_id));
        let before = std::fs::read(&path).unwrap();
        assert!(store.save(&plan).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(store.directory.metadata().unwrap().mode() & 0o777, 0o700);
        let mut bad = plan.clone();
        bad.schema_version += 1;
        std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(
            store
                .load(&plan.plan_id)
                .unwrap_err()
                .to_string()
                .contains("schema")
        );
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(store.load(&plan.plan_id).is_err());
        std::fs::write(&path, &before).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.load(&plan.plan_id).is_err());
        assert!(store.load("../escape").is_err());
        std::fs::remove_file(&path).unwrap();
        symlink(home.join("outside"), &path).unwrap();
        assert!(store.load(&plan.plan_id).is_err());
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn immutable_plan_rejects_root_units_and_tampering() {
        let mut plan = StoredPlan::new(Content {
            created_at_unix_seconds: 1,
            tool_version: "fixture".into(),
            providers: vec![],
        })
        .unwrap();
        plan.content.created_at_unix_seconds = 2;
        assert!(plan.validate().is_err());
        let p = Provider {
            provider: "playwright-browsers".into(),
            root: "/fixture/cache".into(),
            category: "Developer caches".into(),
            class: "rebuildable_cache".into(),
            risk: "review".into(),
            completeness: "complete".into(),
            consequence: "will re-download".into(),
            excluded: vec![],
            targets: vec![Target {
                path: "/fixture/cache".into(),
                kind: "directory".into(),
                device: 1,
                inode: 1,
                owner: 1,
                size: 0,
                modified_seconds: 0,
                modified_nanoseconds: 0,
                logical_bytes: 0,
                allocated_bytes: 0,
            }],
        };
        assert!(
            StoredPlan::new(Content {
                created_at_unix_seconds: 1,
                tool_version: "fixture".into(),
                providers: vec![p]
            })
            .is_err()
        );
    }
}
