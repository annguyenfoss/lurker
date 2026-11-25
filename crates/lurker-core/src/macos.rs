//! macOS file-container backend. Crypto and virtual disks stay in the system tools.
use crate::size::size_gb_to_mib;
use crate::{
    ActiveVolume, AppError, AppResult, CommandAction, CreateCipher, CreateCommand, MountCommand,
    Output, SourceKind, SystemProbe, ToolStatus, UnmountCommand, VolumeType,
};
use serde::Deserialize;
use std::ffi::{CString, OsString};
use std::fs::{self, OpenOptions};
use std::io::{IsTerminal, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output as ProcessOutput, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

mod apfs;
mod filesystems;
mod vm_filesystem;
mod zfs;

pub(super) fn run_filesystem_supervisor_if_requested() -> Option<i32> {
    vm_filesystem::dispatch()
}

const HDIUTIL: &str = "/usr/bin/hdiutil";
const DISKUTIL: &str = "/usr/sbin/diskutil";

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

// Never put a passphrase in argv, the environment, a file, or diagnostic output.
fn execute(
    program: &Path,
    arguments: &[OsString],
    input: Option<&[u8]>,
) -> AppResult<ProcessOutput> {
    let mut child = Command::new(program)
        .args(arguments)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::io(format!("Failed to start {}", program.display()), e))?;
    if let Some(input) = input {
        let result = child.stdin.take().unwrap().write_all(input);
        if let Err(e) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::io("Failed to send passphrase", e));
        }
    }
    child
        .wait_with_output()
        .map_err(|e| AppError::io("Failed waiting for container tool", e))
}

