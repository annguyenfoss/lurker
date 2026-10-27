//! Linux file containers served by unprivileged VeraCrypt and filesystem processes.
//! No command in this module invokes sudo, pkexec, mount, loop, or device-mapper.
use crate::linux::{absolute_path, read_mountinfo, MountInfo};
use crate::{
    ActiveVolume, AppError, AppResult, CommandAction, CreateCommand, MountCommand, Output,
    PrivilegeMode, SourceKind, UnmountCommand, VolumeType,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{IsTerminal, Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const HELPER_VERSION: &str = "lurker-userspace-2";
const READY_TIMEOUT: Duration = Duration::from_secs(45);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Session {
    version: u32,
    source: PathBuf,
    source_device: u64,
    source_inode: u64,
    mountpoint: PathBuf,
    created_mountpoint: bool,
    raw_mount_id: Option<u64>,
    filesystem_mount_id: Option<u64>,
    filesystem: Option<String>,
    readonly: bool,
    process: Option<ProcessIdentity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProcessIdentity {
    pid: u32,
    start_time: u64,
}

struct Runtime {
    path: PathBuf,
    _lock: File,
}

struct Tools {
    crypto: PathBuf,
    filesystem: PathBuf,
    fusermount: PathBuf,
}

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    if crate::effective_uid() == 0 {
        return Err(AppError::new(
            "Unprivileged mode must run as your login user, without sudo.",
        ));
    }
    validate_command(&command)?;
    let runtime = Runtime::lock()?;
    let tools = Tools::resolve(!matches!(command, CommandAction::Unmount(_)))?;
    match command {
        CommandAction::Create(c) => create(&runtime, &tools, c, output),
        CommandAction::Mount(c) => mount(&runtime, &tools, c, output),
        CommandAction::Unmount(c) => unmount(&runtime, &tools, c, output),
    }
}

fn validate_command(command: &CommandAction) -> AppResult<()> {
    let (kind, tag) = match command {
        CommandAction::Create(c) => {
            if c.source_kind != SourceKind::File {
                return Err(AppError::new("Unprivileged mode supports file containers only. Use Privileged for disks and partitions."));
            }
            (c.volume_type, None)
        }
        CommandAction::Mount(c) => (c.volume_type, c.tag.as_deref()),
        CommandAction::Unmount(c) => (c.volume_type, c.tag.as_deref()),
    };
    if !matches!(kind, VolumeType::Veracrypt | VolumeType::Auto)
        || matches!(command, CommandAction::Create(c) if c.volume_type == VolumeType::Auto)
    {
        return Err(AppError::new(
            "Unprivileged mode supports VeraCrypt file containers. Use Privileged for LUKS.",
        ));
    }
    if tag.is_some() {
        return Err(AppError::new(
            "Tags are not supported for VeraCrypt containers.",
        ));
    }
    Ok(())
}

fn secure_directory(path: &Path) -> AppResult<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(AppError::io(
                format!("Failed to create {}", path.display()),
                e,
            ))
        }
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Failed to inspect userspace state directory", e))?;
    if !metadata.is_dir()
        || metadata.uid() != crate::effective_uid()
        || metadata.mode() & 0o077 != 0
    {
        return Err(AppError::new(format!(
            "Userspace state directory must be a private directory owned by you: {}",
            path.display()
        )));
    }
    Ok(())
}

fn runtime_path() -> AppResult<PathBuf> {
    let parent = if let Some(path) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(path);
        let m = fs::symlink_metadata(&path)
            .map_err(|e| AppError::io("Failed to inspect XDG_RUNTIME_DIR", e))?;
        if !m.is_dir() || m.uid() != crate::effective_uid() || m.mode() & 0o077 != 0 {
            return Err(AppError::new(
                "XDG_RUNTIME_DIR must be a private directory owned by you.",
            ));
        }
        path
    } else {
        std::env::temp_dir()
    };
    Ok(parent.join(format!("lurker-userspace-{}", crate::effective_uid())))
}

