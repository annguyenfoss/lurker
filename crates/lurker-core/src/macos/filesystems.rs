//! Optional filesystems over VeraCrypt's authenticated decrypted image.
//! Formatters and filesystem drivers always run as the login user.
use super::*;
use crate::CreateFilesystem;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Kind {
    Btrfs,
    Ext4,
    Xfs,
    Ntfs,
    Zfs,
    Ufs2,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Btrfs => "btrfs",
            Self::Ext4 => "ext4",
            Self::Xfs => "xfs",
            Self::Ntfs => "ntfs",
            Self::Zfs => "zfs",
            Self::Ufs2 => "ufs2",
        }
    }
    fn driver(self) -> &'static str {
        match self {
            Self::Btrfs => "anylinuxfs",
            Self::Ext4 => "fuse-ext2",
            Self::Xfs => "fuse-xfs",
            Self::Ntfs => "ttntfs",
            Self::Zfs => "mount_zfs",
            Self::Ufs2 => "fuse-ufs",
        }
    }
    fn formatter(self) -> &'static str {
        match self {
            Self::Btrfs => unreachable!("Btrfs is formatted in its Linux guest"),
            Self::Ext4 => "mkfs.ext4",
            Self::Xfs => "mkfs.xfs",
            Self::Ntfs => "mkntfs",
            Self::Zfs => "zpool",
            Self::Ufs2 => unreachable!("UFS2 imports are read-only; no formatter is selected"),
        }
    }
}
fn requested(filesystem: Option<CreateFilesystem>) -> Option<Kind> {
    match filesystem {
        Some(CreateFilesystem::Btrfs) => Some(Kind::Btrfs),
        Some(CreateFilesystem::Ext4) => Some(Kind::Ext4),
        Some(CreateFilesystem::Xfs) => Some(Kind::Xfs),
        Some(CreateFilesystem::Ntfs) => Some(Kind::Ntfs),
        Some(CreateFilesystem::Zfs) => Some(Kind::Zfs),
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
        .ok_or_else(|| AppError::new(if name == "fuse-ufs" {
            "Private macOS filesystem tool fuse-ufs is missing. Install the qualified foreground-only reader and approve the separately installed macFUSE kernel extension.".into()
        } else {
            format!("Private macOS filesystem tool {name} is missing. Install the qualified FUSE-T filesystem tools; no driver is run with sudo.")
        }))
}

fn formatter(kind: Kind) -> AppResult<PathBuf> {
    if kind != Kind::Ntfs {
        return tool(kind.formatter());
    }
    // hdiutil authorizes the login user to format their identified virtual disk.
    // Homebrew paths are deliberately never passed through sudo.
    if crate::effective_uid() == 0 {
        return Err(AppError::new("NTFS formatting must run as the login user."));
    }
    if let Ok(path) = tool("mkntfs") {
        return Ok(path);
    }
    [
        "/opt/homebrew/opt/ntfs-3g-mac/sbin/mkntfs",
        "/usr/local/opt/ntfs-3g-mac/sbin/mkntfs",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| executable(path))
    .ok_or_else(|| {
        AppError::new(
            "NTFS creation requires mkntfs from ntfs-3g-mac. Install its free native formatter.",
        )
    })
}

pub(super) fn create_args(c: &CreateCommand, size: u64) -> AppResult<Option<Vec<OsString>>> {
    let Some(kind) = requested(c.filesystem).filter(|_| c.volume_type == VolumeType::Veracrypt)
    else {
        return Ok(None);
    };
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "External macOS filesystem containers must be created as the login user.",
        ));
    }
    if !matches!(kind, Kind::Btrfs | Kind::Ntfs | Kind::Zfs) {
        tool(kind.driver())?;
    }
    if kind == Kind::Btrfs {
        vm_filesystem::available()?;
    } else if kind == Kind::Zfs {
        zfs::available()?;
    } else {
        formatter(kind)?;
    }
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
    if bytes.get(3..11) == Some(b"NTFS    ") {
        Some(Kind::Ntfs)
    } else if bytes.starts_with(b"XFSB") {
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
    read_image_kind(&mut File::open(image).ok()?)
}
fn opened_image_kind(image: &OpenImage) -> Option<Kind> {
    image_kind(&image.path).or_else(|| read_image_kind(&mut File::open(&image.device).ok()?))
}
fn read_image_kind(image: &mut (impl Read + Seek)) -> Option<Kind> {
    let mut bytes = [0; 4096];
    image.read_exact(&mut bytes).ok()?;
    if let Some(kind) = detect(&bytes) {
        return Some(kind);
    }
    // Btrfs's primary superblock starts at 64 KiB; its magic is at +64.
    // Read aligned sectors so the same probe works on native virtual devices.
    image.seek(SeekFrom::Start(65536)).ok()?;
    let mut superblock = [0; 2048];
    image.read_exact(&mut superblock).ok()?;
    if &superblock[64..72] == b"_BHRfS_M" {
        return Some(Kind::Btrfs);
    }
    // FreeBSD UFS2's primary superblock starts at 64 KiB. The magic is
    // 1372 bytes into it; the reader supports both filesystem byte orders.
    let magic: [u8; 4] = superblock[1372..1376].try_into().ok()?;
    matches!(magic, [0x19, 0x01, 0x54, 0x19] | [0x19, 0x54, 0x01, 0x19]).then_some(Kind::Ufs2)
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

fn close_identified_crypto(source: &Path, expected_image: &Path) -> AppResult<()> {
    // VeraCrypt remembers a native device number even after an external driver
    // detaches it. Never let that cached number close another image after reuse.
    let attached = images()?;
    let volumes = read_veracrypt_volumes(false)?;
    let Some(volume) = volumes
        .iter()
        .find(|volume| same_path(&volume.source, source))
    else {
        return Ok(());
    };
    if volume
        .devices
        .iter()
        .any(|device| cached_device_reused(&attached, device, expected_image))
    {
        return Err(AppError::new(
            "VeraCrypt's cached virtual device now belongs to another image; preserving the encrypted session.",
        ));
    }
    close_crypto(source)
}

fn cached_device_reused(attached: &[Image], device: &Path, expected_image: &Path) -> bool {
    attached.iter().any(|image| {
        image.entities.iter().any(|entity| entity.device == device)
            && !same_path(&image.path, expected_image)
    })
}

struct OpenImage {
    path: PathBuf,
    device: PathBuf,
}
fn open_image(
    source: &Path,
    pass: &[u8],
    readonly: bool,
    keyfile: Option<&Path>,
    detach_native: bool,
) -> AppResult<OpenImage> {
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
    let result = identified_image(source).and_then(|image| {
        if detach_native {
            checked(
                Path::new(HDIUTIL),
                &["detach".into(), image.device.as_os_str().into()],
                None,
            )?;
        }
        Ok(image)
    });
    if result.is_err() {
        close_crypto(source)?;
    }
    result
}
fn identified_image(source: &Path) -> AppResult<OpenImage> {
    let volume = read_veracrypt_volumes(false)?
        .into_iter()
        .find(|volume| same_path(&volume.source, source))
        .ok_or_else(|| AppError::new("VeraCrypt did not publish the authenticated volume."))?;
    let device = volume
        .devices
        .first()
        .ok_or_else(|| AppError::new("VeraCrypt has no identified auxiliary virtual device."))?;
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
    Ok(OpenImage {
        path,
        device: device.clone(),
    })
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
    if kind == Kind::Btrfs {
        return format_btrfs(source, pass, keyfile);
    }
    if kind == Kind::Zfs {
        return format_zfs(source, pass, keyfile);
    }
    let opened = open_image(source, pass, false, keyfile, kind != Kind::Ntfs)?;
    let image = &opened.path;
    let arguments = match kind {
        Kind::Btrfs => unreachable!("Btrfs uses its Linux guest"),
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
        Kind::Ntfs => vec![
            "-F".into(),
            "-Q".into(),
            "-s".into(),
            "512".into(),
            opened.device.as_os_str().into(),
        ],
        Kind::Zfs => unreachable!("ZFS uses its authenticated native device"),
        Kind::Ufs2 => unreachable!("UFS2 creation is unsupported"),
    };
    let result = checked(&formatter(kind)?, &arguments, None).map(|_| ());
    close_identified_crypto(source, image)?;
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
    #[serde(default)]
    native_device: Option<PathBuf>,
    #[serde(default)]
    zfs_pool: Option<zfs::PoolIdentity>,
    #[serde(default)]
    vm_protocol: Option<u32>,
    #[serde(default)]
    vm_closed: bool,
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
    if let Some(device) = &session.native_device {
        validate_native_device(&session.image, device)?;
    }
    Ok(())
}

fn validate_native_device(image: &Path, device: &Path) -> AppResult<()> {
    if !images()?.iter().any(|attached| {
        same_path(&attached.path, image)
            && attached
                .entities
                .iter()
                .any(|entity| entity.device == device)
    }) {
        return Err(AppError::new(
            "Native virtual device identity changed; preserving the encrypted session.",
        ));
    }
    Ok(())
}

fn mount_is_readonly(path: &Path) -> AppResult<bool> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AppError::new("Invalid filesystem mount path."))?;
    let mut status = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: path is NUL-terminated and status points to writable storage.
    if unsafe { libc::statfs(path.as_ptr(), status.as_mut_ptr()) } != 0 {
        return Err(AppError::io(
            "Cannot inspect filesystem mount flags",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: successful statfs initialized the entire structure.
    Ok(unsafe { status.assume_init() }.f_flags & libc::MNT_RDONLY as u32 != 0)
}

fn start_native_ntfs(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    image: &OpenImage,
    readonly: bool,
) -> AppResult<bool> {
    validate_native_device(&image.path, &image.device)?;
    let (path, mut session) = reserve_session(
        registry,
        source,
        mountpoint,
        &image.path,
        Kind::Ntfs,
        readonly,
        Some(image.device.clone()),
    )?;
    // hdiutil authorizes this login user's virtual device. FSKit provides the
    // driver; no private binary or native mount command is elevated.
    let mut arguments = args(&["-F", "-t", "ttntfs"]);
    if readonly {
        arguments.extend(args(&["-o", "ro"]));
    }
    arguments.extend([
        image.device.as_os_str().into(),
        mountpoint.as_os_str().into(),
    ]);
    let mounted = checked(Path::new("/sbin/mount"), &arguments, None);
    if let Some(identity) = mount_identity(mountpoint)? {
        if !native_ntfs_identity(&identity, &image.device, mountpoint) {
            return Err(AppError::new(
                "TT NTFS Native published an unexpected mount identity; preserving the encrypted session.",
            ));
        }
        session.mount_identity = Some(identity);
        session.mount_dev = Some(
            fs::metadata(mountpoint)
                .map_err(|error| AppError::io("Cannot inspect native NTFS mount", error))?
                .dev(),
        );
        save(&path, &session)?;
    }
    let result = mounted.and_then(|_| {
        if session.mount_identity.is_none() {
            return Err(AppError::new(
                "TT NTFS Native did not publish a filesystem mount.",
            ));
        }
        if mount_is_readonly(mountpoint)? != readonly {
            return Err(AppError::new(
                "TT NTFS Native did not honor the requested read-only mode.",
            ));
        }
        fs::read_dir(mountpoint)
            .map_err(|error| AppError::io("Cannot read native NTFS filesystem", error))?;
        Ok(true)
    });
    if let Err(error) = result {
        let cleanup = close_session(&path, &mut session);
        return Err(AppError::new(format!(
            "Native NTFS mount failed: {}. Install and enable the free TT NTFS Native FSKit extension. Cleanup: {}",
            error.message,
            cleanup
                .err()
                .map(|error| error.message)
                .unwrap_or_else(|| "complete".into()),
        )));
    }
    result
}

/// A native VeraCrypt mount can succeed through Apple's read-only NTFS driver.
/// Inspect successful mounts too, so an RW request never silently uses it.
pub(super) fn replace_native_ntfs(
    source: &Path,
    mountpoint: &Path,
    readonly: bool,
) -> AppResult<()> {
    // Successful writable mounts already satisfy the request. Read-only
    // imports can retain Apple's native driver; only a silent RW downgrade
    // needs the explicit TT NTFS route.
    if readonly || !mount_is_readonly(mountpoint)? {
        return Ok(());
    }
    let identity = mount_identity(mountpoint)?;
    if !identity.as_deref().is_some_and(|line| {
        line.rsplit_once(" (")
            .is_some_and(|(_, options)| options.starts_with("ntfs,"))
    }) {
        return Ok(());
    }
    let registry = Registry::lock()?;
    let image = identified_image(source)?;
    validate_native_device(&image.path, &image.device)?;
    if !read_veracrypt_volumes(false)?.iter().any(|volume| {
        same_path(&volume.source, source)
            && volume
                .mountpoint
                .as_ref()
                .is_some_and(|path| same_path(path, mountpoint))
    }) {
        return Err(AppError::new(
            "Native NTFS mount does not belong to the authenticated VeraCrypt source.",
        ));
    }
    checked(
        Path::new(DISKUTIL),
        &["unmount".into(), mountpoint.as_os_str().into()],
        None,
    )?;
    let result = start_native_ntfs(&registry, source, mountpoint, &image, readonly).map(|_| ());
    if result.is_err()
        && !registry
            .sessions()?
            .iter()
            .any(|(_, session)| same_path(&session.source, source))
    {
        close_identified_crypto(source, &image.path)?;
    }
    result
}

fn native_ntfs_identity(identity: &str, device: &Path, mountpoint: &Path) -> bool {
    identity
        .strip_prefix(&format!(
            "{} on {} (",
            device.display(),
            mountpoint.display()
        ))
        .and_then(|options| options.strip_suffix(')'))
        .is_some_and(|options| {
            let options: Vec<_> = options.split(", ").collect();
            options.first() == Some(&"ntfs") && options.contains(&"fskit")
        })
}

fn validate_session_mount(session: &Session) -> AppResult<()> {
    if let Some(identity) = mount_identity(&session.mountpoint)? {
        if session.mount_identity.as_ref() != Some(&identity)
            || session.mount_dev
                != Some(
                    fs::metadata(&session.mountpoint)
                        .map_err(|error| AppError::io("Inspect native filesystem mount", error))?
                        .dev(),
                )
        {
            return Err(AppError::new(
                "Filesystem mount identity changed; preserving encrypted container.",
            ));
        }
    }
    Ok(())
}

fn remember_zfs_mount(path: &Path, session: &mut Session) -> AppResult<()> {
    if let Some(identity) = mount_identity(&session.mountpoint)? {
        let pool = session
            .zfs_pool
            .as_ref()
            .ok_or_else(|| AppError::new("Missing ZFS identity."))?;
        let device = session
            .native_device
            .as_ref()
            .ok_or_else(|| AppError::new("Missing ZFS virtual device."))?;
        if !zfs::verify_mounted(device, pool, &session.mountpoint, Some(session.readonly))? {
            return Err(AppError::new(
                "Unexpected ZFS mount; preserving the encrypted session.",
            ));
        }
        session.mount_identity = Some(identity);
        session.mount_dev = Some(
            fs::metadata(&session.mountpoint)
                .map_err(|error| AppError::io("Inspect native ZFS mount", error))?
                .dev(),
        );
        save(path, session)?;
    }
    Ok(())
}

fn format_zfs(source: &Path, pass: &[u8], keyfile: Option<&Path>) -> AppResult<()> {
    let registry = Registry::lock()?;
    let target = source
        .parent()
        .ok_or_else(|| AppError::new("Missing ZFS staging directory."))?
        .join("zfs-initialize");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&target)
        .map_err(|error| AppError::io("Create private ZFS initialization mount", error))?;
    let image = open_image(source, pass, false, keyfile, false)?;
    let (path, mut session) = reserve_session(
        &registry,
        source,
        &target,
        &image.path,
        Kind::Zfs,
        false,
        Some(image.device.clone()),
    )?;
    // Recovery ownership is durable before zpool can change the authenticated device.
    let (device, identity) = zfs::create(&image.device, &image.path)?;
    session.native_device = Some(device.clone());
    session.zfs_pool = Some(identity.clone());
    save(&path, &session)?;
    let result = zfs::initialize(&device, &identity, &target);
    remember_zfs_mount(&path, &mut session)?;
    result?;
    close_session(&path, &mut session)?;
    fs::remove_dir(&target).map_err(|error| AppError::io("Remove ZFS initialization mount", error))
}

fn format_btrfs(source: &Path, pass: &[u8], keyfile: Option<&Path>) -> AppResult<()> {
    let registry = Registry::lock()?;
    let target = source
        .parent()
        .ok_or_else(|| AppError::new("Missing Btrfs staging directory."))?
        .join("btrfs-initialize");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&target)
        .map_err(|error| AppError::io("Create private Btrfs initialization directory", error))?;
    let image = open_image(source, pass, false, keyfile, false)?;
    let (path, mut session) = reserve_session(
        &registry,
        source,
        &target,
        &image.path,
        Kind::Btrfs,
        false,
        Some(image.device.clone()),
    )?;
    vm_filesystem::format(
        &path,
        &image.path,
        &image.device,
        crate::effective_uid(),
        unsafe { libc::getegid() },
    )?;
    close_session(&path, &mut session)?;
    fs::remove_dir(&target)
        .map_err(|error| AppError::io("Remove Btrfs initialization directory", error))
}

