//! Shared, login-user directory encryption lifecycle. Crypto remains upstream.
//! Native mount identities and process birth identities prevent stale records
//! from authorizing detach or termination of unrelated resources.
use crate::{
    ActiveVolume, AppError, AppResult, CommandAction, CreateCipher, CreateCommand, MountCommand,
    Output, PrivilegeMode, ToolStatus, UnmountCommand, VolumeType,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "folder_supervisor.rs"]
mod supervisor;
#[path = "folder/fence.rs"]
mod fence;

pub(crate) fn run_supervisor_if_requested() -> Option<i32> {
    supervisor::dispatch()
}

const WAIT: Duration = Duration::from_secs(60);
const SCHEMA: u32 = 1;

pub(crate) fn is_folder(kind: VolumeType) -> bool {
    matches!(
        kind,
        VolumeType::Gocryptfs | VolumeType::Cryfs | VolumeType::Cryptomator
    )
}

pub(crate) fn requires_fence_protocol(kind: VolumeType, macos: bool) -> bool {
    macos && matches!(kind, VolumeType::Auto | VolumeType::Cryptomator)
}

pub(crate) fn handles(command: &CommandAction) -> AppResult<bool> {
    let kind = command.volume_type();
    if is_folder(kind) {
        return Ok(true);
    }
    if kind != VolumeType::Auto {
        return Ok(false);
    }
    match command {
        CommandAction::Mount(c) if c.source.is_dir() => Ok(is_folder(detect_type(&c.source)?)),
        CommandAction::Unmount(c) => {
            let sessions = session_snapshot(&runtime_path())?;
            if sessions.is_empty() {
                return Ok(false);
            }
            let target = match fs::canonicalize(&c.target) {
                Ok(target) => target,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(AppError::io("Cannot resolve unmount target", error)),
            };
            Ok(sessions
                .iter()
                .any(|(_, s)| s.source == target || s.mountpoint == target))
        }
        _ => Ok(false),
    }
}

pub(crate) fn detect_type(path: &Path) -> AppResult<VolumeType> {
    let candidates = [
        (VolumeType::Gocryptfs, "gocryptfs.conf"),
        (VolumeType::Cryfs, "cryfs.config"),
        (VolumeType::Cryptomator, "vault.cryptomator"),
    ];
    let found: Vec<_> = candidates
        .into_iter()
        .filter(|(_, file)| path.join(file).is_file())
        .collect();
    match found.as_slice() {
        [(kind, _)] => Ok(*kind),
        [] => Err(AppError::new(
            "Directory has no recognized encrypted-vault configuration.",
        )),
        _ => Err(AppError::new(
            "Directory contains conflicting encrypted-vault configurations; select --type explicitly.",
        )),
    }
}

fn supported(kind: VolumeType) -> bool {
    match kind {
        VolumeType::Gocryptfs => cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd"
        )),
        VolumeType::Cryfs | VolumeType::Cryptomator => {
            cfg!(any(target_os = "linux", target_os = "macos"))
        }
        _ => false,
    }
}

fn program_name(kind: VolumeType) -> AppResult<&'static str> {
    match kind {
        VolumeType::Gocryptfs => Ok("gocryptfs"),
        VolumeType::Cryfs => Ok("cryfs"),
        VolumeType::Cryptomator => Ok("lurker-cryptomator"),
        _ => Err(AppError::new("Not a directory encryption backend.")),
    }
}

fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0)
}

fn find_program(kind: VolumeType) -> AppResult<PathBuf> {
    let name = program_name(kind)?;
    let exe = std::env::current_exe().ok();
    let executable_dir = exe.as_deref().and_then(Path::parent);
    if cfg!(target_os = "macos") {
        if let Some(contents) = executable_dir
            .filter(|dir| dir.ends_with("Contents/MacOS"))
            .and_then(Path::parent)
        {
            return fs::canonicalize(contents)
                .ok()
                .map(|path| path.join("Helpers").join(name))
                .filter(|path| fs::canonicalize(path).is_ok_and(|resolved| resolved == *path) && executable(path))
                .ok_or_else(|| AppError::new(format!("Bundled {name} is missing from Lurker.app.")));
        }
    }
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("LURKER_BACKEND_DIR") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(AppError::new("LURKER_BACKEND_DIR must be absolute."));
        }
        dirs.push(path);
    }
    if let Some(parent) = executable_dir {
        dirs.extend([
            parent.join("backends"),
            parent.join("../Helpers"),
            parent.join("../libexec/lurker/backends"),
        ]);
    }
    if !cfg!(target_os = "macos") {
        dirs.extend([
            PathBuf::from("/usr/libexec/lurker/backends"),
            PathBuf::from("/usr/local/libexec/lurker/backends"),
        ]);
    }
    dirs.into_iter().flat_map(|dir| [dir.join(name), dir.join("cryptomator").join(name)]).find(|p| executable(p))
        .and_then(|p| fs::canonicalize(p).ok())
        .ok_or_else(|| AppError::new(format!("Bundled {name} is missing. Build/install the selected Lurker backend; development builds may set LURKER_BACKEND_DIR.")))
}