impl Runtime {
    fn lock() -> AppResult<Self> {
        let path = runtime_path()?;
        secure_directory(&path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.join("operation.lock"))
            .map_err(|e| AppError::io("Failed to open userspace operation lock", e))?;
        // SAFETY: flock operates on this live descriptor; it is released on close.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(AppError::new(
                "Another unprivileged operation is in progress. Wait for it to finish.",
            ));
        }
        Ok(Self { path, _lock: lock })
    }

    fn sessions(&self) -> AppResult<Vec<(PathBuf, Session)>> {
        sessions_in(&self.path)
    }

    fn reserve(
        &self,
        source: &Path,
        mountpoint: &Path,
        readonly: bool,
        created_mountpoint: bool,
    ) -> AppResult<(PathBuf, Session)> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = self
            .path
            .join(format!("volume-{}-{nonce}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| AppError::io("Failed to reserve userspace session", e))?;
        secure_directory(&dir.join("raw"))?;
        let metadata =
            fs::metadata(source).map_err(|e| AppError::io("Failed to inspect container", e))?;
        let session = Session {
            version: 1,
            source: source.into(),
            source_device: metadata.dev(),
            source_inode: metadata.ino(),
            mountpoint: mountpoint.into(),
            created_mountpoint,
            raw_mount_id: None,
            filesystem_mount_id: None,
            filesystem: None,
            readonly,
            process: None,
        };
        save_session(&dir, &session)?;
        Ok((dir, session))
    }
}

fn sessions_in(path: &Path) -> AppResult<Vec<(PathBuf, Session)>> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::io("Failed to list userspace sessions", e)),
    };
    let mut sessions = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| AppError::io("Failed to list userspace sessions", e))?;
        if !entry.file_name().to_string_lossy().starts_with("volume-") {
            continue;
        }
        let dir = entry.path();
        let metadata =
            fs::symlink_metadata(&dir).map_err(|e| AppError::io("Failed to inspect session", e))?;
        if !metadata.is_dir()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o077 != 0
        {
            continue;
        }
        let record = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("session.json"));
        let record = match record {
            Ok(record) => record,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(AppError::io("Failed to open userspace session", e)),
        };
        let session: Session = serde_json::from_reader(record.take(65536)).map_err(|e| {
            AppError::new(format!("Invalid userspace session {}: {e}", dir.display()))
        })?;
        if session.version != 1 {
            return Err(AppError::new("Unsupported userspace session version."));
        }
        sessions.push((dir, session));
    }
    Ok(sessions)
}

fn save_session(dir: &Path, session: &Session) -> AppResult<()> {
    let bytes = serde_json::to_vec(session)
        .map_err(|e| AppError::new(format!("Failed to encode userspace session: {e}")))?;
    let temporary = dir.join("session.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|e| AppError::io("Failed to write userspace session", e))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| AppError::io("Failed to sync userspace session", e))?;
    fs::rename(&temporary, dir.join("session.json"))
        .map_err(|e| AppError::io("Failed to publish userspace session", e))
}

fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0)
}

fn find_program(name: &str) -> AppResult<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|p| p.join(name))
        .find(|p| executable(p))
        .ok_or_else(|| AppError::new(format!("Required command not found: {name}")))
}

