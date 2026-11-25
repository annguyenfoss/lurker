//! Optional login-user ext4/XFS drivers over VeraCrypt's regular decrypted image.
//! Native Apple filesystems retain hdiutil; no external tool is ever elevated.
use super::*;
use crate::CreateFilesystem;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Kind {
    Ext4,
    Xfs,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Ext4 => "ext4",
            Self::Xfs => "xfs",
        }
    }
    fn driver(self) -> &'static str {
        match self {
            Self::Ext4 => "fuse-ext2",
            Self::Xfs => "fuse-xfs",
        }
    }
    fn formatter(self) -> &'static str {
        match self {
            Self::Ext4 => "mkfs.ext4",
            Self::Xfs => "mkfs.xfs",
        }
    }
}
fn requested(filesystem: Option<CreateFilesystem>) -> Option<Kind> {
    match filesystem {
        Some(CreateFilesystem::Ext4) => Some(Kind::Ext4),
        Some(CreateFilesystem::Xfs) => Some(Kind::Xfs),
        _ => None,
    }
}
fn tool(name: &str) -> AppResult<PathBuf> {
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "External macOS filesystem tools must run as the login user, never as root.",
        ));
    }
    let executable_path =
        std::env::current_exe().map_err(|e| AppError::io("Cannot locate filesystem tools", e))?;
    let directory = executable_path
        .parent()
        .ok_or_else(|| AppError::new("Missing executable directory."))?;
    [directory.join("macos-filesystems").join(name), directory.join("../libexec/lurker/filesystems").join(name), directory.join("../Helpers/filesystems").join(name)]
        .into_iter().find(|path| executable(path))
        .ok_or_else(|| AppError::new(format!("Private macOS filesystem tool {name} is missing. Install the qualified FUSE-T filesystem tools; no driver is run with sudo.")))
}

