//! Authenticated LFMC1 observations. This client never detaches or signals a mount.
use super::{AppError, AppResult, Process, alive, process_snapshot};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Lease {
    protocol: u32,
    token: String,
}

impl fmt::Debug for Lease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lease")
            .field("protocol", &self.protocol)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl Lease {
    pub(super) fn new() -> AppResult<Self> {
        Ok(Self {
            protocol: 1,
            token: random_hex::<32>()?,
        })
    }

    pub(super) fn validate(&self) -> AppResult<()> {
        if self.protocol != 1 || !lower_hex(&self.token, 64) {
            return Err(failure());
        }
        Ok(())
    }

    /// Supply only over the backend's private bootstrap channel, never argv/logs.
    pub(super) fn bootstrap_token(&self) -> &str {
        &self.token
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Status,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    Starting,
    Mounted,
    Closing,
    Retained,
    Closed,
}

fn failure() -> AppError {
    AppError::new("Managed folder control could not be verified; preserve its session.")
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn random_hex<const N: usize>() -> AppResult<String> {
    let mut bytes = [0u8; N];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| failure())?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(N * 2);
    for b in bytes {
        text.push(HEX[(b >> 4) as usize] as char);
        text.push(HEX[(b & 15) as usize] as char);
    }
    Ok(text)
}

#[derive(Clone, Copy)]
struct Limits {
    connect: Duration,
    write: Duration,
    total: Duration,
}
const LIMITS: Limits = Limits {
    connect: Duration::from_millis(500),
    write: Duration::from_millis(500),
    total: Duration::from_secs(2),
};

#[derive(Clone, Copy)]
struct Node {
    device: u64,
    inode: u64,
}
impl Node {
    fn read(m: &fs::Metadata) -> Self {
        Self {
            device: m.dev(),
            inode: m.ino(),
        }
    }
    fn matches(self, m: &fs::Metadata) -> bool {
        self.device == m.dev() && self.inode == m.ino()
    }
}

fn private_node(m: &fs::Metadata, uid: u32, mode: u32, socket: bool) -> bool {
    m.uid() == uid
        && m.mode() & 0o7777 == mode
        && if socket {
            m.file_type().is_socket()
        } else {
            m.is_dir()
        }
}

struct Endpoint {
    directory: File,
    directory_node: Node,
    socket_node: Node,
    uid: u32,
}
impl Endpoint {
    fn open(dir: &Path) -> AppResult<Self> {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(dir)
            .map_err(|_| failure())?;
        let held = directory.metadata().map_err(|_| failure())?;
        let named = fs::symlink_metadata(dir).map_err(|_| failure())?;
        let uid = unsafe { libc::geteuid() };
        let directory_node = Node::read(&held);
        if !private_node(&held, uid, 0o700, false)
            || !private_node(&named, uid, 0o700, false)
            || !directory_node.matches(&named)
        {
            return Err(failure());
        }
        let socket = fs::symlink_metadata(dir.join("control.sock")).map_err(|_| failure())?;
        if !private_node(&socket, uid, 0o600, true) {
            return Err(failure());
        }
        Ok(Self {
            directory,
            directory_node,
            socket_node: Node::read(&socket),
            uid,
        })
    }

    fn validate(&self, dir: &Path) -> AppResult<()> {
        let held = self.directory.metadata().map_err(|_| failure())?;
        let named = fs::symlink_metadata(dir).map_err(|_| failure())?;
        let socket = fs::symlink_metadata(dir.join("control.sock")).map_err(|_| failure())?;
        if !private_node(&held, self.uid, 0o700, false)
            || !private_node(&named, self.uid, 0o700, false)
            || !private_node(&socket, self.uid, 0o600, true)
            || !self.directory_node.matches(&held)
            || !self.directory_node.matches(&named)
            || !self.socket_node.matches(&socket)
        {
            return Err(failure());
        }
        Ok(())
    }
}

fn before(deadline: Instant) -> AppResult<()> {
    if Instant::now() >= deadline {
        Err(failure())
    } else {
        Ok(())
    }
}

fn poll(fd: RawFd, events: i16, deadline: Instant) -> AppResult<()> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(failure)?;
        let millis = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let mut descriptor = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut descriptor, 1, millis) };
        if result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        before(deadline)?;
        if result <= 0 || descriptor.revents & libc::POLLNVAL != 0 {
            return Err(failure());
        }
        if descriptor.revents & (events | libc::POLLHUP | libc::POLLERR) != 0 {
            return Ok(());
        }
    }
}

