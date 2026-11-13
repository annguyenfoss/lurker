//! One trust boundary for executables launched through administrator elevation.
use crate::{AppError, AppResult};
use std::fs;
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
    for ancestor in canonical.ancestors() {
        let metadata = fs::metadata(ancestor)
            .map_err(|e| AppError::io("Cannot inspect privileged path ownership", e))?;
        if metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
            return Err(AppError::new(format!(
                "Refusing elevated execution through an untrusted path: {}. Install an administrator-owned Lurker package first.",
                ancestor.display()
            )));
        }
    }
    Ok(canonical)
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
    }
}