pub(crate) fn tool_statuses() -> Vec<ToolStatus> {
    [
        VolumeType::Gocryptfs,
        VolumeType::Cryfs,
        VolumeType::Cryptomator,
    ]
    .into_iter()
    .filter(|k| supported(*k))
    .map(|kind| ToolStatus {
        name: program_name(kind).unwrap().into(),
        path: find_program(kind).ok(),
        required: false,
    })
    .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
}
impl Identity {
    fn read(path: &Path) -> AppResult<Self> {
        Self::read_io(path).map_err(|e| AppError::io("Cannot inspect directory identity", e))
    }
    fn read_io(path: &Path) -> std::io::Result<Self> {
        let m = fs::metadata(path)?;
        Ok(Self {
            device: m.dev(),
            inode: m.ino(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Process {
    pid: u32,
    birth: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Session {
    schema: u32,
    backend: VolumeType,
    source: PathBuf,
    source_identity: Identity,
    #[cfg(target_os = "freebsd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boot_id: Option<[u8; 16]>,
    mountpoint: PathBuf,
    readonly: bool,
    mount_identity: Option<String>,
    mounted_directory: Option<Identity>,
    process: Option<Process>,
    #[serde(default)]
    supervisor: Option<Process>,
    #[serde(default)]
    fence: Option<fence::Journal>,
}

fn secure_dir(path: &Path) -> AppResult<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(AppError::io("Cannot create private folder state", e)),
    }
    let m = fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Cannot inspect private folder state", e))?;
    validate_private_directory(&m)
}

fn validate_private_directory(m: &fs::Metadata) -> AppResult<()> {
    if !m.is_dir() || m.uid() != crate::effective_uid() || m.mode() & 0o077 != 0 {
        return Err(AppError::new(
            "Folder state must be a private directory owned by the login user.",
        ));
    }
    Ok(())
}

fn runtime_path() -> PathBuf {
    // /tmp is a persistent location across CLI invocations on the supported OSs.
    // Boot cleanup may discard records; existing mounts then require explicit
    // native recovery rather than guessing ownership.
    Path::new("/tmp").join(format!("lurker-folders-{}", crate::effective_uid()))
}
struct Runtime {
    path: PathBuf,
    _lock: File,
}
impl Runtime {
    fn lock() -> AppResult<Self> {
        let path = runtime_path();
        secure_dir(&path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.join("lock"))
            .map_err(|e| AppError::io("Cannot open folder operation lock", e))?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(AppError::new(
                "Another directory encryption operation is in progress.",
            ));
        }
        Ok(Self { path, _lock: lock })
    }
    fn sessions(&self) -> AppResult<Vec<(PathBuf, Session)>> {
        session_snapshot(&self.path)
    }
}

// Records are published atomically. Read-only discovery must neither acquire
// the mutation lock nor recreate a directory concurrently removed by cleanup.
fn session_snapshot(path: &Path) -> AppResult<Vec<(PathBuf, Session)>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(AppError::io("Cannot inspect folder state", error)),
    };
    validate_private_directory(&metadata)?;
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(AppError::io("Cannot inspect folder sessions", error)),
    };
    let mut result = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AppError::io("Cannot inspect folder session", error)),
        };
        if !entry.file_name().to_string_lossy().starts_with("session-") {
            continue;
        }
        let directory = entry.path();
        let metadata = match fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AppError::io("Cannot inspect folder session", error)),
        };
        validate_private_directory(&metadata)?;
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(directory.join("session.json"))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AppError::io("Cannot read folder session", error)),
        };
        let metadata = file
            .metadata()
            .map_err(|error| AppError::io("Cannot inspect open folder session", error))?;
        if !metadata.is_file()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() > 1
        {
            return Err(AppError::new(
                "Folder session record must be a private regular file owned by the login user.",
            ));
        }
        if metadata.len() > 65536 {
            return Err(AppError::new(
                "Folder session exceeds the 64 KiB record limit.",
            ));
        }
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::io("Cannot read open folder session", error))?;
        if bytes.len() > 65536 {
            return Err(AppError::new(
                "Folder session exceeds the 64 KiB record limit.",
            ));
        }
        let session: Session = serde_json::from_slice(&bytes).map_err(|_| {
            AppError::new("Malformed folder session; preserve it for manual recovery.")
        })?;
        if session.schema != SCHEMA {
            return Err(AppError::new("Unsupported folder session schema."));
        }
        result.push((directory, session));
    }
    Ok(result)
}

#[cfg(target_os = "freebsd")]
fn freebsd_boot_id() -> AppResult<Option<[u8; 16]>> {
    let mut id = [0u8; 16];
    let mut length = id.len();
    // kern.boot_id is an opaque 16-byte value generated once per kernel boot.
    let result = unsafe {
        libc::sysctlbyname(
            c"kern.boot_id".as_ptr(),
            id.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null(),
            0,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        return Err(AppError::io("Cannot read the FreeBSD kernel boot identity", error));
    }
    if length != id.len() {
        return Err(AppError::new("Unexpected FreeBSD kernel boot identity size."));
    }
    Ok(Some(id))
}

#[cfg(target_os = "freebsd")]
fn from_previous_freebsd_boot(session: &Session) -> AppResult<bool> {
    // Legacy records retain the original device, inode, and process checks.
    let Some(recorded) = session.boot_id else {
        return Ok(false);
    };
    Ok(freebsd_boot_id()?.is_some_and(|current| current != recorded))
}

fn archive_failed_session(runtime: &Path, directory: &Path) -> AppResult<PathBuf> {
    let archive = runtime.join(format!(
        "failed-{}",
        directory.file_name().unwrap().to_string_lossy()
    ));
    fs::rename(directory, &archive)
        .map_err(|error| AppError::io("Cannot preserve failed backend session", error))?;
    sync_dir(runtime)?;
    Ok(archive)
}

#[cfg(target_os = "freebsd")]
fn archive_previous_freebsd_session(
    runtime: &Path,
    directory: &Path,
    session: &Session,
) -> AppResult<Option<PathBuf>> {
    if !from_previous_freebsd_boot(session)? {
        return Ok(None);
    }
    if mount_identity(&session.mountpoint)?.is_some() {
        return Err(AppError::new(
            "A mount occupies the previous boot's mountpoint. Preserving both the mount and the session.",
        ));
    }
    // A previous kernel cannot own a current mount or process. Preserve its
    // journal without using its device identity or issuing any native cleanup.
    archive_failed_session(runtime, directory).map(Some)
}

fn save(dir: &Path, session: &Session) -> AppResult<()> {
    let bytes = serde_json::to_vec(session).map_err(|e| AppError::new(e.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join("record.tmp"))
        .map_err(|e| AppError::io("Cannot save folder session", e))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| AppError::io("Cannot sync folder session", e))?;
    fs::rename(dir.join("record.tmp"), dir.join("session.json"))
        .map_err(|e| AppError::io("Cannot publish folder session", e))?;
    sync_dir(dir)
}
fn sync_dir(dir: &Path) -> AppResult<()> {
    File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| AppError::io("Cannot sync directory", e))
}
fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(target_os = "linux")]
fn linux_stat_process_absent(error: &std::io::Error) -> bool {
    // An opened procfs stat file returns ESRCH if the task is reaped before read.
    error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ESRCH)
}

