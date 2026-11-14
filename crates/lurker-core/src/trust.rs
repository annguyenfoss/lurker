//! One trust boundary for executables launched through administrator elevation.
use crate::{AppError, AppResult};
use std::fs;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
use std::fs::OpenOptions;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
use std::io::Read;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Resolve first and execute only the returned path. Every canonical ancestor
/// must be administrator-controlled, including the executable itself. Sticky
/// directories such as /tmp deliberately do not satisfy this boundary.
pub fn trusted_privileged_executable(path: &Path) -> AppResult<PathBuf> {
    let canonical = fs::canonicalize(path)
        .map_err(|e| AppError::io("Cannot resolve privileged executable", e))?;
    let executable = fs::metadata(&canonical)
        .map_err(|e| AppError::io("Cannot inspect privileged executable", e))?;
    if !executable.is_file() || executable.permissions().mode() & 0o111 == 0 {
        return Err(AppError::new(
            "Privileged component is not an executable file.",
        ));
    }
    validate_runtime_ownership(&canonical)?;
    Ok(canonical)
}

fn validate_runtime_ownership(path: &Path) -> AppResult<()> {
    if !path.is_absolute() {
        return Err(AppError::new("Privileged runtime paths must be absolute."));
    }
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|e| AppError::io("Cannot inspect privileged path ownership", e))?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != 0
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(AppError::new(format!(
                "Refusing elevated execution through an untrusted path: {}. Install an administrator-owned Lurker package first.",
                ancestor.display()
            )));
        }
    }
    Ok(())
}

/// Private loader companions are read from their actual installation paths,
/// without following an alias into a different library or manifest inventory.
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
pub(crate) fn trusted_runtime_directory(path: &Path) -> AppResult<()> {
    validate_runtime_ownership(path)?;
    if !fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Inspect private runtime directory", e))?
        .is_dir()
    {
        return Err(AppError::new(
            "Private runtime directory is not a directory.",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
pub(crate) fn trusted_runtime_file(path: &Path, limit: u64) -> AppResult<Vec<u8>> {
    validate_runtime_ownership(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| AppError::io("Open private runtime file", e))?;
    let metadata = file
        .metadata()
        .map_err(|e| AppError::io("Inspect private runtime file", e))?;
    if !metadata.is_file()
        || metadata.len() > limit
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(AppError::new(
            "Private runtime must be a bounded administrator-controlled regular file.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::io("Read private runtime file", e))?;
    if bytes.len() as u64 > limit {
        return Err(AppError::new(
            "Private runtime file exceeds its inventory limit.",
        ));
    }
    Ok(bytes)
}

/// Authorization agents come from administrator-controlled system directories,
/// never an interactive user's PATH. Return the canonical trusted executable.
pub fn trusted_system_executable(name: &str) -> AppResult<PathBuf> {
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(AppError::new("Invalid system authorization program name."));
    }
    for directory in [
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/pkg/bin",
        "/usr/pkg/sbin",
    ] {
        let path = Path::new(directory).join(name);
        if path.exists() {
            return trusted_privileged_executable(&path);
        }
    }
    Err(AppError::new(format!(
        "Required trusted system program not found: {name}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_staging_paths_cannot_be_elevated_even_through_aliases() {
        let directory =
            std::env::temp_dir().join(format!("lurker-elevation-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let program = directory.join("helper");
        fs::write(&program, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let alias = directory.join("alias");
        std::os::unix::fs::symlink(&program, &alias).unwrap();
        assert!(trusted_privileged_executable(&program).is_err());
        assert!(trusted_privileged_executable(&alias).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn installed_system_program_resolves_to_a_trusted_absolute_path() {
        let program = ["/usr/bin/true", "/bin/true"]
            .into_iter()
            .map(Path::new)
            .find(|p| p.is_file())
            .unwrap();
        let resolved = trusted_privileged_executable(program).unwrap();
        assert!(resolved.is_absolute());
        assert_eq!(resolved, fs::canonicalize(program).unwrap());
        assert_eq!(trusted_system_executable("true").unwrap(), resolved);
        assert!(trusted_system_executable("../sudo").is_err());
    }
}
