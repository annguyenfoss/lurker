//! Strict records for the separately installed FreeBSD filesystem driver.
//!
//! Parsing a record does not authorize encrypted-provider teardown. The eventual
//! consumer must also verify the persisted session/source/status inode, acquire
//! the final status lock, and prove the captured native mount is globally absent.
use crate::{AppError, AppResult};

const PROTOCOL: &str = "freebsd-lkl-host-io-1";
const MAX_RECORD_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
