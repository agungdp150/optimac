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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

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
