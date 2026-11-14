//! Private login-user monitor: persist the backend's real exit result after
//! the mounting frontend exits. Credentials stay on pipes and are redacted
//! before bounded diagnostics reach the session directory.
use super::*;
use std::os::unix::process::ExitStatusExt;

const OUTPUT_LIMIT: usize = 131072;
const DIAGNOSTIC_LIMIT: usize = 8192;

#[derive(Serialize, Deserialize)]
pub(super) struct BackendResult {
    code: Option<i32>,
    signal: Option<i32>,
    closed_ack: bool,
    diagnostic: String,
}

pub(super) fn write_record<T: Serialize>(dir: &Path, name: &str, record: &T) -> AppResult<()> {
    let temporary = dir.join(format!(".{name}-{}", nonce()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(|e| AppError::io("Cannot create backend result", e))?;
    serde_json::to_writer(&mut file, record)
        .map_err(|_| AppError::new("Cannot encode backend result"))?;
    file.sync_all()
        .map_err(|e| AppError::io("Cannot sync backend result", e))?;
    fs::rename(&temporary, dir.join(name))
        .map_err(|e| AppError::io("Cannot publish backend result", e))?;
    sync_dir(dir)
}

pub(super) fn read_record<T: serde::de::DeserializeOwned>(
    dir: &Path,
    name: &str,
) -> AppResult<Option<T>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(dir.join(name))
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::io("Cannot read backend result", e)),
    };
    let m = file
        .metadata()
        .map_err(|e| AppError::io("Cannot inspect backend result", e))?;
    if !m.is_file()
        || m.uid() != crate::effective_uid()
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
        || m.len() > 65536
    {
        return Err(AppError::new(
            "Backend result must be a bounded private regular file.",
        ));
    }
    serde_json::from_reader(file.take(65537))
        .map(Some)
        .map_err(|_| AppError::new("Malformed backend result; preserve the session."))
}

fn collect(mut pipe: impl Read) -> Vec<u8> {
    let mut captured = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => return captured,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return captured,
            Ok(count) => captured.extend_from_slice(
                &chunk[..count.min(OUTPUT_LIMIT.saturating_sub(captured.len()))],
            ),
        }
    }
}

fn diagnostic(mut bytes: Vec<u8>, secret: &[u8]) -> String {
    // A bounded reader may end partway through an echoed password. Remove that
    // suffix as well as complete occurrences, before decoding or truncating.
    for length in (1..secret.len()).rev() {
        if bytes.ends_with(&secret[..length]) {
            bytes.truncate(bytes.len() - length);
            break;
        }
    }
    let mut clean = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if !secret.is_empty() && bytes[offset..].starts_with(secret) {
            clean.extend_from_slice(b"<redacted>");
            offset += secret.len();
        } else {
            clean.push(bytes[offset]);
            offset += 1;
        }
    }
    let text = String::from_utf8_lossy(&clean);
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(DIAGNOSTIC_LIMIT)
        .collect()
}

fn session_for_directory(root: &Path, canonical_dir: &Path) -> AppResult<Session> {
    // macOS exposes /tmp through /private/tmp. Compare paths from one canonical
    // root rather than rejecting a validated session solely for that OS alias.
    let root = fs::canonicalize(root)
        .map_err(|e| AppError::io("Cannot resolve folder runtime directory", e))?;
    session_snapshot(&root)?
        .into_iter()
        .find(|(path, _)| path == canonical_dir)
        .map(|(_, session)| session)
        .ok_or_else(|| AppError::new("Supervisor requires an existing private folder session."))
}

fn run(dir: PathBuf, executable: PathBuf, initializing: bool) -> AppResult<()> {
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "Folder supervisor must run as the login user.",
        ));
    }
    let dir =
        fs::canonicalize(dir).map_err(|e| AppError::io("Cannot resolve folder session", e))?;
    let session = session_for_directory(&runtime_path(), &dir)?;
    if find_program(session.backend)? != executable {
        return Err(AppError::new(
            "Supervisor backend differs from the selected private executable.",
        ));
    }
    let mut password = Vec::new();
    std::io::stdin()
        .take(65538)
        .read_to_end(&mut password)
        .map_err(|e| AppError::io("Cannot read supervisor credential pipe", e))?;
    if password.last() != Some(&b'\n') || password.len() > 65537 {
        return Err(AppError::new("Invalid supervisor credential transport."));
    }
    password.pop();
    let secret = std::str::from_utf8(&password)
        .map_err(|_| AppError::new("Invalid credential encoding."))?;
    crate::credentials::validate(Some(secret), None)?;
    let mut args = mount_args(
        session.backend,
        &session.source,
        &session.mountpoint,
        session.readonly,
    )?;
    if initializing && session.backend == VolumeType::Cryfs {
        args.splice(
            0..0,
            ["--cipher", "aes-256-gcm", "--blocksize", "16384"].map(OsString::from),
        );
    }
    let mut command = backend_command(session.backend, &executable);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| AppError::io("Cannot start monitored backend", e))?;
    let process = process_identity(child.id())
        .ok_or_else(|| AppError::new("Cannot confirm monitored backend identity."))?;
    write_record(&dir, "backend-process.json", &process)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::new("Missing backend output pipe."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::new("Missing backend diagnostic pipe."))?;
    let out = std::thread::spawn(move || collect(stdout));
    let err = std::thread::spawn(move || collect(stderr));
    let send_result = send(&mut child, secret);
    let status = child
        .wait()
        .map_err(|e| AppError::io("Cannot wait for backend exit", e))?;
    let stdout = out
        .join()
        .map_err(|_| AppError::new("Backend output monitor failed."))?;
    let stderr = err
        .join()
        .map_err(|_| AppError::new("Backend diagnostic monitor failed."))?;
    let closed_ack = stdout.split(|byte| *byte == b'\n').any(|line| {
        serde_json::from_slice::<serde_json::Value>(line)
            .is_ok_and(|value| value == serde_json::json!({"protocol": 1, "status": "closed"}))
    });
    // Redact each stream before joining: either bounded capture may end
    // halfway through a password, independently of the other stream.
    let combined = format!(
        "{}\n{}",
        diagnostic(stdout, &password),
        diagnostic(stderr, &password)
    );
    let result = BackendResult {
        code: status.code(),
        signal: status.signal(),
        closed_ack,
        diagnostic: combined.chars().take(DIAGNOSTIC_LIMIT).collect(),
    };
    password.fill(0);
    write_record(&dir, "backend-result.json", &result)?;
    send_result
}