fn checked(
    program: &Path,
    arguments: &[OsString],
    input: Option<&[u8]>,
) -> AppResult<ProcessOutput> {
    let result = execute(program, arguments, input)?;
    if result.status.success() {
        return Ok(result);
    }
    // Tool diagnostics are useful (busy mounts, missing FUSE, bad credentials), but
    // a tool must never be allowed to echo the supplied secret into the UI logs.
    let mut detail = String::from_utf8_lossy(&result.stderr).into_owned();
    if detail.trim().is_empty() {
        detail = String::from_utf8_lossy(&result.stdout).into_owned();
    }
    if let Some(input) = input {
        let secret = String::from_utf8_lossy(input);
        let secret = secret.trim_end_matches(['\0', '\n']);
        if !secret.is_empty() {
            detail = detail.replace(secret, "[redacted]");
        }
    }
    Err(AppError::new(format!(
        "{} failed ({}): {}",
        program.display(),
        result.status,
        detail.trim()
    )))
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> AppResult<T> {
    plist::from_bytes(bytes).map_err(|e| AppError::new(format!("Invalid disk-image response: {e}")))
}

fn veracrypt() -> Option<PathBuf> {
    // Only the private build has the specified FUSE-T dependency. An installed
    // VeraCrypt.app or PATH binary may instead require macFUSE.
    let current = std::env::current_exe().ok()?;
    private_veracrypt_candidates(&current)
        .into_iter()
        .find(|path| executable(path))
}

fn private_veracrypt_candidates(current: &Path) -> Vec<PathBuf> {
    let Some(directory) = current.parent() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if directory.file_name().is_some_and(|name| name == "MacOS")
        && directory
            .parent()
            .is_some_and(|parent| parent.file_name().is_some_and(|name| name == "Contents"))
    {
        candidates.push(directory.join("../Helpers/lurker-veracrypt"));
    }
    // Native CLI package and uninstalled development staging layouts.
    candidates.push(directory.join("../libexec/lurker/backends/lurker-veracrypt"));
    candidates.push(directory.join("macos-backends/lurker-veracrypt"));
    candidates
}

fn executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn require_veracrypt() -> AppResult<PathBuf> {
    veracrypt().ok_or_else(|| AppError::new("The private FUSE-T VeraCrypt backend is missing. Install a complete Lurker package or build scripts/build-macos-backends.py. Install FUSE-T separately for VeraCrypt operations."))
}

pub fn probe_system() -> AppResult<SystemProbe> {
    Ok(SystemProbe {
        is_root: crate::effective_uid() == 0,
        tools: vec![
            ToolStatus {
                name: "hdiutil".into(),
                path: executable(Path::new(HDIUTIL)).then(|| HDIUTIL.into()),
                required: true,
            },
            ToolStatus {
                name: "diskutil".into(),
                path: executable(Path::new(DISKUTIL)).then(|| DISKUTIL.into()),
                required: true,
            },
            ToolStatus {
                name: "veracrypt".into(),
                path: veracrypt(),
                required: false,
            },
        ],
    })
}

fn expand(path: &Path) -> AppResult<PathBuf> {
    if path == Path::new("~") || path.starts_with("~/") {
        let home = std::env::var_os("HOME").ok_or_else(|| AppError::new("HOME is not set."))?;
        return Ok(PathBuf::from(home).join(path.strip_prefix("~").unwrap()));
    }
    Ok(path.to_path_buf())
}

fn existing(path: &Path) -> AppResult<PathBuf> {
    let path = expand(path)?;
    fs::canonicalize(&path)
        .map_err(|e| AppError::io(format!("Failed to resolve {}", path.display()), e))
}

fn target_path(path: &Path) -> AppResult<PathBuf> {
    let path = expand(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| AppError::new("Target needs a file name."))?;
    Ok(existing(parent)?.join(name))
}

fn require_type(kind: VolumeType) -> AppResult<()> {
    if !matches!(
        kind,
        VolumeType::Auto | VolumeType::Apple | VolumeType::Veracrypt
    ) {
        Err(AppError::new(
            "This container backend is unavailable on macOS. Use --type apple or --type veracrypt.",
        ))
    } else {
        Ok(())
    }
}

fn require_no_tag(tag: Option<&str>) -> AppResult<()> {
    if tag.is_some() {
        Err(AppError::new(
            "Tags are supported only by Linux LUKS volumes.",
        ))
    } else {
        Ok(())
    }
}

fn secret(supplied: Option<&str>, create: bool) -> AppResult<Vec<u8>> {
    let pass = if let Some(pass) = supplied {
        pass.to_string()
    } else {
        if !std::io::stdin().is_terminal() {
            return Err(AppError::new(
                "Passphrase entry requires a terminal or the desktop app.",
            ));
        }
        let pass = crate::credentials::prompt_passphrase("Passphrase: ")
            .map_err(|e| AppError::io("Failed to read passphrase", e))?;
        if create {
            let confirm = crate::credentials::prompt_passphrase("Confirm passphrase: ")
                .map_err(|e| AppError::io("Failed to read confirmation", e))?;
            if pass != confirm {
                return Err(AppError::new("Passphrase confirmation does not match."));
            }
        }
        pass
    };
    if pass.is_empty() || pass.contains(['\0', '\n', '\r']) {
        return Err(AppError::new(
            "Passphrase must be non-empty and contain no NUL or newline characters.",
        ));
    }
    Ok(pass.into_bytes())
}

#[derive(Debug, Deserialize, Default)]
struct Entity {
    #[serde(rename = "dev-entry", default)]
    device: PathBuf,
    #[serde(rename = "mount-point")]
    mountpoint: Option<PathBuf>,
    #[serde(rename = "volume-kind")]
    filesystem: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Image {
    #[serde(rename = "image-path")]
    path: PathBuf,
    #[serde(rename = "image-encrypted", default)]
    encrypted: bool,
    #[serde(rename = "system-entities", default)]
    entities: Vec<Entity>,
    #[serde(rename = "writeable", default)]
    writable: bool,
    #[serde(rename = "owner-uid")]
    owner: Option<u32>,
}
#[derive(Deserialize)]
struct Images {
    images: Vec<Image>,
}
#[derive(Deserialize)]
struct Encryption {
    encrypted: bool,
}

#[derive(Debug)]
struct Volume {
    source: PathBuf,
    devices: Vec<PathBuf>,
    mountpoint: Option<PathBuf>,
    kind: VolumeType,
    readonly: bool,
    filesystem: Option<String>,
}

fn images() -> AppResult<Vec<Image>> {
    let result = checked(Path::new(HDIUTIL), &args(&["info", "-plist"]), None)?;
    Ok(decode::<Images>(&result.stdout)?.images)
}

fn apple_volumes() -> AppResult<Vec<Volume>> {
    let uid = crate::effective_uid();
    Ok(images()?
        .into_iter()
        .filter(|i| i.encrypted && i.owner.is_none_or(|owner| owner == uid))
        .map(|i| {
            let mounted = i.entities.iter().find(|e| e.mountpoint.is_some());
            Volume {
                source: i.path,
                mountpoint: mounted.and_then(|e| e.mountpoint.clone()),
                filesystem: mounted.and_then(|e| e.filesystem.clone()),
                devices: i
                    .entities
                    .into_iter()
                    .map(|e| e.device)
                    .filter(|p| !p.as_os_str().is_empty())
                    .collect(),
                kind: VolumeType::Apple,
                readonly: !i.writable,
            }
        })
        .collect())
}

fn parse_veracrypt_list(text: &str) -> AppResult<Vec<Volume>> {
    let mut result = Vec::new();
    let mut current: Option<Volume> = None;
    for line in text.lines() {
        if let Some(source) = line.strip_prefix("Volume: ") {
            if let Some(volume) = current.take() {
                result.push(volume);
            }
            current = Some(Volume {
                source: source.into(),
                devices: Vec::new(),
                mountpoint: None,
                kind: VolumeType::Veracrypt,
                readonly: false,
                filesystem: None,
            });
        } else if let Some(volume) = current.as_mut() {
            if let Some(device) = line.strip_prefix("Virtual Device: ") {
                volume.devices.push(device.into());
            }
            if let Some(mount) = line.strip_prefix("Mount Directory: ") {
                if !mount.is_empty() && mount != "-" {
                    volume.mountpoint = Some(mount.into());
                }
            }
            if let Some(readonly) = line.strip_prefix("Read-Only: ") {
                volume.readonly = readonly.eq_ignore_ascii_case("yes");
            }
        }
    }
    if let Some(volume) = current {
        result.push(volume);
    }
    if result.is_empty() && !text.trim().is_empty() {
        return Err(AppError::new("Unrecognized VeraCrypt volume listing."));
    }
    Ok(result)
}

fn fuse_t_load_failed(result: &ProcessOutput) -> bool {
    if result.status.signal() != Some(libc::SIGABRT) {
        return false;
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    let Some((loader, library)) = stderr
        .lines()
        .next()
        .and_then(|line| line.split_once("]: Library not loaded: "))
    else {
        return false;
    };
    let filename = Path::new(library.trim())
        .file_name()
        .and_then(|name| name.to_str());
    loader.starts_with("dyld[")
        && filename.is_some_and(|name| {
            name == "libfuse-t.dylib" || name.starts_with("libfuse-t-") && name.ends_with(".dylib")
        })
}

fn read_veracrypt_volumes(allow_missing_fuse_t: bool) -> AppResult<Vec<Volume>> {
    let Some(program) = veracrypt() else {
        return Ok(Vec::new());
    };
    let result = execute(
        &program,
        &args(&["-t", "--list", "--verbose", "--non-interactive"]),
        None,
    )?;
    if fuse_t_load_failed(&result) {
        if allow_missing_fuse_t {
            return Ok(Vec::new());
        }
        return Err(AppError::new(
            "The private VeraCrypt engine could not load FUSE-T. Install FUSE-T 1.2.9 or later.",
        ));
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    if !result.status.success() {
        if text.trim() == "Error: No volumes mounted." {
            return Ok(Vec::new());
        }
        return Err(AppError::new(format!(
            "Failed to list VeraCrypt volumes: {}",
            text.trim()
        )));
    }
    parse_veracrypt_list(&String::from_utf8_lossy(&result.stdout))
}

fn vera_volumes() -> AppResult<Vec<Volume>> {
    let mut volumes = read_veracrypt_volumes(false)?;
    filesystems::augment(&mut volumes)?;
    Ok(volumes)
}

fn volumes() -> AppResult<Vec<Volume>> {
    let mut result = apple_volumes()?;
    result.extend(apfs::list()?);
    let mut veracrypt = read_veracrypt_volumes(true)?;
    filesystems::augment(&mut veracrypt)?;
    result.extend(veracrypt);
    Ok(result)
}

pub fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    Ok(volumes()?
        .into_iter()
        .map(|v| ActiveVolume {
            privilege_mode: crate::PrivilegeMode::Privileged,
            mapper_name: v
                .source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            mapper_path: v
                .devices
                .first()
                .cloned()
                .unwrap_or_else(|| v.source.clone()),
            source_path: Some(v.source),
            readonly: v.readonly,
            mountpoint: v.mountpoint,
            filesystem_type: v.filesystem,
        })
        .collect())
}

fn encrypted_apple(path: &Path) -> AppResult<Option<bool>> {
    let mut arguments = args(&["isencrypted", "-plist"]);
    arguments.push(path.into());
    let result = execute(Path::new(HDIUTIL), &arguments, None)?;
    if !result.status.success() {
        return Ok(None);
    }
    if decode::<Encryption>(&result.stdout)?.encrypted {
        return Ok(Some(true));
    }
    // isencrypted reports false even for arbitrary random bytes. imageinfo can
    // also accept arbitrary sector-aligned *.img files as CRawDiskImage, so its
    // exit code alone cannot identify plaintext (including VeraCrypt images).
    let mut arguments = args(&["imageinfo", "-plist", "-stdinpass"]);
    arguments.push(path.into());
    let info = execute(Path::new(HDIUTIL), &arguments, None)?;
    if !info.status.success() {
        return Ok(None);
    }
    plain_image_info(&info.stdout)
}

#[derive(Deserialize)]
struct ImageFormat {
    #[serde(rename = "Class Name")]
    class_name: String,
    #[serde(rename = "Properties")]
    properties: ImageProperties,
}

#[derive(Deserialize)]
struct ImageProperties {
    #[serde(rename = "Partitioned")]
    partitioned: bool,
}

fn plain_image_info(bytes: &[u8]) -> AppResult<Option<bool>> {
    let format: ImageFormat = decode(bytes)?;
    // Recognizable Apple container formats and partitioned plaintext disks are
    // refused. An unrecognized raw file still requires VeraCrypt authentication;
    // it is never attached directly as an unencrypted image.
    Ok((format.class_name != "CRawDiskImage" || format.properties.partitioned).then_some(false))
}

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    match &command {
        CommandAction::Create(c) if c.volume_type == VolumeType::Apfs => {
            return apfs::create(c.clone(), output)
        }
        CommandAction::Mount(c)
            if c.volume_type == VolumeType::Apfs
                || c.volume_type == VolumeType::Auto
                    && (encrypted_apple(&c.source)? != Some(true) && apfs::detects(&c.source)?) =>
        {
            return apfs::mount(c.clone(), output)
        }
        CommandAction::Unmount(c)
            if c.volume_type == VolumeType::Apfs
                || c.volume_type == VolumeType::Auto
                    && apfs::list()?.iter().any(|v| {
                        same_path(&v.source, &c.target)
                            || v.mountpoint
                                .as_ref()
                                .is_some_and(|p| same_path(p, &c.target))
                            || v.devices.iter().any(|p| same_path(p, &c.target))
                    }) =>
        {
            return apfs::unmount(c.clone(), output)
        }
        _ => {}
    }
    match command {
        CommandAction::Create(c) => create(c, output),
        CommandAction::Mount(c) => mount(c, output),
        CommandAction::Unmount(c) => unmount(c, output),
        CommandAction::Keyslots(_) => Err(AppError::new("LUKS keyslot management requires Linux.")),
    }
}

// A private same-directory staging folder prevents another process replacing a
// temporary image. No target is changed until the complete image is ready.
struct Staging(PathBuf);
impl Staging {
    fn new(parent: &Path) -> AppResult<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for attempt in 0..100 {
            let path = parent.join(format!(".lurker-{}-{nonce}-{attempt}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(AppError::io("Failed to stage container", e)),
            }
        }
        Err(AppError::new(
            "Failed to reserve a temporary container directory.",
        ))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_target(path: &Path, force: bool) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.file_type().is_file() => Err(AppError::new(
            "Create target must be a regular file; symlinks and devices are refused.",
        )),
        Ok(_) if !force => Err(AppError::new(
            "Target file already exists. Re-run with --force to overwrite it.",
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::io("Failed to inspect create target", e)),
    }
}

fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    require_type(c.volume_type)?;
    if c.volume_type == VolumeType::Auto {
        return Err(AppError::new(
            "Option --type auto is not valid with create.",
        ));
    }
    if c.source_kind != SourceKind::File {
        return Err(AppError::new("macOS create supports file containers only."));
    }
    if c.volume_type == VolumeType::Apple && c.cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "Apple disk images support only --cipher aes (AES-256).",
        ));
    }
    let size = size_gb_to_mib(
        c.size_gb
            .as_deref()
            .ok_or_else(|| AppError::new("File container size is required."))?,
    )?;
    if size < 32 {
        return Err(AppError::new("macOS containers must be at least 32 MiB."));
    }
    // Validate the inner filesystem before prompting or allocating a staging image.
    let external_args = filesystems::create_args(&c, size)?;
    let external_format = external_args.is_some();
    let mut arguments = if let Some(arguments) = external_args {
        arguments
    } else if c.volume_type == VolumeType::Apple {
        crate::workflow::macos_apple_create_args(size, c.filesystem)?
    } else {
        crate::workflow::macos_veracrypt_create_args(
            c.cipher,
            size,
            c.keyfile.as_deref(),
            c.filesystem,
        )?
    };
    let target = target_path(&c.target)?;
    if let Some(keyfile) = c.keyfile.as_deref() {
        crate::credentials::ensure_distinct_target(&target, keyfile)?;
    }
    assert_target(&target, c.force)?;
    // Forced replacement must check every attached image, including plaintext.
    // Ordinary Apple create cannot overwrite a target and needs no VeraCrypt
    // discovery. VeraCrypt create still checks that its runtime can start.
    let attached_image = c.force && images()?.iter().any(|i| same_path(&i.path, &target));
    let check_veracrypt = c.force || c.volume_type == VolumeType::Veracrypt;
    if attached_image
        || check_veracrypt
            && vera_volumes()?
                .iter()
                .any(|v| same_path(&v.source, &target))
    {
        return Err(AppError::new(
            "Refusing to overwrite an attached container. Unmount it first.",
        ));
    }
    let staging = Staging::new(target.parent().unwrap())?;
    let temp = staging.0.join(if c.volume_type == VolumeType::Apple {
        "container.dmg"
    } else {
        "container.hc"
    });
    let mut pass = if c.keyfile.is_some() {
        Vec::new()
    } else {
        secret(c.passphrase.as_deref(), true)?
    };
    output.msg2(&format!("Creating {} container", c.volume_type));
    let program = if c.volume_type == VolumeType::Apple {
        pass.push(0);
        PathBuf::from(HDIUTIL)
    } else {
        pass.push(b'\n');
        require_veracrypt()?
    };
    arguments.push(temp.as_os_str().into());
    checked(&program, &arguments, Some(&pass))?;
    if external_format {
        if let Err(error) = filesystems::format(&temp, c.filesystem, &pass, c.keyfile.as_deref()) {
            // Preserve the recoverable ciphertext if crypto teardown failed.
            std::mem::forget(staging);
            return Err(error);
        }
    }
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
        .map_err(|e| AppError::io("Failed to protect container permissions", e))?;
    OpenOptions::new()
        .write(true)
        .open(&temp)
        .and_then(|f| f.sync_all())
        .map_err(|e| AppError::io("Failed to sync container", e))?;
    assert_target(&target, c.force)?;
    publish(&temp, &target, c.force)?;
    output.msg2(&format!("Created {}", target.display()));
    Ok(())
}

