use crate::{AppError, AppResult, CommandAction, VolumeType};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

pub(crate) fn validate(passphrase: Option<&str>, keyfile: Option<&Path>) -> AppResult<()> {
    if passphrase.is_some() && keyfile.is_some() {
        return Err(AppError::new(
            "Choose either a password or a keyfile, never both together.",
        ));
    }
    if let Some(passphrase) = passphrase {
        if passphrase.is_empty() || passphrase.contains(['\0', '\n', '\r']) {
            return Err(AppError::new(
                "Passphrase must be non-empty and contain no NUL or newline characters.",
            ));
        }
    }
    if let Some(path) = keyfile {
        // O_NONBLOCK prevents a mistakenly selected FIFO from hanging validation.
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| AppError::io("Failed to read keyfile", e))?;
        let metadata = file
            .metadata()
            .map_err(|e| AppError::io("Failed to inspect keyfile", e))?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(AppError::new("Keyfile must be a non-empty regular file."));
        }
    }
    Ok(())
}

pub(crate) fn validate_request(command: &CommandAction) -> AppResult<()> {
    let (passphrase, keyfile, kind, target) = match command {
        CommandAction::Create(c) => (
            c.passphrase.as_deref(),
            c.keyfile.as_deref(),
            c.volume_type,
            Some(c.target.as_path()),
        ),
        CommandAction::Mount(c) => (
            c.passphrase.as_deref(),
            c.keyfile.as_deref(),
            c.volume_type,
            None,
        ),
        CommandAction::Unmount(_) => return Ok(()),
    };
    validate(passphrase, keyfile)?;
    if keyfile.is_some() && kind == VolumeType::Apple {
        return Err(AppError::new(
            "Keyfile authentication is supported for LUKS and VeraCrypt only.",
        ));
    }
    if let (Some(keyfile), Some(target)) = (keyfile, target) {
        ensure_distinct_target(target, keyfile)?;
    }
    Ok(())
}

pub(crate) fn ensure_distinct_target(target: &Path, keyfile: &Path) -> AppResult<()> {
    let key = fs::metadata(keyfile).map_err(|e| AppError::io("Failed to inspect keyfile", e))?;
    if fs::metadata(target)
        .ok()
        .is_some_and(|m| m.dev() == key.dev() && m.ino() == key.ino())
    {
        return Err(AppError::new(
            "Container target must not be the keyfile or a hard link to it.",
        ));
    }
    Ok(())
}

/// VeraCrypt treats commas as separators and double commas as a literal comma.
pub(crate) fn veracrypt_keyfiles(keyfile: Option<&Path>) -> AppResult<std::ffi::OsString> {
    match keyfile {
        None => Ok("".into()),
        Some(path) => {
            let path =
                fs::canonicalize(path).map_err(|e| AppError::io("Failed to resolve keyfile", e))?;
            let value = path
                .to_str()
                .ok_or_else(|| AppError::new("VeraCrypt keyfile path must be valid UTF-8."))?;
            Ok(value.replace(',', ",,").into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_combined_and_invalid_credentials() {
        assert!(validate(Some("password"), Some(Path::new("missing")))
            .unwrap_err()
            .message
            .contains("never both"));
        for value in ["", "a\0b", "a\nb", "a\rb"] {
            assert!(validate(Some(value), None).is_err());
        }
        assert!(validate(Some("  密碼  "), None).is_ok());
        assert!(validate(None, Some(Path::new("/dev/null"))).is_err());
        assert!(validate(None, Some(Path::new("/missing/keyfile"))).is_err());
        assert!(validate(None, None).is_ok()); // interactive password entry
    }
    #[test]
    fn keyfiles_are_binary_and_cannot_be_overwritten_by_create() {
        let root = std::env::temp_dir().join(format!("lurker-credentials-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let key = root.join("key, with spaces.bin");
        fs::write(&key, [0, 255, 10, 13, 128]).unwrap();
        assert!(validate(None, Some(&key)).is_ok());
        assert!(veracrypt_keyfiles(Some(&key))
            .unwrap()
            .to_string_lossy()
            .contains("key,, with spaces.bin"));
        assert!(ensure_distinct_target(&key, &key).is_err());
        let alias = root.join("hardlink");
        fs::hard_link(&key, &alias).unwrap();
        assert!(ensure_distinct_target(&alias, &key).is_err());
        assert!(ensure_distinct_target(&root.join("new-container"), &key).is_ok());
        let empty = root.join("empty");
        fs::write(&empty, []).unwrap();
        assert!(validate(None, Some(&empty)).is_err());
        assert!(validate(None, Some(&root)).is_err());
        let fifo = root.join("fifo");
        use std::os::unix::ffi::OsStrExt;
        let fifo_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);
        assert!(validate(None, Some(&fifo)).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