impl Tools {
    fn resolve(backends_needed: bool) -> AppResult<Self> {
        let fusermount = find_program("fusermount3")?;
        if !backends_needed {
            return Ok(Self {
                crypto: PathBuf::new(),
                filesystem: PathBuf::new(),
                fusermount,
            });
        }
        let directories = if let Some(path) = std::env::var_os("LURKER_USERSPACE_DIR") {
            vec![PathBuf::from(path)]
        } else {
            let mut paths = Vec::new();
            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    paths.push(parent.join("userspace"));
                }
            }
            paths.push("/usr/lib/lurker/userspace".into());
            paths
        };
        let dir = directories.into_iter().find(|p| executable(&p.join("lurker-veracrypt")) && executable(&p.join("lurker-fs")))
            .ok_or_else(|| AppError::new("Unprivileged helpers are missing. Build them with python scripts/build-linux-userspace.py, or install a Lurker package with userspace support."))?;
        let tools = Self {
            crypto: dir.join("lurker-veracrypt"),
            filesystem: dir.join("lurker-fs"),
            fusermount,
        };
        for program in [&tools.crypto, &tools.filesystem] {
            let output = Command::new(program)
                .arg("--lurker-userspace-version")
                .output()
                .map_err(|e| AppError::io("Failed to inspect userspace helper", e))?;
            if !output.status.success()
                || String::from_utf8_lossy(&output.stdout).trim() != HELPER_VERSION
            {
                return Err(AppError::new(format!(
                    "Incompatible userspace helper: {}. Rebuild the Linux helpers.",
                    program.display()
                )));
            }
        }
        Ok(tools)
    }
}

