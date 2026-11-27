//! Strict records for the separately installed FreeBSD filesystem driver.
//!
//! Parsing a record does not authorize encrypted-provider teardown. The eventual
//! consumer must also verify the persisted session/source/status inode, acquire
//! the final status lock, and prove the captured native mount is globally absent.
use crate::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const PROTOCOL: &str = "freebsd-lkl-host-io-1";
const MAX_RECORD_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProcessIdentity {
    pid: u32,
    birth_seconds: i64,
    birth_microseconds: u32,
    boot_seconds: i64,
    boot_microseconds: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Record {
    Pending,
    Ready(ProcessIdentity),
    Drained(ProcessIdentity),
    Error(ProcessIdentity),
}

/// Persist this before launching the producer. Neither a pathname nor a nonce
/// alone can identify a shutdown acknowledgement after a later helper request.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StatusLease {
    directory: PathBuf,
    directory_identity: super::super::Identity,
    status_identity: super::super::Identity,
    nonce: String,
}

/// Keep the descriptor lock through the caller's final identity checks. This
/// record still requires source, process/boot and global mount-absence proofs.
struct OpenedRecord {
    record: Record,
    _directory: File,
    _status: File,
}

impl StatusLease {
    fn prepare(registry: &super::super::Registry, session_id: &str) -> AppResult<Self> {
        super::super::require_root()?;
        Self::prepare_owned(&registry.directory, session_id, 0)
    }

    fn prepare_owned(parent: &Path, session_id: &str, owner: u32) -> AppResult<Self> {
        if session_id.is_empty()
            || session_id.len() > 128
            || !session_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(invalid_record());
        }
        let parent_fd = open_private_directory(parent, owner, None)?;
        let directory = parent.join(format!("lkl-{session_id}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| AppError::io("Reserve private FreeBSD filesystem status", e))?;
        let directory_fd = open_private_directory(&directory, owner, None)?;
        let status = open_status_at(&directory_fd, true)?;
        check_metadata(
            &status
                .metadata()
                .map_err(|e| AppError::io("Inspect filesystem status", e))?,
            owner,
            false,
            None,
        )?;
        let mut random = [0u8; 32];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|e| AppError::io("Generate filesystem status nonce", e))?;
        let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let bytes = format!("{PROTOCOL}\n{nonce}\npending\n");
        (&status)
            .write_all(bytes.as_bytes())
            .and_then(|()| status.sync_all())
            .map_err(|e| AppError::io("Persist pending filesystem status", e))?;
        directory_fd
            .sync_all()
            .and_then(|()| parent_fd.sync_all())
            .map_err(|e| AppError::io("Persist filesystem status directory", e))?;
        Ok(Self {
            directory,
            directory_identity: super::super::Identity::from_file(&directory_fd)?,
            status_identity: super::super::Identity::from_file(&status)?,
            nonce,
        })
    }

    fn read_current(&self) -> AppResult<OpenedRecord> {
        self.read_owned(0, false)
    }

    /// A producer holds an exclusive lock until its final write/truncate/sync
    /// and close. Never accept a final record while that descriptor is active.
    fn read_final(&self, expected: ProcessIdentity) -> AppResult<OpenedRecord> {
        self.read_final_owned(0, expected)
    }

    fn read_final_owned(&self, owner: u32, expected: ProcessIdentity) -> AppResult<OpenedRecord> {
        let opened = self.read_owned(owner, true)?;
        if opened.record != Record::Drained(expected) {
            return Err(invalid_record());
        }
        Ok(opened)
    }

