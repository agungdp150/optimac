//! Read-only Go cache location discovery.
use super::pip::{Resolution, check_candidate, checked_root};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct GoRoots {
    pub build: Resolution,
    pub modules: Resolution,
}

pub fn resolve(home: &Path, cancel: &AtomicBool) -> GoRoots {
    #[cfg(test)]
    let candidates: &[&str] = &[];
    #[cfg(not(test))]
    let candidates = super::trust::candidates("go").unwrap();
    resolve_with(
        home,
        std::env::var_os("GOCACHE"),
        std::env::var_os("GOMODCACHE"),
        candidates,
        cancel,
    )
}
fn resolve_with(
    home: &Path,
    build: Option<OsString>,
    modules: Option<OsString>,
    candidates: &[&str],
    cancel: &AtomicBool,
) -> GoRoots {
    let mut a = build.map(|p| root(home, p.into(), "from_env", "GOCACHE"));
    let mut b = modules.map(|p| root(home, p.into(), "from_env", "GOMODCACHE"));
    if let (Some(build), Some(modules)) = (&a, &b) {
        return GoRoots {
            build: build.clone(),
            modules: modules.clone(),
        };
    }
    let mut result = None;
    for candidate in candidates {
        let path = Path::new(candidate);
        match check_candidate(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                result = Some(Err(format!("candidate refused: {e}")));
                break;
            }
            Ok(()) => {}
        }
        let env = [
            (OsStr::new("HOME"), home.as_os_str()),
            (OsStr::new("GOTOOLCHAIN"), OsStr::new("local")),
            (OsStr::new("GOPROXY"), OsStr::new("off")),
            (OsStr::new("GOSUMDB"), OsStr::new("off")),
            (OsStr::new("LC_ALL"), OsStr::new("C")),
        ];
        result = Some(
            match crate::actions::process::run(
                path,
                &["env".as_ref(), "GOCACHE".as_ref(), "GOMODCACHE".as_ref()],
                Path::new("/"),
                &env,
                Duration::from_secs(5),
                cancel,
            ) {
                Ok(r)
                    if r.exit_code == Some(0)
                        && !r.timed_out
                        && !r.cancelled
                        && !r.stdout_truncated
                        && !r.stderr_truncated =>
                {
                    match std::str::from_utf8(&r.stdout) {
                        Ok(s) => {
                            let lines: Vec<_> = s.lines().collect();
                            if lines.len() == 2
                                && lines.iter().all(|l| !l.is_empty() && !l.contains('\r'))
                            {
                                Ok((PathBuf::from(lines[0]), PathBuf::from(lines[1])))
                            } else {
                                Err("invalid go env output".into())
                            }
                        }
                        Err(_) => Err("invalid go env output".into()),
                    }
                }
                Ok(_) => Err("go env failed or output incomplete".into()),
                Err(e) => Err(e.to_string()),
            },
        );
        break;
    }
    match result {
        None => {
            if a.is_none() {
                a = Some(root(
                    home,
                    home.join("Library/Caches/go-build"),
                    "default_not_verified",
                    "GOCACHE",
                ));
            }
            if b.is_none() {
                b = Some(root(
                    home,
                    home.join("go/pkg/mod"),
                    "default_not_verified",
                    "GOMODCACHE",
                ));
            }
        }
        Some(Ok((build, modules))) => {
            if a.is_none() {
                a = Some(root(home, build, "from_tool", "GOCACHE"));
            }
            if b.is_none() {
                b = Some(root(home, modules, "from_tool", "GOMODCACHE"));
            }
        }
        Some(Err(detail)) => {
            if a.is_none() {
                a = Some(Resolution {
                    status: "command_failed",
                    path: None,
                    detail: detail.clone(),
                });
            }
            if b.is_none() {
                b = Some(Resolution {
                    status: "command_failed",
                    path: None,
                    detail,
                });
            }
        }
    }
    GoRoots {
        build: a.unwrap(),
        modules: b.unwrap(),
    }
}
fn root(home: &Path, path: PathBuf, source: &'static str, name: &str) -> Resolution {
    let mut r = checked_root(home, path.clone(), source);
    if r.path.is_some() {
        r.detail = format!(
            "{name}: {}{}",
            path.display(),
            if source == "default_not_verified" {
                " (default, not verified)"
            } else {
                ""
            }
        );
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn fake_go_resolves_both_and_preserves_overrides() {
        let home = std::env::current_dir()
            .unwrap()
            .join(format!(".optimac-go-test-{}", std::process::id()));
        std::fs::create_dir_all(home.join("build")).unwrap();
        std::fs::create_dir_all(home.join("modules")).unwrap();
        let exe = home.join("fake-go");
        std::fs::write(
            &exe,
            format!(
                "#!/bin/sh\nprintf '%s\\n%s\\n' '{}' '{}'\n",
                home.join("build").display(),
                home.join("modules").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cancel = AtomicBool::new(false);
        let paths = [exe.to_str().unwrap()];
        let r = resolve_with(&home, None, None, &paths, &cancel);
        assert_eq!(r.build.status, "from_tool");
        assert_eq!(r.modules.status, "from_tool");
        let r = resolve_with(
            &home,
            Some(home.join("build").into()),
            None,
            &paths,
            &cancel,
        );
        assert_eq!(r.build.status, "from_env");
        assert_eq!(r.modules.status, "from_tool");
        let r = resolve_with(&home, Some("/outside".into()), None, &[], &cancel);
        assert_eq!(r.build.status, "root_invalid");
        assert_eq!(r.modules.status, "default_not_verified");
        std::fs::write(&exe, b"#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            resolve_with(&home, None, None, &paths, &cancel)
                .build
                .status,
            "command_failed"
        );
        std::fs::remove_dir_all(home).unwrap();
    }
}
