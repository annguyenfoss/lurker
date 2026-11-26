use super::{FileOperation, FileRequest};
use crate::{AppError, AppResult};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{self, Path};
use std::process::{Command, Output, Stdio};

fn command() -> AppResult<Command> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let elevated = unsafe { libc::geteuid() } == 0;
    let directories = std::env::split_paths(&path).chain(
        [
            "/opt/homebrew/bin",
            "/usr/pkg/bin",
            "/usr/local/bin",
            "/usr/bin",
        ]
        .into_iter()
        .map(std::path::PathBuf::from),
    );
    let executable = directories
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| [dir.join("gpg"), dir.join("gpg2")])
        .find_map(|candidate| {
            if elevated {
                return crate::trusted_privileged_executable(&candidate).ok();
            }
            fs::metadata(&candidate)
                .is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
                .then_some(candidate)
        })
        .ok_or_else(|| {
            AppError::new("GnuPG is unavailable. Install gpg and make it available on PATH.")
        })?;
    let mut command = Command::new(executable);
    // Explicit options keep recipient selection independent of gpg.conf while
    // retaining the user's normal home, keybox and agent configuration.
    command.args([
        "--options",
        "/dev/null",
        "--batch",
        "--no-auto-key-retrieve",
        "--auto-key-locate",
        "clear",
        "--no-symkey-cache",
    ]);
    Ok(command)
}

fn fingerprint(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn add_recipient(command: &mut Command, recipient: &OsStr) -> AppResult<()> {
    if recipient.to_str().is_some_and(fingerprint) {
        command.arg("--recipient").arg(recipient);
        return Ok(());
    }
    let file = path::absolute(recipient)
        .map_err(|error| AppError::io("Resolve OpenPGP recipient file", error))?;
    if !fs::metadata(&file)
        .map_err(|error| AppError::io("Inspect OpenPGP recipient file", error))?
        .is_file()
    {
        return Err(AppError::new("OpenPGP recipient must be a regular file."));
    }
    command.arg("--recipient-file").arg(file);
    Ok(())
}

fn key_file_passphrase(path: &Path) -> AppResult<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| AppError::io("Open GnuPG passphrase file", error))?;
    if !file
        .metadata()
        .map_err(|error| AppError::io("Inspect GnuPG passphrase file", error))?
        .is_file()
    {
        return Err(AppError::new(
            "GnuPG passphrase file must be a regular file.",
        ));
    }
    let mut line = Vec::new();
    BufReader::new(file)
        .read_until(b'\n', &mut line)
        .map_err(|error| AppError::io("Read GnuPG passphrase file", error))?;
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.is_empty() || line.contains(&0) {
        return Err(AppError::new(
            "The first line of the GnuPG passphrase file must contain a nonempty passphrase without NUL characters.",
        ));
    }
    Ok(line)
}

pub(super) fn run(request: &FileRequest, staged: &Path) -> AppResult<()> {
    let passphrase_from_file = request
        .key_file
        .as_deref()
        .map(key_file_passphrase)
        .transpose()?;
    let passphrase = request
        .passphrase
        .as_deref()
        .map(str::as_bytes)
        .or(passphrase_from_file.as_deref());
    let decrypting = matches!(request.operation, FileOperation::Decrypt);
    let passphrase_home = if decrypting && passphrase.is_some() {
        // --no-keyring does not suppress lookups on keyboxd installations.
        // Explicit passphrase decryption must use only the chosen credential.
        Some(
            tempfile::Builder::new()
                .prefix("lurker-gpg-")
                .tempdir()
                .map_err(|error| AppError::io("Create private GnuPG passphrase session", error))?,
        )
    } else {
        None
    };
    let key_decryption = decrypting && passphrase.is_none();
    let mut output = execute(
        request,
        staged,
        passphrase,
        passphrase_home.as_ref().map(tempfile::TempDir::path),
        key_decryption,
    )?;
    if key_decryption && !output.status.success() && private_key_prompt_cancelled(&output.stdout) {
        // Let the regular agent unlock a protected private key if needed.
        // For mixed input, cancel the file-passphrase prompt to use the key.
        output = execute(request, staged, None, None, false)?;
    }

    let status = String::from_utf8_lossy(&output.stdout);
    let has_status =
        |code| statuses(&status).any(|entry| entry.split_ascii_whitespace().next() == Some(code));
    let expected = if decrypting {
        "DECRYPTION_OKAY"
    } else {
        "END_ENCRYPTION"
    };
    if output.status.success() && has_status(expected) {
        return Ok(());
    }
    let message = if has_status("INV_RECP") {
        "GnuPG rejected a recipient. Check that the selected key is usable and trusted."
    } else if passphrase.is_some() && has_status("DECRYPTION_FAILED") {
        "GnuPG could not decrypt the file with the supplied passphrase."
    } else if key_decryption && has_status("NEED_PASSPHRASE_SYM") && !has_status("DECRYPTION_KEY") {
        "GnuPG could not decrypt with a local private key. To use the file passphrase, select --passphrase or --key-file."
    } else if has_status("NO_SECKEY") && !has_status("DECRYPTION_KEY") {
        "GnuPG could not find a matching private key."
    } else if has_status("DECRYPTION_FAILED") {
        "GnuPG could not decrypt the file. Check the credential and encrypted input."
    } else {
        return Err(gpg_error("GnuPG file operation failed", &output.stderr));
    };
    Err(AppError::new(message))
}