fn publish(temp: &Path, target: &Path, force: bool) -> AppResult<()> {
    if force {
        return fs::rename(temp, target)
            .map_err(|e| AppError::io("Failed to publish container", e));
    }
    let temp = CString::new(temp.as_os_str().as_bytes())
        .map_err(|_| AppError::new("Invalid temporary path."))?;
    let target = CString::new(target.as_os_str().as_bytes())
        .map_err(|_| AppError::new("Invalid target path."))?;
    // SAFETY: both paths are valid NUL-terminated strings for this call. RENAME_EXCL
    // atomically refuses an existing name, including on hosts without hard links.
    let status = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            temp.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(AppError::io(
            "Failed to publish container",
            std::io::Error::last_os_error(),
        ))
    }
}

// Darwin automatically renames an AppleDouble companion when its base entry
// moves between directories on filesystems such as exFAT. Accept only that
// exact missing-source case; arbitrary missing vault data must remain an error.
pub(crate) fn apple_double_was_moved(source: &Path, published: &Path) -> bool {
    let Some(name) = published.file_name() else {
        return false;
    };
    if source.file_name() != Some(name) {
        return false;
    }
    let Some(base) = name
        .as_bytes()
        .strip_prefix(b"._")
        .filter(|base| !base.is_empty())
    else {
        return false;
    };
    if !fs::symlink_metadata(source).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        || !fs::symlink_metadata(published).is_ok_and(|m| m.file_type().is_file())
        || !published.parent().is_some_and(|parent| {
            fs::symlink_metadata(parent.join(std::ffi::OsStr::from_bytes(base)))
                .is_ok_and(|m| m.is_file() || m.is_dir())
        })
    {
        return false;
    }
    let mut magic = [0; 4];
    fs::File::open(published)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && magic == [0, 5, 22, 7]
}

