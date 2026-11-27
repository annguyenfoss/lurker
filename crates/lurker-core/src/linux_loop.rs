//! A short-lived, privileged loop descriptor lease. The caller never receives
//! a privileged device FD: only its already-readable source FD travels upward.
use crate::system::AppContext;
use crate::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::mem::{size_of, size_of_val, zeroed};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PROTOCOL: u32 = 1;
const FRAME_LIMIT: usize = 4096;
const WAIT: Duration = Duration::from_secs(15);
const LOOP_CTL_GET_FREE: libc::c_ulong = 0x4c82;
const LOOP_CONFIGURE: libc::c_ulong = 0x4c0a;
const LO_FLAGS_READ_ONLY: u32 = 1;
const LO_FLAGS_AUTOCLEAR: u32 = 4;

// Stable Linux UAPI <linux/loop.h>; all reserved fields must be zero.
#[repr(C)]
struct LoopInfo {
    device: u64,
    inode: u64,
    rdevice: u64,
    offset: u64,
    size_limit: u64,
    number: u32,
    encrypt_type: u32,
    encrypt_key_size: u32,
    flags: u32,
    name: [u8; 64],
    crypt_name: [u8; 64],
    key: [u8; 32],
    init: [u64; 2],
}
#[repr(C)]
struct LoopConfig {
    fd: u32,
    block_size: u32,
    info: LoopInfo,
    reserved: [u64; 8],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u32,
    device: u64,
    inode: u64,
    readonly: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    protocol: u32,
    device: Option<PathBuf>,
    rdevice: u64,
    error: Option<String>,
}