fn checked(
    program: &Path,
    args: &[OsString],
    input: Option<&str>,
    temp_dir: Option<&Path>,
) -> AppResult<()> {
    let mut command = Command::new(program);
    // wxWidgets uses the process locale for argv and passphrase conversion.
    // C alone corrupts non-ASCII paths before VeraCrypt opens the file.
    command
        .args(args)
        .env("LC_ALL", "C.UTF-8")
        .env("LANG", "C.UTF-8")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(dir) = temp_dir {
        command.env("TMPDIR", dir);
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::io(format!("Failed to start {}", program.display()), e))?;
    let sent = if let Some(secret) = input {
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(secret.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
    } else {
        Ok(())
    };
    let result = child
        .wait_with_output()
        .map_err(|e| AppError::io("Failed waiting for userspace tool", e))?;
    if result.status.success() {
        return sent.map_err(|e| AppError::io("Failed to send passphrase", e));
    }
    let mut detail: String = String::from_utf8_lossy(&result.stderr)
        .chars()
        .take(8192)
        .collect();
    if let Some(secret) = input {
        if !secret.is_empty() {
            detail = detail.replace(secret, "[redacted]");
        }
    }
    Err(AppError::new(format!(
        "{} failed: {}",
        program.file_name().unwrap_or_default().to_string_lossy(),
        detail.trim()
    )))
}

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

fn prompt_passphrase(prompt: &str) -> std::io::Result<String> {
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

fn passphrase(supplied: Option<&str>, create: bool) -> AppResult<String> {
    let secret = if let Some(s) = supplied {
        s.to_owned()
    } else {
        if !std::io::stdin().is_terminal() {
            return Err(AppError::new(
                "Passphrase entry requires a terminal or the desktop app.",
            ));
        }
        let value = prompt_passphrase("Passphrase: ")
            .map_err(|e| AppError::io("Failed to read passphrase", e))?;
        if create {
            let confirmed = prompt_passphrase("Confirm passphrase: ")
                .map_err(|e| AppError::io("Failed to confirm passphrase", e))?;
            if value != confirmed {
                return Err(AppError::new("Passphrase confirmation does not match."));
            }
        }
        value
    };
    if secret.is_empty() || secret.contains(['\0', '\n', '\r']) {
        return Err(AppError::new(
            "Passphrase must be non-empty and contain no NUL or newline characters.",
        ));
    }
    Ok(secret)
}

fn mount_at(path: &Path) -> AppResult<Option<MountInfo>> {
    Ok(read_mountinfo()?
        .into_iter()
        .find(|m| m.mount_point == path))
}

fn raw_path(dir: &Path) -> PathBuf {
    dir.join("raw/.veracrypt_aux_mnt1/volume")
}

fn open_raw(tools: &Tools, dir: &Path, session: &mut Session, secret: &str) -> AppResult<()> {
    let mut args: Vec<OsString> = [
        "-t",
        "--mount",
        "--filesystem=none",
        "--stdin",
        "--non-interactive",
        "--pim=0",
        "-k",
        "",
        "--protect-hidden=no",
    ]
    .map(OsString::from)
    .into();
    args.push(
        if session.readonly {
            "--mount-options=nokernelcrypto,ro"
        } else {
            "--mount-options=nokernelcrypto"
        }
        .into(),
    );
    args.push(session.source.as_os_str().into());
    let result = checked(&tools.crypto, &args, Some(secret), Some(&dir.join("raw")));
    // Persist a partial attachment too, so failure recovery only closes our mount.
    session.raw_mount_id = mount_at(raw_path(dir).parent().unwrap())?.map(|m| m.mount_id);
    save_session(dir, session)?;
    result?;
    if session.raw_mount_id.is_none() || !raw_path(dir).is_file() {
        return Err(AppError::new(
            "VeraCrypt did not expose the decrypted file image.",
        ));
    }
    Ok(())
}

fn filesystem_type(raw: &Path) -> AppResult<String> {
    let mut file =
        File::open(raw).map_err(|e| AppError::io("Failed to inspect decrypted filesystem", e))?;
    let mut at = |offset, len| -> AppResult<Vec<u8>> {
        let mut bytes = vec![0; len];
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(|e| AppError::io("Failed to read filesystem signature", e))?;
        Ok(bytes)
    };
    if at(65536 + 64, 8)? == b"_BHRfS_M" {
        return Ok("btrfs".into());
    }
    if at(1024 + 56, 2)? == [0x53, 0xef] {
        return Ok("ext4".into());
    }
    if at(3, 8)? == b"EXFAT   " {
        return Ok("exfat".into());
    }
    if at(3, 8)? == b"NTFS    " {
        return Ok("ntfs3".into());
    }
    if at(0, 4)? == b"XFSB" {
        return Ok("xfs".into());
    }
    if at(54, 3)? == b"FAT" || at(82, 3)? == b"FAT" {
        return Ok("vfat".into());
    }
    Err(AppError::new("Unsupported userspace filesystem. Supported: Btrfs, ext2/3/4, exFAT, FAT and NTFS/XFS when enabled in the helper."))
}

fn start_filesystem(
    tools: &Tools,
    dir: &Path,
    session: &mut Session,
    initialize_owner: bool,
) -> AppResult<Child> {
    let filesystem = filesystem_type(&raw_path(dir))?;
    let ready = dir.join("ready");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join("filesystem.log"))
        .map_err(|e| AppError::io("Failed to open filesystem log", e))?;
    let mut command = Command::new(&tools.filesystem);
    command.args(["-f", "-s", "-o", &format!("type={filesystem},mb=192")]);
    if filesystem == "btrfs" {
        command.args(["-o", "opts=compress=zstd"]);
    }
    if matches!(filesystem.as_str(), "vfat" | "exfat" | "ntfs3") {
        // These filesystems synthesize ownership at mount time. Escape the
        // comma so libfuse passes the whole option string to the LKL driver.
        command.args([
            "-o",
            &format!("opts=uid={}\\,gid={}", crate::effective_uid(), unsafe {
                libc::getegid()
            }),
        ]);
    }
    if session.readonly {
        command.args(["-o", "ro"]);
    }
    command
        .arg(raw_path(dir))
        .arg(&session.mountpoint)
        .env("LURKER_READY_FILE", &ready)
        .env("LURKER_STOPPED_FILE", dir.join("stopped"))
        .env_remove("LURKER_INIT_OWNER")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone()
                .map_err(|e| AppError::io("Failed to open log", e))?,
        ))
        .stderr(Stdio::from(log));
    if initialize_owner {
        command.env("LURKER_INIT_OWNER", "1");
    }
    // A mounted filesystem outlives its CLI/helper. Give the foreground FUSE
    // driver its own session so closing the caller's terminal cannot hang it up.
    // SAFETY: setsid is the only pre-exec syscall; no locks or Rust state are used.
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
        .map_err(|e| AppError::io("Failed to start userspace filesystem", e))?;
    session.process = process_identity(child.id());
    session.filesystem = Some(filesystem);
    save_session(dir, session)?;
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        session.filesystem_mount_id = mount_at(&session.mountpoint)?.map(|m| m.mount_id);
        if ready.exists() && session.filesystem_mount_id.is_some() {
            save_session(dir, session)?;
            return Ok(child);
        }
        if child
            .try_wait()
            .map_err(|e| AppError::io("Failed to inspect filesystem process", e))?
            .is_some()
            || Instant::now() >= deadline
        {
            save_session(dir, session)?;
            let detail = fs::read_to_string(dir.join("filesystem.log")).unwrap_or_default();
            let reason = detail
                .lines()
                .find(|line| line.starts_with("can't mount disk:"))
                .or_else(|| detail.lines().rev().find(|line| !line.starts_with('[')))
                .unwrap_or("driver did not acknowledge startup");
            let recovery = if detail.contains("recovery required") {
                " The filesystem needs recovery; open it read/write first."
            } else {
                ""
            };
            return Err(AppError::new(format!(
                "Userspace filesystem did not start: {reason}.{recovery}"
            )));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn process_identity(pid: u32) -> Option<ProcessIdentity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = stat.rsplit_once(") ")?;
    let values: Vec<_> = fields.split_whitespace().collect();
    if values.first().is_some_and(|v| *v == "Z") {
        return None;
    }
    Some(ProcessIdentity {
        pid,
        start_time: values.get(19)?.parse().ok()?,
    })
}

