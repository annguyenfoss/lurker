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

const WAIT: Duration = Duration::from_secs(60);
const SCHEMA: u32 = 1;

pub(crate) fn is_folder(kind: VolumeType) -> bool {
    matches!(
        kind,
        VolumeType::Gocryptfs | VolumeType::Cryfs | VolumeType::Cryptomator
    )
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
            if !runtime_path().exists() {
                return Ok(false);
            }
            let target = fs::canonicalize(&c.target)
                .map_err(|e| AppError::io("Cannot resolve unmount target", e))?;
            let runtime = Runtime::lock()?;
            Ok(runtime
                .sessions()?
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
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("LURKER_BACKEND_DIR") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(AppError::new("LURKER_BACKEND_DIR must be absolute."));
        }
        dirs.push(path);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.extend([
                parent.join("backends"),
                parent.join("../Helpers"),
                parent.join("../libexec/lurker/backends"),
            ]);
        }
    }
    dirs.extend([
        PathBuf::from("/usr/libexec/lurker/backends"),
        PathBuf::from("/usr/local/libexec/lurker/backends"),
    ]);
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
        let m =
            fs::metadata(path).map_err(|e| AppError::io("Cannot inspect directory identity", e))?;
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
    mountpoint: PathBuf,
    readonly: bool,
    mount_identity: Option<String>,
    mounted_directory: Option<Identity>,
    process: Option<Process>,
}

fn secure_dir(path: &Path) -> AppResult<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(AppError::io("Cannot create private folder state", e)),
    }
    let m = fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Cannot inspect private folder state", e))?;
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
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.path)
            .map_err(|e| AppError::io("Cannot inspect folder sessions", e))?
        {
            let entry = entry.map_err(|e| AppError::io("Cannot inspect folder session", e))?;
            if !entry.file_name().to_string_lossy().starts_with("session-") {
                continue;
            }
            let dir = entry.path();
            secure_dir(&dir)?;
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(dir.join("session.json"))
                .map_err(|e| AppError::io("Cannot read folder session", e))?;
            let session: Session = serde_json::from_reader(file.take(65536)).map_err(|_| {
                AppError::new("Malformed folder session; preserve it for manual recovery.")
            })?;
            if session.schema != SCHEMA {
                return Err(AppError::new("Unsupported folder session schema."));
            }
            result.push((dir, session));
        }
        Ok(result)
    }
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

