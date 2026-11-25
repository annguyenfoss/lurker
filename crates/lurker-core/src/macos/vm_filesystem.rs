//! Private login-user Btrfs VM transport. The caller owns the crypto session
//! and journals it before entering this module. VM errors never authorize
//! closing that transport: only request_close's verified result does.
use super::{args, checked};
use crate::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub(super) const PROTOCOL: u32 = 1;
const DISPATCH: &str = "--lurker-macos-filesystem-supervisor";
const GUEST: &str = include_str!("../../../../native/macos/lurker-vm-btrfs.sh");
const START_TIMEOUT: Duration = Duration::from_secs(120);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(90);
const LOG_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ImageIdentity {
    path: PathBuf,
    dev: u64,
    ino: u64,
    len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceNode {
    path: PathBuf,
    dev: u64,
    ino: u64,
    rdev: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceIdentity {
    block: DeviceNode,
    raw: DeviceNode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Request {
    protocol: u32,
    token: String,
    image: ImageIdentity,
    device: DeviceIdentity,
    mountpoint: Option<PathBuf>,
    readonly: bool,
    uid: u32,
    gid: u32,
    port: u16,
    control: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Process {
    pid: u32,
    start: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct MountInfo {
    source: String,
    filesystem: String,
    dev: u64,
    readonly: bool,
}

#[derive(Serialize, Deserialize)]
struct Ready {
    protocol: u32,
    token: String,
    mountpoint: PathBuf,
    mount: MountInfo,
}

#[derive(Serialize, Deserialize)]
struct ResultRecord {
    protocol: u32,
    token: String,
    clean_closed: bool,
    vm_exit: Option<i32>,
    diagnostic: Option<String>,
}

#[derive(Default)]
struct GuestState {
    visible: bool,
    host_key: Option<String>,
    closed: bool,
}

fn login_user() -> AppResult<()> {
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "The macOS Btrfs VM runs as the login user. Run Lurker without sudo.",
        ));
    }
    Ok(())
}

fn home() -> AppResult<PathBuf> {
    let mut buffer = vec![0u8; 65536];
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut found = std::ptr::null_mut();
    // SAFETY: all output storage remains alive until pw_dir has been copied.
    let code = unsafe {
        libc::getpwuid_r(
            crate::effective_uid(),
            &mut entry,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut found,
        )
    };
    if code != 0 || found.is_null() || entry.pw_dir.is_null() {
        return Err(AppError::new(
            "Cannot locate the login user's anylinuxfs profile.",
        ));
    }
    Ok(PathBuf::from(OsStr::from_bytes(
        unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes(),
    )))
}

fn directory(path: &Path, private: bool) -> AppResult<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| AppError::io("Inspect private VM directory", error))?;
    if !metadata.is_dir()
        || metadata.uid() != crate::effective_uid()
        || metadata.mode() & if private { 0o077 } else { 0o022 } != 0
    {
        return Err(AppError::new(
            "VM state must be in an owned directory with protected permissions.",
        ));
    }
    fs::canonicalize(path).map_err(|error| AppError::io("Resolve private VM directory", error))
}

fn rootfs() -> AppResult<PathBuf> {
    let path = home()?.join(".anylinuxfs/alpine/rootfs");
    directory(&path, false).map_err(|error| AppError::new(format!("The prepared anylinuxfs Alpine rootfs is missing or unsafe: {}. Initialize anylinuxfs and install guest btrfs-progs, openssh-server and openssh-sftp-server before using Btrfs.", error.message)))
}

fn runtime_tool(name: &str) -> AppResult<PathBuf> {
    let current =
        std::env::current_exe().map_err(|error| AppError::io("Locate VM tools", error))?;
    let parent = current
        .parent()
        .ok_or_else(|| AppError::new("Missing executable directory."))?;
    let mut candidates = vec![
        parent.join("macos-filesystems").join(name),
        parent.join("../libexec/lurker/filesystems").join(name),
        parent.join("../Helpers/filesystems").join(name),
    ];
    candidates.push(PathBuf::from(match name {
        "anylinuxfs" => "/opt/homebrew/bin/anylinuxfs",
        "sshfs" => "/opt/homebrew/opt/sshfs-mac/bin/sshfs",
        _ => return Err(AppError::new("Unknown VM transport tool.")),
    }));
    let path = candidates.into_iter().find(|path| super::executable(path)).ok_or_else(|| AppError::new(format!("Btrfs requires the prepared anylinuxfs runtime and modern sshfs-mac with separately installed macFUSE. Missing tool: {name}.")))?;
    if name == "sshfs" {
        let output = checked(
            &system_tool("/usr/bin/otool")?,
            &["-L".into(), path.as_os_str().into()],
            None,
        )?;
        let listing = String::from_utf8_lossy(&output.stdout);
        let native_fuse = listing
            .lines()
            .skip(1)
            .filter_map(|line| line.split_whitespace().next())
            .filter(|dependency| Path::new(dependency).is_absolute())
            .filter_map(|dependency| fs::canonicalize(dependency).ok())
            .any(|dependency| {
                let native_path = dependency == Path::new("/usr/local/lib/libfuse3.4.dylib")
                    || (dependency.starts_with("/Library/Filesystems/macfuse.fs")
                        && dependency
                            .file_name()
                            .is_some_and(|name| name.to_string_lossy().starts_with("libfuse")));
                native_path
                    && dependency.ancestors().all(|path| {
                        fs::symlink_metadata(path).is_ok_and(|metadata| {
                            !metadata.file_type().is_symlink()
                                // These tools are never elevated. A Homebrew
                                // library directory owned by this login user
                                // is valid; other users must not control it.
                                && (metadata.uid() == 0 || metadata.uid() == crate::effective_uid())
                                && metadata.mode() & 0o022 == 0
                        })
                    })
            });
        if !native_fuse || listing.contains("fuse-t") {
            return Err(AppError::new(
                "Btrfs requires native sshfs-mac linked to the installed macFUSE library. A FUSE-T SSHFS runtime is not accepted for this transport.",
            ));
        }
    }
    Ok(path)
}

