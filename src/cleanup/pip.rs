//! Lightweight checks for future pip inventory probes. Never grants mutation authority.
use std::{
    io,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

/// Check an explicit candidate without searching PATH or executing it.
pub(super) fn check_candidate(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::other("candidate must be absolute"));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if !matches!(part, Component::RootDir | Component::Normal(_)) {
            return Err(io::Error::other("candidate contains traversal"));
        }
        current.push(part);
        let metadata = std::fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::other("symlink candidate or ancestor refused"));
        }
        if metadata.mode() & 0o022 != 0 {
            return Err(io::Error::other(
                "candidate or ancestor writable by other users",
            ));
        }
    }
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.mode() & 0o111 == 0 {
        return Err(io::Error::other("candidate is not an executable file"));
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Resolution {
    pub status: &'static str,
    pub path: Option<PathBuf>,
    pub detail: String,
}

fn checked_root(home: &Path, root: PathBuf, source: &'static str) -> Resolution {
    let validation = super::scan::validate(&root);
    let absent_default = source == "default_not_verified"
        && validation
            .as_ref()
            .err()
            .is_some_and(|e| e.kind() == io::ErrorKind::NotFound);
    if !root.is_absolute()
        || !root.starts_with(home)
        || root == home
        || (validation.is_err() && !absent_default)
    {
        return Resolution {
            status: "root_invalid",
            path: None,
            detail: "root is outside home, inaccessible, missing or has an unsafe ancestor".into(),
        };
    }
    Resolution {
        status: source,
        path: Some(root),
        detail: String::new(),
    }
}

pub fn resolve(home: &Path, cancel: &std::sync::atomic::AtomicBool) -> Resolution {
    #[cfg(test)]
    let candidates: &[&str] = &[];
    #[cfg(not(test))]
    let candidates = super::trust::candidates("pip").unwrap();
    resolve_with(home, std::env::var_os("PIP_CACHE_DIR"), candidates, cancel)
}

fn resolve_with(
    home: &Path,
    env: Option<std::ffi::OsString>,
    candidates: &[&str],
    cancel: &std::sync::atomic::AtomicBool,
) -> Resolution {
    if let Some(root) = env {
        return checked_root(home, root.into(), "from_env");
    }
    for candidate in candidates {
        let path = Path::new(candidate);
        match check_candidate(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Resolution {
                    status: "command_failed",
                    path: None,
                    detail: format!("candidate refused: {e}"),
                };
            }
            Ok(()) => {}
        }
        let result = crate::actions::process::run(
            path,
            &["cache".as_ref(), "dir".as_ref()],
            Path::new("/"),
            &[
                ("HOME".as_ref(), home.as_os_str()),
                ("PIP_NO_INDEX".as_ref(), "1".as_ref()),
                ("PIP_DISABLE_PIP_VERSION_CHECK".as_ref(), "1".as_ref()),
                ("LC_ALL".as_ref(), "C".as_ref()),
            ],
            std::time::Duration::from_secs(5),
            cancel,
        );
        return match result {
            Ok(r)
                if r.exit_code == Some(0)
                    && !r.timed_out
                    && !r.cancelled
                    && !r.stdout_truncated
                    && !r.stderr_truncated =>
            {
                match std::str::from_utf8(&r.stdout).ok().and_then(|s| {
                    let s = s.trim();
                    if s.is_empty() || s.contains('\n') || s.contains('\r') {
                        None
                    } else {
                        Some(s)
                    }
                }) {
                    Some(value) => checked_root(home, value.into(), "from_tool"),
                    None => Resolution {
                        status: "command_failed",
                        path: None,
                        detail: "invalid pip output".into(),
                    },
                }
            }
            Ok(_) => Resolution {
                status: "command_failed",
                path: None,
                detail: "pip probe failed, timed out or output was truncated".into(),
            },
            Err(e) => Resolution {
                status: "command_failed",
                path: None,
                detail: e.to_string(),
            },
        };
    }
    let mut result = checked_root(
        home,
        home.join("Library/Caches/pip"),
        "default_not_verified",
    );
    result.detail = "pip not installed; default, not verified".into();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn resolves_fake_pip_environment_failure_and_default() {
        let base = std::env::current_dir()
            .unwrap()
            .join(format!(".optimac-pip-run-{}", std::process::id()));
        std::fs::create_dir_all(base.join("Library/Caches/pip")).unwrap();
        std::fs::create_dir_all(base.join("custom")).unwrap();
        let binary = base.join("fake-pip");
        std::fs::write(
            &binary,
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{}'\n",
                base.join("custom").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let list = [binary.to_str().unwrap()];
        assert_eq!(
            resolve_with(&base, Some(base.join("custom").into()), &list, &cancel).status,
            "from_env"
        );
        let r = resolve_with(&base, None, &list, &cancel);
        assert_eq!(r.status, "from_tool", "{}", r.detail);
        assert_eq!(r.path, Some(base.join("custom")));
        assert_eq!(
            resolve_with(&base, None, &[], &cancel).status,
            "default_not_verified"
        );
        std::fs::write(&binary, b"#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            resolve_with(&base, None, &list, &cancel).status,
            "command_failed"
        );
        std::fs::write(&binary, b"#!/bin/sh\nprintf /outside\n").unwrap();
        assert_eq!(
            resolve_with(&base, None, &list, &cancel).status,
            "root_invalid"
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn rejects_symlink_and_writable_parent_without_running_candidate() {
        let base = std::env::current_dir()
            .unwrap()
            .join(format!(".optimac-pip-fixture-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        let binary = base.join("pip");
        std::fs::write(&binary, b"fixture, not executable code").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = base.join("link");
        symlink(&binary, &link).unwrap();
        assert!(check_candidate(&link).is_err());
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(check_candidate(&binary).is_err());
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(binary).unwrap();
        std::fs::remove_dir(base).unwrap();
    }
}