fn process_alive(identity: &ProcessIdentity) -> bool {
    process_identity(identity.pid).is_some_and(|p| p.start_time == identity.start_time)
}

fn needs_recovery(dir: &Path) -> bool {
    (dir.join("ready").exists() || dir.join("recovering").exists())
        && fs::read(dir.join("stopped")).ok().as_deref() != Some(b"stopped\n")
}

fn close_session(tools: &Tools, dir: &Path, session: &Session) -> AppResult<()> {
    if let Some(mount) = mount_at(&session.mountpoint)? {
        if session.filesystem_mount_id != Some(mount.mount_id) {
            return Err(AppError::new(
                "Mountpoint now belongs to a different mount. Refusing to unmount it.",
            ));
        }
        checked(
            &tools.fusermount,
            &["-u".into(), session.mountpoint.as_os_str().into()],
            None,
            None,
        )?;
    }
    if let Some(process) = &session.process {
        let deadline = Instant::now() + DRAIN_TIMEOUT;
        while process_alive(process) {
            if Instant::now() >= deadline {
                return Err(AppError::new("Filesystem is still flushing. The encrypted image remains open; retry unmount after it finishes."));
            }
            thread::sleep(Duration::from_millis(25));
        }
        // Reap the driver when this process also performed the mount. A
        // desktop helper's detached driver is instead reaped by its adopter.
        unsafe {
            libc::waitpid(process.pid as i32, std::ptr::null_mut(), libc::WNOHANG);
        }
    }
    if needs_recovery(dir) {
        return Err(AppError::new("The filesystem did not confirm a clean shutdown. Its encrypted image remains open for recovery; inspect the session filesystem.log before retrying."));
    }
    let raw = raw_path(dir);
    if let Some(mount) = mount_at(raw.parent().unwrap())? {
        if session.raw_mount_id != Some(mount.mount_id) {
            return Err(AppError::new(
                "Encrypted image mount changed. Refusing to close an unrelated mount.",
            ));
        }
        checked(
            &tools.fusermount,
            &["-u".into(), raw.parent().unwrap().as_os_str().into()],
            None,
            None,
        )?;
    }
    if session.created_mountpoint {
        let _ = fs::remove_dir(&session.mountpoint);
    }
    if read_mountinfo()?
        .iter()
        .any(|m| m.mount_point.starts_with(dir))
    {
        return Err(AppError::new(
            "Session contains an unexpected mount. Retaining its files for recovery.",
        ));
    }
    fs::remove_dir_all(dir).map_err(|e| AppError::io("Failed to remove closed session", e))
}

