use crate::{AppError, AppResult, CommandAction, VolumeType};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

// Configure the terminal before showing the prompt. rpassword's convenience
// prompt prints first, which can echo a fast paste before its raw mode starts.
struct PasswordTerminal {
    file: File,
    original: libc::termios,
}

impl Read for PasswordTerminal {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.file.read(bytes)?;
        if bytes[..count].contains(&3) {
            // Return through the terminal guard instead of raising SIGINT while
            // echo is disabled. rpassword still handles UTF-8 and line editing.
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Passphrase entry cancelled",
            ));
        }
        Ok(count)
    }
}

impl Drop for PasswordTerminal {
    fn drop(&mut self) {
        // SAFETY: the descriptor and saved terminal attributes are still live.
        unsafe {
            libc::tcsetattr(self.file.as_raw_fd(), libc::TCSANOW, &self.original);
        }
    }
}

pub(crate) fn prompt_passphrase(prompt: &str) -> std::io::Result<String> {
    let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr initializes the supplied termios on success.
    if unsafe { libc::tcgetattr(file.as_raw_fd(), original.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let original = unsafe { original.assume_init() };
    let mut input = PasswordTerminal { file, original };
    let mut hidden = original;
    hidden.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG);
    hidden.c_cc[libc::VMIN] = 1;
    hidden.c_cc[libc::VTIME] = 0;
    // SAFETY: this descriptor is our controlling terminal, with valid attributes.
    if unsafe { libc::tcsetattr(input.file.as_raw_fd(), libc::TCSANOW, &hidden) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut output = input.file.try_clone()?;
    input.file.write_all(prompt.as_bytes())?;
    input.file.flush()?;
    let config = rpassword::ConfigBuilder::new()
        .input_reader(input)
        .output_discard()
        .build();
    let result = rpassword::read_password_with_config(config);
    let _ = output.write_all(b"\n");
    result
}

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
        CommandAction::Keyslots(c) => {
            use crate::KeyslotOperation;
            if !cfg!(target_os = "linux") {
                return Err(AppError::new("LUKS keyslot management requires Linux."));
            }
            if c.privilege_mode == crate::PrivilegeMode::Unprivileged {
                return Err(AppError::new(
                    "LUKS keyslot management requires Privileged mode.",
                ));
            }
            if c.slot.is_some_and(|slot| slot > 31) {
                return Err(AppError::new("Keyslot must be between 0 and 31."));
            }
            validate(c.passphrase.as_deref(), c.keyfile.as_deref())?;
            validate(c.new_passphrase.as_deref(), c.new_keyfile.as_deref())?;
            if c.operation != KeyslotOperation::Add
                && (c.new_passphrase.is_some() || c.new_keyfile.is_some())
            {
                return Err(AppError::new(
                    "New credentials are valid only when adding a keyslot.",
                ));
            }
            if c.operation == KeyslotOperation::List
                && (c.slot.is_some() || c.passphrase.is_some() || c.keyfile.is_some())
            {
                return Err(AppError::new(
                    "Listing keyslots does not take a slot or credentials.",
                ));
            }
            if c.operation == KeyslotOperation::Remove && c.slot.is_none() {
                return Err(AppError::new("Choose the keyslot to remove."));
            }
            for keyfile in [c.keyfile.as_deref(), c.new_keyfile.as_deref()]
                .into_iter()
                .flatten()
            {
                ensure_distinct_target(&c.source, keyfile)?;
            }
            return Ok(());
        }
    };
    validate(passphrase, keyfile)?;
    if keyfile.is_some()
        && !matches!(
            kind,
            VolumeType::Auto
                | VolumeType::Luks
                | VolumeType::Luks1
                | VolumeType::Veracrypt
                | VolumeType::Geli
                | VolumeType::Pefs
        )
    {
        return Err(AppError::new(
            "Keyfile authentication is supported for LUKS, VeraCrypt, GELI and PEFS only.",
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