fn process_identity(pid: u32) -> Option<Process> {
    #[cfg(target_os = "linux")]
    {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let fields: Vec<_> = stat.rsplit_once(") ")?.1.split_whitespace().collect();
        if fields.first() == Some(&"Z") {
            return None;
        }
        Some(Process {
            pid,
            birth: fields.get(19)?.to_string(),
        })
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
        if result != size as i32 {
            return None;
        }
        let info = unsafe { info.assume_init() };
        Some(Process {
            pid,
            birth: format!("{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec),
        })
    }
    #[cfg(target_os = "freebsd")]
    {
        let mut mib = [
            libc::CTL_KERN,
            libc::KERN_PROC,
            libc::KERN_PROC_PID,
            pid as i32,
        ];
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
        if result != 0 || size != std::mem::size_of::<libc::kinfo_proc>() {
            return None;
        }
        let info = unsafe { info.assume_init() };
        if info.ki_pid != pid as i32 {
            return None;
        }
        Some(Process {
            pid,
            birth: format!("{}:{}", info.ki_start.tv_sec, info.ki_start.tv_usec),
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
    {
        let _ = pid;
        None // No selected folder backend on the other BSD targets.
    }
}
fn alive(process: &Process) -> bool {
    process_identity(process.pid).is_some_and(|p| p.birth == process.birth)
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
    args.extend([source.as_os_str().into(), mountpoint.as_os_str().into()]);
    if readonly && kind == VolumeType::Cryptomator {
        args.push("--readonly".into());
    }
    Ok(args)
}

pub(crate) fn run(command: CommandAction, output: Output) -> AppResult<()> {
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "Directory encryption runs as your login user; do not use sudo.",
        ));
    }
    let runtime = Runtime::lock()?;
    match command {
        CommandAction::Create(c) => create(&runtime, c, output),
        CommandAction::Mount(c) => mount(&runtime, c, output),
        CommandAction::Unmount(c) => unmount(&runtime, c, output),
        _ => Err(AppError::new(
            "Directory backends do not support LUKS keyslots.",
        )),
    }
}

fn create(runtime: &Runtime, c: CreateCommand, output: Output) -> AppResult<()> {
    if !supported(c.volume_type) {
        return Err(AppError::new(
            "This directory backend is not selected for this OS.",
        ));
    }
    if c.force || c.size_gb.is_some() || c.keyfile.is_some() || c.cipher != CreateCipher::Aes {
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
                fs::rename(entry.path(), target.join(entry.file_name()))
                    .map_err(|e| AppError::io("Cannot publish vault data", e))?;
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
    for (_, s) in runtime.sessions()? {
        if s.source_identity == source_identity {
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
    let dir = runtime
        .path
        .join(format!("session-{}-{}", std::process::id(), nonce()));
    secure_dir(&dir)?;
    let mut session = Session {
        schema: SCHEMA,
        backend: c.volume_type,
        source,
        source_identity,
        mountpoint,
        readonly: c.readonly,
        mount_identity: None,
        mounted_directory: None,
        process: None,
    };
    save(&dir, &session)?;
    let mut args = mount_args(
        c.volume_type,
        &session.source,
        &session.mountpoint,
        c.readonly,
    )?;
    if initializing && c.volume_type == VolumeType::Cryfs {
        // Existing vaults retain their own cipher/KDF; mount never migrates.
        args.splice(
            0..0,
            ["--cipher", "aes-256-gcm", "--blocksize", "16384"].map(OsString::from),
        );
    }
    let mut command = backend_command(c.volume_type, &executable);
    command.args(args);
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::io("Cannot start directory backend", e))?;
    session.process = process_identity(child.id());
    save(&dir, &session)?;
    send(&mut child, &secret)?;
    let started = Instant::now();
    loop {
        if let Some(identity) = mount_identity(&session.mountpoint)? {
            session.mount_identity = Some(identity);
            session.mounted_directory = Some(Identity::read(&session.mountpoint)?);
            save(&dir, &session)?;
            output.success(&format!(
                "Mounted {} at {}",
                session.backend,
                session.mountpoint.display()
            ));
            return Ok(());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| AppError::io("Cannot monitor directory backend", e))?
        {
            fs::remove_dir_all(&dir)
                .map_err(|e| AppError::io("Cannot remove failed session", e))?;
            return Err(AppError::new(format!(
                "{} mount failed ({status}). Check credentials and the required FUSE runtime.",
                session.backend
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

fn native_unmount(path: &Path) -> AppResult<()> {
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
    let status = command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| AppError::io("Cannot unmount directory", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(
            "Directory is busy or native unmount failed; session preserved. Close consumers and retry.",
        ))
    }
}

fn unmount(runtime: &Runtime, c: UnmountCommand, output: Output) -> AppResult<()> {
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
        .filter(|(_, s)| s.source == target || s.mountpoint == target)
        .collect();
    let [(dir, mut session)] = matches.try_into().map_err(|_| {
        AppError::new("Target must identify exactly one directory session owned by you.")
    })?;
    if c.volume_type != VolumeType::Auto && c.volume_type != session.backend {
        return Err(AppError::new(
            "Requested backend does not match the recorded session.",
        ));
    }
    if Identity::read(&session.source)? != session.source_identity {
        return Err(AppError::new(
            "Encrypted directory was replaced; refusing to close a stale session.",
        ));
    }
    let current = mount_identity(&session.mountpoint)?;
    if let Some(ref identity) = current {
        if session.mount_identity.as_ref() != Some(identity)
            || session.mounted_directory.as_ref() != Some(&Identity::read(&session.mountpoint)?)
        {
            return Err(AppError::new(
                "Mount identity changed or was never confirmed; refusing to unmount an unverified filesystem.",
            ));
        }
        native_unmount(&session.mountpoint)?;
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
        if session.backend == VolumeType::Cryptomator && alive(process) {
            unsafe {
                libc::kill(process.pid as i32, libc::SIGTERM);
            }
        }
        let start = Instant::now();
        while alive(process) {
            if start.elapsed() > Duration::from_secs(15) {
                return Err(AppError::new(
                    "Filesystem detached but backend has not exited; session retained for retry.",
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fs::remove_dir_all(dir).map_err(|e| AppError::io("Cannot remove closed session", e))?;
    output.success("Directory vault closed.");
    Ok(())
}

pub(crate) fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    if crate::effective_uid() == 0 || !runtime_path().exists() {
        return Ok(Vec::new());
    }
    let runtime = Runtime::lock()?;
    let mut result = Vec::new();
    for (dir, s) in runtime.sessions()? {
        let mounted = mount_identity(&s.mountpoint)?.is_some();
        if mounted || s.process.as_ref().is_some_and(alive) {
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
            assert!(
                args.iter()
                    .any(|a| matches!(a.to_str(), Some("-ro" | "ro" | "--readonly")))
            );
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