fn system_tool(path: &str) -> AppResult<PathBuf> {
    crate::trusted_privileged_executable(Path::new(path))
}

pub(super) fn available() -> AppResult<()> {
    login_user()?;
    runtime_tool("anylinuxfs")?;
    runtime_tool("sshfs")?;
    for path in ["/usr/bin/ssh-keygen", "/usr/bin/sftp", "/bin/ps"] {
        system_tool(path)?;
    }
    rootfs()?;
    if !Path::new("/Library/Filesystems/macfuse.fs").is_dir() {
        return Err(AppError::new(
            "Btrfs requires the separately installed and enabled macFUSE kernel extension plus modern sshfs-mac.",
        ));
    }
    Ok(())
}

fn image_identity(path: &Path, _readonly: bool) -> AppResult<ImageIdentity> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| AppError::io("Resolve authenticated plaintext image", error))?;
    // hdiutil owns this image's exclusive open. Validate its private inode
    // without reopening it; actual I/O uses the identified native device.
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| AppError::io("Inspect authenticated plaintext image", error))?;
    if !metadata.is_file()
        || metadata.uid() != crate::effective_uid()
        || metadata.mode() & 0o077 != 0
        || metadata.len() < 256 * 1024 * 1024
        || metadata.len() % 512 != 0
    {
        return Err(AppError::new(
            "The VM requires an owned private regular raw image of at least 256 MiB, aligned to 512 bytes.",
        ));
    }
    Ok(ImageIdentity {
        path: canonical,
        dev: metadata.dev(),
        ino: metadata.ino(),
        len: metadata.len(),
    })
}

fn validate_image(request: &Request) -> AppResult<()> {
    let actual = image_identity(&request.image.path, request.readonly)?;
    if actual.path != request.image.path
        || actual.dev != request.image.dev
        || actual.ino != request.image.ino
        || actual.len != request.image.len
    {
        return Err(AppError::new(
            "The plaintext image identity changed; preserving the VM session.",
        ));
    }
    if device_identity(&request.device.block.path, &request.image, request.readonly)?
        != request.device
    {
        return Err(AppError::new(
            "The authenticated native virtual device changed; preserving the VM session.",
        ));
    }
    Ok(())
}

fn device_node(path: &Path, raw: bool) -> AppResult<DeviceNode> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| AppError::io("Inspect authenticated native virtual device", error))?;
    if (raw && !metadata.file_type().is_char_device())
        || (!raw && !metadata.file_type().is_block_device())
    {
        return Err(AppError::new(
            "The VM input is not the expected native block/raw device pair.",
        ));
    }
    Ok(DeviceNode {
        path: path.into(),
        dev: metadata.dev(),
        ino: metadata.ino(),
        rdev: metadata.rdev(),
    })
}

fn device_identity(
    device: &Path,
    image: &ImageIdentity,
    readonly: bool,
) -> AppResult<DeviceIdentity> {
    let unit = device.to_str().and_then(|device| device.strip_prefix("/dev/disk"))
        .filter(|unit| !unit.is_empty() && unit.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| AppError::new("The Btrfs VM requires the identified whole virtual /dev/diskN device, not a physical disk or partition."))?;
    let raw = PathBuf::from(format!("/dev/rdisk{unit}"));
    let attached = super::images()?;
    let mut expected = attached
        .iter()
        .filter(|attached| super::same_path(&attached.path, &image.path));
    let selected = expected.next().ok_or_else(|| {
        AppError::new(
            "The authenticated plaintext image is no longer attached to its native virtual device.",
        )
    })?;
    if expected.next().is_some()
        || selected.entities.len() != 1
        || selected.entities[0].device != device
        || selected.entities[0].mountpoint.is_some()
        || selected
            .owner
            .is_some_and(|owner| owner != crate::effective_uid())
        || (!readonly && !selected.writable)
        || attached.iter().any(|other| {
            !std::ptr::eq(other, selected)
                && other
                    .entities
                    .iter()
                    .any(|entity| entity.device == device || entity.device == raw)
        })
    {
        return Err(AppError::new(
            "The native VM disk is not the unique unmounted whole device of the authenticated private image.",
        ));
    }
    let identity = DeviceIdentity {
        block: device_node(device, false)?,
        raw: device_node(&raw, true)?,
    };
    // hdiutil holds the regular image's exclusive writer. The VM writes its
    // authenticated raw virtual device instead; check the login user's actual
    // access there without changing permissions or elevating the runtime.
    let file = OpenOptions::new()
        .read(true)
        .write(!readonly)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&raw)
        .map_err(|error| AppError::io("Open authenticated raw VM device", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| AppError::io("Verify opened raw VM device", error))?;
    if metadata.dev() != identity.raw.dev
        || metadata.ino() != identity.raw.ino
        || metadata.rdev() != identity.raw.rdev
    {
        return Err(AppError::new(
            "The raw virtual device changed while validating access.",
        ));
    }
    Ok(identity)
}