fn io_error(context: &str) -> AppError {
    AppError::io(context, std::io::Error::last_os_error())
}
fn peer(stream: &UnixStream) -> AppResult<libc::ucred> {
    // SAFETY: the live socket and correctly sized writable credential buffer
    // remain valid throughout this getsockopt call.
    let mut credentials: libc::ucred = unsafe { zeroed() };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut _ as *mut _,
            &mut length,
        )
    } != 0
    {
        return Err(io_error("Cannot verify loop-lease peer"));
    }
    if length as usize != size_of::<libc::ucred>() {
        return Err(AppError::new("Invalid loop-lease peer credentials."));
    }
    Ok(credentials)
}
fn write_frame(stream: &mut UnixStream, bytes: &[u8]) -> AppResult<()> {
    if bytes.len() > FRAME_LIMIT {
        return Err(AppError::new("Loop-lease frame is too large."));
    }
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .and_then(|_| stream.write_all(bytes))
        .map_err(|e| AppError::io("Cannot send loop-lease reply", e))
}
fn read_frame(stream: &mut UnixStream) -> AppResult<Vec<u8>> {
    let mut length = [0; 4];
    stream
        .read_exact(&mut length)
        .map_err(|e| AppError::io("Cannot read loop-lease reply", e))?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > FRAME_LIMIT {
        return Err(AppError::new("Invalid loop-lease frame length."));
    }
    let mut bytes = vec![0; length];
    stream
        .read_exact(&mut bytes)
        .map_err(|e| AppError::io("Cannot read loop-lease payload", e))?;
    Ok(bytes)
}
fn send_source(stream: &mut UnixStream, source: &File, request: &Request) -> AppResult<()> {
    let body = serde_json::to_vec(request).map_err(|e| AppError::new(e.to_string()))?;
    let mut bytes = (body.len() as u32).to_be_bytes().to_vec();
    bytes.extend(body);
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // usize storage guarantees cmsghdr alignment. Exactly one borrowed FD is
    // copied into ancillary data; sendmsg duplicates it in the receiving process.
    let mut control = [0usize; 8];
    let mut message: libc::msghdr = unsafe { zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = unsafe { libc::CMSG_SPACE(size_of::<libc::c_int>() as _) } as usize;
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(size_of::<libc::c_int>() as _) as usize;
        std::ptr::write_unaligned(
            libc::CMSG_DATA(header).cast::<libc::c_int>(),
            source.as_raw_fd(),
        );
    }
    let sent = unsafe { libc::sendmsg(stream.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
    if sent <= 0 {
        return Err(io_error("Cannot transfer locked source descriptor"));
    }
    stream
        .write_all(&bytes[sent as usize..])
        .map_err(|e| AppError::io("Cannot finish loop-lease request", e))
}
fn receive_source(stream: &mut UnixStream) -> AppResult<(File, Request)> {
    let mut prefix = [0u8; 4];
    let mut iov = libc::iovec {
        iov_base: prefix.as_mut_ptr().cast(),
        iov_len: prefix.len(),
    };
    let mut control = [0usize; 16];
    let mut message: libc::msghdr = unsafe { zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = size_of_val(&control);
    let received =
        unsafe { libc::recvmsg(stream.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
    if received <= 0 {
        return Err(io_error("Cannot receive locked source descriptor"));
    }
    let mut files = Vec::<OwnedFd>::new();
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let base = libc::CMSG_LEN(0) as usize;
                if (*header).cmsg_len < base {
                    return Err(AppError::new("Invalid descriptor message."));
                }
                let count = ((*header).cmsg_len - base) / size_of::<libc::c_int>();
                for index in 0..count {
                    let fd = std::ptr::read_unaligned(
                        libc::CMSG_DATA(header).cast::<libc::c_int>().add(index),
                    );
                    files.push(OwnedFd::from_raw_fd(fd));
                }
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    if message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0 || files.len() != 1 {
        return Err(AppError::new(
            "Loop lease requires exactly one complete source descriptor.",
        ));
    }
    stream
        .read_exact(&mut prefix[received as usize..])
        .map_err(|e| AppError::io("Cannot read loop-lease header", e))?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > FRAME_LIMIT {
        return Err(AppError::new("Invalid loop-lease request length."));
    }
    let mut body = vec![0; length];
    stream
        .read_exact(&mut body)
        .map_err(|e| AppError::io("Cannot read loop-lease request", e))?;
    let request: Request = serde_json::from_slice(&body)
        .map_err(|_| AppError::new("Malformed loop-lease request."))?;
    if request.protocol != PROTOCOL {
        return Err(AppError::new("Unsupported loop-lease protocol."));
    }
    Ok((File::from(files.pop().unwrap()), request))
}
fn readable_regular(source: &File, request: &Request) -> AppResult<()> {
    let metadata = source
        .metadata()
        .map_err(|e| AppError::io("Cannot identify loop source", e))?;
    let flags = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || flags & libc::O_PATH != 0
        || flags & libc::O_ACCMODE == libc::O_WRONLY
        || !metadata.is_file()
        || (metadata.dev(), metadata.ino()) != (request.device, request.inode)
    {
        return Err(AppError::new(
            "Loop lease requires the caller's readable, unchanged regular-file descriptor.",
        ));
    }
    if unsafe { libc::flock(source.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(AppError::new("Container is already in use."));
    }
    Ok(())
}
fn allocate(source: &File, request: &Request) -> AppResult<(File, PathBuf)> {
    readable_regular(source, request)?;
    let backing = OpenOptions::new()
        .read(true)
        .write(!request.readonly)
        .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(format!("/proc/self/fd/{}", source.as_raw_fd()))
        .map_err(|e| AppError::io("Cannot reopen pinned loop source", e))?;
    let metadata = backing
        .metadata()
        .map_err(|e| AppError::io("Cannot inspect pinned loop source", e))?;
    if !metadata.is_file() || (metadata.dev(), metadata.ino()) != (request.device, request.inode) {
        return Err(AppError::new("Pinned loop source identity changed."));
    }
    let controller = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open("/dev/loop-control")
        .map_err(|e| AppError::io("Cannot open loop controller", e))?;
    for _ in 0..16 {
        let unit = unsafe { libc::ioctl(controller.as_raw_fd(), LOOP_CTL_GET_FREE) };
        if unit < 0 {
            return Err(io_error("Cannot allocate loop device"));
        }
        let path = PathBuf::from(format!("/dev/loop{unit}"));
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|e| AppError::io("Cannot open allocated loop device", e))?;
        if !device
            .metadata()
            .map_err(|e| AppError::io("Cannot inspect loop device", e))?
            .file_type()
            .is_block_device()
        {
            return Err(AppError::new("Allocated loop path is not a block device."));
        }
        let mut config: LoopConfig = unsafe { zeroed() };
        config.fd = backing.as_raw_fd() as u32;
        config.block_size = 512;
        config.info.flags = LO_FLAGS_AUTOCLEAR
            | if request.readonly {
                LO_FLAGS_READ_ONLY
            } else {
                0
            };
        if unsafe { libc::ioctl(device.as_raw_fd(), LOOP_CONFIGURE, &config) } == 0 {
            return Ok((device, path));
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EBUSY) {
            return Err(AppError::io(
                "Atomic autoclear loop setup failed (Linux 5.8+ is required)",
                error,
            ));
        }
        // A concurrent allocator won. Closing our FD cannot detach its device.
    }
    Err(AppError::new(
        "Loop allocation remained busy; retry the operation.",
    ))
}

/// Internal trusted-helper role. No command, pathname source, cipher or device
/// selector is accepted. A private socket supplies one readable regular-file FD.
pub fn serve_loop_lease(endpoint: &Path) -> AppResult<()> {
    if crate::effective_uid() != 0 {
        return Err(AppError::new(
            "Loop lease requires administrator authorization.",
        ));
    }
    let mut stream =
        UnixStream::connect(endpoint).map_err(|e| AppError::io("Cannot connect loop lease", e))?;
    stream
        .set_read_timeout(Some(WAIT))
        .map_err(|e| AppError::io("Cannot bound loop request", e))?;
    let credentials = peer(&stream)?;
    let authorized = ["SUDO_UID", "PKEXEC_UID"]
        .iter()
        .filter_map(|name| std::env::var(name).ok()?.parse::<u32>().ok())
        .any(|uid| uid == credentials.uid);
    if credentials.uid != 0 && !authorized {
        return Err(AppError::new(
            "Loop-lease peer does not match the authorized user.",
        ));
    }
    let result = (|| {
        let (source, request) = receive_source(&mut stream)?;
        let (device, path) = allocate(&source, &request)?;
        let rdevice = device
            .metadata()
            .map_err(|e| AppError::io("Cannot identify leased loop", e))?
            .rdev();
        write_frame(
            &mut stream,
            &serde_json::to_vec(&Reply {
                protocol: PROTOCOL,
                device: Some(path),
                rdevice,
                error: None,
            })
            .unwrap(),
        )?;
        stream
            .set_read_timeout(None)
            .map_err(|e| AppError::io("Cannot retain loop lease", e))?;
        // EOF is the ownership boundary. Both abrupt caller death and normal
        // handoff close the socket; AUTOCLEAR was set at the initial association.
        let mut byte = [0];
        loop {
            match stream.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => return Err(AppError::new("Unexpected loop-lease control data.")),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(AppError::io("Cannot monitor loop owner", e)),
            }
        }
        drop(device);
        drop(source);
        Ok(())
    })();
    if let Err(ref error) = result {
        let _ = write_frame(
            &mut stream,
            &serde_json::to_vec(&Reply {
                protocol: PROTOCOL,
                device: None,
                rdevice: 0,
                error: Some(error.message.clone()),
            })
            .unwrap(),
        );
    }
    result
}

pub(crate) struct LoopLease {
    device: PathBuf,
    stream: Option<UnixStream>,
    child: Child,
    directory: PathBuf,
}

fn isolate_nonconsole_lease_group(command: &mut Command, elevated_by_sudo: bool) {
    if elevated_by_sudo {
        // This socket-only helper must not share the foreground command's job
        // control group. Preserve its session/TTY for cached sudo authorization.
        command.process_group(0);
    }
}

impl LoopLease {
    pub(crate) fn start(ctx: &mut AppContext, source: &File, readonly: bool) -> AppResult<Self> {
        let helper = helper_program()?;
        let capabilities = Command::new(&helper)
            .arg("capabilities")
            .output()
            .map_err(|e| AppError::io("Cannot inspect loop helper", e))?;
        if !capabilities.status.success() {
            return Err(AppError::new(
                "Trusted loop helper capability query failed.",
            ));
        }
        let capabilities: serde_json::Value = serde_json::from_slice(&capabilities.stdout)
            .map_err(|_| AppError::new("Invalid trusted helper capabilities."))?;
        if capabilities["linux_loop_lease_protocol"].as_u64() != Some(PROTOCOL as u64) {
            return Err(AppError::new("Installed trusted helper lacks safe Linux loop leases; upgrade the complete package."));
        }
        let directory = lease_directory()?;
        let endpoint = directory.join("control.sock");
        let result = (|| {
            let listener = UnixListener::bind(&endpoint)
                .map_err(|e| AppError::io("Cannot reserve loop control socket", e))?;
            listener
                .set_nonblocking(true)
                .map_err(|e| AppError::io("Cannot bound loop helper startup", e))?;
            let mut command = ctx.prepare_command(
                &helper,
                &["loop-lease".into(), endpoint.as_os_str().into()],
                true,
                false,
            )?;
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            isolate_nonconsole_lease_group(&mut command, crate::effective_uid() != 0);
            let child = command
                .spawn()
                .map_err(|e| AppError::io("Cannot start trusted loop helper", e))?;
            let mut lease = Self {
                device: PathBuf::new(),
                stream: None,
                child,
                directory: directory.clone(),
            };
            let start = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if peer(&stream)?.uid != 0 {
                            continue;
                        }
                        break stream;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(AppError::io("Cannot accept trusted loop helper", e)),
                }
                if lease
                    .child
                    .try_wait()
                    .map_err(|e| AppError::io("Cannot inspect loop helper", e))?
                    .is_some()
                {
                    return Err(AppError::new(
                        "Trusted loop helper exited before ownership handoff.",
                    ));
                }
                if start.elapsed() > WAIT {
                    return Err(AppError::new("Trusted loop helper startup timed out."));
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            stream
                .set_read_timeout(Some(WAIT))
                .map_err(|e| AppError::io("Cannot bound loop reply", e))?;
            stream
                .set_write_timeout(Some(WAIT))
                .map_err(|e| AppError::io("Cannot bound loop request", e))?;
            // Retain ownership even on a partial request/reply failure.
            lease.stream = Some(
                stream
                    .try_clone()
                    .map_err(|e| AppError::io("Cannot retain loop control", e))?,
            );
            let metadata = source
                .metadata()
                .map_err(|e| AppError::io("Cannot inspect source", e))?;
            send_source(
                &mut stream,
                source,
                &Request {
                    protocol: PROTOCOL,
                    device: metadata.dev(),
                    inode: metadata.ino(),
                    readonly,
                },
            )?;
            let reply: Reply = serde_json::from_slice(&read_frame(&mut stream)?)
                .map_err(|_| AppError::new("Malformed loop helper response."))?;
            if let Some(error) = reply.error {
                return Err(AppError::new(error));
            }
            let path = reply
                .device
                .ok_or_else(|| AppError::new("Loop helper omitted its device."))?;
            if reply.protocol != PROTOCOL
                || !path
                    .to_str()
                    .and_then(|s| s.strip_prefix("/dev/loop"))
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
                || fs::metadata(&path)
                    .map_err(|e| AppError::io("Cannot verify leased loop node", e))?
                    .rdev()
                    != reply.rdevice
            {
                return Err(AppError::new("Invalid leased loop identity."));
            }
            lease.device = path;
            Ok(lease)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&endpoint);
            let _ = fs::remove_dir(&directory);
        }
        result
    }
    pub(crate) fn device(&self) -> &Path {
        &self.device
    }
    pub(crate) fn release(&mut self) -> AppResult<()> {
        self.stream.take();
        let start = Instant::now();
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|e| AppError::io("Cannot reap loop helper", e))?
            {
                return if status.success() {
                    Ok(())
                } else {
                    Err(AppError::new(format!(
                        "Trusted loop helper exited unexpectedly ({status})."
                    )))
                };
            }
            if start.elapsed() > Duration::from_secs(5) {
                return Err(AppError::new(
                    "Loop helper has not released its descriptor; preserving its identity.",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for LoopLease {
    fn drop(&mut self) {
        let _ = self.release();
        let _ = fs::remove_file(self.directory.join("control.sock"));
        let _ = fs::remove_dir(&self.directory);
    }
}
fn lease_directory() -> AppResult<PathBuf> {
    let mut nonce = [0; 16];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut nonce))
        .map_err(|e| AppError::io("Cannot reserve loop lease", e))?;
    let name: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let directory = PathBuf::from(format!(
        "/tmp/lurker-loop-{}-{name}",
        crate::effective_uid()
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|e| AppError::io("Cannot create private loop control directory", e))?;
    Ok(directory)
}
fn helper_program() -> AppResult<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| AppError::io("Cannot locate application", e))?;
    let mut candidates = vec![exe.parent().unwrap_or(Path::new("/")).join("lurker-helper")];
    if exe.file_name().is_some_and(|n| n == "lurker-helper") {
        candidates.insert(0, exe);
    }
    candidates.extend(
        [
            "/usr/lib/lurker/lurker-helper",
            "/usr/libexec/lurker-helper",
            "/usr/local/libexec/lurker-helper",
            "/usr/bin/lurker-helper",
            "/usr/local/bin/lurker-helper",
        ]
        .map(PathBuf::from),
    );
    for path in candidates {
        if let Ok(path) = crate::trusted_privileged_executable(&path) {
            return Ok(path);
        }
    }
    Err(AppError::new("An administrator-owned lurker-helper is required for safe Linux file mounts. Install the complete package."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsFd;

    #[test]
    fn sudo_lease_group_is_separate_without_changing_session() {
        const PROBE: &str = "LURKER_TEST_LEASE_GROUP_PROBE";
        if std::env::var_os(PROBE).is_some() {
            let identity = unsafe { (libc::getpid(), libc::getpgrp(), libc::getsid(0)) };
            println!(
                "LEASE_GROUP_IDENTITY={}",
                serde_json::to_string(&identity).unwrap()
            );
            return;
        }
        let parent_group = unsafe { libc::getpgrp() };
        let parent_session = unsafe { libc::getsid(0) };
        for sudo in [false, true] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "linux_loop::tests::sudo_lease_group_is_separate_without_changing_session",
                    "--nocapture",
                ])
                .env(PROBE, "1");
            isolate_nonconsole_lease_group(&mut command, sudo);
            let output = command.output().unwrap();
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            let identity = text
                .lines()
                .find_map(|line| line.strip_prefix("LEASE_GROUP_IDENTITY="))
                .unwrap();
            let (pid, group, session): (libc::pid_t, libc::pid_t, libc::pid_t) =
                serde_json::from_str(identity).unwrap();
            assert_eq!(
                session, parent_session,
                "preserve cached authorization context"
            );
            assert_eq!(group, if sudo { pid } else { parent_group });
        }
    }

    fn fixture(name: &str) -> (PathBuf, File, Request) {
        let path = PathBuf::from(format!(
            "/tmp/lurker-loop-protocol-{}-{name}",
            std::process::id()
        ));
        let source = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let metadata = source.metadata().unwrap();
        let request = Request {
            protocol: PROTOCOL,
            device: metadata.dev(),
            inode: metadata.ino(),
            readonly: true,
        };
        (path, source, request)
    }

    #[test]
    fn source_transfer_retains_inode_lock_and_close_on_exec() {
        let (path, source, request) = fixture("transfer");
        readable_regular(&source, &request).unwrap();
        let (mut client, mut server) = UnixStream::pair().unwrap();
        send_source(&mut client, &source, &request).unwrap();
        drop(source);
        let rival = File::open(&path).unwrap();
        assert_ne!(
            unsafe { libc::flock(rival.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0,
            "queued SCM_RIGHTS must retain source exclusion"
        );
        let (received, actual) = receive_source(&mut server).unwrap();
        assert_eq!(
            (actual.device, actual.inode),
            (request.device, request.inode)
        );
        for descriptor in [received.as_fd(), client.as_fd(), server.as_fd()] {
            assert_ne!(
                unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
                0
            );
        }
        drop(received);
        drop(client);
        drop(server);
        assert_eq!(
            unsafe { libc::flock(rival.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        drop(rival);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_descriptor_and_wrong_protocol_are_rejected() {
        let (path, source, mut request) = fixture("protocol");
        let (mut client, mut server) = UnixStream::pair().unwrap();
        write_frame(&mut client, &serde_json::to_vec(&request).unwrap()).unwrap();
        assert!(receive_source(&mut server).is_err());
        let (mut client, mut server) = UnixStream::pair().unwrap();
        request.protocol += 1;
        send_source(&mut client, &source, &request).unwrap();
        assert!(receive_source(&mut server)
            .unwrap_err()
            .message
            .contains("Unsupported loop-lease protocol"));
        drop(source);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_only_path_only_nonregular_and_replaced_sources_are_rejected() {
        let (path, source, mut request) = fixture("source-types");
        let write_only = OpenOptions::new().write(true).open(&path).unwrap();
        assert!(readable_regular(&write_only, &request).is_err());
        let path_only = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH)
            .open(&path)
            .unwrap();
        assert!(readable_regular(&path_only, &request).is_err());
        let directory = File::open("/tmp").unwrap();
        assert!(readable_regular(&directory, &request).is_err());
        request.inode += 1;
        assert!(readable_regular(&source, &request).is_err());
        drop(source);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn kernel_loop_uapi_layout_matches_linux_header() {
        assert_eq!(size_of::<LoopInfo>(), 232);
        assert_eq!(size_of::<LoopConfig>(), 304);
        assert_eq!(std::mem::offset_of!(LoopInfo, flags), 52);
        assert_eq!(std::mem::offset_of!(LoopConfig, info), 8);
        assert_eq!(std::mem::offset_of!(LoopConfig, reserved), 240);
    }
}
