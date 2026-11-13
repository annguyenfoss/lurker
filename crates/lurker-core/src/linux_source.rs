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
    let current = fs::metadata(path)
        .map_err(|error| AppError::io("Container path changed during startup", error))?;
    if (current.dev(), current.ino()) != (original.dev(), original.ino()) {
        return Err(AppError::new("Container identity changed during startup."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

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