fn validate_source(source: &Path) -> AppResult<PathBuf> {
    let source = fs::canonicalize(expand(source)?)
        .map_err(|e| AppError::io("Failed to resolve container file", e))?;
    if !source.is_file() {
        return Err(AppError::new(
            "Unprivileged mode supports regular file containers only.",
        ));
    }
    let mut header = [0; 6];
    File::open(&source)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|e| AppError::io("Failed to read container", e))?;
    if header == *b"LUKS\xba\xbe" {
        return Err(AppError::new(
            "This is a LUKS container. Switch to Privileged mode.",
        ));
    }
    Ok(source)
}

fn expand(path: &Path) -> AppResult<PathBuf> {
    if path == Path::new("~") || path.starts_with("~/") {
        let home = std::env::var_os("HOME").ok_or_else(|| AppError::new("HOME is not set."))?;
        return Ok(PathBuf::from(home).join(path.strip_prefix("~").unwrap()));
    }
    Ok(path.into())
}

fn assert_unused(runtime: &Runtime, source: &Path) -> AppResult<()> {
    let identity = fs::metadata(source).ok().map(|m| (m.dev(), m.ino()));
    for (_, session) in runtime.sessions()? {
        if session.source == source
            || identity == Some((session.source_device, session.source_inode))
        {
            return Err(AppError::new(
                "Container has an active userspace session. Unmount it first.",
            ));
        }
    }
    if identity.is_some() {
        let entries = fs::read_dir("/sys/class/block")
            .map_err(|e| AppError::io("Failed to inspect active file devices", e))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| AppError::io("Failed to inspect active file devices", e))?;
            if !entry.file_name().to_string_lossy().starts_with("loop") {
                continue;
            }
            let backing = match fs::read_to_string(entry.path().join("loop/backing_file")) {
                Ok(value) => value,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(AppError::io(
                        "Failed to inspect active loop backing file",
                        e,
                    ))
                }
            };
            let path = backing
                .trim_end_matches('\n')
                .trim_end_matches(" (deleted)");
            if fs::metadata(path).ok().map(|m| (m.dev(), m.ino())) == identity {
                return Err(AppError::new(
                    "Container is already attached to a kernel loop device. Unmount it first.",
                ));
            }
        }
    }
    Ok(())
}

fn mount(runtime: &Runtime, tools: &Tools, c: MountCommand, output: Output) -> AppResult<()> {
    let secret = passphrase(c.passphrase.as_deref(), false)?;
    let source = validate_source(&c.source)?;
    assert_unused(runtime, &source)?;
    let mountpoint = absolute_path(&expand(&c.mountpoint)?)?;
    if mount_at(&mountpoint)?.is_some() {
        return Err(AppError::new("Mountpoint is already mounted."));
    }
    let created = !mountpoint.exists();
    if created {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&mountpoint)
            .map_err(|e| AppError::io("Failed to create mountpoint", e))?;
    }
    if !mountpoint.is_dir()
        || fs::read_dir(&mountpoint)
            .map_err(|e| AppError::io("Failed to inspect mountpoint", e))?
            .next()
            .is_some()
    {
        return Err(AppError::new("Mountpoint must be an empty directory."));
    }
    let (dir, mut session) = runtime.reserve(&source, &mountpoint, c.readonly, created)?;
    let result = (|| {
        open_raw(tools, &dir, &mut session, &secret)?;
        start_filesystem(tools, &dir, &mut session, false)?;
        Ok(())
    })();
    if let Err(error) = result {
        if let Err(cleanup) = close_session(tools, &dir, &session) {
            output.warn(&format!(
                "Session retained for recovery: {}",
                cleanup.message
            ));
        }
        return Err(error);
    }
    output.msg2(&format!(
        "Mounted {} at {} without elevation",
        source.display(),
        mountpoint.display()
    ));
    Ok(())
}