// Distinguish a confirmed exit from an inspection/permission failure. An
// unreadable process must never authorize recovery of an unreadable mount.
fn process_snapshot(pid: u32) -> AppResult<Option<Process>> {
    #[cfg(target_os = "linux")]
    {
        let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(value) => value,
            Err(error) if linux_stat_process_absent(&error) => return Ok(None),
            Err(error) => return Err(AppError::io("Cannot verify backend process", error)),
        };
        let (_, fields) = stat
            .rsplit_once(") ")
            .ok_or_else(|| AppError::new("Invalid backend process identity."))?;
        let fields: Vec<_> = fields.split_whitespace().collect();
        if matches!(fields.first(), Some(&"Z" | &"X" | &"x")) {
            return Ok(None);
        }
        let birth = fields
            .get(19)
            .ok_or_else(|| AppError::new("Incomplete backend process identity."))?;
        Ok(Some(Process {
            pid,
            birth: birth.to_string(),
        }))
    }
    #[cfg(target_os = "macos")]
    {
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
        let size = std::mem::size_of::<libc::proc_bsdinfo>();
        let result = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size as i32,
            )
        };
        // A disappearing Darwin task can report ESRCH while its unreaped BSD
        // process still answers kill(pid, 0). Capture libproc's errno before
        // another syscall and trust only this explicit native absence result.
        let query_error = if result == 0 {
            std::io::Error::last_os_error().raw_os_error()
        } else {
            None
        };
        if query_error == Some(libc::ESRCH) {
            return Ok(None);
        }
        if result != size as i32 {
            // kill(pid, 0) performs an existence/permission query, no signal.
            if unsafe { libc::kill(pid as i32, 0) } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                return Ok(None);
            }
            return Err(AppError::new(
                "Cannot verify native backend process; preserve its session.",
            ));
        }
        let info = unsafe { info.assume_init() };
        if info.pbi_status == libc::SZOMB {
            return Ok(None);
        }
        Ok(Some(Process {
            pid,
            birth: format!("{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec),
        }))
    }
    #[cfg(target_os = "freebsd")]
    {
        let mut mib = [
            libc::CTL_KERN,
            libc::KERN_PROC,
            libc::KERN_PROC_PID,
            pid as i32,
        ];
        for attempt in 0..3 {
            let mut info = std::mem::MaybeUninit::<libc::kinfo_proc>::uninit();
            let mut size = std::mem::size_of::<libc::kinfo_proc>();
            let result = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as u32,
                    info.as_mut_ptr().cast(),
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            };
            // FreeBSD 15 returns -1/ESRCH for a reaped PID while other
            // supported kernels may return success with no kinfo_proc bytes.
            // Capture errno before another native call can overwrite it.
            let error = if result != 0 {
                std::io::Error::last_os_error().raw_os_error()
            } else {
                None
            };
            if process_query_absent(result, size, error) {
                return Ok(None);
            }
            if result != 0 && error == Some(libc::EINTR) && attempt < 2 {
                continue;
            }
            if result != 0 {
                return Err(AppError::io(
                    "Cannot verify native backend process; preserve its session",
                    std::io::Error::from_raw_os_error(error.unwrap_or(libc::EIO)),
                ));
            }
            if size != std::mem::size_of::<libc::kinfo_proc>() {
                return Err(AppError::new(
                    "Native backend process identity has an unexpected size; preserve its session.",
                ));
            }
            let info = unsafe { info.assume_init() };
            if info.ki_pid != pid as i32 {
                return Err(AppError::new(
                    "Native backend process identity changed unexpectedly.",
                ));
            }
            if info.ki_stat == libc::SZOMB {
                return Ok(None);
            }
            return Ok(Some(Process {
                pid,
                birth: format!("{}:{}", info.ki_start.tv_sec, info.ki_start.tv_usec),
            }));
        }
        unreachable!("bounded process query always returns on its final attempt")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
    {
        let _ = pid;
        Err(AppError::new(
            "No directory backend is selected for this OS.",
        ))
    }
}
#[cfg(any(test, target_os = "freebsd"))]
fn process_query_absent(result: i32, size: usize, error: Option<i32>) -> bool {
    (result == 0 && size == 0) || (result != 0 && error == Some(libc::ESRCH))
}

fn process_identity(pid: u32) -> Option<Process> {
    process_snapshot(pid).ok().flatten()
}
fn alive(process: &Process) -> AppResult<bool> {
    Ok(process_snapshot(process.pid)?.is_some_and(|p| p.birth == process.birth))
}

fn validate_directory_identity(
    expected: &Identity,
    observed: std::io::Result<Identity>,
    process_verified_dead: impl FnOnce() -> AppResult<bool>,
) -> AppResult<()> {
    match observed {
        Ok(identity) if &identity == expected => Ok(()),
        Ok(_) => Err(AppError::new(
            "Mount directory identity changed; refusing to unmount its replacement.",
        )),
        Err(error) if matches!(error.raw_os_error(), Some(libc::EIO | libc::ENOTCONN)) => {
            if process_verified_dead()? {
                Ok(())
            } else {
                Err(AppError::new(
                    "Mount directory is unreadable while its backend is live; preserve the session.",
                ))
            }
        }
        Err(error) => Err(AppError::io("Cannot inspect directory identity", error)),
    }
}

fn confirmed_mount_directory<'a>(session: &'a Session, current: &str) -> AppResult<&'a Identity> {
    if session.mount_identity.as_deref() != Some(current) {
        return Err(AppError::new(
            "Mount identity changed or was never confirmed; refusing to unmount an unverified filesystem.",
        ));
    }
    session.mounted_directory.as_ref().ok_or_else(|| {
        AppError::new("Mount directory identity was never confirmed; preserve the session.")
    })
}

fn unescape(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}
fn mount_identity(path: &Path) -> AppResult<Option<String>> {
    #[cfg(target_os = "linux")]
    {
        let text = fs::read_to_string("/proc/self/mountinfo")
            .map_err(|e| AppError::io("Cannot inspect mounts", e))?;
        for line in text.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() > 6 && Path::new(&unescape(fields[4])) == path {
                return Ok(Some(line.into()));
            }
        }
        Ok(None)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let value = Command::new("/sbin/mount")
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| AppError::io("Cannot inspect native mounts", e))?;
        if !value.status.success() {
            return Err(AppError::new("Native mount inspection failed."));
        }
        let needle = format!(" on {} (", path.display());
        Ok(String::from_utf8_lossy(&value.stdout)
            .lines()
            .find(|l| l.contains(&needle))
            .map(str::to_owned))
    }
}

fn password(value: Option<String>, create: bool) -> AppResult<String> {
    let value = match value {
        Some(v) => v,
        None => {
            let p = crate::credentials::prompt_passphrase("Passphrase: ")
                .map_err(|e| AppError::io("Cannot read passphrase", e))?;
            if create {
                let confirmation = crate::credentials::prompt_passphrase("Confirm passphrase: ")
                    .map_err(|e| AppError::io("Cannot confirm passphrase", e))?;
                if p != confirmation {
                    return Err(AppError::new("Passphrases do not match."));
                }
            }
            p
        }
    };
    crate::credentials::validate(Some(&value), None)?;
    Ok(value)
}

fn backend_command(kind: VolumeType, executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .env_remove("LD_PRELOAD")
        .env_remove("DYLD_INSERT_LIBRARIES")
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if kind == VolumeType::Cryfs {
        command
            .env("CRYFS_FRONTEND", "noninteractive")
            .env("CRYFS_NO_UPDATE_CHECK", "1");
    }
    command
}
fn send(child: &mut Child, secret: &str) -> AppResult<()> {
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| AppError::new("Missing private credential pipe."))?;
    input
        .write_all(secret.as_bytes())
        .and_then(|_| input.write_all(b"\n"))
        .map_err(|e| AppError::io("Backend rejected credential input", e))
}

