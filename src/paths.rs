//! Shared path resolution; this does not authorize filesystem access.
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    home: PathBuf,
}
impl AppPaths {
    pub fn from_env() -> io::Result<Self> {
        Self::from_home(std::env::var_os("HOME"))
    }
    pub fn from_home(home: Option<OsString>) -> io::Result<Self> {
        let home = home
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::other("HOME unavailable"))?;
        if !home.is_absolute() {
            return Err(io::Error::other("HOME must be absolute"));
        }
        Ok(Self { home })
    }
    pub fn home(&self) -> &Path {
        &self.home
    }
    pub fn recovery(&self) -> PathBuf {
        self.home.join(".optimac-rust-recovery")
    }
    pub fn config(&self) -> PathBuf {
        self.home.join("Library/Application Support/OptiMac")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn injected_paths_resolve_consistently_and_missing_home_fails_closed() {
        let fixture = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("optimac-path-context-{}", std::process::id()));
        std::fs::create_dir(&fixture).unwrap();
        let paths = AppPaths::from_home(Some(fixture.as_os_str().to_owned())).unwrap();
        assert_eq!(paths.home(), fixture);
        assert_eq!(paths.recovery(), fixture.join(".optimac-rust-recovery"));
        let history = crate::recovery::history(paths.home()).unwrap();
        assert!(history.contains("No Rust recovery records"));
        assert!(!paths.recovery().exists());
        let config = crate::cleanup::config::Config::parse("schema_version=2\n[scope]\nproject_roots=['~/projects']\n[[rule]]\nid='project-artifacts'\nenabled=true\nscope='projects'").unwrap();
        std::fs::create_dir(fixture.join("projects")).unwrap();
        let inventory = crate::cleanup::configured_inventory(
            paths.home(),
            &std::sync::atomic::AtomicBool::new(false),
            crate::cleanup::Budget::quick(),
            &config,
        )
        .unwrap();
        assert!(
            inventory
                .providers
                .iter()
                .any(|p| p.root.starts_with(&fixture))
        );
        assert!(
            inventory
                .providers
                .iter()
                .all(|p| p.root.starts_with(&fixture))
        );
        assert_eq!(
            paths.config(),
            fixture.join("Library/Application Support/OptiMac")
        );
        assert!(AppPaths::from_home(None).is_err());
        assert!(AppPaths::from_home(Some("relative".into())).is_err());
        std::fs::remove_dir(fixture.join("projects")).unwrap();
        std::fs::remove_dir(fixture).unwrap();
    }
}
