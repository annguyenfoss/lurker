//! Optional login-user filesystem drivers over native encrypted Apple images.
//! No driver or formatter in this module is ever elevated.
use super::*;
use crate::CreateFilesystem;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Kind {
    Xfs,
    Ntfs,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Xfs => "xfs",
            Self::Ntfs => "ntfs",
        }
    }
    fn driver(self) -> &'static str {
        match self {
            Self::Xfs => "fuse-xfs",
            Self::Ntfs => "ntfs-3g",
        }
    }
    fn formatter(self) -> &'static str {
        match self {
            Self::Xfs => "mkfs.xfs",
            Self::Ntfs => "mkntfs",
        }
    }
}
fn requested(filesystem: Option<CreateFilesystem>) -> Option<Kind> {
    match filesystem {
        Some(CreateFilesystem::Xfs) => Some(Kind::Xfs),
        Some(CreateFilesystem::Ntfs) => Some(Kind::Ntfs),
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

pub(super) fn create_args(
    size: u64,
    filesystem: Option<CreateFilesystem>,
) -> AppResult<Option<Vec<OsString>>> {
    let Some(kind) = requested(filesystem) else {
        return Ok(None);
    };
    tool(kind.driver())?;
    tool(kind.formatter())?;
    if size < 384 {
        return Err(AppError::new(
            "External macOS filesystem containers require at least 384 MiB.",
        ));
    }
    Ok(Some(args(&[
        "create",
        "-size",
        &format!("{size}m"),
        "-type",
        "UDIF",
        "-layout",
        "NONE",
        "-encryption",
        "AES-256",
        "-stdinpass",
    ])))
}

#[derive(Deserialize)]
struct Attachment {
    #[serde(rename = "system-entities")]
    entities: Vec<Entity>,
}
fn attach(source: &Path, pass: &[u8], readonly: bool) -> AppResult<Vec<Entity>> {
    let mut a = args(&["attach", "-nomount", "-plist", "-stdinpass"]);
    if readonly {
        a.push("-readonly".into());
    }
    a.push(source.into());
    let response = checked(Path::new(HDIUTIL), &a, Some(pass))?;
    Ok(decode::<Attachment>(&response.stdout)?.entities)
}
fn detach_device(device: &Path) -> AppResult<()> {
    checked(Path::new(HDIUTIL), &["detach".into(), device.into()], None)?;
    Ok(())
}
fn detect(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(b"XFSB") {
        Some(Kind::Xfs)
    } else if bytes.get(3..11) == Some(b"NTFS    ") {
        Some(Kind::Ntfs)
    } else {
        None
    }
}
fn device_kind(device: &Path) -> Option<Kind> {
    let mut bytes = [0; 4096];
    let mut file = File::open(device).ok()?;
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_exact(&mut bytes).ok()?;
    detect(&bytes)
}
pub(super) fn format(
    source: &Path,
    filesystem: Option<CreateFilesystem>,
    pass: &[u8],
) -> AppResult<()> {
    let Some(kind) = requested(filesystem) else {
        return Ok(());
    };
    let entities = attach(source, pass, false)?;
    let device = entities
        .first()
        .ok_or_else(|| AppError::new("Blank encrypted image has no virtual device."))?
        .device
        .clone();
    let arguments = match kind {
        Kind::Xfs => vec!["-f".into(), device.as_os_str().into()],
        Kind::Ntfs => vec!["-F".into(), "-Q".into(), device.as_os_str().into()],
    };
    let result = checked(&tool(kind.formatter())?, &arguments, None).map(|_| ());
    let close = detach_device(&device);
    close?;
    result
}

#[derive(Serialize, Deserialize)]
struct Session {
    source: PathBuf,
    source_dev: u64,
    source_ino: u64,
    device: PathBuf,
    filesystem_device: PathBuf,
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
fn validate_image(session: &Session) -> AppResult<bool> {
    let metadata = fs::metadata(&session.source)
        .map_err(|e| AppError::io("Cannot inspect filesystem container", e))?;
    if metadata.dev() != session.source_dev || metadata.ino() != session.source_ino {
        return Err(AppError::new(
            "Container identity changed; preserving filesystem session.",
        ));
    }
    let image = images()?
        .into_iter()
        .find(|image| same_path(&image.path, &session.source));
    match image {
        Some(image)
            if image.encrypted
                && image
                    .owner
                    .is_none_or(|owner| owner == crate::effective_uid())
                && image
                    .entities
                    .iter()
                    .any(|entity| entity.device == session.device)
                && image
                    .entities
                    .iter()
                    .any(|entity| entity.device == session.filesystem_device) =>
        {
            Ok(true)
        }
        Some(_) => Err(AppError::new(
            "Encrypted virtual-device identity changed; preserving filesystem session.",
        )),
        None => Ok(false),
    }
}
fn close_session(path: &Path, session: &mut Session) -> AppResult<()> {
    let attached = validate_image(session)?;
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
        if !attached {
            return Err(AppError::new(
                "Encrypted device disappeared while its filesystem is mounted.",
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
    if attached {
        detach_device(&session.device)?;
    }
    fs::remove_dir_all(path).map_err(|e| AppError::io("Cannot remove closed filesystem session", e))
}
fn start(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    entities: &[Entity],
    readonly: bool,
) -> AppResult<bool> {
    let Some((filesystem_device, kind)) = entities
        .iter()
        .find_map(|entity| device_kind(&entity.device).map(|kind| (entity.device.clone(), kind)))
    else {
        return Ok(false);
    };
    let driver = tool(kind.driver())?;
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
    let mut a = match kind {
        Kind::Xfs => {
            let mut a = Vec::new();
            if !readonly {
                a.push("-rw".into());
            }
            a.extend([
                filesystem_device.as_os_str().into(),
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
        Kind::Ntfs => vec![
            "-o".into(),
            if readonly {
                "no_detach,backend=nfs,ro".into()
            } else {
                "no_detach,backend=nfs".into()
            },
            filesystem_device.as_os_str().into(),
            mountpoint.as_os_str().into(),
        ],
    };
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.join("driver.log"))
        .map_err(|e| AppError::io("Cannot open private filesystem log", e))?;
    let mut child = Command::new(driver)
        .args(a.drain(..))
        .env("LC_ALL", "C")
        .env_remove("DYLD_INSERT_LIBRARIES")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone()
                .map_err(|e| AppError::io("Cannot clone filesystem log", e))?,
        ))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| AppError::io("Cannot start filesystem driver", e))?;
    let mut session = Session {
        source: source.into(),
        source_dev: metadata.dev(),
        source_ino: metadata.ino(),
        device: entities[0].device.clone(),
        filesystem_device,
        mountpoint: mountpoint.into(),
        kind,
        readonly,
        pid: child.id(),
        process_start: process_start(child.id())?.unwrap_or_default(),
        mount_identity: None,
        mount_dev: None,
    };
    save(&path, &session)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(identity) = mount_identity(mountpoint)? {
            let expected_transport = match kind {
                Kind::Xfs => " (smbfs,",
                Kind::Ntfs => " (nfs,",
            };
            if !identity.contains(expected_transport) {
                return Err(AppError::new(
                    "Filesystem driver published an unexpected transport; preserving session.",
                ));
            }
            session.mount_identity = Some(identity);
            session.mount_dev = Some(
                fs::metadata(mountpoint)
                    .map_err(|e| AppError::io("Cannot inspect new filesystem mount", e))?
                    .dev(),
            );
            save(&path, &session)?;
            return Ok(true);
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

/// Used only after the native mount reports an unknown filesystem. Native
/// APFS/exFAT/FAT and read-only NTFS retain the original hdiutil route.
pub(super) fn mount_missing(
    source: &Path,
    mountpoint: &Path,
    pass: &[u8],
    readonly: bool,
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
    let entities = attach(source, pass, readonly)?;
    let result = start(&registry, source, mountpoint, &entities, readonly);
    if !matches!(result, Ok(true)) {
        // A persisted session owns cleanup once a driver has started.
        if !registry
            .sessions()?
            .iter()
            .any(|(_, session)| same_path(&session.source, source))
        {
            if let Some(entity) = entities.first() {
                detach_device(&entity.device)?;
            }
        }
    }
    result
}
pub(super) fn replace_native_ntfs(
    source: &Path,
    mountpoint: &Path,
    readonly: bool,
) -> AppResult<()> {
    if readonly {
        return Ok(());
    }
    let Some(image) = images()?
        .into_iter()
        .find(|image| same_path(&image.path, source))
    else {
        return Ok(());
    };
    if !image
        .entities
        .iter()
        .any(|entity| device_kind(&entity.device) == Some(Kind::Ntfs))
    {
        return Ok(());
    }
    let registry = Registry::lock()?;
    tool(Kind::Ntfs.driver())?;
    checked(
        Path::new(DISKUTIL),
        &["unmount".into(), mountpoint.into()],
        None,
    )?;
    match start(&registry, source, mountpoint, &image.entities, false) {
        Ok(true) => Ok(()),
        Ok(false) => Err(AppError::new(
            "Native NTFS volume has no verified NTFS signature.",
        )),
        Err(error) => Err(error),
    }
}
pub(super) fn unmount(target: &Path, kind: VolumeType) -> AppResult<bool> {
    if !registry_path().exists() {
        return Ok(false);
    }
    let registry = Registry::lock()?;
    let mut matches: Vec<_> = registry
        .sessions()?
        .into_iter()
        .filter(|(_, session)| {
            same_path(&session.source, target)
                || session.mountpoint == target
                || session.device == target
                || session.filesystem_device == target
        })
        .collect();
    if matches.is_empty() {
        return Ok(false);
    }
    if matches.len() != 1 || !matches!(kind, VolumeType::Auto | VolumeType::Apple) {
        return Err(AppError::new(
            "Target does not identify one matching external filesystem session.",
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
        if !validate_image(&session)? {
            continue;
        }
        if let Some(volume) = volumes
            .iter_mut()
            .find(|volume| same_path(&volume.source, &session.source))
        {
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
        assert_eq!(detect(b"\xeb\x52\x90NTFS    "), Some(Kind::Ntfs));
        assert_eq!(detect(b"APFS"), None);
        assert_eq!(detect(b"NTFS"), None);
        assert_eq!(detect(&[]), None);
    }
}