pub(super) fn create_args(c: &CreateCommand, size: u64) -> AppResult<Option<Vec<OsString>>> {
    let Some(kind) = requested(c.filesystem).filter(|_| c.volume_type == VolumeType::Veracrypt)
    else {
        return Ok(None);
    };
    tool(kind.driver())?;
    tool(kind.formatter())?;
    if size < 384 {
        return Err(AppError::new(
            "External macOS filesystem containers require at least 384 MiB.",
        ));
    }
    let mut arguments =
        crate::workflow::macos_veracrypt_create_args(c.cipher, size, c.keyfile.as_deref(), None)?;
    let filesystem = arguments
        .iter_mut()
        .find(|argument| argument.to_string_lossy().starts_with("--filesystem="))
        .ok_or_else(|| AppError::new("Missing VeraCrypt filesystem argument."))?;
    *filesystem = "--filesystem=none".into();
    Ok(Some(arguments))
}
fn detect(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(b"XFSB") {
        Some(Kind::Xfs)
    } else if bytes.get(1080..1082) == Some(&[0x53, 0xef])
        && bytes
            .get(1120..1124)
            .is_some_and(|flags| u32::from_le_bytes(flags.try_into().unwrap()) & 0x40 != 0)
    {
        Some(Kind::Ext4)
    } else {
        None
    }
}
fn image_kind(image: &Path) -> Option<Kind> {
    let mut bytes = [0; 4096];
    File::open(image).ok()?.read_exact(&mut bytes).ok()?;
    detect(&bytes)
}
fn close_crypto(source: &Path) -> AppResult<()> {
    checked(
        &require_veracrypt()?,
        &[
            "-t".into(),
            "--non-interactive".into(),
            "--dismount".into(),
            source.into(),
        ],
        None,
    )?;
    Ok(())
}
fn open_image(
    source: &Path,
    pass: &[u8],
    readonly: bool,
    keyfile: Option<&Path>,
) -> AppResult<PathBuf> {
    let mut arguments = args(&[
        "-t",
        "--mount",
        "--filesystem=none",
        "--stdin",
        "--non-interactive",
        "--pim=0",
        "-k",
    ]);
    arguments.push(crate::credentials::veracrypt_keyfiles(keyfile)?);
    arguments.push("--protect-hidden=no".into());
    if readonly {
        arguments.push("--mount-options=ro".into());
    }
    arguments.push(source.into());
    checked(&require_veracrypt()?, &arguments, Some(pass))?;
    let result = (|| {
        let volume = read_veracrypt_volumes(false)?
            .into_iter()
            .find(|volume| same_path(&volume.source, source))
            .ok_or_else(|| AppError::new("VeraCrypt did not publish the authenticated volume."))?;
        let device = volume.devices.first().ok_or_else(|| {
            AppError::new("VeraCrypt has no identified auxiliary virtual device.")
        })?;
        let image = images()?
            .into_iter()
            .find(|image| image.entities.iter().any(|entity| &entity.device == device))
            .ok_or_else(|| AppError::new("Cannot identify VeraCrypt's decrypted image."))?;
        let path = existing(&image.path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| AppError::io("Cannot inspect decrypted image", e))?;
        if !metadata.is_file()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o077 != 0
            || path.file_name() != Some(std::ffi::OsStr::new("volume.dmg"))
        {
            return Err(AppError::new(
                "VeraCrypt decrypted image must be a private regular file owned by the login user.",
            ));
        }
        // The ordinary native virtual device exclusively locks this regular
        // image. No native filesystem is mounted; detach only that identified
        // device and retain VeraCrypt's live encryption/FUSE service.
        checked(
            Path::new(HDIUTIL),
            &["detach".into(), device.as_os_str().into()],
            None,
        )?;
        Ok(path)
    })();
    if result.is_err() {
        close_crypto(source)?;
    }
    result
}
pub(super) fn format(
    source: &Path,
    filesystem: Option<CreateFilesystem>,
    pass: &[u8],
    keyfile: Option<&Path>,
) -> AppResult<()> {
    let Some(kind) = requested(filesystem) else {
        return Ok(());
    };
    let image = open_image(source, pass, false, keyfile)?;
    let arguments = match kind {
        Kind::Ext4 => vec![
            "-F".into(),
            "-E".into(),
            format!("root_owner={}:{}", crate::effective_uid(), unsafe {
                libc::getegid()
            })
            .into(),
            image.as_os_str().into(),
        ],
        Kind::Xfs => vec!["-f".into(), image.as_os_str().into()],
    };
    let result = checked(&tool(kind.formatter())?, &arguments, None).map(|_| ());
    close_crypto(source)?;
    result
}
#[derive(Serialize, Deserialize)]
struct Session {
    source: PathBuf,
    source_dev: u64,
    source_ino: u64,
    image: PathBuf,
    image_dev: u64,
    image_ino: u64,
    auxiliary_mount: String,
    mountpoint: PathBuf,
    kind: Kind,
    readonly: bool,
    pid: u32,
    process_start: String,
    mount_identity: Option<String>,
    mount_dev: Option<u64>,
}
struct Registry {
    path: PathBuf,
    _lock: File,
}
fn registry_path() -> PathBuf {
    Path::new("/tmp").join(format!(
        "lurker-macos-filesystems-{}",
        crate::effective_uid()
    ))
}
fn secure_dir(path: &Path) -> AppResult<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => {
            return Err(AppError::io(
                "Cannot create filesystem session directory",
                error,
            ));
        }
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Cannot inspect filesystem sessions", e))?;
    if !metadata.is_dir()
        || metadata.uid() != crate::effective_uid()
        || metadata.mode() & 0o077 != 0
    {
        return Err(AppError::new(
            "Filesystem session directory must be private and owned by the login user.",
        ));
    }
    Ok(())
}
impl Registry {
    fn lock() -> AppResult<Self> {
        let path = registry_path();
        secure_dir(&path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.join("lock"))
            .map_err(|e| AppError::io("Cannot open filesystem session lock", e))?;
        let metadata = lock
            .metadata()
            .map_err(|e| AppError::io("Cannot inspect filesystem lock", e))?;
        if !metadata.is_file()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o077 != 0
        {
            return Err(AppError::new("Unsafe filesystem session lock."));
        }
        // SAFETY: lock is an open, owned descriptor retained for this scope.
        if unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock), libc::LOCK_EX) } != 0 {
            return Err(AppError::io(
                "Cannot lock filesystem sessions",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(Self { path, _lock: lock })
    }
    fn sessions(&self) -> AppResult<Vec<(PathBuf, Session)>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.path)
            .map_err(|e| AppError::io("Cannot read filesystem sessions", e))?
        {
            let path = entry
                .map_err(|e| AppError::io("Cannot read filesystem session", e))?
                .path();
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("session-"))
            {
                continue;
            }
            secure_dir(&path)?;
            let record = path.join("session.json");
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&record)
                .map_err(|e| AppError::io("Cannot open filesystem session", e))?;
            let metadata = file
                .metadata()
                .map_err(|e| AppError::io("Cannot inspect filesystem session", e))?;
            if !metadata.is_file()
                || metadata.uid() != crate::effective_uid()
                || metadata.mode() & 0o077 != 0
                || metadata.len() > 65536
            {
                return Err(AppError::new("Unsafe filesystem session record."));
            }
            let session: Session = serde_json::from_reader(file)
                .map_err(|e| AppError::new(format!("Invalid filesystem session: {e}")))?;
            result.push((path, session));
        }
        Ok(result)
    }
}
fn save(path: &Path, session: &Session) -> AppResult<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path.join("session.tmp"))
        .map_err(|e| AppError::io("Cannot stage filesystem session", e))?;
    serde_json::to_writer(&mut file, session)
        .map_err(|e| AppError::new(format!("Cannot encode filesystem session: {e}")))?;
    file.sync_all()
        .map_err(|e| AppError::io("Cannot sync filesystem session", e))?;
    fs::rename(path.join("session.tmp"), path.join("session.json"))
        .map_err(|e| AppError::io("Cannot publish filesystem session", e))
}
fn process_start(pid: u32) -> AppResult<Option<String>> {
    if pid == 0 {
        return Ok(None);
    }
    let result = execute(
        Path::new("/bin/ps"),
        &args(&["-p", &pid.to_string(), "-o", "lstart=", "-o", "stat="]),
        None,
    )?;
    let text = String::from_utf8_lossy(&result.stdout);
    let fields: Vec<_> = text.split_whitespace().collect();
    Ok(
        (result.status.success() && fields.len() == 6 && !fields[5].starts_with('Z'))
            .then(|| fields[..5].join(" ")),
    )
}
fn mount_identity(path: &Path) -> AppResult<Option<String>> {
    let result = checked(Path::new("/sbin/mount"), &[], None)?;
    let needle = format!(" on {} (", path.display());
    Ok(String::from_utf8_lossy(&result.stdout)
        .lines()
        .find(|line| line.contains(&needle))
        .map(str::to_owned))
}
fn validate_image(session: &Session) -> AppResult<()> {
    let metadata = fs::metadata(&session.source)
        .map_err(|e| AppError::io("Cannot inspect encrypted container", e))?;
    if metadata.dev() != session.source_dev || metadata.ino() != session.source_ino {
        return Err(AppError::new(
            "Container identity changed; preserving filesystem session.",
        ));
    }
    let image = fs::symlink_metadata(&session.image)
        .map_err(|e| AppError::io("Cannot inspect decrypted image", e))?;
    if !image.is_file()
        || image.dev() != session.image_dev
        || image.ino() != session.image_ino
        || mount_identity(
            session
                .image
                .parent()
                .ok_or_else(|| AppError::new("Missing auxiliary directory."))?,
        )?
        .as_ref()
            != Some(&session.auxiliary_mount)
        || !read_veracrypt_volumes(false)?
            .iter()
            .any(|volume| same_path(&volume.source, &session.source))
    {
        return Err(AppError::new(
            "VeraCrypt auxiliary image identity changed; preserving session.",
        ));
    }
    Ok(())
}
fn close_session(path: &Path, session: &mut Session) -> AppResult<()> {
    validate_image(session)?;
    if let Some(identity) = mount_identity(&session.mountpoint)? {
        if session.mount_identity.as_ref() != Some(&identity)
            || session.mount_dev
                != Some(
                    fs::metadata(&session.mountpoint)
                        .map_err(|e| AppError::io("Cannot inspect filesystem mount", e))?
                        .dev(),
                )
        {
            return Err(AppError::new(
                "Filesystem mount identity changed; preserving encrypted container.",
            ));
        }
        checked(
            Path::new(DISKUTIL),
            &["unmount".into(), session.mountpoint.as_os_str().into()],
            None,
        )?;
        if mount_identity(&session.mountpoint)?.is_some() {
            return Err(AppError::new(
                "Filesystem remains mounted; preserving encrypted container.",
            ));
        }
    }
    session.mount_identity = None;
    session.mount_dev = None;
    save(path, session)?;
    if let Some(start) = process_start(session.pid)? {
        if start != session.process_start {
            return Err(AppError::new(
                "Filesystem process identity changed; preserving encrypted container.",
            ));
        }
        // The verified filesystem is already drained and unmounted. Stop only
        // this session's exact backend; this is orderly transport cleanup.
        // SAFETY: start identity was checked immediately before the signal.
        if unsafe { libc::kill(session.pid as libc::pid_t, libc::SIGTERM) } != 0 {
            return Err(AppError::io(
                "Cannot stop filesystem transport",
                std::io::Error::last_os_error(),
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while process_start(session.pid)?.as_deref() == Some(&session.process_start)
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(100));
        }
        if process_start(session.pid)?.as_deref() == Some(&session.process_start) {
            return Err(AppError::new(
                "Filesystem process has not stopped; preserving encrypted device.",
            ));
        }
    }
    close_crypto(&session.source)?;
    fs::remove_dir_all(path).map_err(|e| AppError::io("Cannot remove closed filesystem session", e))
}
fn start(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    image: &Path,
    readonly: bool,
) -> AppResult<bool> {
    let Some(kind) = image_kind(image) else {
        return Ok(false);
    };
    let driver = tool(kind.driver())?;
    if mount_identity(mountpoint)?.is_some() {
        return Err(AppError::new(
            "Refusing to replace an existing filesystem mount.",
        ));
    }
    if registry
        .sessions()?
        .iter()
        .any(|(_, session)| same_path(&session.source, source) || session.mountpoint == mountpoint)
    {
        return Err(AppError::new(
            "An external filesystem session already owns this container or mountpoint.",
        ));
    }
    let metadata =
        fs::metadata(source).map_err(|e| AppError::io("Cannot inspect encrypted container", e))?;
    let path = registry
        .path
        .join(format!("session-{}-{}", metadata.dev(), metadata.ino()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|e| AppError::io("Cannot reserve filesystem session", e))?;
    let mut session = Session {
        source: source.into(),
        source_dev: metadata.dev(),
        source_ino: metadata.ino(),
        image: image.into(),
        image_dev: fs::metadata(image)
            .map_err(|e| AppError::io("Cannot inspect decrypted image", e))?
            .dev(),
        image_ino: fs::metadata(image)
            .map_err(|e| AppError::io("Cannot inspect decrypted image", e))?
            .ino(),
        auxiliary_mount: mount_identity(
            image
                .parent()
                .ok_or_else(|| AppError::new("Missing auxiliary mount directory."))?,
        )?
        .ok_or_else(|| AppError::new("Decrypted image has no auxiliary mount."))?,
        mountpoint: mountpoint.into(),
        kind,
        readonly,
        pid: 0,
        process_start: String::new(),
        mount_identity: None,
        mount_dev: None,
    };
    // Publish recovery ownership before starting a process that can mount.
    if let Err(error) = save(&path, &session) {
        let _ = fs::remove_dir_all(&path);
        return Err(error);
    }
    let mut a: Vec<OsString> = match kind {
        Kind::Ext4 => vec![
            image.as_os_str().into(),
            mountpoint.as_os_str().into(),
            "-o".into(),
            if readonly {
                "ro,debug,backend=nfs".into()
            } else {
                "rw+,debug,backend=nfs".into()
            },
        ],
        Kind::Xfs => {
            let mut a = Vec::new();
            if !readonly {
                a.push("-rw".into());
            }
            a.extend([
                image.as_os_str().into(),
                "--".into(),
                mountpoint.as_os_str().into(),
                "-f".into(),
                "-o".into(),
                if readonly {
                    "backend=smb,ro".into()
                } else {
                    "backend=smb".into()
                },
            ]);
            a
        }
    };
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.join("driver.log"))
        .map_err(|e| AppError::io("Cannot open private filesystem log", e))?;
    let mut command = Command::new(driver);
    command
        .args(a.drain(..))
        .env("LC_ALL", "C")
        .env_remove("DYLD_INSERT_LIBRARIES")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone()
                .map_err(|e| AppError::io("Cannot clone filesystem log", e))?,
        ))
        .stderr(Stdio::from(log));
    // The CLI's controlling terminal may close immediately after mount. Keep
    // the long-lived driver out of that terminal's session and SIGHUP group.
    // SAFETY: the child hook only calls the async-signal-safe setsid syscall.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::io("Cannot start filesystem driver", e))?;
    session.pid = child.id();
    session.process_start = process_start(child.id())?.unwrap_or_default();
    save(&path, &session)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut root_ready = false;
    loop {
        if let Some(identity) = mount_identity(mountpoint)? {
            let expected_transport = match kind {
                Kind::Xfs => " (smbfs,",
                Kind::Ext4 => " (nfs,",
            };
            if !identity.contains(expected_transport) {
                return Err(AppError::new(
                    "Filesystem driver published an unexpected transport; preserving session.",
                ));
            }
            if session
                .mount_identity
                .as_ref()
                .is_some_and(|expected| expected != &identity)
            {
                return Err(AppError::new(
                    "Filesystem mount changed during startup; preserving session.",
                ));
            }
            session.mount_identity = Some(identity);
            session.mount_dev = Some(
                fs::metadata(mountpoint)
                    .map_err(|e| AppError::io("Cannot inspect new filesystem mount", e))?
                    .dev(),
            );
            save(&path, &session)?;
            let readable = fs::read_dir(mountpoint)
                .and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
                .is_ok();
            if readable && root_ready {
                return Ok(true);
            }
            root_ready = readable;
        }
        if child
            .try_wait()
            .map_err(|e| AppError::io("Cannot inspect filesystem driver", e))?
            .is_some()
            || Instant::now() >= deadline
        {
            let detail = fs::read_to_string(path.join("driver.log")).unwrap_or_default();
            let close = close_session(&path, &mut session);
            return Err(AppError::new(format!(
                "Filesystem driver did not mount: {}. Cleanup: {}",
                detail.chars().take(2000).collect::<String>(),
                close
                    .err()
                    .map(|error| error.message)
                    .unwrap_or_else(|| "complete".into())
            )));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn mount_missing(
    source: &Path,
    mountpoint: &Path,
    pass: &[u8],
    readonly: bool,
    keyfile: Option<&Path>,
) -> AppResult<bool> {
    let registry = Registry::lock()?;
    if registry
        .sessions()?
        .iter()
        .any(|(_, session)| same_path(&session.source, source))
    {
        return Err(AppError::new(
            "Container already has an external filesystem session.",
        ));
    }
    let image = open_image(source, pass, readonly, keyfile)?;
    let result = start(&registry, source, mountpoint, &image, readonly);
    if !matches!(result, Ok(true))
        && !registry
            .sessions()?
            .iter()
            .any(|(_, session)| same_path(&session.source, source))
    {
        close_crypto(source)?;
    }
    result
}
pub(super) fn unmount(target: &Path, kind: VolumeType) -> AppResult<bool> {
    if !registry_path().exists() {
        return Ok(false);
    }
    let registry = Registry::lock()?;
    let mut matches: Vec<_> = registry
        .sessions()?
        .into_iter()
        .filter(|(_, session)| same_path(&session.source, target) || session.mountpoint == target)
        .collect();
    if matches.is_empty() {
        return Ok(false);
    }
    if matches.len() != 1 || !matches!(kind, VolumeType::Auto | VolumeType::Veracrypt) {
        return Err(AppError::new(
            "Target does not identify one matching VeraCrypt filesystem session.",
        ));
    }
    let (path, session) = &mut matches[0];
    close_session(path, session)?;
    Ok(true)
}
pub(super) fn augment(volumes: &mut [Volume]) -> AppResult<()> {
    if !registry_path().exists() {
        return Ok(());
    }
    let registry = Registry::lock()?;
    for (_, session) in registry.sessions()? {
        validate_image(&session)?;
        if let Some(volume) = volumes.iter_mut().find(|volume| {
            volume.kind == VolumeType::Veracrypt && same_path(&volume.source, &session.source)
        }) {
            if session.mount_identity.as_ref() == mount_identity(&session.mountpoint)?.as_ref()
                && session.mount_identity.is_some()
            {
                volume.mountpoint = Some(session.mountpoint);
            }
            volume.filesystem = Some(session.kind.name().into());
            volume.readonly = session.readonly;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_magic_does_not_guess_unknown_filesystems() {
        assert_eq!(detect(b"XFSB"), Some(Kind::Xfs));
        assert_eq!(detect(b"APFS"), None);
        assert_eq!(detect(b"NTFS"), None);
        assert_eq!(detect(&[]), None);
        let mut ext4 = [0; 4096];
        ext4[1080..1082].copy_from_slice(&[0x53, 0xef]);
        assert_eq!(detect(&ext4), None);
        ext4[1120] = 0x40;
        assert_eq!(detect(&ext4), Some(Kind::Ext4));
    }
}