fn record<T: Serialize>(dir: &Path, name: &str, value: &T) -> AppResult<()> {
    let temporary = dir.join(format!(".{name}-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(|error| AppError::io("Create VM journal record", error))?;
    serde_json::to_writer(&mut file, value)
        .map_err(|error| AppError::new(format!("Encode VM journal: {error}")))?;
    file.sync_all()
        .map_err(|error| AppError::io("Sync VM journal", error))?;
    fs::rename(&temporary, dir.join(name))
        .map_err(|error| AppError::io("Publish VM journal", error))?;
    File::open(dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| AppError::io("Sync VM journal directory", error))
}

fn read_record<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> AppResult<Option<T>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(dir.join(name))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io("Read VM journal", error)),
    };
    let metadata = file
        .metadata()
        .map_err(|error| AppError::io("Inspect VM journal", error))?;
    if !metadata.is_file()
        || metadata.uid() != crate::effective_uid()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
        || metadata.len() > 65536
    {
        return Err(AppError::new(
            "VM journal is not a bounded private regular file.",
        ));
    }
    serde_json::from_reader(file.take(65537))
        .map(Some)
        .map_err(|error| AppError::new(format!("Invalid VM journal: {error}")))
}

fn write_flag(path: &Path, token: &str) -> AppResult<()> {
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(token.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|error| AppError::io("Write private guest control", error))?;
            File::open(path.parent().unwrap())
                .and_then(|directory| directory.sync_all())
                .map_err(|error| AppError::io("Sync private guest control", error))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|error| AppError::io("Read existing guest control", error))?;
            let metadata = file
                .metadata()
                .map_err(|error| AppError::io("Inspect guest control", error))?;
            let mut contents = String::new();
            file.take(128)
                .read_to_string(&mut contents)
                .map_err(|error| AppError::io("Read guest control", error))?;
            if !metadata.is_file()
                || metadata.uid() != crate::effective_uid()
                || metadata.mode() & 0o077 != 0
                || contents != token
            {
                return Err(AppError::new(
                    "Existing guest control has a different identity.",
                ));
            }
            Ok(())
        }
        Err(error) => Err(AppError::io("Create private guest control", error)),
    }
}

fn request(dir: &Path) -> AppResult<Request> {
    let request: Request = read_record(dir, "vm-request.json")?
        .ok_or_else(|| AppError::new("Missing VM ownership journal."))?;
    if request.protocol != PROTOCOL
        || request.token.len() != 32
        || !request.token.bytes().all(|byte| byte.is_ascii_hexdigit())
        || request.control.parent() != Some(rootfs()?.as_path())
        || request.control.file_name() != Some(OsStr::new(&format!(".lurker-vm-{}", request.token)))
        || request.uid != crate::effective_uid()
        || request.gid != unsafe { libc::getegid() }
    {
        return Err(AppError::new("VM journal identity or protocol changed."));
    }
    validate_image(&request)?;
    Ok(request)
}