    fn read_owned(&self, owner: u32, final_record: bool) -> AppResult<OpenedRecord> {
        let directory =
            open_private_directory(&self.directory, owner, Some(&self.directory_identity))?;
        let status = open_status_at(&directory, false)?;
        check_metadata(
            &status
                .metadata()
                .map_err(|e| AppError::io("Inspect filesystem status", e))?,
            owner,
            false,
            Some(&self.status_identity),
        )?;
        if final_record
            && unsafe { libc::flock(status.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0
        {
            return Err(AppError::io(
                "Filesystem status is still in use; preserve its encrypted session",
                std::io::Error::last_os_error(),
            ));
        }
        let mut bytes = Vec::new();
        (&status)
            .take((MAX_RECORD_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::io("Read bounded filesystem status", e))?;
        let record = parse_record(&bytes, &self.nonce)?;
        // Reject replacement, new hard links or permission changes after open.
        // The held directory descriptor prevents following a replaced status
        // symlink; the pathname checks additionally bind the persisted lease.
        check_metadata(
            &status
                .metadata()
                .map_err(|e| AppError::io("Recheck filesystem status", e))?,
            owner,
            false,
            Some(&self.status_identity),
        )?;
        check_metadata(
            &fs::symlink_metadata(self.directory.join("status"))
                .map_err(|e| AppError::io("Recheck filesystem status path", e))?,
            owner,
            false,
            Some(&self.status_identity),
        )?;
        check_metadata(
            &fs::symlink_metadata(&self.directory)
                .map_err(|e| AppError::io("Recheck filesystem status directory", e))?,
            owner,
            true,
            Some(&self.directory_identity),
        )?;
        Ok(OpenedRecord {
            record,
            _directory: directory,
            _status: status,
        })
    }
}

fn check_metadata(
    metadata: &fs::Metadata,
    owner: u32,
    directory: bool,
    expected: Option<&super::super::Identity>,
) -> AppResult<()> {
    if metadata.uid() != owner
        || metadata.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > MAX_RECORD_BYTES as u64
        }
        || expected.is_some_and(|identity| {
            identity.dev != metadata.dev() || identity.ino != metadata.ino()
        })
    {
        return Err(invalid_record());
    }
    Ok(())
}

fn open_private_directory(
    path: &Path,
    owner: u32,
    expected: Option<&super::super::Identity>,
) -> AppResult<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| AppError::io("Open private filesystem status directory", e))?;
    check_metadata(
        &file
            .metadata()
            .map_err(|e| AppError::io("Inspect filesystem status directory", e))?,
        owner,
        true,
        expected,
    )?;
    Ok(file)
}