fn start_btrfs(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    image: &OpenImage,
    readonly: bool,
) -> AppResult<bool> {
    vm_filesystem::available()?;
    let (path, mut session) = reserve_session(
        registry,
        source,
        mountpoint,
        &image.path,
        Kind::Btrfs,
        readonly,
        Some(image.device.clone()),
    )?;
    session.pid = vm_filesystem::start(&path, &image.path, &image.device, mountpoint, readonly)?;
    session.process_start = process_start(session.pid)?.ok_or_else(|| {
        AppError::new("Btrfs VM supervisor exited; preserving its recovery journal.")
    })?;
    vm_filesystem::verify_mount(&path, mountpoint, readonly)?;
    session.mount_identity = mount_identity(mountpoint)?;
    session.mount_dev = Some(
        fs::metadata(mountpoint)
            .map_err(|error| AppError::io("Inspect Btrfs host mount", error))?
            .dev(),
    );
    save(&path, &session)?;
    Ok(true)
}

fn start_native_zfs(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    image: &OpenImage,
    readonly: bool,
    pool: zfs::PoolIdentity,
) -> AppResult<bool> {
    validate_native_device(&image.path, &image.device)?;
    let (path, mut session) = reserve_session(
        registry,
        source,
        mountpoint,
        &image.path,
        Kind::Zfs,
        readonly,
        Some(image.device.clone()),
    )?;
    session.zfs_pool = Some(pool.clone());
    save(&path, &session)?;
    let result = zfs::mount(&image.device, &pool, mountpoint, readonly);
    remember_zfs_mount(&path, &mut session)?;
    result?;
    if session.mount_identity.is_none() || mount_is_readonly(mountpoint)? != readonly {
        return Err(AppError::new(
            "ZFS did not publish the requested mount; preserving recovery state.",
        ));
    }
    Ok(true)
}