fn process(pid: u32) -> AppResult<Option<Process>> {
    let output = super::execute(
        &system_tool("/bin/ps")?,
        &args(&["-p", &pid.to_string(), "-o", "lstart=", "-o", "stat="]),
        None,
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<_> = text.split_whitespace().collect();
    Ok(
        (output.status.success() && fields.len() == 6 && !fields[5].starts_with('Z')).then(|| {
            Process {
                pid,
                start: fields[..5].join(" "),
            }
        }),
    )
}

fn alive(expected: &Process) -> AppResult<bool> {
    match process(expected.pid)? {
        Some(actual) if actual != *expected => Err(AppError::new(
            "VM transport process identity changed; preserving the encrypted session.",
        )),
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

fn process_record(dir: &Path, name: &str, pid: u32) -> AppResult<Process> {
    let identity = process(pid)?.ok_or_else(|| {
        AppError::new("VM transport process exited before its identity was journaled.")
    })?;
    record(dir, name, &identity)?;
    Ok(identity)
}

fn mount_info(target: &Path) -> AppResult<Option<MountInfo>> {
    let mut table: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo supplies initialized entries until the next such call.
    let count = unsafe { libc::getmntinfo(&mut table, libc::MNT_NOWAIT) };
    if count <= 0 || table.is_null() {
        return Err(AppError::io(
            "Read native VM mount table",
            std::io::Error::last_os_error(),
        ));
    }
    let mut found = None;
    for entry in unsafe { std::slice::from_raw_parts(table, count as usize) } {
        let path = Path::new(OsStr::from_bytes(
            unsafe { CStr::from_ptr(entry.f_mntonname.as_ptr()) }.to_bytes(),
        ));
        if path != target {
            continue;
        }
        if found.is_some() {
            return Err(AppError::new(
                "Ambiguous host filesystem mount; preserving the VM.",
            ));
        }
        found = Some(MountInfo {
            source: unsafe { CStr::from_ptr(entry.f_mntfromname.as_ptr()) }
                .to_str()
                .map_err(|_| AppError::new("Invalid native VM mount source."))?
                .into(),
            filesystem: unsafe { CStr::from_ptr(entry.f_fstypename.as_ptr()) }
                .to_str()
                .map_err(|_| AppError::new("Invalid native VM filesystem type."))?
                .into(),
            dev: fs::metadata(target)
                .map_err(|error| AppError::io("Inspect VM mount identity", error))?
                .dev(),
            readonly: entry.f_flags & libc::MNT_RDONLY as u32 != 0,
        });
    }
    Ok(found)
}

fn fsname(request: &Request) -> String {
    format!("lurker-vm-{}", request.token)
}

fn validate_mount_shape(request: &Request, mount: &MountInfo) -> AppResult<()> {
    let name = fsname(request);
    // Modern SSHFS publishes the remote path even when fsname was supplied.
    // That exact path also contains this session's random identity.
    let remote = format!(
        "root@127.0.0.1:/run/lurker-vm-{}/export/data",
        request.token
    );
    let exact_source = mount.source == remote
        || mount.source == name
        || mount
            .source
            .strip_prefix(&name)
            .and_then(|suffix| suffix.strip_prefix("@macfuse"))
            .is_some_and(|unit| !unit.is_empty() && unit.bytes().all(|byte| byte.is_ascii_digit()));
    if mount.filesystem != "macfuse" || !exact_source || mount.readonly != request.readonly {
        return Err(AppError::new(
            "The VM did not publish its identified macFUSE mount with the expected access capability; preserving the session.",
        ));
    }
    Ok(())
}

pub(super) fn verify_mount(dir: &Path, mountpoint: &Path, readonly: bool) -> AppResult<()> {
    login_user()?;
    let dir = directory(dir, true)?;
    let request = request(&dir)?;
    let target =
        fs::canonicalize(mountpoint).map_err(|error| AppError::io("Resolve VM mount", error))?;
    if request.mountpoint.as_ref() != Some(&target) || request.readonly != readonly {
        return Err(AppError::new("VM mount request changed."));
    }
    let ready: Ready = read_record(&dir, "vm-ready.json")?
        .ok_or_else(|| AppError::new("VM mount readiness was not journaled."))?;
    if ready.protocol != PROTOCOL || ready.token != request.token || ready.mountpoint != target {
        return Err(AppError::new("VM mount readiness identity changed."));
    }
    let actual = mount_info(&target)?
        .ok_or_else(|| AppError::new("The recorded VM filesystem is no longer mounted."))?;
    validate_mount_shape(&request, &actual)?;
    if actual != ready.mount {
        return Err(AppError::new(
            "The recorded VM filesystem mount was replaced.",
        ));
    }
    for name in ["vm-supervisor.json", "vm-child.json", "vm-sshfs.json"] {
        let process: Process = read_record(&dir, name)?
            .ok_or_else(|| AppError::new("Missing VM process ownership record."))?;
        if !alive(&process)? {
            return Err(AppError::new(
                "A VM transport process has stopped; preserving the session.",
            ));
        }
    }
    Ok(())
}

fn make_request(
    dir: &Path,
    image: &Path,
    device: &Path,
    mountpoint: Option<&Path>,
    readonly: bool,
    uid: u32,
    gid: u32,
) -> AppResult<Request> {
    available()?;
    if uid != crate::effective_uid() || gid != unsafe { libc::getegid() } {
        return Err(AppError::new(
            "New Btrfs ownership must match the invoking login user.",
        ));
    }
    if dir.as_os_str().as_bytes().contains(&b',') {
        return Err(AppError::new(
            "VM session paths containing commas are unsupported by SSHFS options.",
        ));
    }
    if dir.join("vm-request.json").exists() {
        return Err(AppError::new(
            "A VM request already owns this filesystem session.",
        ));
    }
    let mut random = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|error| AppError::io("Create VM session identity", error))?;
    let token = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let control = rootfs()?.join(format!(".lurker-vm-{token}"));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&control)
        .map_err(|error| AppError::io("Reserve private guest control directory", error))?;
    write_flag(&control.join("visible"), &token)?;
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| AppError::io("Reserve private SFTP port", error))?;
    let port = listener
        .local_addr()
        .map_err(|error| AppError::io("Read private SFTP port", error))?
        .port();
    let image = image_identity(image, readonly)?;
    let device = device_identity(device, &image, readonly)?;
    let request = Request {
        protocol: PROTOCOL,
        token,
        image,
        device,
        mountpoint: mountpoint.map(|path| directory(path, true)).transpose()?,
        readonly,
        uid,
        gid,
        port,
        control,
    };
    if let Some(target) = &request.mountpoint {
        if mount_info(target)?.is_some() {
            return Err(AppError::new("Refusing to replace an existing host mount."));
        }
    }
    // The plaintext identity and normal-stop control are durable before any
    // executable capable of starting a VM is launched.
    record(dir, "vm-request.json", &request)?;
    Ok(request)
}

fn detached(command: &mut Command) {
    command
        .stdin(Stdio::null())
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env_remove("SSH_AUTH_SOCK")
        .env_remove("SSH_AGENT_PID")
        .env_remove("DYLD_INSERT_LIBRARIES");
    // SAFETY: the child hook invokes only the async-signal-safe setsid syscall.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

fn private_log(dir: &Path, name: &str) -> AppResult<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(dir.join(name))
        .map_err(|error| AppError::io("Create private VM log", error))
}

fn launch(dir: &Path) -> AppResult<Child> {
    let log = private_log(dir, "vm-supervisor.log")?;
    let mut command = Command::new(
        std::env::current_exe().map_err(|error| AppError::io("Locate VM supervisor", error))?,
    );
    command
        .arg(DISPATCH)
        .arg(dir)
        .stdout(Stdio::from(log.try_clone().map_err(|error| {
            AppError::io("Clone VM supervisor log", error)
        })?))
        .stderr(Stdio::from(log));
    detached(&mut command);
    command
        .spawn()
        .map_err(|error| AppError::io("Start VM supervisor; retain the plaintext transport", error))
}