pub(super) fn check_close(dir: &Path, backend: VolumeType) -> AppResult<()> {
    let result = read_record::<BackendResult>(dir, "backend-result.json")?.ok_or_else(|| {
        AppError::new(
            "Backend exit result is unavailable; session retained. No clean close was confirmed.",
        )
    })?;
    let accepted = if backend == VolumeType::Cryptomator {
        result.closed_ack && result.signal.is_none() && matches!(result.code, Some(0 | 143))
    } else {
        result.code == Some(0) && result.signal.is_none()
    };
    if accepted {
        return Ok(());
    }
    Err(AppError::new(format!(
        "Backend did not confirm a clean close (exit {:?}, signal {:?}, close acknowledgment {}). Inspect the retained backend-result.json for redacted diagnostics.",
        result.code, result.signal, result.closed_ack
    )))
}

pub(super) fn dispatch() -> Option<i32> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|v| v.to_str()) != Some("--lurker-folder-supervisor") {
        return None;
    }
    if args.len() != 5 {
        return Some(64);
    }
    let initializing = match args[4].to_str() {
        Some("init") => true,
        Some("mount") => false,
        _ => return Some(64),
    };
    Some(
        match run(args[2].clone().into(), args[3].clone().into(), initializing) {
            Ok(()) => 0,
            // The caller retains the session if no durable backend result exists.
            Err(_) => 1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supervisor_resolves_a_symlinked_runtime_root_without_recreating_state() {
        let base = std::env::temp_dir().join(format!("lurker-supervisor-alias-{}", nonce()));
        secure_dir(&base).unwrap();
        let real = base.join("real-runtime");
        secure_dir(&real).unwrap();
        let dir = real.join("session-test");
        secure_dir(&dir).unwrap();
        let alias = base.join("runtime-alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let session = Session {
            schema: SCHEMA,
            backend: VolumeType::Gocryptfs,
            source: base.join("vault"),
            source_identity: Identity { device: 1, inode: 2 },
            mountpoint: base.join("mount"),
            readonly: false,
            mount_identity: None,
            mounted_directory: None,
            process: None,
            supervisor: None,
        };
        write_record(&dir, "session.json", &session).unwrap();
        let canonical = fs::canonicalize(&dir).unwrap();
        let found = session_for_directory(&alias, &canonical).unwrap();
        assert_eq!(found.source, session.source);
        fs::remove_dir_all(&dir).unwrap();
        assert!(session_for_directory(&alias, &canonical).is_err());
        assert!(!dir.exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn bounded_diagnostics_redact_complete_and_truncated_credentials() {
        assert_eq!(
            diagnostic(
                b"before secret-pass after secret-p".to_vec(),
                b"secret-pass"
            ),
            "before <redacted> after "
        );
        assert_eq!(
            diagnostic("密码 repeated 密码".as_bytes().to_vec(), "密码".as_bytes()),
            "<redacted> repeated <redacted>"
        );
        assert_eq!(
            diagnostic(vec![b'x'; OUTPUT_LIMIT], b"private").len(),
            DIAGNOSTIC_LIMIT
        );
        assert!(!diagnostic(b"\x1b[31merror\0".to_vec(), b"secret").contains('\x1b'));
    }
    #[test]
    fn missing_or_failed_exit_never_becomes_clean_close() {
        let dir = std::env::temp_dir().join(format!("lurker-folder-supervisor-test-{}", nonce()));
        secure_dir(&dir).unwrap();
        assert!(check_close(&dir, VolumeType::Cryfs).is_err());
        write_record(
            &dir,
            "backend-result.json",
            &BackendResult {
                code: None,
                signal: Some(6),
                closed_ack: false,
                diagnostic: "storage failure".into(),
            },
        )
        .unwrap();
        assert!(check_close(&dir, VolumeType::Cryfs).is_err());
        write_record(
            &dir,
            "backend-result.json",
            &BackendResult {
                code: Some(143),
                signal: None,
                closed_ack: false,
                diagnostic: String::new(),
            },
        )
        .unwrap();
        assert!(check_close(&dir, VolumeType::Cryptomator).is_err());
        write_record(
            &dir,
            "backend-result.json",
            &BackendResult {
                code: Some(143),
                signal: None,
                closed_ack: true,
                diagnostic: String::new(),
            },
        )
        .unwrap();
        check_close(&dir, VolumeType::Cryptomator).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