fn close_session(path: &Path, session: &mut Session) -> AppResult<()> {
    validate_image(session)?;
    if session.kind == Kind::Btrfs {
        if session.vm_protocol != Some(vm_filesystem::PROTOCOL) {
            return Err(AppError::new(
                "Unsupported Btrfs VM session protocol; preserving encrypted storage.",
            ));
        }
        if mount_identity(&session.mountpoint)?.is_some() {
            vm_filesystem::verify_mount(path, &session.mountpoint, session.readonly)?;
            if session.mount_identity.is_none() {
                session.mount_identity = mount_identity(&session.mountpoint)?;
                session.mount_dev = Some(
                    fs::metadata(&session.mountpoint)
                        .map_err(|error| AppError::io("Inspect recovered Btrfs mount", error))?
                        .dev(),
                );
                save(path, session)?;
            }
            validate_session_mount(session)?;
            checked(
                Path::new(DISKUTIL),
                &["unmount".into(), session.mountpoint.as_os_str().into()],
                None,
            )?;
            if mount_identity(&session.mountpoint)?.is_some() {
                return Err(AppError::new(
                    "Btrfs host mount remains; preserving the VM and encrypted storage.",
                ));
            }
        }
        session.mount_identity = None;
        session.mount_dev = None;
        save(path, session)?;
        if !session.vm_closed {
            vm_filesystem::request_close(path)?;
            session.vm_closed = true;
            save(path, session)?;
        }
        validate_image(session)?;
        if let Some(device) = &session.native_device {
            checked(
                Path::new(HDIUTIL),
                &["detach".into(), device.as_os_str().into()],
                None,
            )?;
            session.native_device = None;
            save(path, session)?;
        }
        File::open(&session.image)
            .and_then(|image| image.sync_all())
            .map_err(|error| AppError::io("Sync closed Btrfs crypto image", error))?;
        close_identified_crypto(&session.source, &session.image)?;
        return fs::remove_dir_all(path)
            .map_err(|error| AppError::io("Remove closed Btrfs session", error));
    }
    if session.kind == Kind::Zfs {
        let device = session.native_device.clone().ok_or_else(|| {
            AppError::new("Missing native ZFS device; preserving the encrypted session.")
        })?;
        if session.zfs_pool.is_none() {
            let (leaf, pool) = zfs::resolve_device(&device, &session.image)?.ok_or_else(|| {
                AppError::new(
                    "Cannot identify the ZFS pool for recovery; preserving its encrypted device.",
                )
            })?;
            session.native_device = Some(leaf);
            session.zfs_pool = Some(pool);
            save(path, session)?;
        }
        validate_image(session)?;
        // Import/mount may have completed before startup journaled the mount.
        // Adopt only the exact root proven by the recorded pool and sole-vdev
        // GUIDs; an existing recorded identity must still match unchanged.
        if session.mount_identity.is_none() {
            remember_zfs_mount(path, session)?;
        }
        let device = session.native_device.as_ref().unwrap();
        let pool = session.zfs_pool.as_ref().unwrap();
        validate_session_mount(session)?;
        zfs::close(device, pool, &session.mountpoint)?;
        session.mount_identity = None;
        session.mount_dev = None;
        save(path, session)?;
        close_identified_crypto(&session.source, &session.image)?;
        return fs::remove_dir_all(path)
            .map_err(|error| AppError::io("Cannot remove closed ZFS session", error));
    }
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
    close_identified_crypto(&session.source, &session.image)?;
    fs::remove_dir_all(path).map_err(|e| AppError::io("Cannot remove closed filesystem session", e))
}
fn reserve_session(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    image: &Path,
    kind: Kind,
    readonly: bool,
    native_device: Option<PathBuf>,
) -> AppResult<(PathBuf, Session)> {
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
    let session = Session {
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
        native_device,
        zfs_pool: None,
        vm_protocol: (kind == Kind::Btrfs).then_some(vm_filesystem::PROTOCOL),
        vm_closed: false,
    };
    // Publish recovery ownership before starting a process that can mount.
    if let Err(error) = save(&path, &session) {
        let _ = fs::remove_dir_all(&path);
        return Err(error);
    }
    Ok((path, session))
}
fn start(
    registry: &Registry,
    source: &Path,
    mountpoint: &Path,
    image: &Path,
    readonly: bool,
) -> AppResult<bool> {
    let Some(kind) =
        image_kind(image).filter(|kind| !matches!(kind, Kind::Btrfs | Kind::Ntfs | Kind::Zfs))
    else {
        return Ok(false);
    };
    if kind == Kind::Ufs2 && !readonly {
        return Err(AppError::new(
            "The installed macOS UFS2 reader cannot create or write this filesystem.",
        ));
    }
    let driver = tool(kind.driver())?;
    let (path, mut session) =
        reserve_session(registry, source, mountpoint, image, kind, readonly, None)?;
    let mut a: Vec<OsString> = match kind {
        Kind::Btrfs => unreachable!("Btrfs uses its supervised Linux guest"),
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
        Kind::Ntfs => unreachable!("native NTFS does not spawn a FUSE driver"),
        Kind::Zfs => unreachable!("native ZFS does not spawn a FUSE driver"),
        Kind::Ufs2 => vec![
            "-f".into(),
            "-o".into(),
            "ro,nodev,nosuid".into(),
            image.as_os_str().into(),
            mountpoint.as_os_str().into(),
        ],
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
                Kind::Btrfs => unreachable!("Btrfs has its own mount lifecycle"),
                Kind::Xfs => " (smbfs,",
                Kind::Ext4 => " (nfs,",
                Kind::Ntfs => unreachable!("native NTFS has its own mount lifecycle"),
                Kind::Zfs => unreachable!("native ZFS has its own mount lifecycle"),
                Kind::Ufs2 => " (macfuse,",
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
            if kind == Kind::Ufs2 && !mount_is_readonly(mountpoint)? {
                return Err(AppError::new(
                    "UFS2 reader published a writable mount; preserving its identified session for normal close.",
                ));
            }
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
    let image = open_image(source, pass, readonly, keyfile, false)?;
    let result = (|| {
        let kind = opened_image_kind(&image);
        if kind == Some(Kind::Ntfs) {
            start_native_ntfs(&registry, source, mountpoint, &image, readonly)
        } else if kind == Some(Kind::Btrfs) {
            start_btrfs(&registry, source, mountpoint, &image, readonly)
        } else {
            if kind.is_none() && Path::new("/usr/local/zfs/bin/zdb").exists() {
                if let Some((device, pool)) = zfs::resolve_device(&image.device, &image.path)? {
                    let native = OpenImage {
                        path: image.path.clone(),
                        device,
                    };
                    return start_native_zfs(
                        &registry, source, mountpoint, &native, readonly, pool,
                    );
                }
            }
            checked(
                Path::new(HDIUTIL),
                &["detach".into(), image.device.as_os_str().into()],
                None,
            )?;
            start(&registry, source, mountpoint, &image.path, readonly)
        }
    })();
    if !matches!(result, Ok(true))
        && !registry
            .sessions()?
            .iter()
            .any(|(_, session)| same_path(&session.source, source))
    {
        close_identified_crypto(source, &image.path)?;
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

    #[test]
    fn ntfs_probe_requires_the_complete_oem_signature_at_its_real_offset() {
        let mut header = [0; 512];
        header[3..11].copy_from_slice(b"NTFS    ");
        assert_eq!(detect(&header), Some(Kind::Ntfs));
        assert_eq!(detect(&header[..10]), None);
        assert_eq!(detect(b"NTFS    "), None);
        header[3..11].copy_from_slice(b"EXFAT   ");
        assert_eq!(detect(&header), None);
    }

    #[test]
    fn btrfs_probe_requires_the_primary_superblock_magic() {
        use std::io::Cursor;
        const OFFSET: usize = 65536 + 64;
        let mut image = vec![0; 65536 + 2048];
        image[OFFSET..OFFSET + 8].copy_from_slice(b"_BHRfS_M");
        assert_eq!(read_image_kind(&mut Cursor::new(&image)), Some(Kind::Btrfs));
        assert_eq!(
            read_image_kind(&mut Cursor::new(&image[..OFFSET + 7])),
            None
        );
        image[OFFSET] = 0;
        assert_eq!(read_image_kind(&mut Cursor::new(&image)), None);
    }

    #[test]
    fn ufs2_probe_reads_its_real_superblock_magic_in_both_byte_orders() {
        use std::io::Cursor;
        const OFFSET: usize = 65536 + 1372;
        for magic in [[0x19, 0x01, 0x54, 0x19], [0x19, 0x54, 0x01, 0x19]] {
            let mut image = vec![0; 65536 + 2048];
            image[OFFSET..OFFSET + 4].copy_from_slice(&magic);
            assert_eq!(read_image_kind(&mut Cursor::new(&image)), Some(Kind::Ufs2));
            assert_eq!(
                read_image_kind(&mut Cursor::new(&image[..OFFSET + 3])),
                None
            );
            image[OFFSET..OFFSET + 4].copy_from_slice(&[0x54, 0x19, 0x01, 0]);
            assert_eq!(read_image_kind(&mut Cursor::new(&image)), None);
        }
        assert_eq!(requested(Some(CreateFilesystem::Ufs2)), None);
    }

    #[test]
    fn native_ntfs_mount_must_use_the_owned_device_and_fskit() {
        let target = Path::new("/private/tmp/mounted vault");
        let device = Path::new("/dev/disk4");
        let identity = "/dev/disk4 on /private/tmp/mounted vault (ntfs, local, noowners, fskit, mounted by adjutant)";
        assert!(native_ntfs_identity(identity, device, target));
        assert!(!native_ntfs_identity(
            identity,
            Path::new("/dev/disk5"),
            target
        ));
        assert!(!native_ntfs_identity(
            identity,
            device,
            Path::new("/private/tmp/mounted")
        ));
        assert!(!native_ntfs_identity(
            &identity.replace(", fskit", ""),
            device,
            target
        ));
        assert!(!native_ntfs_identity(
            &identity.replace("(ntfs,", "(exfat,"),
            device,
            target
        ));
    }

    #[test]
    fn crypto_close_rejects_a_cached_device_reassigned_to_another_image() {
        let expected = Path::new("/private/tmp/owned/volume.dmg");
        let device = Path::new("/dev/disk4");
        let mut attached = vec![Image {
            path: expected.into(),
            entities: vec![Entity {
                device: device.into(),
                ..Entity::default()
            }],
            encrypted: false,
            writable: true,
            owner: None,
        }];
        assert!(!cached_device_reused(&attached, device, expected));
        assert!(!cached_device_reused(&[], device, expected));
        attached[0].path = "/private/tmp/unrelated.dmg".into();
        assert!(cached_device_reused(&attached, device, expected));
        assert!(!cached_device_reused(
            &attached,
            Path::new("/dev/disk5"),
            expected
        ));
    }
}