fn same_path(left: &Path, right: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (fs::metadata(left), fs::metadata(right)) {
        if left.dev() == right.dev() && left.ino() == right.ino() {
            return true;
        }
    }
    fs::canonicalize(left).unwrap_or_else(|_| left.into())
        == fs::canonicalize(right).unwrap_or_else(|_| right.into())
}

fn mount(c: MountCommand, output: Output) -> AppResult<()> {
    require_type(c.volume_type)?;
    require_no_tag(c.tag.as_deref())?;
    let source = existing(&c.source)?;
    if !source.is_file() {
        return Err(AppError::new("macOS supports file containers only."));
    }
    let mountpoint = target_path(&c.mountpoint)?;
    if mountpoint.exists()
        && (!mountpoint.is_dir()
            || fs::read_dir(&mountpoint)
                .map_err(|e| AppError::io("Failed to inspect mount point", e))?
                .next()
                .is_some())
    {
        return Err(AppError::new("Mount point must be an empty directory."));
    }
    let apple = encrypted_apple(&source)?;
    let kind = match (c.volume_type, apple) {
        (_, Some(false)) => {
            return Err(AppError::new(
                "Refusing to mount an unencrypted disk image.",
            ))
        }
        (VolumeType::Auto, Some(true)) => VolumeType::Apple,
        (VolumeType::Auto, None) => VolumeType::Veracrypt,
        (VolumeType::Apple, None) => {
            return Err(AppError::new(
                "Source is not an Apple encrypted disk image.",
            ))
        }
        (VolumeType::Veracrypt, Some(true)) => {
            return Err(AppError::new(
                "Source is an Apple encrypted disk image, not VeraCrypt.",
            ))
        }
        (kind, _) => kind,
    };
    let mounted = if kind == VolumeType::Apple {
        apple_volumes()?
    } else {
        vera_volumes()?
    };
    if mounted.iter().any(|v| {
        same_path(&v.source, &source)
            || v.mountpoint
                .as_ref()
                .is_some_and(|p| same_path(p, &mountpoint))
    }) {
        return Err(AppError::new("Container or mount point is already in use."));
    }
    if kind == VolumeType::Apple && c.keyfile.is_some() {
        return Err(AppError::new(
            "Keyfile authentication is supported for LUKS and VeraCrypt only.",
        ));
    }
    let mut pass = if c.keyfile.is_some() {
        Vec::new()
    } else {
        secret(c.passphrase.as_deref(), false)?
    };
    let (program, mut arguments) = if kind == VolumeType::Apple {
        pass.push(0);
        let mut a = args(&["attach", "-plist", "-stdinpass", "-nobrowse", "-mountpoint"]);
        a.push(mountpoint.as_os_str().into());
        if c.readonly {
            a.push("-readonly".into());
        }
        (HDIUTIL.into(), a)
    } else {
        pass.push(b'\n');
        let mut a = args(&[
            "-t",
            "--mount",
            "--stdin",
            "--non-interactive",
            "--pim=0",
            "-k",
            "",
            "--protect-hidden=no",
        ]);
        let key_index = a.iter().position(|arg| arg == "-k").unwrap() + 1;
        a[key_index] = crate::credentials::veracrypt_keyfiles(c.keyfile.as_deref())?;
        if c.readonly {
            a.push("--mount-options=ro".into());
        }
        (require_veracrypt()?, a)
    };
    arguments.push(source.as_os_str().into());
    if kind == VolumeType::Veracrypt {
        arguments.push(mountpoint.as_os_str().into());
    }
    if let Err(error) = checked(&program, &arguments, Some(&pass)) {
        if kind == VolumeType::Veracrypt
            && (error.message.contains("no mountable file systems")
                || error.message.contains("Unable to invoke task"))
            && filesystems::mount_missing(
                &source,
                &mountpoint,
                &pass,
                c.readonly,
                c.keyfile.as_deref(),
            )?
        {
            output.msg2(&format!(
                "Mounted {} at {}",
                source.display(),
                mountpoint.display()
            ));
            return Ok(());
        }
        // Native tools normally roll back failed mounts themselves. If one
        // left an attachment behind, close only this source at this mountpoint.
        let attached = if kind == VolumeType::Apple {
            apple_volumes()
        } else {
            vera_volumes()
        };
        if let Ok(attached) = attached {
            for v in attached.into_iter().filter(|v| {
                same_path(&v.source, &source)
                    && v.mountpoint
                        .as_ref()
                        .is_some_and(|p| same_path(p, &mountpoint))
            }) {
                if let Err(cleanup_error) = detach(&v) {
                    output.warn(&format!(
                        "Failed to clean up container: {}",
                        cleanup_error.message
                    ));
                }
            }
        }
        return Err(error);
    }
    if kind == VolumeType::Veracrypt {
        filesystems::replace_native_ntfs(&source, &mountpoint, c.readonly)?;
    }
    output.msg2(&format!(
        "Mounted {} at {}",
        source.display(),
        mountpoint.display()
    ));
    Ok(())
}

