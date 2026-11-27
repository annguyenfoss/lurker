//! Opt-in first real FreeBSD Cryptomator pilot. No production admission and no
//! signal, unmount, path removal or failure cleanup of any real resource.
use super::super::{AppError, AppResult, Process, alive, freebsd_boot_id, process_snapshot};
use super::{Command as Rpc, Lease, State, random_hex, request};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::ExitStatusExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const UID: u32 = 1300;
const OUTPUT_LIMIT: u64 = 65536;
const STARTUP: Duration = Duration::from_secs(15);
const IO_WAIT: Duration = Duration::from_secs(10);
const CLOSE_WAIT: Duration = Duration::from_secs(15);

fn failed() -> AppError {
    AppError::new(
        "Real managed pilot stopped; retain all private evidence, credentials, processes and storage.",
    )
}
fn require(condition: bool) -> AppResult<()> {
    if condition { Ok(()) } else { Err(failed()) }
}
fn time() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn private_dir(path: &Path) -> AppResult<()> {
    DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|_| failed())
}
fn private_file(path: &Path) -> AppResult<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| failed())
}
fn durable(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut f = private_file(path)?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| failed())?;
    File::open(path.parent().ok_or_else(failed)?)
        .and_then(|f| f.sync_all())
        .map_err(|_| failed())
}
fn record(path: &Path, value: &impl Serialize) -> AppResult<()> {
    durable(path, &serde_json::to_vec(value).map_err(|_| failed())?)
}
fn bounded(path: &Path, limit: u64) -> AppResult<Vec<u8>> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| failed())?;
    let m = f.metadata().map_err(|_| failed())?;
    require(
        m.is_file()
            && m.uid() == UID
            && m.mode() & 0o077 == 0
            && m.nlink() == 1
            && m.len() <= limit,
    )?;
    let mut bytes = Vec::new();
    f.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failed())?;
    require(bytes.len() as u64 <= limit)?;
    Ok(bytes)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Node {
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
}
impl Node {
    fn from(m: &fs::Metadata) -> Self {
        Self {
            device: m.dev(),
            inode: m.ino(),
            owner: m.uid(),
            mode: m.mode() & 0o7777,
        }
    }
    fn directory(path: &Path) -> AppResult<Self> {
        let m = fs::symlink_metadata(path).map_err(|_| failed())?;
        require(m.is_dir() && m.uid() == UID && m.mode() & 0o7777 == 0o700)?;
        Ok(Self::from(&m))
    }
    fn verify_directory(&self, path: &Path) -> AppResult<()> {
        require(&Self::directory(path)? == self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Mount {
    fsid: Vec<u8>,
    source: String,
    target: PathBuf,
    filesystem: String,
    owner: u32,
    flags: u64,
}
fn native_text(bytes: &[libc::c_char]) -> AppResult<String> {
    String::from_utf8(
        bytes
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| *b as u8)
            .collect(),
    )
    .map_err(|_| failed())
}
fn mount_nowait(point: &Path) -> AppResult<Option<Mount>> {
    // Caller-owned getfsstat buffer avoids getmntinfo's shared static storage.
    // NOWAIT omission is never completion/release authority.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    require(count >= 0 && count <= 4096)?;
    let capacity = count as usize + 16;
    let mut entries: Vec<libc::statfs> = (0..capacity)
        .map(|_| unsafe { std::mem::zeroed() })
        .collect();
    let bytes = entries.len() * std::mem::size_of::<libc::statfs>();
    let count = unsafe { libc::getfsstat(entries.as_mut_ptr(), bytes as _, libc::MNT_NOWAIT) };
    require(count >= 0 && (count as usize) < capacity)?;
    let mut result = None;
    for entry in &entries[..count as usize] {
        if Path::new(&native_text(&entry.f_mntonname)?) != point {
            continue;
        }
        require(result.is_none())?;
        let fsid = unsafe {
            std::slice::from_raw_parts(
                (&entry.f_fsid as *const libc::fsid_t).cast::<u8>(),
                std::mem::size_of::<libc::fsid_t>(),
            )
        }
        .to_vec();
        result = Some(Mount {
            fsid,
            source: native_text(&entry.f_mntfromname)?,
            target: point.into(),
            filesystem: native_text(&entry.f_fstypename)?,
            owner: entry.f_owner,
            flags: entry.f_flags as u64,
        });
    }
    Ok(result)
}
fn positive_mount(point: &Path) -> AppResult<Mount> {
    let m = mount_nowait(point)?.ok_or_else(failed)?;
    let nonce = m
        .source
        .strip_prefix("lurker-cryptomator-")
        .ok_or_else(failed)?;
    require(
        m.filesystem == "fusefs"
            && m.owner == UID
            && m.flags & libc::MNT_RDONLY as u64 == 0
            && m.fsid.iter().any(|b| *b != 0)
            && nonce.len() == 32
            && nonce
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
    )?;
    Ok(m)
}

fn wait_status(status: &ExitStatus) -> Value {
    json!({"code":status.code(),"signal":status.signal(),"raw_status":status.clone().into_raw()})
}

struct Owned {
    child: Child,
    process: Process,
    stdout: PathBuf,
    stderr: PathBuf,
}
// Deliberately no Drop implementation. Even failures never kill/signal/unmount.
impl Owned {
    fn live(&mut self) -> AppResult<()> {
        require(
            self.child.try_wait().map_err(|_| failed())?.is_none()
                && alive(&self.process).map_err(|_| failed())?,
        )
    }
    fn wait(&mut self, duration: Duration) -> AppResult<ExitStatus> {
        let end = Instant::now() + duration;
        loop {
            if let Some(status) = self.child.try_wait().map_err(|_| failed())? {
                return Ok(status);
            }
            require(Instant::now() < end)?;
            thread::sleep(Duration::from_millis(10));
        }
    }
}

struct Session {
    directory: PathBuf,
    directory_node: Node,
    evidence: PathBuf,
    evidence_node: Node,
    lease: Lease,
    process: Owned,
    mount: Option<Mount>,
    mounted_root: Option<Node>,
}
struct Pilot {
    root: PathBuf,
    root_node: Node,
    vault: PathBuf,
    source: Node,
    short: PathBuf,
    short_node: Node,
    point: PathBuf,
    underlay: Node,
    held_underlay: File,
    launcher: PathBuf,
    launcher_node: Node,
    launcher_hash: String,
    password: String,
    payload: Vec<u8>,
    payload_hash: String,
    boot: Option<[u8; 16]>,
    journal: File,
}
impl Pilot {
    fn stage(&mut self, stage: &str, outcome: &str, observation: Value) -> AppResult<()> {
        let line = serde_json::to_vec(
            &json!({"time_ms":time(),"stage":stage,"outcome":outcome,"observation":observation}),
        )
        .map_err(|_| failed())?;
        self.journal
            .write_all(&line)
            .and_then(|_| self.journal.write_all(b"\n"))
            .and_then(|_| self.journal.sync_all())
            .map_err(|_| failed())
    }
    fn stable(&self) -> AppResult<()> {
        require(unsafe { libc::getuid() } == UID && unsafe { libc::geteuid() } == UID)?;
        self.root_node.verify_directory(&self.root)?;
        self.source.verify_directory(&self.vault)?;
        self.short_node.verify_directory(&self.short)?;
        let held = self.held_underlay.metadata().map_err(|_| failed())?;
        require(
            Node::from(&held) == self.underlay
                && freebsd_boot_id().map_err(|_| failed())? == self.boot,
        )?;
        let m = fs::symlink_metadata(&self.launcher).map_err(|_| failed())?;
        require(m.is_file() && Node::from(&m) == self.launcher_node && m.len() <= 1024 * 1024)?;
        require(
            crate::sha256::hex_digest(&fs::read(&self.launcher).map_err(|_| failed())?)
                == self.launcher_hash,
        )
    }
    fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut c = Command::new(program);
        c.env_clear()
            .env(
                "PATH",
                "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/sbin:/usr/local/bin",
            )
            .env("LC_ALL", "C")
            .env("HOME", "/home/adjutant")
            .current_dir(&self.root);
        c
    }
    fn spawn(&mut self, dir: &Path, name: &str, mut command: Command) -> AppResult<Owned> {
        self.stable()?;
        let stdout = dir.join(format!("{name}.stdout"));
        let stderr = dir.join(format!("{name}.stderr"));
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        record(
            &dir.join(format!("{name}-launch.json")),
            &json!({"program":command.get_program().to_string_lossy(),"argv":args,"controller":process_snapshot(std::process::id()).map_err(|_| failed())?,"uid":UID,"phase":"before_spawn"}),
        )?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::from(private_file(&stdout)?))
            .stderr(Stdio::from(private_file(&stderr)?));
        let child = command.spawn().map_err(|_| failed())?;
        let process = process_snapshot(child.id())
            .map_err(|_| failed())?
            .ok_or_else(failed)?;
        // Actual child birth is durable before any credential or worker JSON input.
        record(&dir.join(format!("{name}-process.json")), &process)?;
        self.stage(
            name,
            "spawned-input-not-yet-sent",
            json!({"process":process}),
        )?;
        Ok(Owned {
            child,
            process,
            stdout,
            stderr,
        })
    }
    fn send(owned: &mut Owned, bytes: &[u8]) -> AppResult<()> {
        let mut input = owned.child.stdin.take().ok_or_else(failed)?;
        input.write_all(bytes).map_err(|_| failed())?;
        drop(input);
        Ok(())
    }
    fn outputs(&self, owned: &Owned, lease: Option<&Lease>) -> AppResult<(Vec<u8>, Vec<u8>)> {
        let out = bounded(&owned.stdout, OUTPUT_LIMIT)?;
        let err = bounded(&owned.stderr, OUTPUT_LIMIT)?;
        for bytes in [&out, &err] {
            require(
                !bytes
                    .windows(self.password.len())
                    .any(|part| part == self.password.as_bytes()),
            )?;
            if let Some(lease) = lease {
                require(
                    !bytes
                        .windows(lease.bootstrap_token().len())
                        .any(|part| part == lease.bootstrap_token().as_bytes()),
                )?;
            }
        }
        Ok((out, err))
    }
    fn initialize(&mut self) -> AppResult<()> {
        self.stage("initialize", "started", json!({}))?;
        let mut command = self.command(&self.launcher);
        command.arg("init").arg(&self.vault);
        let root = self.root.clone();
        let mut owned = self.spawn(&root, "initialize", command)?;
        Self::send(&mut owned, format!("{}\n", self.password).as_bytes())?;
        let status = owned.wait(Duration::from_secs(30))?;
        record(
            &self.root.join("initialize-wait.json"),
            &json!({"process":owned.process,"actual_wait":wait_status(&status)}),
        )?;
        let (out, _) = self.outputs(&owned, None)?;
        require(
            status.code() == Some(0)
                && records(&out, true)? == vec![json!({"protocol":1,"status":"initialized"})],
        )?;
        self.stable()?;
        self.stage(
            "initialize",
            "passed",
            json!({"process":owned.process,"actual_wait_code":0,"initialized_ack":true}),
        )
    }
    fn launch(&mut self, number: u8) -> AppResult<Session> {
        self.stable()?;
        self.underlay.verify_directory(&self.point)?;
        require(mount_nowait(&self.point)?.is_none())?; // supporting refusal only, not release authority
        let evidence = self.root.join(format!("mount-{number}"));
        private_dir(&evidence)?;
        let directory = self.short.join(format!("s{number}"));
        private_dir(&directory)?;
        let directory_node = Node::directory(&directory)?;
        let evidence_node = Node::directory(&evidence)?;
        let lease = Lease::new()?;
        record(
            &evidence.join("private-session.json"),
            &json!({"protocol":1,"lease":lease,"backend_process":null,"controller":process_snapshot(std::process::id()).map_err(|_| failed())?,"source":self.source,"vault":self.vault,"underlay":self.underlay,"point":self.point,"session":directory,"session_identity":directory_node,"evidence_identity":evidence_node,"root_identity":self.root_node,"boot":self.boot}),
        )?;
        self.stage(
            "mount-launch",
            "prepared",
            json!({"number":number,"session":directory,"evidence":evidence}),
        )?;
        let mut command = self.command(&self.launcher);
        command
            .arg("mount")
            .arg(&self.vault)
            .arg(&self.point)
            .arg("--managed-close")
            .arg(&directory);
        let mut process = self.spawn(&evidence, "backend", command)?;
        Self::send(
            &mut process,
            format!("{}\n{}\n", self.password, lease.bootstrap_token()).as_bytes(),
        )?;
        let mut session = Session {
            directory,
            directory_node,
            evidence,
            evidence_node,
            lease,
            process,
            mount: None,
            mounted_root: None,
        };
        let end = Instant::now() + STARTUP;
        let mut previous_state = None;
        loop {
            session.process.live()?;
            self.session_stable(&session)?;
            let (out, _) = self.outputs(&session.process, Some(&session.lease))?;
            let status = request(
                &session.directory,
                &session.lease,
                &session.process.process,
                Rpc::Status,
            );
            // Before the endpoint exists, readiness is pending. Once socket exists,
            // an unverifiable RPC is an unknown outcome, not a new mount/retry.
            let exists = fs::symlink_metadata(session.directory.join("control.sock")).is_ok();
            if exists {
                let state = status?;
                if previous_state != Some(state) {
                    self.stage(
                        "startup-state",
                        "observed",
                        json!({"state":format!("{state:?}"),"process":session.process.process}),
                    )?;
                    previous_state = Some(state);
                }
                require(matches!(state, State::Starting | State::Mounted))?;
                if state == State::Mounted {
                    let ready = records(&out, false)?;
                    require(
                        ready.is_empty() || ready == vec![json!({"protocol":1,"status":"mounted"})],
                    )?;
                    if ready.is_empty() {
                        require(Instant::now() < end)?;
                        thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    session.mount = Some(positive_mount(&self.point)?);
                    self.stage("mounted", "passed", json!({"number":number,"process":session.process.process,"mount":session.mount}))?;
                    return Ok(session);
                }
            }
            require(Instant::now() < end)?;
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn session_stable(&self, s: &Session) -> AppResult<()> {
        self.stable()?;
        s.directory_node.verify_directory(&s.directory)?;
        s.evidence_node.verify_directory(&s.evidence)
    }
    fn same_mount(&self, s: &Session) -> AppResult<()> {
        self.session_stable(s)?;
        require(s.mount.as_ref() == Some(&positive_mount(&self.point)?))
    }
    fn payload(&mut self, s: &mut Session, name: &str, write: bool) -> AppResult<()> {
        self.same_mount(s)?;
        s.process.live()?;
        let mut command = self.command(std::env::current_exe().map_err(|_| failed())?);
        command.args([
            "--exact",
            "folder::managed::real::native_cryptomator_real_payload",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]);
        let mut worker = self.spawn(&s.evidence, name, command)?;
        let input = Payload {
            point: self.point.clone(),
            write,
            bytes: self.payload.clone(),
            expected_root: s.mounted_root.clone(),
            parent: self.short_node.clone(),
            underlay: self.underlay.clone(),
        };
        Self::send(
            &mut worker,
            &serde_json::to_vec(&input).map_err(|_| failed())?,
        )?;
        let status = worker.wait(IO_WAIT)?;
        record(
            &s.evidence.join(format!("{name}-wait.json")),
            &json!({"process":worker.process,"actual_wait":wait_status(&status)}),
        )?;
        let (out, _) = self.outputs(&worker, Some(&s.lease))?;
        require(status.code() == Some(0))?;
        let text = std::str::from_utf8(&out).map_err(|_| failed())?;
        let found: Vec<_> = text
            .lines()
            .filter_map(|line| {
                line.split_once("LFMC_REAL_PAYLOAD ")
                    .map(|(_, value)| value)
            })
            .collect();
        require(found.len() == 1)?;
        let receipt: PayloadReceipt = serde_json::from_str(found[0]).map_err(|_| failed())?;
        require(
            receipt.protocol == 1
                && receipt.uid == UID
                && receipt.euid == UID
                && receipt.sha256 == self.payload_hash
                && receipt.bytes == self.payload.len()
                && receipt.file_fsync
                && receipt.directory_fsync
                && receipt.file_close
                && receipt.directory_close
                && receipt.file.owner == UID
                && receipt.file.mode == 0o600
                && receipt.root.owner == UID,
        )?;
        if let Some(expected) = &s.mounted_root {
            require(expected == &receipt.root)?;
        }
        s.mounted_root = Some(receipt.root.clone());
        self.same_mount(s)?;
        record(&s.evidence.join(format!("{name}-receipt.json")), &receipt)?;
        self.stage(
            name,
            "passed",
            json!({"process":worker.process,"actual_wait_code":0,"receipt":receipt}),
        )
    }
    fn close(&mut self, s: &mut Session, expected: State) -> AppResult<()> {
        self.same_mount(s)?;
        s.process.live()?;
        self.stage(
            "close",
            "started",
            json!({"expected":format!("{expected:?}"),"process":s.process.process,"mount":s.mount}),
        )?;
        let mut state = request(&s.directory, &s.lease, &s.process.process, Rpc::Close)?;
        self.stage(
            "close-first-reply",
            "observed",
            json!({"state":format!("{state:?}"),"process":s.process.process}),
        )?;
        let end = Instant::now() + CLOSE_WAIT;
        while state == State::Closing {
            require(Instant::now() < end)?;
            thread::sleep(Duration::from_millis(20));
            state = request(&s.directory, &s.lease, &s.process.process, Rpc::Status)?;
        }
        self.stage("close-response", "observed", json!({"state":format!("{state:?}"),"expected":format!("{expected:?}"),"process":s.process.process}))?;
        require(state == expected)?;
        if expected == State::Retained {
            s.process.live()?;
            self.same_mount(s)?;
            self.stage("busy-close", "retained-confirmed", json!({"process":s.process.process,"mount":s.mount,"live_authenticated_retained":true}))?;
            return Ok(());
        }
        require(expected == State::Closed)?;
        // Only the live authenticated CLOSED authorizes waiting for natural exit;
        // no statfs omission, trace or result file can substitute for this reply.
        let status = s.process.wait(Duration::from_secs(15))?;
        record(
            &s.evidence.join("backend-wait.json"),
            &json!({"process":s.process.process,"actual_wait":wait_status(&status),"live_bound_closed":true}),
        )?;
        let (out, _) = self.outputs(&s.process, Some(&s.lease))?;
        require(
            status.code() == Some(0)
                && records(&out, true)?
                    == vec![
                        json!({"protocol":1,"status":"mounted"}),
                        json!({"protocol":1,"status":"closed"}),
                    ],
        )?;
        self.session_stable(s)?;
        let after = mount_nowait(&self.point)?;
        self.stage(
            "post-close-nowait",
            "supporting-observation-only",
            json!({"mount":after,"live_closed_and_actual_wait_already_verified":true}),
        )?;
        require(after.is_none())?;
        self.underlay.verify_directory(&self.point)?;
        self.stage("normal-close", "passed", json!({"process":s.process.process,"live_bound_closed":true,"actual_wait_code":0,"strict_independent_closed_stdout":true,"held_underlay_restored":true,"nowait_absence_supporting_only":true,"native_provider_worker_receipts":"operator collects independently"}))
    }
    fn busy(&mut self, s: &mut Session) -> AppResult<()> {
        self.same_mount(s)?;
        let fifo = s.evidence.join("cwd-release.fifo");
        let bytes = fifo.as_os_str().as_bytes();
        let c = std::ffi::CString::new(bytes).map_err(|_| failed())?;
        require(unsafe { libc::mkfifo(c.as_ptr(), 0o600) } == 0)?;
        let mut pipe = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&fifo)
            .map_err(|_| failed())?;
        let fm = fs::symlink_metadata(&fifo).map_err(|_| failed())?;
        require(
            fm.file_type().is_fifo()
                && fm.uid() == UID
                && fm.mode() & 0o7777 == 0o600
                && fm.nlink() == 1,
        )?;
        let pin = Node::from(&fm);
        require(Node::from(&pipe.metadata().map_err(|_| failed())?) == pin)?;
        record(
            &s.evidence.join("cwd-fifo.json"),
            &json!({"path":fifo,"identity":pin,"normal_release":"literal RELEASE line; no signal"}),
        )?;
        let script = "cd \"$1\" || exit 71; exec 3<>\"$2\" || exit 72; printf 'CWD_READY\\n'; while :; do IFS= read -r command <&3 || { /bin/sleep 3600; continue; }; [ \"$command\" = RELEASE ] || continue; cd / || exit 73; printf 'CWD_RELEASED\\n'; exit 0; done";
        let mut command = self.command("/bin/sh");
        command
            .args(["-c", script, "lfmc-cwd"])
            .arg(&self.point)
            .arg(&fifo);
        let mut consumer = self.spawn(&s.evidence, "cwd-consumer", command)?;
        // Consumer uses its persistent RDWR FIFO, not stdin/EOF cleanup.
        drop(consumer.child.stdin.take());
        let end = Instant::now() + IO_WAIT;
        loop {
            consumer.live()?;
            let (out, _) = self.outputs(&consumer, Some(&s.lease))?;
            if out == b"CWD_READY\n" {
                break;
            }
            require(b"CWD_READY\n".starts_with(&out) && Instant::now() < end)?;
            thread::sleep(Duration::from_millis(20));
        }
        self.close(s, State::Retained)?;
        self.payload(s, "busy-retained-read", false)?;
        consumer.live()?;
        s.process.live()?;
        self.same_mount(s)?;
        let fm = fs::symlink_metadata(&fifo).map_err(|_| failed())?;
        require(
            fm.file_type().is_fifo()
                && fm.nlink() == 1
                && Node::from(&fm) == pin
                && Node::from(&pipe.metadata().map_err(|_| failed())?) == pin,
        )?;
        pipe.write_all(b"RELEASE\n").map_err(|_| failed())?;
        let status = consumer.wait(IO_WAIT)?;
        record(
            &s.evidence.join("cwd-consumer-wait.json"),
            &json!({"process":consumer.process,"actual_wait":wait_status(&status)}),
        )?;
        let (out, _) = self.outputs(&consumer, Some(&s.lease))?;
        require(status.code() == Some(0) && out == b"CWD_READY\nCWD_RELEASED\n")?;
        self.stage(
            "cwd-release",
            "passed",
            json!({"process":consumer.process,"actual_wait_code":0,"fifo_identity":pin}),
        )?;
        self.close(s, State::Closed)
    }
    fn run(&mut self) -> AppResult<()> {
        self.initialize()?;
        let mut first = self.launch(1)?;
        self.payload(&mut first, "payload-write", true)?;
        self.close(&mut first, State::Closed)?;
        // Never reached after an unknown/failed first mount, I/O or close.
        let mut second = self.launch(2)?;
        self.payload(&mut second, "reopen-read", false)?;
        self.busy(&mut second)?;
        self.stage("pilot", "passed", json!({"real_mounts":2,"normal_closes":2,"busy_retained_then_fresh_retry":true,"production_admission":false}))
    }
}

fn records(bytes: &[u8], eof: bool) -> AppResult<Vec<Value>> {
    require(bytes.len() as u64 <= OUTPUT_LIMIT)?;
    let complete = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    if eof {
        require(complete == bytes.len())?;
    }
    bytes[..complete]
        .split_inclusive(|b| *b == b'\n')
        .map(|line| serde_json::from_slice(line).map_err(|_| failed()))
        .collect()
}
fn safe_ancestors(path: &Path) -> AppResult<()> {
    for p in path.ancestors() {
        let m = fs::symlink_metadata(p).map_err(|_| failed())?;
        require(m.is_dir() && (m.uid() == 0 || m.uid() == UID) && m.mode() & 0o022 == 0)?;
    }
    Ok(())
}
fn prepare() -> AppResult<Pilot> {
    require(unsafe { libc::getuid() } == UID && unsafe { libc::geteuid() } == UID)?;
    let launcher =
        PathBuf::from(std::env::var_os("LURKER_TEST_REAL_CRYPTOMATOR").ok_or_else(failed)?);
    let root = PathBuf::from(std::env::var_os("LURKER_TEST_REAL_EVIDENCE").ok_or_else(failed)?);
    require(
        launcher.is_absolute()
            && root.is_absolute()
            && root
                .components()
                .all(|c| !matches!(c, Component::ParentDir | Component::CurDir)),
    )?;
    require(fs::canonicalize(&launcher).map_err(|_| failed())? == launcher)?;
    safe_ancestors(launcher.parent().ok_or_else(failed)?)?;
    let parent = root.parent().ok_or_else(failed)?;
    require(fs::canonicalize(parent).map_err(|_| failed())? == parent)?;
    safe_ancestors(parent)?;
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(failed()),
    }
    let lm = fs::symlink_metadata(&launcher).map_err(|_| failed())?;
    require(
        lm.is_file()
            && (lm.uid() == UID || lm.uid() == 0)
            && lm.mode() & 0o022 == 0
            && lm.mode() & 0o111 != 0
            && lm.len() <= 1024 * 1024,
    )?;
    let launcher_hash = crate::sha256::hex_digest(&fs::read(&launcher).map_err(|_| failed())?);
    private_dir(&root)?;
    let root_node = Node::directory(&root)?;
    let journal = private_file(&root.join("stages.jsonl"))?;
    let password = random_hex::<32>()?;
    durable(
        &root.join("private-password"),
        format!("{password}\n").as_bytes(),
    )?;
    let vault = root.join("vault");
    private_dir(&vault)?;
    let source = Node::directory(&vault)?;
    let short = tempfile::Builder::new()
        .prefix("lcm-pilot-")
        .tempdir_in("/tmp")
        .map_err(|_| failed())?
        .keep();
    let short_node = Node::directory(&short)?;
    let point = short.join("m");
    private_dir(&point)?;
    let underlay = Node::directory(&point)?;
    let held_underlay = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&point)
        .map_err(|_| failed())?;
    require(Node::from(&held_underlay.metadata().map_err(|_| failed())?) == underlay)?;
    let payload = format!("Lurker real managed pilot {}\n", random_hex::<16>()?).into_bytes();
    let payload_hash = crate::sha256::hex_digest(&payload);
    durable(&root.join("payload.expected"), &payload)?;
    let boot = freebsd_boot_id().map_err(|_| failed())?;
    record(
        &root.join("prepared.json"),
        &json!({"protocol":1,"uid":UID,"controller":process_snapshot(std::process::id()).map_err(|_| failed())?,"boot":boot,"root":root,"root_identity":root_node,"vault":vault,"source_identity":source,"short":short,"short_identity":short_node,"point":point,"underlay":underlay,"launcher":launcher,"launcher_sha256":launcher_hash,"payload_sha256":payload_hash,"payload_bytes":payload.len(),"secrets":"private-password and mount-N/private-session.json only; never publish"}),
    )?;
    let mut pilot = Pilot {
        root,
        root_node,
        vault,
        source,
        short,
        short_node,
        point,
        underlay,
        held_underlay,
        launcher,
        launcher_node: Node::from(&lm),
        launcher_hash,
        password,
        payload,
        payload_hash,
        boot,
        journal,
    };
    pilot.stable()?;
    pilot.stage(
        "prepare",
        "passed",
        json!({"uid":UID,"no_runtime_started":true}),
    )?;
    Ok(pilot)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    point: PathBuf,
    write: bool,
    bytes: Vec<u8>,
    expected_root: Option<Node>,
    parent: Node,
    underlay: Node,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadReceipt {
    protocol: u32,
    uid: u32,
    euid: u32,
    root: Node,
    file: Node,
    sha256: String,
    bytes: usize,
    file_fsync: bool,
    directory_fsync: bool,
    file_close: bool,
    directory_close: bool,
}
fn checked_payload_close(file: File) -> AppResult<()> {
    // A close error has ambiguous completion. Never retry the numeric fd and
    // never dispose any backend or mount on this payload failure.
    let fd = file.into_raw_fd();
    require(unsafe { libc::close(fd) } == 0)
}

fn payload_child() -> AppResult<PayloadReceipt> {
    require(unsafe { libc::getuid() } == UID && unsafe { libc::geteuid() } == UID)?;
    let input: Payload =
        serde_json::from_reader(std::io::stdin().lock().take(16385)).map_err(|_| failed())?;
    require(input.bytes.len() <= 4096 && input.point.file_name() == Some(OsStr::new("m")))?;
    let parent = input.point.parent().ok_or_else(failed)?;
    require(
        parent.parent() == Some(Path::new("/tmp"))
            && parent
                .file_name()
                .is_some_and(|s| s.as_bytes().starts_with(b"lcm-pilot-")),
    )?;
    input.parent.verify_directory(parent)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&input.point)
        .map_err(|_| failed())?;
    let dm = directory.metadata().map_err(|_| failed())?;
    require(dm.is_dir() && dm.uid() == UID)?;
    let root = Node::from(&dm);
    require(root != input.underlay)?;
    if let Some(expected) = input.expected_root {
        require(expected == root)?;
    }
    let open_payload = |write_new: bool, readonly: bool| -> AppResult<File> {
        let flags = (if readonly {
            libc::O_RDONLY
        } else {
            libc::O_RDWR
        }) | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if write_new {
                libc::O_CREAT | libc::O_EXCL
            } else {
                0
            };
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                c"pilot.txt".as_ptr(),
                flags,
                0o600 as libc::c_uint,
            )
        };
        if fd < 0 {
            return Err(failed());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    };
    let mut file = open_payload(input.write, false)?;
    let fm = file.metadata().map_err(|_| failed())?;
    require(fm.is_file() && fm.uid() == UID && fm.mode() & 0o7777 == 0o600 && fm.nlink() == 1)?;
    if input.write {
        file.write_all(&input.bytes).map_err(|_| failed())?;
    }
    file.sync_all().map_err(|_| failed())?;
    checked_payload_close(file)?;
    let mut file = open_payload(false, true)?;
    let fm2 = file.metadata().map_err(|_| failed())?;
    require(fm2.is_file() && Node::from(&fm2) == Node::from(&fm))?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| failed())?;
    require(
        bytes == input.bytes && Node::from(&directory.metadata().map_err(|_| failed())?) == root,
    )?;
    checked_payload_close(file)?;
    directory.sync_all().map_err(|_| failed())?;
    checked_payload_close(directory)?;
    Ok(PayloadReceipt {
        protocol: 1,
        uid: unsafe { libc::getuid() },
        euid: unsafe { libc::geteuid() },
        root,
        file: Node::from(&fm2),
        sha256: crate::sha256::hex_digest(&bytes),
        bytes: bytes.len(),
        file_fsync: true,
        directory_fsync: true,
        file_close: true,
        directory_close: true,
    })
}

#[test]
#[ignore = "REAL FreeBSD mount pilot; requires root-reviewed frozen controller and native host owner approval"]
fn native_cryptomator_real_pilot() {
    let mut pilot = prepare().unwrap_or_else(|_| {
        panic!(
            "Real pilot preparation failed; preserve private fixture paths; no cleanup attempted."
        )
    });
    if pilot.run().is_err() {
        let _ = pilot.stage("pilot", "failed-retained", json!({"no_kill_signal_unmount_or_path_removal":true,"operator_recovery_review_required":true,"publish_raw_logs_only_after_secret_guard":true}));
        panic!(
            "Real pilot failed; retain all private fixtures, processes, credentials and storage for operator review."
        );
    }
}

#[test]
#[ignore = "internal bounded payload worker; stdin JSON only, never password/token; invoked solely by approved real pilot"]
fn native_cryptomator_real_payload() {
    let receipt = payload_child().unwrap_or_else(|_| {
        panic!(
            "Real payload failed; preserve worker/backend/storage evidence; no cleanup attempted."
        )
    });
    println!(
        "LFMC_REAL_PAYLOAD {}",
        serde_json::to_string(&receipt).expect("payload receipt encoding")
    );
}