fn execute(
    request: &FileRequest,
    staged: &Path,
    passphrase: Option<&[u8]>,
    passphrase_home: Option<&Path>,
    cancel_pinentry: bool,
) -> AppResult<Output> {
    let mut command = command()?;
    command
        .args(["--yes", "--status-fd", "1", "--output"])
        .arg(staged);
    if passphrase.is_some() {
        command.args(["--pinentry-mode", "loopback", "--passphrase-fd", "0"]);
    } else if cancel_pinentry {
        // GnuPG tries the symmetric session-key packet first. An explicit
        // cancellation allows available private keys to succeed without a
        // prompt, including when the child has no interactive terminal.
        command.args(["--pinentry-mode", "cancel"]);
    }
    if let Some(home) = passphrase_home {
        command.arg("--homedir").arg(home).arg("--no-autostart");
    }
    match request.operation {
        FileOperation::Encrypt => {
            command.args(["--compress-algo", "none", "--set-filename", ""]);
            if passphrase.is_some() {
                command.arg("--symmetric");
            }
            if !request.public_keys.is_empty() {
                command.arg("--encrypt");
                for recipient in &request.public_keys {
                    add_recipient(&mut command, recipient)?;
                }
            }
        }
        FileOperation::Decrypt => {
            command.arg("--decrypt");
        }
    }
    let input = path::absolute(&request.input)
        .map_err(|error| AppError::io("Resolve GnuPG input file", error))?;
    let mut child = command
        .arg("--")
        .arg(input)
        .stdin(if passphrase.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::io("Start GnuPG", error))?;
    std::thread::scope(|scope| {
        let writer = passphrase
            .zip(child.stdin.take())
            .map(|(passphrase, mut stdin)| {
                scope.spawn(move || {
                    stdin
                        .write_all(passphrase)
                        .and_then(|()| stdin.write_all(b"\n"))
                })
            });
        let output = child
            .wait_with_output()
            .map_err(|error| AppError::io("Wait for GnuPG", error))?;
        if let Some(writer) = writer {
            let sent = writer
                .join()
                .map_err(|_| AppError::new("Could not send the GnuPG passphrase."))?;
            if output.status.success() {
                sent.map_err(|error| AppError::io("Send passphrase to GnuPG", error))?;
            }
        }
        Ok::<_, AppError>(output)
    })
}

fn statuses(output: &str) -> impl Iterator<Item = &str> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix("[GNUPG:] "))
}

fn private_key_prompt_cancelled(output: &[u8]) -> bool {
    // libgpg-error stores the source above the 16-bit error code.
    const GPG_ERR_CANCELED: u32 = 99;
    statuses(&String::from_utf8_lossy(output)).any(|entry| {
        entry
            .strip_prefix("ERROR pkdecrypt_failed ")
            .and_then(|value| value.split_ascii_whitespace().next())
            .and_then(|code| code.parse::<u32>().ok())
            .is_some_and(|code| code & 0xffff == GPG_ERR_CANCELED)
    })
}

fn gpg_error(context: &str, stderr: &[u8]) -> AppError {
    let stderr = String::from_utf8_lossy(stderr);
    let message = stderr.lines().rev().find(|line| !line.is_empty());
    match message {
        Some(line) => AppError::new(format!(
            "{context}: {}",
            line.chars()
                .flat_map(char::escape_debug)
                .collect::<String>()
        )),
        None => AppError::new(context),
    }
}

pub(super) fn list_keys() -> AppResult<Vec<String>> {
    let output = command()?
        .args(["--no-auto-check-trustdb", "--with-colons", "--list-keys"])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| AppError::io("List GnuPG keys", error))?;
    if !output.status.success() {
        return Err(gpg_error("Could not list GnuPG keys", &output.stderr));
    }
    let records = String::from_utf8_lossy(&output.stdout);
    let mut keys = Vec::new();
    let mut awaiting_fingerprint = false;
    let mut primary = None;
    for line in records.lines() {
        let fields: Vec<_> = line.split(':').collect();
        match fields.first().copied() {
            Some("pub") => {
                keys.extend(primary.take());
                let validity = fields.get(1).copied().unwrap_or_default();
                let capabilities = fields.get(11).copied().unwrap_or_default();
                awaiting_fingerprint = !matches!(validity, "r" | "e" | "d" | "i")
                    && capabilities.contains('E')
                    && !capabilities.contains('D');
            }
            Some("sub") => {
                keys.extend(primary.take());
                awaiting_fingerprint = false;
            }
            Some("fpr") if awaiting_fingerprint => {
                primary = fields
                    .get(9)
                    .copied()
                    .filter(|fpr| fingerprint(fpr))
                    .map(str::to_owned);
                awaiting_fingerprint = false;
            }
            Some("uid") if primary.is_some() => {
                let validity = fields.get(1).copied().unwrap_or_default();
                if let Some(label) = fields
                    .get(9)
                    .filter(|label| !label.is_empty() && !matches!(validity, "r" | "e" | "d" | "i"))
                {
                    if let Some(fingerprint) = primary.take() {
                        keys.push(format!("{fingerprint}\t{label}"));
                    }
                }
            }
            _ => {}
        }
    }
    keys.extend(primary);
    Ok(keys)
}