#[cfg(any(target_os = "freebsd", all(test, target_os = "linux")))]
fn connect(path: &Path, deadline: Instant) -> AppResult<OwnedFd> {
    let bytes = path.as_os_str().as_bytes();
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.is_empty() || bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(failure());
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, value) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *value as libc::c_char;
    }
    let length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;
    #[cfg(target_os = "freebsd")]
    {
        address.sun_len = length as u8;
    }
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(failure());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    before(deadline)?;
    let result = unsafe {
        libc::connect(
            raw,
            (&address as *const libc::sockaddr_un).cast(),
            length as libc::socklen_t,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(failure());
        }
        poll(raw, libc::POLLOUT, deadline)?;
        let mut error: libc::c_int = 0;
        let mut size = std::mem::size_of_val(&error) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                raw,
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                (&mut error as *mut libc::c_int).cast(),
                &mut size,
            )
        } != 0
            || size as usize != std::mem::size_of_val(&error)
            || error != 0
        {
            return Err(failure());
        }
    }
    before(deadline)?;
    Ok(fd)
}

#[cfg(not(any(target_os = "freebsd", all(test, target_os = "linux"))))]
fn connect(_: &Path, _: Instant) -> AppResult<OwnedFd> {
    Err(failure())
}

#[cfg(not(any(target_os = "freebsd", all(test, target_os = "linux"))))]
fn transmit(_: RawFd, _: &[u8], _: Instant) -> AppResult<()> {
    Err(failure())
}

fn peer_uid(fd: RawFd, pid: u32) -> AppResult<u32> {
    #[cfg(target_os = "freebsd")]
    {
        let (mut uid, mut gid) = (0, 0);
        if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } != 0 {
            return Err(failure());
        }
        let _ = pid;
        Ok(uid)
    }
    #[cfg(all(test, target_os = "linux"))]
    {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of_val(&credentials) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        } != 0
            || length as usize != std::mem::size_of_val(&credentials)
            || credentials.pid as u32 != pid
        {
            return Err(failure());
        }
        Ok(credentials.uid)
    }
    #[cfg(not(any(target_os = "freebsd", all(test, target_os = "linux"))))]
    {
        let _ = (fd, pid);
        Err(failure())
    }
}

#[cfg(any(target_os = "freebsd", all(test, target_os = "linux")))]
fn transmit(fd: RawFd, mut bytes: &[u8], deadline: Instant) -> AppResult<()> {
    while !bytes.is_empty() {
        before(deadline)?;
        let size =
            unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) };
        if size > 0 {
            bytes = &bytes[size as usize..];
            continue;
        }
        let error = std::io::Error::last_os_error();
        if size < 0 && error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if size < 0 && error.kind() == std::io::ErrorKind::WouldBlock {
            poll(fd, libc::POLLOUT, deadline)?;
            continue;
        }
        return Err(failure());
    }
    before(deadline)
}

