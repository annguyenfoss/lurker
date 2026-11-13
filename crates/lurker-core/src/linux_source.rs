use crate::{AppError, AppResult};
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

// The inode lock serializes startup across aliases and with the private
// userspace crypto driver, which retains the same lock through dismount.
pub(crate) fn lock_file_source(path: &Path) -> AppResult<File> {
    let source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| AppError::io("Cannot read the container to lock its identity", error))?;
    let metadata = source
        .metadata()
        .map_err(|error| AppError::io("Cannot inspect container identity", error))?;
    if !metadata.is_file() {
        return Err(AppError::new(
            "Container identity lock requires a regular file.",
        ));
    }
    // SAFETY: source owns this live file descriptor. Closing it releases flock.
    if unsafe { libc::flock(source.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(AppError::new("Container is already in use or another operation is starting. Unmount it before opening it again."));
    }
    verify_path_identity(&source, path)?;
    Ok(source)
}

pub(crate) fn verify_path_identity(source: &File, path: &Path) -> AppResult<()> {
    let original = source
        .metadata()
        .map_err(|error| AppError::io("Cannot inspect locked container", error))?;
    let current = fs::symlink_metadata(path)
        .map_err(|error| AppError::io("Container path changed during startup", error))?;
    if !current.is_file() || (current.dev(), current.ino()) != (original.dev(), original.ino()) {
        return Err(AppError::new("Container identity changed during startup."));
    }
    Ok(())
}

// Force authorizes replacing only the destination inode locked at startup.
// An initially absent destination always uses atomic no-clobber publication.
// The identity check protects cooperative Lurker operations; it is not an
// atomic compare-and-rename against malicious changes to the parent directory.
pub(crate) fn publish_locked_container(
    temporary: &Path,
    target: &Path,
    force: bool,
    existing: Option<&File>,
) -> AppResult<()> {
    if let Some(existing) = existing.filter(|_| force) {
        verify_path_identity(existing, target)?;
        fs::rename(temporary, target)
    } else {
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            temporary,
            rustix::fs::CWD,
            target,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)
    }
    .map_err(|error| AppError::io(format!("Failed to publish {}", target.display()), error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    fn publication_fixture(
        name: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("lurker-publication-{name}-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let target = root.join("target");
        let temporary = root.join("incoming");
        fs::write(&temporary, b"new encrypted container").unwrap();
        (root, target, temporary)
    }

    #[test]
    fn forced_create_cannot_replace_a_destination_that_appeared_after_startup() {
        let (root, target, temporary) = publication_fixture("appeared");
        // Startup observed no destination. Another creator publishes and mounts
        // it while this operation is still preparing its incoming container.
        fs::write(&target, b"another published container").unwrap();
        let mounted = lock_file_source(&target).unwrap();
        assert!(publish_locked_container(&temporary, &target, true, None).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"another published container");
        assert_eq!(fs::read(&temporary).unwrap(), b"new encrypted container");
        drop(mounted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn forced_create_preserves_a_replacement_for_the_originally_locked_path() {
        let (root, target, temporary) = publication_fixture("replaced");
        fs::write(&target, b"original").unwrap();
        let original = lock_file_source(&target).unwrap();
        fs::rename(&target, root.join("retained-original")).unwrap();
        fs::write(&target, b"external replacement").unwrap();
        assert!(publish_locked_container(&temporary, &target, true, Some(&original)).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"external replacement");
        assert!(temporary.exists());
        drop(original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn forced_create_replaces_only_the_unchanged_locked_destination() {
        let (root, target, temporary) = publication_fixture("unchanged");
        fs::write(&target, b"original").unwrap();
        let original = lock_file_source(&target).unwrap();
        assert!(
            lock_file_source(&target).is_err(),
            "rival startup must be excluded"
        );
        publish_locked_container(&temporary, &target, true, Some(&original)).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new encrypted container");
        assert!(!temporary.exists());
        drop(original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_symlink_to_the_locked_inode_cannot_substitute_for_the_destination() {
        let (root, target, temporary) = publication_fixture("symlink");
        fs::write(&target, b"original").unwrap();
        let original = lock_file_source(&target).unwrap();
        let renamed = root.join("retained-original");
        fs::rename(&target, &renamed).unwrap();
        std::os::unix::fs::symlink(&renamed, &target).unwrap();
        assert!(publish_locked_container(&temporary, &target, true, Some(&original)).is_err());
        assert!(target.is_symlink());
        assert_eq!(fs::read(&renamed).unwrap(), b"original");
        drop(original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nonregular_sources_are_rejected_without_blocking_on_a_fifo() {
        let root = std::env::temp_dir().join(format!("lurker-fifo-source-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("pipe");
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a NUL-terminated path inside this test's directory.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(lock_file_source(&path).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacing_a_locked_path_is_detected_before_use() {
        let root =
            std::env::temp_dir().join(format!("lurker-replaced-source-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("source");
        fs::write(&path, b"original").unwrap();
        let file = lock_file_source(&path).unwrap();
        fs::rename(&path, root.join("old-source")).unwrap();
        fs::write(&path, b"replacement").unwrap();
        assert!(verify_path_identity(&file, &path).is_err());
        drop(file);
        fs::remove_dir_all(root).unwrap();
    }
}