fn unmount(runtime: &Runtime, tools: &Tools, c: UnmountCommand, output: Output) -> AppResult<()> {
    let expanded = expand(&c.target)?;
    let target = absolute_path(&expanded).unwrap_or(expanded);
    let identity = fs::metadata(&target).ok().map(|m| (m.dev(), m.ino()));
    let matches: Vec<_> = runtime
        .sessions()?
        .into_iter()
        .filter(|(dir, s)| {
            s.source == target
                || s.mountpoint == target
                || raw_path(dir) == target
                || identity == Some((s.source_device, s.source_inode))
        })
        .collect();
    if matches.len() != 1 {
        return Err(AppError::new(
            "Target must identify exactly one of your active unprivileged containers.",
        ));
    }
    let (dir, session) = &matches[0];
    if let Err(error) = close_session(tools, dir, session) {
        // A dead driver cannot flush its lost buffers. Recover its on-disk
        // journal through a new normal-user driver, then require a clean close.
        // Keep both the raw mount and recovery marker on any failure.
        let raw_mount = mount_at(raw_path(dir).parent().unwrap())?;
        if !needs_recovery(dir)
            || mount_at(&session.mountpoint)?.is_some()
            || session.process.as_ref().is_some_and(process_alive)
            || raw_mount.as_ref().map(|m| m.mount_id) != session.raw_mount_id
            || raw_mount.is_none()
        {
            return Err(error);
        }
        output.warn("The filesystem stopped unexpectedly. Recovering its journal before closing the encrypted image.");
        let backends = Tools::resolve(true)?;
        OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("recovering"))
            .map_err(|e| AppError::io("Failed to record filesystem recovery", e))?;
        for marker in ["ready", "stopped"] {
            match fs::remove_file(dir.join(marker)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(AppError::io(
                        "Failed to reset filesystem acknowledgement",
                        e,
                    ))
                }
            }
        }
        let mut recovered = session.clone();
        recovered.process = None;
        recovered.filesystem_mount_id = None;
        save_session(dir, &recovered)?;
        start_filesystem(&backends, dir, &mut recovered, false)?;
        close_session(&backends, dir, &recovered)?;
    }
    output.msg2(&format!(
        "Unmounted {} without elevation",
        session.source.display()
    ));
    Ok(())
}