fn mount_args(
    kind: VolumeType,
    source: &Path,
    mountpoint: &Path,
    readonly: bool,
) -> AppResult<Vec<OsString>> {
    let mut args: Vec<OsString> = match kind {
        VolumeType::Gocryptfs => ["-fg", "-q", "-passfile", "/dev/stdin"]
            .map(OsString::from)
            .into(),
        VolumeType::Cryfs => ["--foreground"].map(OsString::from).into(),
        VolumeType::Cryptomator => ["mount"].map(OsString::from).into(),
        _ => return Err(AppError::new("Unsupported folder backend.")),
    };
    if readonly && kind == VolumeType::Gocryptfs {
        args.push("-ro".into());
    }
    if readonly && kind == VolumeType::Cryfs {
        args.extend(["-o".into(), "ro".into()]);
    }
    if cfg!(target_os = "macos") && kind == VolumeType::Cryfs {
        args.extend(["-o".into(), "backend=fskit".into()]);
    }
    args.extend([source.as_os_str().into(), mountpoint.as_os_str().into()]);
    if readonly && kind == VolumeType::Cryptomator {
        args.push("--readonly".into());
    }
    Ok(args)
}

pub(crate) fn run(command: CommandAction, output: Output) -> AppResult<()> {
    let runtime = user_runtime()?;
    match command {
        CommandAction::Create(c) => create(&runtime, c, output),
        CommandAction::Mount(c) => mount(&runtime, c, output),
        CommandAction::Unmount(c) => unmount(&runtime, c, output, false),
        _ => Err(AppError::new(
            "Directory backends do not support LUKS keyslots.",
        )),
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn unmount_source(command: UnmountCommand, output: Output) -> AppResult<()> {
    unmount(&user_runtime()?, command, output, true)
}

fn user_runtime() -> AppResult<Runtime> {
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "Directory encryption runs as your login user; do not use sudo.",
        ));
    }
    Runtime::lock()
}