fn unmount(c: UnmountCommand, output: Output) -> AppResult<()> {
    require_type(c.volume_type)?;
    require_no_tag(c.tag.as_deref())?;
    let target = existing(&c.target)?;
    if filesystems::unmount(&target, c.volume_type)? {
        output.msg2(&format!("Unmounted {}", target.display()));
        return Ok(());
    }
    let matches_target = |v: &Volume| {
        same_path(&v.source, &target)
            || v.mountpoint.as_ref().is_some_and(|p| same_path(p, &target))
            || v.devices.iter().any(|p| same_path(p, &target))
    };
    let mut matches: Vec<_> = apple_volumes()?
        .into_iter()
        .filter(matches_target)
        .collect();
    if matches.is_empty() && c.volume_type != VolumeType::Apple {
        matches = vera_volumes()?.into_iter().filter(matches_target).collect();
    }
    if matches.len() != 1 {
        return Err(AppError::new(
            "Target must identify exactly one attached encrypted container.",
        ));
    }
    let volume = &matches[0];
    if c.volume_type != VolumeType::Auto && c.volume_type != volume.kind {
        return Err(AppError::new(
            "Target does not match the requested container type.",
        ));
    }
    detach(volume)?;
    output.msg2(&format!("Unmounted {}", volume.source.display()));
    Ok(())
}