fn receive(fd: RawFd, deadline: Instant) -> AppResult<Vec<u8>> {
    let mut frame = Vec::with_capacity(129);
    loop {
        before(deadline)?;
        let mut bytes = [0u8; 129];
        let size = unsafe {
            libc::recv(
                fd,
                bytes.as_mut_ptr().cast(),
                (129 - frame.len()).min(bytes.len()),
                0,
            )
        };
        if size == 0 {
            return Ok(frame);
        }
        if size > 0 {
            frame.extend_from_slice(&bytes[..size as usize]);
            if frame.len() > 128 {
                return Err(failure());
            }
            continue;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if error.kind() == std::io::ErrorKind::WouldBlock {
            poll(fd, libc::POLLIN, deadline)?;
            continue;
        }
        return Err(failure());
    }
}

fn parse(frame: &[u8], nonce: &str, pid: u32) -> AppResult<State> {
    if frame.is_empty()
        || frame.len() > 128
        || frame.last() != Some(&b'\n')
        || !frame[..frame.len() - 1]
            .iter()
            .all(|b| (0x20..=0x7e).contains(b))
    {
        return Err(failure());
    }
    let text = std::str::from_utf8(&frame[..frame.len() - 1]).map_err(|_| failure())?;
    let fields: Vec<_> = text.split(' ').collect();
    if fields.len() != 4
        || fields[0] != "LFMC1"
        || fields[2] != nonce
        || fields[3] != pid.to_string()
    {
        return Err(failure());
    }
    match fields[1] {
        "starting" => Ok(State::Starting),
        "mounted" => Ok(State::Mounted),
        "closing" => Ok(State::Closing),
        "retained" => Ok(State::Retained),
        "closed" => Ok(State::Closed),
        _ => Err(failure()),
    }
}

fn verify_snapshot(expected: &Process, current: Option<Process>, state: State) -> AppResult<()> {
    match current {
        Some(current) if current.pid == expected.pid && current.birth == expected.birth => Ok(()),
        None if state == State::Closed => Ok(()),
        _ => Err(failure()),
    }
}

pub(super) fn request(
    dir: &Path,
    lease: &Lease,
    process: &Process,
    command: Command,
) -> AppResult<State> {
    request_with_limits(dir, lease, process, command, LIMITS)
}

#[cfg(any(target_os = "freebsd", all(test, target_os = "linux")))]
fn request_with_limits(
    dir: &Path,
    lease: &Lease,
    process: &Process,
    command: Command,
    limits: Limits,
) -> AppResult<State> {
    let start = Instant::now();
    let total = start + limits.total;
    lease.validate()?;
    if process.pid == 0 || !alive(process).map_err(|_| failure())? {
        return Err(failure());
    }
    let endpoint = Endpoint::open(dir)?;
    let nonce = random_hex::<16>()?;
    let socket = connect(
        &dir.join("control.sock"),
        (start + limits.connect).min(total),
    )?;
    endpoint.validate(dir)?;
    if peer_uid(socket.as_raw_fd(), process.pid)? != endpoint.uid {
        return Err(failure());
    }
    if !alive(process).map_err(|_| failure())? {
        return Err(failure());
    }
    let command = match command {
        Command::Status => "status",
        Command::Close => "close",
    };
    let frame = format!("LFMC1 {command} {} {nonce}\n", lease.bootstrap_token());
    transmit(
        socket.as_raw_fd(),
        frame.as_bytes(),
        (Instant::now() + limits.write).min(total),
    )?;
    let frame = receive(socket.as_raw_fd(), total)?;
    let state = parse(&frame, &nonce, process.pid)?;
    endpoint.validate(dir)?;
    verify_snapshot(
        process,
        process_snapshot(process.pid).map_err(|_| failure())?,
        state,
    )?;
    before(total)?;
    Ok(state)
}

#[cfg(not(any(target_os = "freebsd", all(test, target_os = "linux"))))]
fn request_with_limits(
    _: &Path,
    _: &Lease,
    _: &Process,
    _: Command,
    _: Limits,
) -> AppResult<State> {
    Err(failure())
}

#[cfg(all(test, any(target_os = "linux", target_os = "freebsd")))]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::thread;

    const SHORT: Limits = Limits {
        connect: Duration::from_millis(100),
        write: Duration::from_millis(100),
        total: Duration::from_millis(200),
    };

    fn fixture() -> (tempfile::TempDir, UnixListener, Lease, Process) {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let listener = UnixListener::bind(dir.path().join("control.sock")).unwrap();
        fs::set_permissions(
            dir.path().join("control.sock"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        (
            dir,
            listener,
            Lease::new().unwrap(),
            process_snapshot(std::process::id()).unwrap().unwrap(),
        )
    }

    fn frame(stream: &mut UnixStream) -> Vec<String> {
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        assert!(line.ends_with('\n'));
        let fields: Vec<String> = line
            .trim_end_matches('\n')
            .split(' ')
            .map(str::to_owned)
            .collect();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0], "LFMC1");
        assert!(lower_hex(&fields[2], 64));
        assert!(lower_hex(&fields[3], 32));
        fields
    }

    fn response<F>(respond: F) -> AppResult<State>
    where
        F: FnOnce(&[String]) -> Vec<u8> + Send + 'static,
    {
        let (dir, listener, lease, process) = fixture();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let fields = frame(&mut stream);
            let _ = stream.write_all(&respond(&fields));
        });
        let result = request_with_limits(dir.path(), &lease, &process, Command::Status, SHORT);
        server.join().unwrap();
        result
    }

    #[test]
    fn lease_generation_validation_roundtrip_and_redaction() {
        let first = Lease::new().unwrap();
        let second = Lease::new().unwrap();
        assert_ne!(first.bootstrap_token(), second.bootstrap_token());
        first.validate().unwrap();
        assert!(!format!("{first:?}").contains(first.bootstrap_token()));
        let decoded: Lease = serde_json::from_str(&serde_json::to_string(&first).unwrap()).unwrap();
        assert_eq!(first.bootstrap_token(), decoded.bootstrap_token());
        for (protocol, token) in [
            (2, "a".repeat(64)),
            (1, "A".repeat(64)),
            (1, "a".repeat(63)),
            (1, format!("{}\n", "a".repeat(63))),
        ] {
            let lease = Lease { protocol, token };
            assert!(lease.validate().is_err());
        }
    }

    #[test]
    fn actual_socket_accepts_all_exact_states_and_close_command() {
        for (wire, state) in [
            ("starting", State::Starting),
            ("mounted", State::Mounted),
            ("closing", State::Closing),
            ("retained", State::Retained),
            ("closed", State::Closed),
        ] {
            assert_eq!(
                response(
                    move |f| format!("LFMC1 {wire} {} {}\n", f[3], std::process::id()).into_bytes()
                )
                .unwrap(),
                state
            );
        }
        let (dir, listener, lease, process) = fixture();
        let token = lease.bootstrap_token().to_owned();
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let f = frame(&mut s);
            assert_eq!(f[1], "close");
            assert_eq!(f[2], token);
            s.write_all(format!("LFMC1 closing {} {}\n", f[3], std::process::id()).as_bytes())
                .unwrap();
        });
        assert_eq!(
            request(dir.path(), &lease, &process, Command::Close).unwrap(),
            State::Closing
        );
        server.join().unwrap();
    }

    #[test]
    fn actual_socket_rejects_nonce_pid_malformed_duplicate_and_oversized() {
        for kind in 0..10 {
            assert!(
                response(move |f| {
                    let good = format!("LFMC1 mounted {} {}\n", f[3], std::process::id());
                    match kind {
                        0 => format!("LFMC1 mounted {} {}\n", "0".repeat(32), std::process::id())
                            .into_bytes(),
                        1 => format!("LFMC1 mounted {} 0\n", f[3]).into_bytes(),
                        2 => good.replace("LFMC1 ", "LFMC1  ").into_bytes(),
                        3 => good.replace('\n', "\r\n").into_bytes(),
                        4 => good.repeat(2).into_bytes(),
                        5 => vec![b'x'; 129],
                        6 => good.trim_end().as_bytes().to_vec(),
                        7 => good.replace("mounted", "unknown").into_bytes(),
                        8 => good.replace("LFMC1", "LFMC2").into_bytes(),
                        _ => vec![0xff, b'\n'],
                    }
                })
                .is_err(),
                "accepted malformed variant {kind}"
            );
        }
    }

    #[test]
    fn blocked_peer_and_missing_eof_are_bounded() {
        for send_line in [false, true] {
            let (dir, listener, lease, process) = fixture();
            let server = thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                let f = frame(&mut s);
                if send_line {
                    let _ = s.write_all(
                        format!("LFMC1 closed {} {}\n", f[3], std::process::id()).as_bytes(),
                    );
                }
                thread::sleep(Duration::from_millis(350));
            });
            let start = Instant::now();
            assert!(
                request_with_limits(dir.path(), &lease, &process, Command::Close, SHORT).is_err()
            );
            assert!(start.elapsed() < Duration::from_secs(1));
            server.join().unwrap();
        }
    }

    #[test]
    fn missing_socket_and_wrong_process_birth_fail_without_secrets() {
        let (dir, listener, lease, mut process) = fixture();
        process.birth.push_str("-replaced");
        let error = request(dir.path(), &lease, &process, Command::Status)
            .unwrap_err()
            .to_string();
        assert!(!error.contains(lease.bootstrap_token()));
        assert!(!error.contains(&dir.path().display().to_string()));
        process = process_snapshot(std::process::id()).unwrap().unwrap();
        drop(listener);
        fs::remove_file(dir.path().join("control.sock")).unwrap();
        assert!(request(dir.path(), &lease, &process, Command::Status).is_err());
    }

    #[test]
    fn permissions_owner_type_and_symlinks_are_not_authority() {
        let (dir, _listener, lease, process) = fixture();
        let path = dir.path().join("control.sock");
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!private_node(
            &metadata,
            unsafe { libc::geteuid() }.wrapping_add(1),
            0o600,
            true
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
        assert!(request(dir.path(), &lease, &process, Command::Status).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o750)).unwrap();
        assert!(request(dir.path(), &lease, &process, Command::Status).is_err());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(&path).unwrap();
        File::create(&path).unwrap();
        assert!(request(dir.path(), &lease, &process, Command::Status).is_err());
        fs::remove_file(&path).unwrap();
        symlink("absent", &path).unwrap();
        assert!(request(dir.path(), &lease, &process, Command::Status).is_err());
    }

    #[test]
    fn socket_replacement_after_valid_closed_reply_is_rejected() {
        let (dir, listener, lease, process) = fixture();
        let path = dir.path().join("control.sock");
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let f = frame(&mut s);
            fs::remove_file(&path).unwrap();
            let _replacement = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            s.write_all(format!("LFMC1 closed {} {}\n", f[3], std::process::id()).as_bytes())
                .unwrap();
        });
        assert!(request_with_limits(dir.path(), &lease, &process, Command::Close, SHORT).is_err());
        server.join().unwrap();
    }

    #[test]
    fn closed_can_outlive_original_process_but_never_a_replacement() {
        let expected = Process {
            pid: 123,
            birth: "original".into(),
        };
        assert!(verify_snapshot(&expected, None, State::Closed).is_ok());
        assert!(verify_snapshot(&expected, None, State::Mounted).is_err());
        assert!(
            verify_snapshot(
                &expected,
                Some(Process {
                    pid: 123,
                    birth: "replacement".into()
                }),
                State::Closed
            )
            .is_err()
        );
        assert!(verify_snapshot(&expected, Some(expected.clone()), State::Closed).is_ok());
    }
    #[test]
    fn actual_peer_credentials_and_socket_write_deadline() {
        let (a, b) = UnixStream::pair().unwrap();
        assert_eq!(
            peer_uid(a.as_raw_fd(), std::process::id()).unwrap(),
            unsafe { libc::geteuid() }
        );
        #[cfg(target_os = "linux")]
        assert!(peer_uid(a.as_raw_fd(), std::process::id().wrapping_add(1)).is_err());
        a.set_nonblocking(true).unwrap();
        let start = Instant::now();
        assert!(
            transmit(
                a.as_raw_fd(),
                &vec![b'x'; 8 * 1024 * 1024],
                start + Duration::from_millis(50)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(b);
    }

    #[test]
    fn trickled_reply_does_not_reset_total_deadline() {
        let (dir, listener, lease, process) = fixture();
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let f = frame(&mut s);
            for b in format!("LFMC1 mounted {} {}\n", f[3], std::process::id()).bytes() {
                if s.write_all(&[b]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(40));
            }
        });
        let start = Instant::now();
        assert!(request_with_limits(dir.path(), &lease, &process, Command::Status, SHORT).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
        server.join().unwrap();
    }

    #[test]
    fn held_directory_replacement_and_socket_removal_reject_closed() {
        for replace_directory in [false, true] {
            let (dir, listener, lease, process) = fixture();
            let path = dir.path().to_owned();
            let displaced = path.with_extension("displaced");
            let displaced_server = displaced.clone();
            let server = thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                let f = frame(&mut s);
                if replace_directory {
                    fs::rename(&path, &displaced_server).unwrap();
                    fs::create_dir(&path).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                } else {
                    fs::remove_file(path.join("control.sock")).unwrap();
                }
                s.write_all(format!("LFMC1 closed {} {}\n", f[3], std::process::id()).as_bytes())
                    .unwrap();
            });
            assert!(
                request_with_limits(dir.path(), &lease, &process, Command::Close, SHORT).is_err()
            );
            server.join().unwrap();
            if replace_directory {
                fs::remove_dir_all(displaced).unwrap();
            }
        }
    }

    #[test]
    fn each_wire_request_has_a_fresh_nonce_and_errors_do_not_echo_reply() {
        let (dir, listener, lease, process) = fixture();
        let secret = lease.bootstrap_token().to_owned();
        let secret_server = secret.clone();
        let path_text = dir.path().display().to_string();
        let path_server = path_text.clone();
        let server = thread::spawn(move || {
            let mut nonces = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = listener.accept().unwrap();
                let f = frame(&mut s);
                nonces.push(f[3].clone());
                s.write_all(format!("{} {}\n", secret_server, path_server).as_bytes())
                    .unwrap();
            }
            assert_ne!(nonces[0], nonces[1]);
        });
        for _ in 0..2 {
            let error = request_with_limits(dir.path(), &lease, &process, Command::Status, SHORT)
                .unwrap_err()
                .to_string();
            assert!(!error.contains(&secret));
            assert!(!error.contains(&path_text));
        }
        server.join().unwrap();
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn full_native_accept_queue_cannot_block_connect() {
        let (dir, listener, lease, process) = fixture();
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
        let _queued = UnixStream::connect(dir.path().join("control.sock")).unwrap();
        let start = Instant::now();
        assert!(request_with_limits(dir.path(), &lease, &process, Command::Status, SHORT).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    /// Real FreeBSD getpeereid/process_snapshot + genuine Java LFMC1 lifecycle;
    /// all provider/mount/CryptoFS objects in the JVM are deliberately fake.
    #[test]
    #[cfg(target_os = "freebsd")]
    #[ignore = "requires qualified native JDK/classpath; run only through the FreeBSD owner"]
    fn native_java_managed_close_interop() {
        java_interop::run();
    }

    #[cfg(target_os = "freebsd")]
    mod java_interop {
        use super::*;
        use std::ffi::OsString;
        use std::process::{Child, Command as SpawnCommand, ExitStatus, Stdio};

        struct FakeJvm {
            child: Child,
            process: Process,
            directory: std::path::PathBuf,
            lease: Lease,
            mode: &'static str,
        }

        impl FakeJvm {
            fn output(&self, name: &str) -> String {
                let path = self.directory.join(name);
                assert!(
                    fs::metadata(&path).unwrap().len() <= 65536,
                    "unbounded fake fixture output"
                );
                fs::read_to_string(path).unwrap()
            }

            fn trace(&self) -> Vec<String> {
                self.output("fake-close.trace")
                    .lines()
                    .map(str::to_owned)
                    .collect()
            }

            fn live(&mut self) {
                assert!(
                    self.child.try_wait().unwrap().is_none(),
                    "fake JVM exited before live completion proof"
                );
                assert!(
                    alive(&self.process).unwrap(),
                    "fake JVM birth identity changed"
                );
            }

            fn state(&mut self, expected: State) {
                let deadline = Instant::now() + Duration::from_secs(15);
                loop {
                    self.live();
                    let state =
                        request(&self.directory, &self.lease, &self.process, Command::Status)
                            .unwrap();
                    if state == expected {
                        return;
                    }
                    assert!(
                        matches!(state, State::Starting | State::Closing),
                        "unexpected live fake fixture state: {state:?}"
                    );
                    assert!(Instant::now() < deadline, "fake JVM state deadline");
                    thread::sleep(Duration::from_millis(10));
                }
            }

            fn close(&mut self, eventual: State) {
                self.live();
                let state =
                    request(&self.directory, &self.lease, &self.process, Command::Close).unwrap();
                assert!(
                    state == State::Closing || state == eventual,
                    "unexpected close response: {state:?}"
                );
                if state != eventual {
                    self.state(eventual);
                }
            }

            fn natural_exit(&mut self) {
                let deadline = Instant::now() + Duration::from_secs(5);
                let status = loop {
                    if let Some(status) = self.child.try_wait().unwrap() {
                        break status;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "no natural JVM exit after bound CLOSED reply"
                    );
                    thread::sleep(Duration::from_millis(10));
                };
                assert_eq!(
                    status.code(),
                    Some(0),
                    "positive fake fixture must exit naturally with zero"
                );
                let output = self.output("java.stdout");
                assert_eq!(output.lines().next(), Some("FIXTURE_READY"));
                let records: Vec<serde_json::Value> = output
                    .lines()
                    .skip(1)
                    .map(|line| {
                        serde_json::from_str(line).expect("strict independent Java stdout JSON")
                    })
                    .collect();
                let closed = serde_json::json!({"protocol": 1, "status": "closed"});
                let mounted = serde_json::json!({"protocol": 1, "status": "mounted"});
                if self.mode == "startup-failure" {
                    assert_eq!(records, vec![closed]);
                } else {
                    assert_eq!(records, vec![mounted, closed]);
                }
                assert!(!output.contains(self.lease.bootstrap_token()));
                assert!(
                    !self
                        .output("java.stderr")
                        .contains(self.lease.bootstrap_token())
                );
                eprintln!(
                    "LFMC1_INTEROP_NATURAL_EXIT mode={} pid={} exit=0 bound_closed=true independent_stdout_closed=true",
                    self.mode, self.process.pid
                );
            }

            fn manual_cleanup(&mut self, label: &str) -> Option<ExitStatus> {
                if let Ok(Some(status)) = self.child.try_wait() {
                    return Some(status);
                }
                // Only this test-created FAKE child is eligible. No native FUSE,
                // mounted filesystem or CryptoFS object exists in the fixture.
                if !alive(&self.process).unwrap_or(false) {
                    eprintln!(
                        "MANUAL_FIXTURE_CLEANUP_REFUSED mode={} pid={} identity_unverified",
                        self.mode, self.process.pid
                    );
                    return None;
                }
                eprintln!(
                    "{label} mode={} pid={} fake_only=true not_normal_close=true",
                    self.mode, self.process.pid
                );
                if self.child.kill().is_err() {
                    return None;
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    if let Ok(Some(status)) = self.child.try_wait() {
                        return Some(status);
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                None
            }
        }

        impl Drop for FakeJvm {
            fn drop(&mut self) {
                // Failure cleanup is explicitly distinguishable from a positive
                // natural-exit result; preserve stdout/stderr/trace on disk.
                if !matches!(self.child.try_wait(), Ok(Some(_))) {
                    let _ = self.manual_cleanup("MANUAL_FIXTURE_FAILURE_CLEANUP");
                }
            }
        }

        fn spawn(java: &Path, classpath: &OsString, mode: &'static str) -> FakeJvm {
            let directory = tempfile::Builder::new()
                .prefix("lfmc1-")
                .tempdir_in("/tmp")
                .unwrap()
                .keep();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            let lease = Lease::new().unwrap();
            let mut child = SpawnCommand::new(java)
                .args(["--add-modules", "jdk.net", "-cp"])
                .arg(classpath)
                .arg("org.lurker.cryptomator.ManagedCloseInteropFixture")
                .arg(&directory)
                .arg(mode)
                .env_remove("CLASSPATH")
                .env_remove("JAVA_TOOL_OPTIONS")
                .env_remove("JDK_JAVA_OPTIONS")
                .env_remove("_JAVA_OPTIONS")
                .stdin(Stdio::piped())
                .stdout(Stdio::from(
                    File::create(directory.join("java.stdout")).unwrap(),
                ))
                .stderr(Stdio::from(
                    File::create(directory.join("java.stderr")).unwrap(),
                ))
                .spawn()
                .unwrap();
            eprintln!(
                "LFMC1_INTEROP_SPAWN mode={mode} directory={} pid={} fake_only=true",
                directory.display(),
                child.id()
            );
            let process = process_snapshot(child.id()).unwrap().expect("actual native Java child birth identity; preserve recorded fake PID/logs if unavailable");
            eprintln!(
                "LFMC1_INTEROP_EVIDENCE mode={mode} directory={} pid={}",
                directory.display(),
                process.pid
            );
            let mut input = child.stdin.take().unwrap();
            let mut fixture = FakeJvm {
                child,
                process,
                directory,
                lease,
                mode,
            };
            writeln!(input, "{}", fixture.lease.bootstrap_token()).unwrap();
            drop(input); // Main.readManagedToken requires exact token + LF + EOF.
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                fixture.live();
                if fixture.output("java.stdout").lines().next() == Some("FIXTURE_READY") {
                    break;
                }
                assert!(Instant::now() < deadline, "fake Java bootstrap deadline");
                thread::sleep(Duration::from_millis(10));
            }
            fixture
        }

        pub(super) fn run() {
            // Test-only opt-in selection. Production never reads these variables.
            let java = std::path::PathBuf::from(
                std::env::var_os("LURKER_TEST_MANAGED_JAVA")
                    .expect("set existing qualified native JDK java path"),
            );
            let classpath = std::env::var_os("LURKER_TEST_MANAGED_CLASSPATH")
                .expect("set existing native fixture/Main/ManagedClose dependency classpath");
            assert!(java.is_absolute() && java.is_file() && !classpath.is_empty());
            for mode in ["normal", "busy", "startup-failure", "sticky-cryptofs"] {
                let mut fixture = spawn(&java, &classpath, mode);
                fixture.state(if mode == "startup-failure" {
                    State::Retained
                } else {
                    State::Mounted
                });
                assert!(fixture.trace().is_empty());
                if mode == "busy" {
                    fixture.close(State::Retained);
                    fixture.live();
                    assert_eq!(fixture.trace(), vec!["provider-close"]);
                    // request() creates a fresh nonce for this explicit retry.
                    fixture.close(State::Closed);
                    assert_eq!(
                        fixture.trace(),
                        vec!["provider-close", "provider-close", "cryptofs-close"]
                    );
                    fixture.natural_exit();
                } else if mode == "sticky-cryptofs" {
                    fixture.close(State::Retained);
                    fixture.live();
                    let first = fixture.trace();
                    assert_eq!(first, vec!["provider-close", "cryptofs-close"]);
                    fixture.close(State::Retained);
                    fixture.state(State::Retained);
                    fixture.live();
                    assert_eq!(
                        fixture.trace(),
                        first,
                        "sticky CryptoFS error must never repeat either closer"
                    );
                    assert!(
                        !fixture
                            .output("java.stdout")
                            .lines()
                            .any(|line| line.contains("\"closed\""))
                    );
                    assert!(
                        !fixture
                            .output("java.stdout")
                            .contains(fixture.lease.bootstrap_token())
                    );
                    assert!(
                        !fixture
                            .output("java.stderr")
                            .contains(fixture.lease.bootstrap_token())
                    );
                    let status = fixture
                        .manual_cleanup("MANUAL_FIXTURE_CLEANUP_AFTER_PROVEN_RETENTION")
                        .expect("reap only the birth-verified fake retained JVM");
                    assert!(
                        !status.success(),
                        "manual fake cleanup is not a natural positive exit"
                    );
                } else {
                    fixture.close(State::Closed);
                    assert_eq!(fixture.trace(), vec!["provider-close", "cryptofs-close"]);
                    fixture.natural_exit();
                }
            }
        }
    }
}

#[cfg(all(test, target_os = "freebsd"))]
#[path = "managed_real.rs"]
mod real;