fn assert_create_target(target: &Path, force: bool) -> AppResult<()> {
    match fs::symlink_metadata(target) {
        Ok(m) if !m.is_file() => Err(AppError::new(
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

fn create(runtime: &Runtime, tools: &Tools, c: CreateCommand, output: Output) -> AppResult<()> {
    let secret = passphrase(c.passphrase.as_deref(), true)?;
    let size = crate::size::size_gb_to_mib(
        c.size_gb
            .as_deref()
            .ok_or_else(|| AppError::new("File container size is required."))?,
    )?;
    if size < 128 {
        return Err(AppError::new(
            "Btrfs file containers must be at least 128 MiB.",
        ));
    }
    let expanded = expand(&c.target)?;
    let parent = expanded
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)
        .map_err(|e| AppError::io("Failed to resolve container directory", e))?;
    let target = parent.join(
        expanded
            .file_name()
            .ok_or_else(|| AppError::new("Target needs a file name."))?,
    );
    assert_create_target(&target, c.force)?;
    assert_unused(runtime, &target)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = parent.join(format!(".volume.lurker-tmp-{}-{nonce}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&staging)
        .map_err(|e| AppError::io("Failed to stage new container", e))?;
    let temporary = staging.join("container.hc");
    let mut live_session = None;
    let result = (|| {
        output.msg2("Creating VeraCrypt container without elevation");
        let mut args = crate::workflow::userspace_veracrypt_create_args(c.cipher, size);
        args.push(temporary.as_os_str().into());
        checked(&tools.crypto, &args, Some(&secret), Some(&staging))?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            .map_err(|e| AppError::io("Failed to protect new container", e))?;
        let init_mount = staging.join("filesystem");
        fs::create_dir(&init_mount)
            .map_err(|e| AppError::io("Failed to reserve initial filesystem mount", e))?;
        let (dir, mut session) = runtime.reserve(&temporary, &init_mount, false, true)?;
        live_session = Some((dir.clone(), session.clone()));
        let prepared = (|| {
            open_raw(tools, &dir, &mut session, &secret)?;
            let mkfs = find_program("mkfs.btrfs")?;
            checked(
                &mkfs,
                &["-q".into(), raw_path(&dir).as_os_str().into()],
                None,
                None,
            )?;
            start_filesystem(tools, &dir, &mut session, true)?;
            Ok(())
        })();
        live_session = Some((dir.clone(), session.clone()));
        prepared?;
        close_session(tools, &dir, &session)?;
        live_session = None;
        assert_unused(runtime, &target)?;
        assert_create_target(&target, c.force)?;
        File::open(&temporary)
            .and_then(|f| f.sync_all())
            .map_err(|e| AppError::io("Failed to sync new container", e))?;
        if c.force {
            fs::rename(&temporary, &target)
                .map_err(|e| AppError::io("Failed to publish new container", e))?;
        } else {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &temporary,
                rustix::fs::CWD,
                &target,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|e| AppError::io("Failed to publish new container", e.into()))?;
        }
        Ok(())
    })();
    let mut clean = true;
    if let Some((dir, session)) = live_session {
        if let Err(error) = close_session(tools, &dir, &session) {
            clean = false;
            output.warn(&format!(
                "Container staging retained for recovery at {}: {}",
                staging.display(),
                error.message
            ));
        }
    }
    if clean {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    output.msg2(&format!(
        "Created {} (VeraCrypt, Btrfs, {}) without elevation",
        target.display(),
        c.cipher
    ));
    Ok(())
}

pub fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    if crate::effective_uid() == 0 {
        return Ok(Vec::new());
    }
    let path = runtime_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    secure_directory(&path)?;
    let mounts = read_mountinfo()?;
    let mut result = Vec::new();
    for (dir, session) in sessions_in(&path)? {
        let raw_open = mounts.iter().any(|m| {
            Some(m.mount_id) == session.raw_mount_id
                && m.mount_point == raw_path(&dir).parent().unwrap()
        });
        let mounted = mounts.iter().find(|m| {
            Some(m.mount_id) == session.filesystem_mount_id && m.mount_point == session.mountpoint
        });
        if !raw_open && mounted.is_none() && !session.process.as_ref().is_some_and(process_alive) {
            continue;
        }
        result.push(ActiveVolume {
            privilege_mode: PrivilegeMode::Unprivileged,
            mapper_name: session
                .source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            mapper_path: raw_path(&dir),
            source_path: Some(session.source),
            readonly: session.readonly,
            mountpoint: mounted.map(|_| session.mountpoint),
            filesystem_type: session.filesystem,
        });
    }
    Ok(result)
}

pub fn check_privileged_conflicts(command: &CommandAction) -> AppResult<()> {
    if crate::effective_uid() == 0 {
        return Ok(());
    }
    let source = match command {
        CommandAction::Create(c) => &c.target,
        CommandAction::Mount(c) => &c.source,
        CommandAction::Unmount(_) => return Ok(()),
    };
    let source = absolute_path(&expand(source)?)?;
    let identity = fs::metadata(&source).ok().map(|m| (m.dev(), m.ino()));
    for (_, s) in sessions_in(&runtime_path()?)? {
        if s.source == source || identity == Some((s.source_device, s.source_inode)) {
            return Err(AppError::new("Container has an active unprivileged session. Unmount it before using the privileged route."));
        }
    }
    Ok(())
}