fn detach(volume: &Volume) -> AppResult<()> {
    if volume.kind == VolumeType::Apple {
        let device = volume
            .devices
            .first()
            .ok_or_else(|| AppError::new("Disk image has no virtual device."))?;
        checked(
            Path::new(HDIUTIL),
            &["detach".into(), device.as_os_str().into()],
            None,
        )?;
    } else {
        // Some VeraCrypt macOS builds force-detach internally, even without
        // --force. Let Disk Arbitration reject busy filesystems before closing
        // the encrypted mapping, so ordinary unmount never discards open data.
        if let Some(mountpoint) = &volume.mountpoint {
            checked(
                Path::new(DISKUTIL),
                &["unmount".into(), mountpoint.as_os_str().into()],
                None,
            )?;
        }
        checked(
            &require_veracrypt()?,
            &[
                "-t".into(),
                "--non-interactive".into(),
                "--dismount".into(),
                volume.source.as_os_str().into(),
            ],
            None,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_identity_includes_hardlink_aliases() {
        let stage = Staging::new(&std::env::temp_dir()).unwrap();
        let source = stage.0.join("container");
        let alias = stage.0.join("hardlink alias");
        let separate = stage.0.join("copy");
        fs::write(&source, b"source data").unwrap();
        fs::hard_link(&source, &alias).unwrap();
        fs::copy(&source, &separate).unwrap();
        assert!(same_path(&source, &alias));
        assert!(!same_path(&source, &separate));
    }

    #[test]
    fn generic_raw_img_is_not_misidentified_as_plaintext() {
        let raw = br#"<plist version="1.0"><dict><key>Class Name</key><string>CRawDiskImage</string><key>Properties</key><dict><key>Partitioned</key><false/></dict></dict></plist>"#;
        assert_eq!(plain_image_info(raw).unwrap(), None);
        let partitioned = String::from_utf8_lossy(raw).replace("<false/>", "<true/>");
        assert_eq!(
            plain_image_info(partitioned.as_bytes()).unwrap(),
            Some(false)
        );
        let udif = String::from_utf8_lossy(raw).replace("CRawDiskImage", "CUDIFDiskImage");
        assert_eq!(plain_image_info(udif.as_bytes()).unwrap(), Some(false));
        assert!(plain_image_info(b"<plist version=\"1.0\"><dict/></plist>").is_err());
    }

    #[test]
    fn private_backends_precede_compatibility_installations() {
        let paths = private_veracrypt_candidates(Path::new(
            "/Applications/Lurker.app/Contents/MacOS/lurker-helper",
        ));
        assert_eq!(
            paths[0],
            Path::new("/Applications/Lurker.app/Contents/MacOS/../Helpers/lurker-veracrypt")
        );
        let paths = private_veracrypt_candidates(Path::new("/usr/local/bin/lurker"));
        assert_eq!(
            paths[0],
            Path::new("/usr/local/bin/../libexec/lurker/backends/lurker-veracrypt")
        );
    }

    #[test]
    fn parses_veracrypt_paths_with_spaces() {
        let v = parse_veracrypt_list("Slot: 1\nVolume: /tmp/a b.hc\nVirtual Device: /dev/disk12\nMount Directory: /tmp/a b\nRead-Only: Yes\n\nSlot: 2\nVolume: /tmp/other.hc\nMount Directory: -\n").unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].source, Path::new("/tmp/a b.hc"));
        assert_eq!(v[0].mountpoint.as_deref(), Some(Path::new("/tmp/a b")));
        assert!(v[0].readonly);
        assert!(v[1].mountpoint.is_none());
        assert!(parse_veracrypt_list("unexpected output").is_err());
    }

    #[test]
    fn recognizes_only_fuse_t_dynamic_loader_failure() {
        let mut result = ProcessOutput {
            status: std::process::ExitStatus::from_raw(libc::SIGABRT),
            stdout: Vec::new(),
            stderr: b"dyld[123]: Library not loaded: /usr/local/lib/libfuse-t.dylib\n".to_vec(),
        };
        assert!(fuse_t_load_failed(&result));
        result.stderr =
            b"dyld[123]: Library not loaded: @loader_path/libfuse-t-1.2.9.dylib\n".to_vec();
        assert!(fuse_t_load_failed(&result));
        result.stderr = b"dyld[123]: Library not loaded: @loader_path/libwx.dylib\n".to_vec();
        assert!(!fuse_t_load_failed(&result));
        result.status = std::process::ExitStatus::from_raw(1 << 8);
        result.stderr = b"dyld[123]: Library not loaded: /usr/local/lib/libfuse-t.dylib\n".to_vec();
        assert!(!fuse_t_load_failed(&result));
    }

    #[test]
    fn publish_never_clobbers_a_competing_target_without_force() {
        let stage = Staging::new(&std::env::temp_dir()).unwrap();
        let source = stage.0.join("source");
        let target = stage.0.join("target");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        assert!(publish(&source, &target, false).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert_eq!(fs::read(&source).unwrap(), b"new");
        fs::remove_file(&target).unwrap();
        publish(&source, &target, false).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!source.exists());
        fs::write(&source, b"replacement").unwrap();
        publish(&source, &target, true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
    }

    #[test]
    fn recognizes_only_a_published_native_apple_double_companion() {
        let stage = Staging::new(&std::env::temp_dir()).unwrap();
        let source_dir = stage.0.join("source");
        fs::create_dir(&source_dir).unwrap();
        let source = source_dir.join("._data");
        let sidecar = stage.0.join("._data");
        let base = stage.0.join("data");
        fs::write(&sidecar, [0, 5, 22, 7]).unwrap();
        assert!(!apple_double_was_moved(&source, &sidecar));
        fs::write(&base, b"vault data").unwrap();
        assert!(apple_double_was_moved(&source, &sidecar));
        fs::write(&source, b"still present").unwrap();
        assert!(!apple_double_was_moved(&source, &sidecar));
        fs::remove_file(&source).unwrap();
        fs::write(&sidecar, b"ordinary file").unwrap();
        assert!(!apple_double_was_moved(&source, &sidecar));
        assert!(!apple_double_was_moved(&source, &base));
    }

    #[test]
    fn apple_plist_handles_multiple_entities_and_user_ownership() {
        let xml = br#"<?xml version="1.0"?><plist version="1.0"><dict><key>images</key><array><dict>
            <key>image-path</key><string>/tmp/a &amp; b.dmg</string><key>image-encrypted</key><true/>
            <key>owner-uid</key><integer>501</integer><key>writeable</key><false/>
            <key>system-entities</key><array><dict><key>dev-entry</key><string>/dev/disk9</string></dict>
            <dict><key>dev-entry</key><string>/dev/disk10s1</string><key>mount-point</key><string>/tmp/a &amp; b</string><key>volume-kind</key><string>apfs</string></dict></array>
            </dict></array></dict></plist>"#;
        let images = decode::<Images>(xml).unwrap();
        assert_eq!(images.images[0].path, Path::new("/tmp/a & b.dmg"));
        assert!(images.images[0].encrypted);
        assert!(!images.images[0].writable);
        assert_eq!(images.images[0].owner, Some(501));
        assert_eq!(
            images.images[0].entities[1].filesystem.as_deref(),
            Some("apfs")
        );
        assert!(decode::<Images>(b"malformed response").is_err());
    }
    #[test]
    fn failed_tools_cannot_echo_padded_passphrases_into_logs() {
        let error = checked(
            Path::new("/bin/sh"),
            &args(&["-c", "cat >&2; exit 1"]),
            Some(b"  private test password  \n"),
        )
        .unwrap_err();
        assert!(!error.message.contains("private test password"));
        assert!(error.message.contains("[redacted]"));
    }

    #[test]
    fn rejects_bad_passwords_without_launching_tools() {
        for pass in ["", "a\0b", "a\nb", "a\rb"] {
            assert!(secret(Some(pass), false).is_err());
        }
        assert_eq!(secret(Some("  secret  "), false).unwrap(), b"  secret  ");
    }
}