pub(super) fn start(
    dir: &Path,
    image: &Path,
    device: &Path,
    mountpoint: &Path,
    readonly: bool,
) -> AppResult<u32> {
    let dir = directory(dir, true)?;
    let request = make_request(
        &dir,
        image,
        device,
        Some(mountpoint),
        readonly,
        crate::effective_uid(),
        unsafe { libc::getegid() },
    )?;
    let mut supervisor = launch(&dir)?;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if read_record::<Ready>(&dir, "vm-ready.json")?.is_some() {
            verify_mount(&dir, request.mountpoint.as_ref().unwrap(), readonly)?;
            return Ok(supervisor.id());
        }
        if let Some(result) = read_record::<ResultRecord>(&dir, "vm-result.json")? {
            return Err(AppError::new(result.diagnostic.unwrap_or_else(|| {
                "The VM closed before its host mount was ready.".into()
            })));
        }
        if supervisor
            .try_wait()
            .map_err(|error| AppError::io("Inspect VM supervisor", error))?
            .is_some()
        {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(AppError::new(
        "VM startup did not complete. Its journal and plaintext transport are retained; retry normal close to recover.",
    ))
}

pub(super) fn format(dir: &Path, image: &Path, device: &Path, uid: u32, gid: u32) -> AppResult<()> {
    let dir = directory(dir, true)?;
    let request = make_request(&dir, image, device, None, false, uid, gid)?;
    let mut supervisor = launch(&dir)?;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(result) = read_record::<ResultRecord>(&dir, "vm-result.json")? {
            if result.protocol == PROTOCOL
                && result.token == request.token
                && result.clean_closed
                && result.vm_exit == Some(0)
                && result.diagnostic.is_none()
            {
                return Ok(());
            }
            return Err(AppError::new(result.diagnostic.unwrap_or_else(|| "Btrfs formatting did not confirm a normal VM close; preserve the plaintext transport.".into())));
        }
        if supervisor
            .try_wait()
            .map_err(|error| AppError::io("Inspect formatting supervisor", error))?
            .is_some()
        {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(AppError::new(
        "Btrfs format is incomplete. Its VM ownership journal is retained for normal close.",
    ))
}

pub(super) fn request_close(dir: &Path) -> AppResult<()> {
    login_user()?;
    let dir = directory(dir, true)?;
    let request = request(&dir)?;
    if let Some(target) = &request.mountpoint {
        if mount_info(target)?.is_some() {
            return Err(AppError::new(
                "The host filesystem remains mounted; preserving its guest and crypto transport.",
            ));
        }
    }
    let deadline = Instant::now() + CLOSE_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(result) = read_record::<ResultRecord>(&dir, "vm-result.json")? {
            if result.protocol != PROTOCOL || result.token != request.token || !result.clean_closed
            {
                return Err(AppError::new(
                    "The VM did not confirm a normal filesystem close; preserving the crypto transport.",
                ));
            }
            for name in ["vm-child.json", "vm-sshfs.json"] {
                if let Some(process) = read_record::<Process>(&dir, name)? {
                    if alive(&process)? {
                        return Err(AppError::new(
                            "A recorded VM transport process is still alive.",
                        ));
                    }
                }
            }
            validate_image(&request)?;
            // The caller normally detaches the verified native image after
            // this acknowledgement, then syncs the regular crypto view.
            cleanup_control(&request)?;
            return Ok(());
        }
        if let Some(target) = &request.mountpoint {
            if mount_info(target)?.is_some() {
                return Err(AppError::new(
                    "A host filesystem reappeared during VM close; preserving the guest.",
                ));
            }
        }
        let client_gone = match read_record::<Process>(&dir, "vm-sshfs.json")? {
            Some(client) => !alive(&client)?,
            None => true,
        };
        if client_gone {
            directory(&request.control, true)?;
            write_flag(&request.control.join("stop"), &request.token)?;
        }
        if let Some(owner) = read_record::<Process>(&dir, "vm-supervisor.json")? {
            if !alive(&owner)? {
                return Err(AppError::new(
                    "The VM supervisor stopped without a verified close result; retain the crypto transport and private recovery journal.",
                ));
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(AppError::new(
        "The guest is busy or has not confirmed normal close. Its VM and plaintext transport remain retained.",
    ))
}

fn cleanup_control(request: &Request) -> AppResult<()> {
    if !request
        .control
        .try_exists()
        .map_err(|error| AppError::io("Inspect closed guest control directory", error))?
    {
        return Ok(());
    }
    directory(&request.control, true)?;
    for entry in fs::read_dir(&request.control)
        .map_err(|error| AppError::io("Inspect closed guest control", error))?
    {
        let path = entry
            .map_err(|error| AppError::io("Inspect closed guest control entry", error))?
            .path();
        if !matches!(
            path.file_name().and_then(OsStr::to_str),
            Some("visible" | "launch" | "stop")
        ) {
            return Err(AppError::new(
                "Unexpected guest control entry; retain its private recovery directory.",
            ));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| AppError::io("Inspect closed guest control flag", error))?;
        if !metadata.is_file()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o077 != 0
        {
            return Err(AppError::new("Closed guest control flag identity changed."));
        }
        fs::remove_file(path)
            .map_err(|error| AppError::io("Remove closed guest control flag", error))?;
    }
    fs::remove_dir(&request.control)
        .map_err(|error| AppError::io("Remove closed guest control directory", error))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        result.push(ALPHABET[(a >> 2) as usize] as char);
        result.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        result.push(if chunk.len() > 1 {
            ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ALPHABET[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

fn guest_program(request: &Request, public_key: &str) -> String {
    let mut program = GUEST.to_owned();
    for (name, value) in [
        ("__TOKEN__", shell_quote(&request.token)),
        (
            "__CONTROL__",
            shell_quote(&format!("/.lurker-vm-{}", request.token)),
        ),
        (
            "__MODE__",
            shell_quote(if request.mountpoint.is_some() {
                "mount"
            } else {
                "format"
            }),
        ),
        (
            "__READONLY__",
            if request.readonly {
                "1".into()
            } else {
                "0".into()
            },
        ),
        ("__SIZE__", request.image.len.to_string()),
        ("__UID__", request.uid.to_string()),
        ("__GID__", request.gid.to_string()),
        ("__PORT__", request.port.to_string()),
        ("__PUBLIC_KEY__", shell_quote(public_key)),
    ] {
        program = program.replace(name, &value);
    }
    format!(
        "printf %s {} | base64 -d | /bin/sh",
        shell_quote(&base64(program.as_bytes()))
    )
}

fn collect_guest(
    mut input: UnixStream,
    mut log: File,
    token: String,
    state: Arc<Mutex<GuestState>>,
) {
    let mut buffer = [0u8; 8192];
    let mut line = Vec::new();
    let mut logged = 0;
    loop {
        let count = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let keep = count.min(LOG_LIMIT - logged);
        if keep != 0 {
            let _ = log.write_all(&buffer[..keep]);
            logged += keep;
        }
        for &byte in &buffer[..count] {
            if byte == b'\n' {
                if let Ok(text) = std::str::from_utf8(&line) {
                    let text = text.trim_end_matches('\r');
                    if let Ok(mut state) = state.lock() {
                        if text == format!("__LURKER_VISIBLE__ {token}") {
                            state.visible = true;
                        }
                        if text == format!("__LURKER_CLOSED__ {token}") {
                            state.closed = true;
                        }
                        if let Some(key) =
                            text.strip_prefix(&format!("__LURKER_HOSTKEY__ {token} "))
                        {
                            let fields: Vec<_> = key.split_whitespace().collect();
                            if fields.len() >= 2
                                && fields[0] == "ssh-ed25519"
                                && fields[1].len() <= 128
                                && fields[1].bytes().all(|b| {
                                    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')
                                })
                            {
                                let key = format!("{} {}", fields[0], fields[1]);
                                if state.host_key.as_ref().is_none_or(|old| old == &key) {
                                    state.host_key = Some(key);
                                }
                            }
                        }
                    }
                }
                line.clear();
            } else if line.len() < 8192 {
                line.push(byte);
            }
        }
    }
    let _ = log.sync_all();
}

fn lease() -> AppResult<File> {
    let parent = Path::new("/tmp").join(format!("lurker-macos-vm-{}", crate::effective_uid()));
    match fs::DirBuilder::new().mode(0o700).create(&parent) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(AppError::io("Create per-user VM lease", error)),
    }
    directory(&parent, true)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent.join("lease"))
        .map_err(|error| AppError::io("Open per-user VM lease", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| AppError::io("Inspect per-user VM lease", error))?;
    if !metadata.is_file()
        || metadata.uid() != crate::effective_uid()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(AppError::new("Unsafe per-user VM lease."));
    }
    // SAFETY: the descriptor is retained for the entire VM supervisor lifetime.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(AppError::new(
            "Another Btrfs VM session is active for this user. Close it before starting another.",
        ));
    }
    Ok(file)
}

fn public_key(dir: &Path) -> AppResult<String> {
    let key = dir.join("vm-client");
    checked(
        &system_tool("/usr/bin/ssh-keygen")?,
        &[
            "-q".into(),
            "-t".into(),
            "ed25519".into(),
            "-N".into(),
            "".into(),
            "-f".into(),
            key.as_os_str().into(),
        ],
        None,
    )?;
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600))
        .map_err(|error| AppError::io("Protect private VM client key", error))?;
    fs::read_to_string(dir.join("vm-client.pub"))
        .map(|value| value.trim_end().to_owned())
        .map_err(|error| AppError::io("Read VM public client key", error))
}

fn authenticate(dir: &Path, request: &Request, host_key: &str) -> AppResult<()> {
    let known_hosts = dir.join("vm-known-hosts");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&known_hosts)
        .map_err(|error| AppError::io("Pin private guest host key", error))?;
    writeln!(file, "[127.0.0.1]:{} {host_key}", request.port)
        .and_then(|_| file.sync_all())
        .map_err(|error| AppError::io("Sync pinned guest host key", error))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let output = super::execute(
            &system_tool("/usr/bin/sftp")?,
            &[
                "-F".into(),
                "/dev/null".into(),
                "-P".into(),
                request.port.to_string().into(),
                "-o".into(),
                "IdentitiesOnly=yes".into(),
                "-o".into(),
                "BatchMode=yes".into(),
                "-o".into(),
                "IdentityAgent=none".into(),
                "-o".into(),
                "StrictHostKeyChecking=yes".into(),
                "-o".into(),
                format!("UserKnownHostsFile={}", known_hosts.display()).into(),
                "-o".into(),
                "GlobalKnownHostsFile=/dev/null".into(),
                "-o".into(),
                "ConnectTimeout=3".into(),
                "-i".into(),
                dir.join("vm-client").into_os_string(),
                "-b".into(),
                "-".into(),
                "root@127.0.0.1".into(),
            ],
            Some(b"pwd\nbye\n"),
        )?;
        if output.status.success() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(AppError::new(
                "Pinned-key guest SFTP authentication did not become ready. Inspect the private VM log; no unauthenticated fallback is used.",
            ));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn spawn_sshfs(dir: &Path, request: &Request) -> AppResult<Child> {
    let mut options = format!(
        "idmap=user,IdentityFile={},IdentitiesOnly=yes,BatchMode=yes,StrictHostKeyChecking=yes,UserKnownHostsFile={},GlobalKnownHostsFile=/dev/null,ServerAliveInterval=5,ServerAliveCountMax=3,fsname={}",
        dir.join("vm-client").display(),
        dir.join("vm-known-hosts").display(),
        fsname(request)
    );
    if request.readonly {
        options.push_str(",ro");
    }
    let log = private_log(dir, "vm-sshfs.log")?;
    let mut command = Command::new(runtime_tool("sshfs")?);
    command
        .args([
            OsString::from("-f"),
            "-F".into(),
            "/dev/null".into(),
            "-p".into(),
            request.port.to_string().into(),
            "-o".into(),
            options.into(),
            format!(
                "root@127.0.0.1:/run/lurker-vm-{}/export/data",
                request.token
            )
            .into(),
            request.mountpoint.as_ref().unwrap().as_os_str().into(),
        ])
        .stdout(Stdio::from(
            log.try_clone()
                .map_err(|error| AppError::io("Clone SSHFS log", error))?,
        ))
        .stderr(Stdio::from(log));
    detached(&mut command);
    command
        .spawn()
        .map_err(|error| AppError::io("Start authenticated macFUSE SSHFS", error))
}

fn supervise(dir: &Path, request: &Request) -> AppResult<()> {
    let _lease = lease()?;
    process_record(dir, "vm-supervisor.json", std::process::id())?;
    let key = if request.mountpoint.is_some() {
        public_key(dir)?
    } else {
        String::new()
    };
    let state = Arc::new(Mutex::new(GuestState::default()));
    let log = private_log(dir, "vm-guest.log")?;
    let (reader, writer) =
        UnixStream::pair().map_err(|error| AppError::io("Create owned VM output pipe", error))?;
    let output = writer
        .try_clone()
        .map_err(|error| AppError::io("Clone VM output pipe", error))?;
    let drain_state = state.clone();
    let token = request.token.clone();
    let mut drain = Some(thread::spawn(move || {
        collect_guest(reader, log, token, drain_state)
    }));
    let mut command = Command::new(runtime_tool("anylinuxfs")?);
    command
        .arg("shell")
        .arg(&request.device.block.path)
        .arg("--skip-init")
        .arg("--command")
        .arg(guest_program(request, &key))
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::from(OwnedFd::from(output)));
    detached(&mut command);
    // Keep the same locked open-file description in the VMM as well. An
    // interrupted supervisor must not release the one-VM lease while its VM
    // still owns the plaintext image.
    let lease_fd = _lease.as_raw_fd();
    unsafe {
        command.pre_exec(move || {
            let flags = libc::fcntl(lease_fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(lease_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    validate_image(request)?;
    record(dir, "vm-launching.json", &request.token)?;
    let mut vm = match command.spawn() {
        Ok(vm) => vm,
        Err(error) => {
            let error = AppError::io("Start private Btrfs VM", error);
            record(
                dir,
                "vm-result.json",
                &ResultRecord {
                    protocol: PROTOCOL,
                    token: request.token.clone(),
                    clean_closed: true,
                    vm_exit: None,
                    diagnostic: Some(error.message.clone()),
                },
            )?;
            return Err(error);
        }
    };
    // Command is reusable and retains its configured Stdio descriptors.
    // Release our writer copies now so the guest drain gets EOF at VM exit.
    drop(command);
    let mut sshfs: Option<Child> = None;
    let outcome = (|| -> AppResult<()> {
        // Persist the exact process before the guest's launch gate permits either
        // formatting or mounting the authenticated disk.
        let vm_identity = process_record(dir, "vm-child.json", vm.id())?;
        write_flag(&request.control.join("launch"), &request.token)?;
        let mut failure: Option<String> = None;
        let mut mounted = false;
        let started = Instant::now();
        loop {
            if let Some(status) = vm
                .try_wait()
                .map_err(|error| AppError::io("Wait for owned VM", error))?
            {
                if let Some(drain) = drain.take() {
                    let _ = drain.join();
                }
                let closed = state
                    .lock()
                    .map_err(|_| AppError::new("VM output state is unavailable."))?
                    .closed;
                let host_absent = request
                    .mountpoint
                    .as_ref()
                    .map(|target| mount_info(target).map(|mount| mount.is_none()))
                    .transpose()?
                    .unwrap_or(true);
                let sshfs_gone = match sshfs.as_mut() {
                    Some(child) => child
                        .try_wait()
                        .map_err(|error| AppError::io("Wait for SSHFS close", error))?
                        .is_some(),
                    None => true,
                };
                let mut clean = closed && host_absent && sshfs_gone;
                if clean {
                    if let Err(error) = validate_image(request) {
                        clean = false;
                        failure = Some(error.message);
                    }
                }
                if !status.success() && failure.is_none() {
                    failure = Some(format!(
                        "Btrfs VM exited with {status}; inspect vm-guest.log."
                    ));
                }
                record(
                    dir,
                    "vm-result.json",
                    &ResultRecord {
                        protocol: PROTOCOL,
                        token: request.token.clone(),
                        clean_closed: clean,
                        vm_exit: status.code(),
                        diagnostic: failure,
                    },
                )?;
                return if clean {
                    Ok(())
                } else {
                    Err(AppError::new(
                        "The VM exited without a verified normal close. Preserve the plaintext transport.",
                    ))
                };
            }
            if let Some(target) = &request.mountpoint {
                if !mounted && failure.is_none() {
                    let host_key = {
                        let state = state
                            .lock()
                            .map_err(|_| AppError::new("VM output state is unavailable."))?;
                        if state.visible {
                            state.host_key.clone()
                        } else {
                            None
                        }
                    };
                    if let Some(host_key) = host_key {
                        let startup = (|| -> AppResult<()> {
                            authenticate(dir, request, &host_key)?;
                            if mount_info(target)?.is_some() {
                                return Err(AppError::new(
                                    "A host mount appeared before SSHFS launch.",
                                ));
                            }
                            let underlay = fs::metadata(target)
                                .map_err(|error| AppError::io("Inspect VM mount underlay", error))?
                                .dev();
                            let child = spawn_sshfs(dir, request)?;
                            let pid = child.id();
                            sshfs = Some(child);
                            process_record(dir, "vm-sshfs.json", pid)?;
                            let deadline = Instant::now() + Duration::from_secs(45);
                            while Instant::now() < deadline {
                                if let Some(mount) = mount_info(target)? {
                                    validate_mount_shape(request, &mount)?;
                                    if mount.dev == underlay {
                                        return Err(AppError::new(
                                            "SSHFS did not replace the recorded underlay.",
                                        ));
                                    }
                                    record(
                                        dir,
                                        "vm-ready.json",
                                        &Ready {
                                            protocol: PROTOCOL,
                                            token: request.token.clone(),
                                            mountpoint: target.clone(),
                                            mount,
                                        },
                                    )?;
                                    mounted = true;
                                    return Ok(());
                                }
                                if sshfs
                                    .as_mut()
                                    .unwrap()
                                    .try_wait()
                                    .map_err(|error| AppError::io("Inspect SSHFS startup", error))?
                                    .is_some()
                                {
                                    return Err(AppError::new(
                                        "Modern SSHFS exited before its mount was ready. Inspect vm-sshfs.log.",
                                    ));
                                }
                                if !alive(&vm_identity)? {
                                    return Err(AppError::new(
                                        "The VM exited during SSHFS startup.",
                                    ));
                                }
                                thread::sleep(Duration::from_millis(100));
                            }
                            Err(AppError::new(
                                "Authenticated SSHFS did not publish its mount within 45 seconds.",
                            ))
                        })();
                        if let Err(error) = startup {
                            failure = Some(error.message);
                        }
                    } else if started.elapsed() >= START_TIMEOUT {
                        failure = Some("The guest did not publish its nonce-bound host key. Inspect vm-guest.log and install guest OpenSSH and Btrfs tools.".into());
                    }
                }
                let host_absent = mount_info(target)?.is_none();
                if (mounted || failure.is_some()) && host_absent {
                    let client_gone = match sshfs.as_mut() {
                        Some(child) => child
                            .try_wait()
                            .map_err(|error| AppError::io("Inspect drained SSHFS", error))?
                            .is_some(),
                        None => true,
                    };
                    if client_gone {
                        write_flag(&request.control.join("stop"), &request.token)?;
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    })();
    if let Err(error) = outcome {
        // Once a VM exists, an incidental journal/inspection error must not
        // release its lease or break its console pipe. Retain both until an
        // observed normal guest close (or an abnormal actual VM exit).
        let status = loop {
            if let Ok(Some(status)) = vm.try_wait() {
                break status;
            }
            let absent = request
                .mountpoint
                .as_ref()
                .is_none_or(|target| mount_info(target).is_ok_and(|mount| mount.is_none()));
            let client_gone = sshfs
                .as_mut()
                .is_none_or(|client| client.try_wait().is_ok_and(|status| status.is_some()));
            if absent && client_gone {
                let _ = write_flag(&request.control.join("stop"), &request.token);
            }
            thread::sleep(Duration::from_millis(200));
        };
        if let Some(drain) = drain.take() {
            let _ = drain.join();
        }
        if read_record::<ResultRecord>(dir, "vm-result.json")?.is_none() {
            let absent = request
                .mountpoint
                .as_ref()
                .is_none_or(|target| mount_info(target).is_ok_and(|mount| mount.is_none()));
            let client_gone = sshfs
                .as_mut()
                .is_none_or(|client| client.try_wait().is_ok_and(|status| status.is_some()));
            let closed = state.lock().is_ok_and(|state| state.closed);
            let image_valid = validate_image(request).is_ok();
            record(
                dir,
                "vm-result.json",
                &ResultRecord {
                    protocol: PROTOCOL,
                    token: request.token.clone(),
                    clean_closed: closed && absent && client_gone && image_valid,
                    vm_exit: status.code(),
                    diagnostic: Some(error.message.clone()),
                },
            )?;
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn dispatch() -> Option<i32> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(OsStr::new(DISPATCH)) {
        return None;
    }
    let Some(dir) = arguments.next() else {
        return Some(64);
    };
    if arguments.next().is_some() {
        return Some(64);
    }
    let run = (|| -> AppResult<()> {
        login_user()?;
        let dir = directory(Path::new(&dir), true)?;
        let request = request(&dir)?;
        match supervise(&dir, &request) {
            Ok(()) => Ok(()),
            Err(error) => {
                // Never invent a clean close on an error path. If a VM may
                // exist, its owner-only stop flag and journal stay available.
                if read_record::<ResultRecord>(&dir, "vm-result.json")?.is_none() {
                    record(
                        &dir,
                        "vm-result.json",
                        &ResultRecord {
                            protocol: PROTOCOL,
                            token: request.token.clone(),
                            clean_closed: read_record::<String>(&dir, "vm-launching.json")?
                                .is_none(),
                            vm_exit: None,
                            diagnostic: Some(error.message.clone()),
                        },
                    )?;
                }
                Err(error)
            }
        }
    })();
    Some(match run {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", error.message);
            error.exit_code
        }
    })
}