fn open_status_at(directory: &File, create: bool) -> AppResult<File> {
    let flags = libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | if create {
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL
        } else {
            libc::O_RDONLY
        };
    // SAFETY: directory is a held directory descriptor; the fixed NUL-terminated
    // basename cannot escape it. A successful descriptor is transferred once.
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            c"status".as_ptr(),
            flags,
            0o600 as libc::c_int,
        )
    };
    if descriptor < 0 {
        return Err(AppError::io(
            "Open leased filesystem status",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn invalid_record() -> AppError {
    AppError::new("Invalid FreeBSD filesystem status; preserve the encrypted session.")
}

fn valid_nonce(nonce: &str) -> bool {
    nonce.len() == 64
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Accept only the producer's complete, canonical three-line protocol. Missing,
/// partial, oversized, foreign or malformed bytes are failures, never a drain.
fn parse_record(bytes: &[u8], expected_nonce: &str) -> AppResult<Record> {
    if bytes.len() > MAX_RECORD_BYTES || !valid_nonce(expected_nonce) {
        return Err(invalid_record());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| invalid_record())?;
    let mut lines = text.split('\n');
    if lines.next() != Some(PROTOCOL) || lines.next() != Some(expected_nonce) {
        return Err(invalid_record());
    }
    let state = lines.next().ok_or_else(invalid_record)?;
    if lines.next() != Some("") || lines.next().is_some() {
        return Err(invalid_record());
    }
    if state == "pending" {
        return Ok(Record::Pending);
    }
    // A literal space is intentional: tabs, repeated spaces, signs and trailing
    // whitespace cannot silently change the accepted protocol.
    let fields: Vec<_> = state.split(' ').collect();
    if fields.len() != 6 {
        return Err(invalid_record());
    }
    let identity = ProcessIdentity {
        pid: decimal(fields[1], 1, i32::MAX as u64)? as u32,
        birth_seconds: decimal(fields[2], 1, i64::MAX as u64)? as i64,
        birth_microseconds: decimal(fields[3], 0, 999_999)? as u32,
        boot_seconds: decimal(fields[4], 1, i64::MAX as u64)? as i64,
        boot_microseconds: decimal(fields[5], 0, 999_999)? as u32,
    };
    match fields[0] {
        "ready" => Ok(Record::Ready(identity)),
        "drained" => Ok(Record::Drained(identity)),
        "error" => Ok(Record::Error(identity)),
        _ => Err(invalid_record()),
    }
}

fn decimal(text: &str, minimum: u64, maximum: u64) -> AppResult<u64> {
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(invalid_record());
    }
    let value = text.parse::<u64>().map_err(|_| invalid_record())?;
    if !(minimum..=maximum).contains(&value) {
        return Err(invalid_record());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONCE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn record(state: &str) -> Vec<u8> {
        format!("{PROTOCOL}\n{NONCE}\n{state}\n").into_bytes()
    }

    fn test_lease() -> (tempfile::TempDir, StatusLease, u32) {
        use std::os::unix::fs::PermissionsExt;
        let parent = tempfile::tempdir().unwrap();
        fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let owner = unsafe { libc::geteuid() };
        let lease = StatusLease::prepare_owned(parent.path(), "test-session", owner).unwrap();
        (parent, lease, owner)
    }

    fn leased_record(lease: &StatusLease, state: &str) -> Vec<u8> {
        format!("{PROTOCOL}\n{}\n{state}\n", lease.nonce).into_bytes()
    }

    #[test]
    fn status_inode_lock_blocks_early_final_acceptance() {
        let (_parent, lease, owner) = test_lease();
        let expected = ProcessIdentity {
            pid: 1234,
            birth_seconds: 1795750000,
            birth_microseconds: 42,
            boot_seconds: 1795700000,
            boot_microseconds: 0,
        };
        assert_eq!(
            lease.read_owned(owner, false).unwrap().record,
            Record::Pending
        );
        assert!(lease.read_final_owned(owner, expected).is_err());
        assert!(lease.read_owned(owner.wrapping_add(1), false).is_err());
        let producer = OpenOptions::new()
            .read(true)
            .write(true)
            .open(lease.directory.join("status"))
            .unwrap();
        assert_eq!(
            unsafe { libc::flock(producer.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        fs::write(
            lease.directory.join("status"),
            leased_record(&lease, "drained 1234 1795750000 42 1795700000 0"),
        )
        .unwrap();
        producer.sync_all().unwrap();
        assert!(lease.read_final_owned(owner, expected).is_err());
        drop(producer);
        assert!(lease
            .read_final_owned(
                owner,
                ProcessIdentity {
                    pid: 1235,
                    ..expected
                }
            )
            .is_err());
        let final_record = lease.read_final_owned(owner, expected).unwrap();
        assert!(matches!(final_record.record, Record::Drained(_)));
        let contender = File::open(lease.directory.join("status")).unwrap();
        assert_ne!(
            unsafe { libc::flock(contender.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        drop(final_record);
        assert_eq!(
            unsafe { libc::flock(contender.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
    }

    #[test]
    fn status_lease_rejects_replaced_symlinked_or_shared_state() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let (parent, lease, owner) = test_lease();
        let status = lease.directory.join("status");
        let bytes = leased_record(&lease, "error 1234 1795750000 42 1795700000 0");
        fs::write(&status, &bytes).unwrap();
        assert!(matches!(
            lease.read_owned(owner, true).unwrap().record,
            Record::Error(_)
        ));
        fs::set_permissions(&status, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        fs::set_permissions(&status, fs::Permissions::from_mode(0o600)).unwrap();
        let link = parent.path().join("shared");
        fs::hard_link(&status, &link).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        fs::remove_file(&link).unwrap();
        fs::rename(&status, &link).unwrap();
        symlink(&link, &status).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        fs::remove_file(&status).unwrap();
        fs::write(&status, &bytes).unwrap();
        fs::set_permissions(&status, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
    }

    #[test]
    fn missing_partial_foreign_or_relocated_status_preserves_failure() {
        use std::os::unix::fs::PermissionsExt;
        let (parent, lease, owner) = test_lease();
        let status = lease.directory.join("status");
        fs::write(&status, leased_record(&lease, "drained 1234 1795750000")).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        fs::write(&status, record("drained 1234 1795750000 42 1795700000 0")).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        fs::remove_file(&status).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        let moved = parent.path().join("moved");
        fs::rename(&lease.directory, &moved).unwrap();
        fs::create_dir(&lease.directory).unwrap();
        fs::set_permissions(&lease.directory, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(lease.read_owned(owner, true).is_err());
        assert!(StatusLease::prepare_owned(parent.path(), "../escape", owner).is_err());
        assert!(StatusLease::prepare_owned(parent.path(), "test-session", owner).is_err());
    }

    #[test]
    fn complete_records_keep_failure_distinct_from_drain() {
        let identity = ProcessIdentity {
            pid: 1234,
            birth_seconds: 1795750000,
            birth_microseconds: 42,
            boot_seconds: 1795700000,
            boot_microseconds: 0,
        };
        assert_eq!(
            parse_record(&record("pending"), NONCE).unwrap(),
            Record::Pending
        );
        for (state, expected) in [
            ("ready", Record::Ready(identity)),
            ("drained", Record::Drained(identity)),
            ("error", Record::Error(identity)),
        ] {
            let bytes = record(&format!("{state} 1234 1795750000 42 1795700000 0"));
            assert_eq!(parse_record(&bytes, NONCE).unwrap(), expected);
        }
    }

    #[test]
    fn incomplete_or_foreign_records_never_acknowledge_drain() {
        let bytes = record("drained 1234 1795750000 42 1795700000 0");
        for length in 0..bytes.len() {
            assert!(
                parse_record(&bytes[..length], NONCE).is_err(),
                "length {length}"
            );
        }
        let mut extra = bytes.clone();
        extra.push(b'\n');
        assert!(parse_record(&extra, NONCE).is_err());
        assert!(parse_record(&bytes, &"f".repeat(64)).is_err());
        assert!(parse_record(&bytes, &"F".repeat(64)).is_err());
        assert!(parse_record(&bytes, &"0".repeat(63)).is_err());
        assert!(parse_record(&vec![b'x'; MAX_RECORD_BYTES + 1], NONCE).is_err());
        assert!(parse_record(&[0xff], NONCE).is_err());
        assert!(parse_record(&record("drained\0 1234 1795750000 42 1795700000 0"), NONCE).is_err());
        let foreign = String::from_utf8(bytes)
            .unwrap()
            .replace(PROTOCOL, "freebsd-lkl-host-io-2");
        assert!(parse_record(foreign.as_bytes(), NONCE).is_err());
    }

    #[test]
    fn identities_are_bounded_and_canonical() {
        for state in [
            "done 1234 1795750000 42 1795700000 0",
            "drained 0 1795750000 42 1795700000 0",
            "drained 2147483648 1795750000 42 1795700000 0",
            "drained 1234 0 42 1795700000 0",
            "drained 1234 9223372036854775808 42 1795700000 0",
            "drained 1234 18446744073709551616 42 1795700000 0",
            "drained 1234 1795750000 1000000 1795700000 0",
            "drained 1234 1795750000 42 0 0",
            "drained 1234 1795750000 42 1795700000 1000000",
            "drained +1234 1795750000 42 1795700000 0",
            "drained 01234 1795750000 42 1795700000 0",
            "drained -1 1795750000 42 1795700000 0",
            "drained 1234 1795750000 042 1795700000 0",
            "drained 1234\t1795750000 42 1795700000 0",
            "drained  1234 1795750000 42 1795700000 0",
            "drained 1234 1795750000 42 1795700000 0 ",
            "drained 1234 1795750000 42 1795700000 0\r",
            "pending 1234 1795750000 42 1795700000 0",
        ] {
            assert!(parse_record(&record(state), NONCE).is_err(), "{state:?}");
        }
    }
}