fn create(runtime: &Runtime, c: CreateCommand, output: Output) -> AppResult<()> {
    if !supported(c.volume_type) {
        return Err(AppError::new(
            "This directory backend is not selected for this OS.",
        ));
    }
    if c.source_kind != crate::SourceKind::Directory
        || c.force
        || c.size_gb.is_some()
        || c.keyfile.is_some()
        || c.cipher != CreateCipher::Aes
    {
        return Err(AppError::new(
            "Directory create takes no size, force, keyfile or alternate cipher; each backend uses its fixed authenticated format.",
        ));
    }
    let executable = find_program(c.volume_type)?;
    let parent = fs::canonicalize(
        c.target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(|e| AppError::io("Cannot resolve vault parent", e))?;
    let name = c
        .target
        .file_name()
        .ok_or_else(|| AppError::new("Vault needs a directory name."))?;
    let target = parent.join(name);
    match fs::symlink_metadata(&target) {
        Ok(_) => return Err(AppError::new("Vault target must not already exist.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(AppError::io("Cannot inspect vault target", e)),
    }
    let secret = password(c.passphrase, true)?;
    // Atomic reservation refuses existing directories and symlinks. The vault
    // is not recognizable until its configuration is published last.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&target)
        .map_err(|e| AppError::io("Vault target must not already exist", e))?;
    let staging = target.join(".lurker-initialize");
    secure_dir(&staging)?;
    let result = (|| {
        if c.volume_type == VolumeType::Cryfs {
            let mp = target.join(".lurker-initial-mount");
            secure_dir(&mp)?;
            mount_inner(
                runtime,
                MountCommand {
                    peer_sources: Vec::new(),
                    allow_degraded: false,
                    privilege_mode: PrivilegeMode::Unprivileged,
                    source: staging.clone(),
                    mountpoint: mp.clone(),
                    tag: None,
                    volume_type: c.volume_type,
                    passphrase: Some(secret),
                    keyfile: None,
                    readonly: false,
                },
                Output::silent(),
                true,
            )?;
            unmount(
                runtime,
                UnmountCommand {
                    privilege_mode: PrivilegeMode::Unprivileged,
                    target: mp.clone(),
                    tag: None,
                    volume_type: c.volume_type,
                },
                Output::silent(),
                false,
            )?;
            fs::remove_dir(mp).map_err(|e| AppError::io("Cannot remove initial mountpoint", e))?;
        } else {
            let mut command = backend_command(c.volume_type, &executable);
            if c.volume_type == VolumeType::Gocryptfs {
                command.args(["-q", "-init", "-scryptn", "16", "-passfile", "/dev/stdin"]);
            } else {
                command.arg("init");
            }
            command.arg(&staging);
            let mut child = command
                .spawn()
                .map_err(|e| AppError::io("Cannot start vault initializer", e))?;
            send(&mut child, &secret)?;
            let started = Instant::now();
            loop {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|e| AppError::io("Cannot monitor vault initializer", e))?
                {
                    if !status.success() {
                        return Err(AppError::new(format!(
                            "Vault initialization failed ({status}); no usable vault was published."
                        )));
                    }
                    break;
                }
                if started.elapsed() > WAIT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::new("Vault initialization timed out."));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let config = match c.volume_type {
            VolumeType::Gocryptfs => "gocryptfs.conf",
            VolumeType::Cryfs => "cryfs.config",
            _ => "vault.cryptomator",
        };
        if !staging.join(config).is_file() {
            return Err(AppError::new(
                "Backend did not initialize the required vault configuration.",
            ));
        }
        sync_tree(&staging)?;
        for entry in fs::read_dir(&staging)
            .map_err(|e| AppError::io("Cannot inspect initialized vault", e))?
        {
            let entry = entry.map_err(|e| AppError::io("Cannot inspect vault data", e))?;
            if entry.file_name() != config {
                let published = target.join(entry.file_name());
                if let Err(error) = fs::rename(entry.path(), &published) {
                    // Darwin moves AppleDouble metadata with its base entry on
                    // exFAT. The directory iterator can still return the old name.
                    #[cfg(target_os = "macos")]
                    if error.kind() == std::io::ErrorKind::NotFound
                        && crate::macos::apple_double_was_moved(&entry.path(), &published)
                    {
                        continue;
                    }
                    return Err(AppError::io("Cannot publish vault data", error));
                }
            }
        }
        fs::rename(staging.join(config), target.join(config))
            .map_err(|e| AppError::io("Cannot publish vault configuration", e))?;
        fs::remove_dir(&staging).map_err(|e| AppError::io("Cannot finish vault publication", e))?;
        sync_dir(&target)?;
        sync_dir(&parent)?;
        Ok(())
    })();
    if result.is_err() {
        output.warn(&format!("Incomplete initialization retained at {}. Inspect it before retrying; Lurker will not overwrite it.",target.display()));
    }
    result?;
    output.success(&format!(
        "Created {} directory: {}",
        c.volume_type,
        target.display()
    ));
    Ok(())
}

fn sync_tree(path: &Path) -> AppResult<()> {
    for entry in
        fs::read_dir(path).map_err(|e| AppError::io("Cannot inspect initialized data", e))?
    {
        let entry = entry.map_err(|e| AppError::io("Cannot inspect initialized data", e))?;
        if entry
            .file_type()
            .map_err(|e| AppError::io("Cannot inspect initialized data", e))?
            .is_dir()
        {
            sync_tree(&entry.path())?;
        } else {
            File::open(entry.path())
                .and_then(|f| f.sync_all())
                .map_err(|e| AppError::io("Cannot sync initialized data", e))?;
        }
    }
    sync_dir(path)
}

fn mount(runtime: &Runtime, c: MountCommand, output: Output) -> AppResult<()> {
    mount_inner(runtime, c, output, false)
}

fn mount_inner(
    runtime: &Runtime,
    mut c: MountCommand,
    output: Output,
    initializing: bool,
) -> AppResult<()> {
    if c.volume_type == VolumeType::Auto {
        c.volume_type = detect_type(&c.source)?;
    }
    if !supported(c.volume_type) {
        return Err(AppError::new(
            "This directory backend is not selected for this OS.",
        ));
    }
    if c.keyfile.is_some() || c.tag.is_some() {
        return Err(AppError::new(
            "Directory mounts do not accept keyfiles or mapper tags.",
        ));
    }
    let executable = find_program(c.volume_type)?;
    if fence::required(c.volume_type) {
        let capabilities = backend_command(c.volume_type, &executable).arg("--capabilities")
            .stdout(Stdio::piped()).output()
            .map_err(|e| AppError::io("Cannot inspect Cryptomator fencing capability", e))?;
        let supported = capabilities.status.success() && serde_json::from_slice::<serde_json::Value>(&capabilities.stdout)
            .ok().and_then(|v| v.get("fenced_mountpoint_protocol").and_then(|n| n.as_u64())) == Some(1);
        if !supported { return Err(AppError::new("The private Cryptomator backend must support mountpoint fencing protocol 1; update it before unlocking.")); }
    }
    let secret = password(c.passphrase, false)?;
    let source = fs::canonicalize(&c.source)
        .map_err(|e| AppError::io("Cannot resolve encrypted directory", e))?;
    if !source.is_dir() {
        return Err(AppError::new(
            "Directory backend needs an encrypted directory.",
        ));
    }
    if !initializing && detect_type(&source)? != c.volume_type {
        return Err(AppError::new(
            "Vault configuration does not match the selected directory backend.",
        ));
    }
    let source_identity = Identity::read(&source)?;
    let sessions = runtime.sessions()?;
    for (_, s) in &sessions {
        if s.source_identity == source_identity
            || (cfg!(target_os = "freebsd") && s.source == source)
        {
            return Err(AppError::new(
                "Vault already has an active or recoverable session; close it before reopening.",
            ));
        }
    }
    if !c.mountpoint.exists() {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&c.mountpoint)
            .map_err(|e| AppError::io("Cannot create mountpoint", e))?;
    }
    let mountpoint = fs::canonicalize(&c.mountpoint)
        .map_err(|e| AppError::io("Cannot resolve mountpoint", e))?;
    if cfg!(target_os = "freebsd") && sessions.iter().any(|(_, s)| s.mountpoint == mountpoint) {
        return Err(AppError::new(
            "Mountpoint has an active or recoverable session; close it before reusing it.",
        ));
    }
    if source.starts_with(&mountpoint) || mountpoint.starts_with(&source) {
        return Err(AppError::new(
            "Encrypted directory and mountpoint must not contain one another.",
        ));
    }
    if mount_identity(&mountpoint)?.is_some()
        || fs::read_dir(&mountpoint)
            .map_err(|e| AppError::io("Cannot inspect mountpoint", e))?
            .next()
            .is_some()
    {
        return Err(AppError::new(
            "Mountpoint must be an empty, unmounted directory.",
        ));
    }
    #[cfg(target_os = "freebsd")]
    let boot_id = freebsd_boot_id()?;
    let dir = runtime
        .path
        .join(format!("session-{}-{}", std::process::id(), nonce()));
    secure_dir(&dir)?;
    let held_fence = if fence::required(c.volume_type) { Some(fence::prepare(&mountpoint)?) } else { None };
    let mut session = Session {
        schema: SCHEMA,
        backend: c.volume_type,
        source,
        source_identity,
        #[cfg(target_os = "freebsd")]
        boot_id,
        mountpoint,
        readonly: c.readonly,
        mount_identity: None,
        mounted_directory: None,
        process: None,
        supervisor: None,
        fence: held_fence.as_ref().map(|(journal, _)| journal.clone()),
    };
    save(&dir, &session)?;
    if let Some((_, file)) = held_fence.as_ref() {
        fence::arm(&session.mountpoint, file, session.fence.as_mut().unwrap())?;
        save(&dir, &session)?;
    }
    let mut command = Command::new(std::env::current_exe()
        .map_err(|e| AppError::io("Cannot locate folder supervisor", e))?);
    command.arg("--lurker-folder-supervisor").arg(&dir).arg(&executable)
        .arg(if initializing { "init" } else { "mount" })
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    let fence_descriptor = held_fence.as_ref().map(|(_, file)| file.as_raw_fd());
    if let Some(fd) = fence_descriptor { command.arg(fd.to_string()); }
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if let Some(fd) = fence_descriptor {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags == -1 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::io("Cannot start directory backend", e))?;
    session.supervisor = process_identity(child.id());
    save(&dir, &session)?;
    send(&mut child, &secret)?;
    let started = Instant::now();
    loop {
        if session.process.is_none() {
            if let Some(process) = supervisor::read_record(&dir, "backend-process.json")? {
                session.process = Some(process);
                save(&dir, &session)?;
            }
        }
        if let Some(identity) = mount_identity(&session.mountpoint)? {
            if session.process.is_none() {
                return Err(AppError::new("Mounted backend identity unavailable; session retained."));
            }
            if session.mount_identity.as_ref().is_some_and(|expected| expected != &identity) {
                return Err(AppError::new("Mount changed before backend readiness; session retained."));
            }
            if session.mount_identity.is_none() {
                session.mount_identity = Some(identity);
                session.mounted_directory = Some(Identity::read(&session.mountpoint)?);
                save(&dir, &session)?;
            }
            // Java installs its close hook after native mount() returns. The
            // kernel can expose the mount first; an immediate close must wait
            // for the helper's mounted ACK, or SIGTERM could skip the hook.
            if session.backend != VolumeType::Cryptomator
                || supervisor::read_record::<bool>(&dir, "backend-ready.json")? == Some(true)
            {
                output.success(&format!(
                    "Mounted {} at {}",
                    session.backend,
                    session.mountpoint.display()
                ));
                return Ok(());
            }
        }
        if session.fence.is_some() && supervisor::read_record::<supervisor::BackendResult>(&dir, "backend-result.json")?.is_some() {
            return Err(AppError::new(format!("Cryptomator startup failed; its mountpoint remains protected. Explicitly unmount {} to verify and restore it before retrying. Evidence: {}", session.source.display(), dir.display())));
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| AppError::io("Cannot monitor directory backend", e))?
        {
            let diagnostic = supervisor::check_close(&dir, session.backend).err()
                .map(|error| error.message).unwrap_or_default();
            let archive = runtime.path.join(format!("failed-{}", dir.file_name().unwrap().to_string_lossy()));
            fs::rename(&dir, &archive).map_err(|e| AppError::io("Cannot preserve failed startup", e))?;
            sync_dir(&runtime.path)?;
            return Err(AppError::new(format!(
                "{} mount failed ({status}). Evidence retained at {}. {diagnostic}",
                session.backend, archive.display()
            )));
        }
        if started.elapsed() > WAIT {
            // A live backend may still acquire a mount. Preserve its identity
            // rather than terminating a potentially active filesystem blindly.
            return Err(AppError::new(format!(
                "Mount timed out; recoverable session retained at {}. Retry unmount to reconcile it.",
                dir.display()
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn native_unmount(path: &Path, capture_busy: bool) -> AppResult<std::process::Output> {
    #[cfg(target_os = "linux")]
    let mut command = {
        let program = [
            "/usr/bin/fusermount3",
            "/bin/fusermount3",
            "/usr/bin/fusermount",
            "/bin/fusermount",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| executable(p))
        .ok_or_else(|| AppError::new("FUSE unmount helper is missing."))?;
        let mut c = Command::new(program);
        c.args(["-u", "--"]);
        c
    };
    #[cfg(not(target_os = "linux"))]
    let mut command = Command::new("/sbin/umount");
    if capture_busy {
        command.env("LC_ALL", "C").stderr(Stdio::piped());
    } else {
        command.stderr(Stdio::null());
    }
    command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .map_err(|e| AppError::io("Cannot unmount directory", e))
}

fn darwin_unmount_busy(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr);
    let Some((_, detail)) = stderr.trim_end().rsplit_once("): ") else {
        return false;
    };
    matches!(
        detail,
        "Resource busy" | "Resource busy -- try 'diskutil unmount'"
    )
}

fn darwin_fuse_t_fskit_mount(identity: &str) -> bool {
    let Some((_, options)) = identity.rsplit_once(" (") else {
        return false;
    };
    let Some(options) = options.strip_suffix(')') else {
        return false;
    };
    let mut options = options.split(", ");
    options.next() == Some("fuse-t") && options.any(|option| option == "fskit")
}

fn unmount(runtime: &Runtime, c: UnmountCommand, output: Output, source_only: bool) -> AppResult<()> {
    if c.tag.is_some() {
        return Err(AppError::new(
            "Directory unmount does not accept mapper tags.",
        ));
    }
    let target = fs::canonicalize(&c.target)
        .map_err(|e| AppError::io("Cannot resolve session target", e))?;
    let sessions = runtime.sessions()?;
    let matches: Vec<_> = sessions
        .into_iter()
        .filter(|(_, s)| s.source == target || (!source_only && s.mountpoint == target))
        .collect();
    let [(dir, mut session)] = matches.try_into().map_err(|_| {
        AppError::new("Target must identify exactly one directory session owned by you.")
    })?;
    if c.volume_type != VolumeType::Auto && c.volume_type != session.backend {
        return Err(AppError::new(
            "Requested backend does not match the recorded session.",
        ));
    }
    #[cfg(target_os = "freebsd")]
    if let Some(archive) = archive_previous_freebsd_session(&runtime.path, &dir, &session)? {
        return Err(AppError::new(format!(
            "Previous-boot directory session has no confirmed close. Evidence retained at {}. You can reopen the vault.",
            archive.display()
        )));
    }
    let mut retry_available = cfg!(target_os = "macos");
    loop {
        if Identity::read(&session.source)? != session.source_identity {
            return Err(AppError::new(
                "Encrypted directory was replaced; refusing to close a stale session.",
            ));
        }
        let Some(identity) = mount_identity(&session.mountpoint)? else {
            break;
        };
        let expected = confirmed_mount_directory(&session, &identity)?;
        // A confirmed kernel mount ID remains available after a FUSE daemon
        // dies, although stat of its root returns ENOTCONN/EIO. Recover only
        // this exact recorded mount after independently proving daemon exit.
        validate_directory_identity(expected, Identity::read_io(&session.mountpoint), || {
            let process = session.process.as_ref().ok_or_else(|| {
                AppError::new("Backend identity is missing; cannot authorize dead-mount recovery.")
            })?;
            Ok(!alive(process)?)
        })?;
        let retry_busy = retry_available
            && (darwin_fuse_t_fskit_mount(&identity)
                || (session.backend == VolumeType::Cryptomator && session.fence.is_some()));
        let result = native_unmount(&session.mountpoint, retry_busy)?;
        if result.status.success() {
            break;
        }
        if !retry_busy || !darwin_unmount_busy(&result.stderr) {
            return Err(AppError::new(
                "Directory is busy or native unmount failed; session preserved. Close consumers and retry.",
            ));
        }
        // FSKit can briefly report a newly mounted volume as busy. Retry once
        // with ordinary unmount after revalidating the exact source and mount.
        retry_available = false;
        std::thread::sleep(Duration::from_millis(150));
    }
    if mount_identity(&session.mountpoint)?.is_some() {
        return Err(AppError::new(
            "Mount remains active; preserving its session.",
        ));
    }
    session.mount_identity = None;
    session.mounted_directory = None;
    save(&dir, &session)?;
    if let Some(process) = &session.process {
        // FUSE unmount returns before the daemon's final destructor/metadata
        // flush has necessarily finished. gocryptfs/CryFS exit naturally;
        // signalling them here can discard buffered root metadata. The private
        // Java helper uses a shutdown hook and stays alive awaiting a signal.
        if session.backend == VolumeType::Cryptomator && alive(process)? {
            unsafe {
                libc::kill(process.pid as i32, libc::SIGTERM);
            }
        }
        let start = Instant::now();
        while alive(process)? {
            if start.elapsed() > Duration::from_secs(15) {
                return Err(AppError::new(
                    "Filesystem detached but backend has not exited; session retained for retry.",
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fence::request_restore(&dir, &session)?;
    if let Some(journal) = session.fence.as_mut() {
        journal.armed = false;
        save(&dir, &session)?;
    }
    if let Some(monitor) = &session.supervisor {
        let started = Instant::now();
        while alive(monitor)? {
            if started.elapsed() > Duration::from_secs(15) {
                return Err(AppError::new("Backend monitor has not completed; session retained."));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    let close_result = if session.supervisor.is_some() {
        supervisor::check_close(&dir, session.backend)
    } else {
        Err(AppError::new("Legacy backend session has no recorded exit result; a clean close cannot be confirmed."))
    };
    if let Err(error) = close_result {
        // No mount or live backend remains: retain evidence without preventing
        // an explicit reopen for inspection. Live/busy resources never reach here.
        let archive = archive_failed_session(&runtime.path, &dir)?;
        return Err(AppError::new(format!("Filesystem detached, but backend close failed. Evidence retained at {}. {}", archive.display(), error.message)));
    }
    fs::remove_dir_all(dir).map_err(|e| AppError::io("Cannot remove closed session", e))?;
    output.success("Directory vault closed.");
    Ok(())
}

pub(crate) fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    if crate::effective_uid() == 0 {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for (dir, s) in session_snapshot(&runtime_path())? {
        #[cfg(target_os = "freebsd")]
        if from_previous_freebsd_boot(&s)? {
            continue;
        }
        let mounted = mount_identity(&s.mountpoint)?
            .is_some_and(|identity| s.mount_identity.as_ref() == Some(&identity));
        let running = match s.process.as_ref() {
            Some(process) => alive(process)?,
            None => false,
        };
        if mounted || running || s.fence.as_ref().is_some_and(|journal| journal.armed) {
            result.push(ActiveVolume {
                privilege_mode: PrivilegeMode::Unprivileged,
                source_path: Some(s.source),
                readonly: s.readonly,
                mapper_name: format!(
                    "{}:{}",
                    s.backend,
                    dir.file_name().unwrap().to_string_lossy()
                ),
                mapper_path: dir,
                mountpoint: Some(s.mountpoint),
                filesystem_type: Some(s.backend.to_string()),
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn distinguishes_darwin_unmount_busy_from_other_failures() {
        assert!(darwin_unmount_busy(b"umount(/tmp/vault): Resource busy\n"));
        assert!(darwin_unmount_busy(
            b"umount(/tmp/vault): Resource busy -- try 'diskutil unmount'\n"
        ));
        assert!(!darwin_unmount_busy(
            b"umount(/tmp/Resource busy): Operation not permitted\n"
        ));
    }

    #[test]
    fn recognizes_fuse_t_fskit_from_native_mount_options() {
        assert!(darwin_fuse_t_fskit_mount(
            "file:///private/tmp/fuset-session-42/session.json on /tmp/vault (fuse-t, local, nodev, fskit, mounted by tester)"
        ));
        assert!(!darwin_fuse_t_fskit_mount(
            "fuse-t:/fskit on /tmp/vault (nfs, nodev, nosuid)"
        ));
        assert!(!darwin_fuse_t_fskit_mount(
            "gocryptfs on /tmp/vault (macfuse, local, fskit)"
        ));
        assert!(!darwin_fuse_t_fskit_mount(
            "fuse-t on /tmp/vault (fuse-t, nodev, nofskit)"
        ));
    }
    #[test]
    fn helper_fence_contract_is_conservative_for_mac_auto_and_unchanged_elsewhere() {
        assert!(requires_fence_protocol(VolumeType::Auto, true));
        assert!(requires_fence_protocol(VolumeType::Cryptomator, true));
        for kind in [VolumeType::Auto, VolumeType::Cryptomator, VolumeType::Gocryptfs, VolumeType::Cryfs, VolumeType::Apple] {
            assert!(!requires_fence_protocol(kind, false));
        }
        assert!(!requires_fence_protocol(VolumeType::Gocryptfs, true));
        assert!(!requires_fence_protocol(VolumeType::Apple, true));
    }
    #[test]
    fn only_definite_process_absence_authorizes_exit_recovery() {
        assert!(process_query_absent(-1, 1088, Some(libc::ESRCH)));
        assert!(process_query_absent(0, 0, None));
        for error in [libc::EPERM, libc::EACCES, libc::EINTR, libc::EIO] {
            assert!(!process_query_absent(-1, 1088, Some(error)));
        }
        assert!(!process_query_absent(0, 1088, None));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_read_recognizes_a_task_reaped_after_open() {
        let mut child = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stat = File::open(format!("/proc/{}/stat", child.id())).unwrap();
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());

        let error = stat.read_to_string(&mut String::new()).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::ESRCH));
        assert!(linux_stat_process_absent(&error));
        for errno in [libc::EPERM, libc::EACCES, libc::EINTR, libc::EIO] {
            assert!(!linux_stat_process_absent(&std::io::Error::from_raw_os_error(errno)));
        }
    }
    #[cfg(target_os = "freebsd")]
    #[test]
    fn native_process_query_distinguishes_live_child_from_reaped_exit() {
        let mut child = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(process_snapshot(pid).unwrap().is_some());
        drop(child.stdin.take()); // EOF exits only this test's child, no signal.
        assert!(child.wait().unwrap().success());
        assert!(process_snapshot(pid).unwrap().is_none());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn native_process_query_recognizes_exit_before_child_reaping() {
        let mut child = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(process_snapshot(pid).unwrap().is_some());
        drop(child.stdin.take());
        // Keep our Child unreaped, reproducing the initialization close path.
        let started = Instant::now();
        let result = loop {
            match process_snapshot(pid) {
                Ok(None) => break Ok(()),
                Err(error) => break Err(error),
                Ok(Some(_)) if started.elapsed() < Duration::from_secs(5) => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(Some(_)) => break Err(AppError::new("Owned child did not exit")),
            }
        };
        assert!(child.wait().unwrap().success());
        result.unwrap();
    }
    fn snapshot_record() -> Session {
        Session {
            schema: SCHEMA,
            backend: VolumeType::Gocryptfs,
            source: "source".into(),
            source_identity: Identity {
                device: 1,
                inode: 2,
            },
            #[cfg(target_os = "freebsd")]
            boot_id: None,
            mountpoint: "mountpoint".into(),
            readonly: false,
            mount_identity: None,
            mounted_directory: None,
            process: None,
            supervisor: None,
            fence: None,
        }
    }
    #[cfg(target_os = "freebsd")]
    #[test]
    fn previous_boot_preserves_an_occupied_mountpoint_and_archives_its_journal_when_vacant() {
        let root = std::env::temp_dir().join(format!("lurker-folder-boot-{}", nonce()));
        secure_dir(&root).unwrap();
        let directory = root.join("session-test");
        secure_dir(&directory).unwrap();

        let mut session = snapshot_record();
        let boot_id = freebsd_boot_id().unwrap().expect("FreeBSD kern.boot_id");
        assert_eq!(freebsd_boot_id().unwrap(), Some(boot_id));
        assert!(!from_previous_freebsd_boot(&session).unwrap());
        session.boot_id = Some(boot_id);
        assert!(!from_previous_freebsd_boot(&session).unwrap());

        let mut other_boot = boot_id;
        other_boot[0] ^= 1;
        session.boot_id = Some(other_boot);
        session.mountpoint = "/".into();
        save(&directory, &session).unwrap();
        assert!(from_previous_freebsd_boot(&session).unwrap());
        assert!(archive_previous_freebsd_session(&root, &directory, &session).is_err());
        assert_eq!(session_snapshot(&root).unwrap().len(), 1);

        session.mountpoint = root.join("vacant");
        secure_dir(&session.mountpoint).unwrap();
        save(&directory, &session).unwrap();
        let evidence = fs::read(directory.join("session.json")).unwrap();
        let archive = archive_previous_freebsd_session(&root, &directory, &session)
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(archive.join("session.json")).unwrap(), evidence);
        assert!(!directory.exists());
        assert!(session_snapshot(&root).unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn readonly_snapshot_ignores_operation_lock_and_never_recreates_state() {
        let root = std::env::temp_dir().join(format!("lurker-folder-snapshot-{}", nonce()));
        assert!(session_snapshot(&root).unwrap().is_empty());
        assert!(!root.exists());
        secure_dir(&root).unwrap();
        let directory = root.join("session-test");
        secure_dir(&directory).unwrap();
        // An operation may reserve its private directory before publishing JSON.
        assert!(session_snapshot(&root).unwrap().is_empty());
        let lock = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(root.join("lock"))
            .unwrap();
        // SAFETY: this test owns the live descriptor, released when lock drops.
        assert_eq!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        save(&directory, &snapshot_record()).unwrap();
        assert_eq!(session_snapshot(&root).unwrap().len(), 1);
        fs::remove_dir_all(&root).unwrap();
        assert!(session_snapshot(&root).unwrap().is_empty());
        assert!(!root.exists());
    }
    #[test]
    fn readonly_snapshot_keeps_malformed_private_and_oversized_records_explicit() {
        let root = std::env::temp_dir().join(format!("lurker-folder-invalid-snapshot-{}", nonce()));
        secure_dir(&root).unwrap();
        let directory = root.join("session-test");
        secure_dir(&directory).unwrap();
        save(&directory, &snapshot_record()).unwrap();
        let record = directory.join("session.json");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&record, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(session_snapshot(&root)
            .unwrap_err()
            .message
            .contains("private regular file"));
        fs::set_permissions(&record, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&record, b"invalid JSON").unwrap();
        assert!(session_snapshot(&root)
            .unwrap_err()
            .message
            .contains("Malformed"));
        fs::write(&record, vec![b' '; 65537]).unwrap();
        assert!(session_snapshot(&root)
            .unwrap_err()
            .message
            .contains("64 KiB"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn dead_fuse_recovery_requires_confirmed_exit_and_only_known_io_errors() {
        let expected = Identity {
            device: 1,
            inode: 2,
        };
        for errno in [libc::ENOTCONN, libc::EIO] {
            assert!(validate_directory_identity(
                &expected,
                Err(std::io::Error::from_raw_os_error(errno)),
                || Ok(true)
            )
            .is_ok());
            assert!(validate_directory_identity(
                &expected,
                Err(std::io::Error::from_raw_os_error(errno)),
                || Ok(false)
            )
            .is_err());
            assert!(validate_directory_identity(
                &expected,
                Err(std::io::Error::from_raw_os_error(errno)),
                || Err(AppError::new("process inspection denied"))
            )
            .is_err());
        }
        for errno in [libc::EPERM, libc::EACCES, libc::ENOENT] {
            assert!(validate_directory_identity(
                &expected,
                Err(std::io::Error::from_raw_os_error(errno)),
                || panic!("Permissions cannot use the dead-daemon exception")
            )
            .is_err());
        }
        assert!(validate_directory_identity(
            &expected,
            Ok(Identity {
                device: 1,
                inode: 3
            }),
            || panic!("Replaced inode cannot use the dead-daemon exception")
        )
        .is_err());
    }
    #[test]
    fn mount_replacement_and_unconfirmed_mounts_cannot_authorize_recovery() {
        let mut session = Session {
            schema: SCHEMA,
            backend: VolumeType::Gocryptfs,
            source: "source".into(),
            source_identity: Identity {
                device: 1,
                inode: 2,
            },
            #[cfg(target_os = "freebsd")]
            boot_id: None,
            mountpoint: "mountpoint".into(),
            readonly: false,
            mount_identity: Some("confirmed mount".into()),
            mounted_directory: Some(Identity {
                device: 3,
                inode: 4,
            }),
            process: None,
            supervisor: None,
            fence: None,
        };
        assert!(confirmed_mount_directory(&session, "confirmed mount").is_ok());
        assert!(confirmed_mount_directory(&session, "replacement mount").is_err());
        session.mount_identity = None;
        assert!(confirmed_mount_directory(&session, "confirmed mount").is_err());
        session.mount_identity = Some("confirmed mount".into());
        session.mounted_directory = None;
        assert!(confirmed_mount_directory(&session, "confirmed mount").is_err());
    }
    #[test]
    fn readonly_and_secret_transport_are_backend_specific() {
        let source = Path::new("/tmp/encrypted vault");
        let mp = Path::new("/tmp/open vault");
        for kind in [
            VolumeType::Gocryptfs,
            VolumeType::Cryfs,
            VolumeType::Cryptomator,
        ] {
            let args = mount_args(kind, source, mp, true).unwrap();
            assert!(args.contains(&source.as_os_str().into()));
            assert!(args.contains(&mp.as_os_str().into()));
            assert!(args
                .iter()
                .any(|a| matches!(a.to_str(), Some("-ro" | "ro" | "--readonly"))));
        }
    }
    #[test]
    fn conflicting_vault_markers_are_rejected() {
        let dir = std::env::temp_dir().join(format!("lurker-folder-markers-{}", nonce()));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("gocryptfs.conf"), b"{}").unwrap();
        assert_eq!(detect_type(&dir).unwrap(), VolumeType::Gocryptfs);
        fs::write(dir.join("cryfs.config"), b"{}").unwrap();
        assert!(detect_type(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
