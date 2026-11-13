//! NetBSD CGD adapter. Parameters are mandatory recovery metadata, not a keyfile.
//!
//! Only the generated AES-XTS/Argon2id profile is accepted. In particular, an
//! arbitrary cgdconfig file may contain shell_cmd and must never be passed to an
//! elevated cgdconfig. Device allocation uses the kernel's exclusive configure
//! operation and never clears a device it did not successfully configure.
use super::{args, exec, Credential};
use crate::{AppError, AppResult, CreateCipher};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const CGDCONFIG: &str = "/sbin/cgdconfig";
const VNDCONFIG: &str = "/usr/sbin/vndconfig";

pub(super) fn attach_file(path: &Path, readonly: bool) -> AppResult<String> {
    let mut last_error = None;
    for n in 0..4 {
        // Full raw device paths avoid opendisk's current-directory lookup.
        let raw = format!("/dev/rvnd{n}d");
        let mut argv = args(&["-c"]);
        if readonly {
            argv.push("-r".into());
        }
        argv.extend([raw.into(), path.as_os_str().into()]);
        match exec(VNDCONFIG, &argv, None) {
            Ok(_) => return Ok(format!("/dev/vnd{n}d")),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::new("No free NetBSD vnode device.")))
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let n = device_number(device, "vnd")?;
    exec(VNDCONFIG, &args(&["-u", &format!("/dev/rvnd{n}d")]), None)?;
    Ok(())
}

/// Generate recovery metadata and initialize the filesystem. CGD does not have
/// an embedded header: publishing the image without these params loses access.
/// The caller owns publication/rollback and must retain params with the image.
pub(super) fn format(
    device: &str,
    cipher: CreateCipher,
    credential: &Credential,
    params: &Path,
) -> AppResult<()> {
    if cipher != CreateCipher::Aes {
        return Err(AppError::new("CGD creation supports AES-XTS only."));
    }
    device_number(device, "vnd")?;
    let input = password_input(credential, true)?;
    let generated = exec(
        CGDCONFIG,
        &args(&["-g", "-k", "argon2id", "-V", "ffs", "aes-xts", "512"]),
        None,
    )?;
    // Salt generation stays in the native tool; KDF costs are fixed policy.
    let normalized = normalize_generated(&generated)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(params)
        .map_err(|e| AppError::io("Failed to create CGD recovery parameters", e))?;
    file.write_all(normalized.as_bytes())
        .map_err(|e| AppError::io("Failed to save CGD parameters", e))?;
    file.sync_all()
        .map_err(|e| AppError::io("Failed to sync CGD parameters", e))?;
    let mapped = configure(device, &input, params, true)?;
    let initialized = make_filesystem(&mapped);
    let closed = close(&mapped);
    initialized?;
    closed
}

pub(super) fn unlock(
    device: &str,
    _readonly: bool,
    credential: &Credential,
    params: &Path,
) -> AppResult<String> {
    device_number(device, "vnd")?;
    let input = password_input(credential, false)?;
    configure(device, &input, params, false)
}

fn configure(device: &str, input: &[u8], params: &Path, initial: bool) -> AppResult<String> {
    // Snapshot exactly the validated bytes to a private, owned directory. An
    // attacker replacing an external params file cannot insert a shell_cmd
    // between our validation and the elevated native tool's second open.
    let snapshot = ParametersSnapshot::new(params)?;
    let listing = exec(CGDCONFIG, &args(&["-lv"]), None)?;
    let name = free_cgd(&listing).ok_or_else(|| AppError::new("No free NetBSD CGD device."))?;
    let raw = format!("/dev/r{name}d");
    let mut argv = args(&["-p"]);
    if initial {
        argv.extend(args(&["-V", "re-enter"]));
    }
    argv.extend([raw.into(), device.into(), snapshot.path.as_os_str().into()]);
    // A concurrently claimed CGD fails atomically. Never unconfigure on error:
    // that could destroy another process's mapping.
    exec(CGDCONFIG, &argv, Some(input))?;
    Ok(format!("/dev/{name}d"))
}

pub(super) fn close(device: &str) -> AppResult<()> {
    let n = device_number(device, "cgd")?;
    exec(CGDCONFIG, &args(&["-u", &format!("/dev/rcgd{n}d")]), None)?;
    Ok(())
}

pub(super) fn make_filesystem(device: &str) -> AppResult<()> {
    let n = device_number(device, "cgd")?;
    // -I permits a filesystem spanning the whole raw device without a disklabel;
    // it does not disable kernel bounds or force a busy mapping closed.
    exec(
        "/sbin/newfs",
        &args(&["-I", "-O", "2", &format!("/dev/rcgd{n}d")]),
        None,
    )?;
    Ok(())
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let n = device_number(device, "vnd")?;
    let listing = exec(VNDCONFIG, &args(&["-l", &format!("/dev/rvnd{n}d")]), None)?;
    // Compare the kernel-reported backing inode/device, not a substring path.
    use std::os::unix::fs::MetadataExt;
    let identity =
        fs::metadata(path).map_err(|e| AppError::io("Failed to inspect CGD image", e))?;
    let reported = parse_backing(&listing, n)
        .ok_or_else(|| AppError::new("Unrecognized vnode identity; refusing detach."))?;
    let filesystem = fs::metadata(reported.0)
        .map_err(|e| AppError::io("Failed to identify vnode backing filesystem", e))?;
    if reported.1 != identity.ino() || filesystem.rdev() != identity.dev() {
        return Err(AppError::new(
            "NetBSD vnode backing identity changed; refusing detach.",
        ));
    }
    Ok(())
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    let n = device_number(mapped, "cgd")?;
    device_number(backing, "vnd")?;
    let listing = exec(CGDCONFIG, &args(&["-lv", &format!("/dev/rcgd{n}d")]), None)?;
    if !listing
        .split_whitespace()
        .any(|part| part.trim_end_matches(',') == backing.trim_start_matches("/dev/"))
    {
        return Err(AppError::new(
            "CGD backing mapping changed; refusing close.",
        ));
    }
    Ok(())
}

struct ParametersSnapshot {
    directory: PathBuf,
    path: PathBuf,
}
impl ParametersSnapshot {
    fn new(source: &Path) -> AppResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(source)
            .map_err(|e| AppError::io("Failed to open CGD parameters", e))?;
        let metadata = file
            .metadata()
            .map_err(|e| AppError::io("Failed to inspect CGD parameters", e))?;
        if !metadata.is_file() || metadata.len() > 4096 {
            return Err(AppError::new(
                "CGD parameters must be a small regular file.",
            ));
        }
        let mut value = String::new();
        file.take(4097)
            .read_to_string(&mut value)
            .map_err(|e| AppError::io("Failed to read CGD parameters", e))?;
        if value.len() > 4096 {
            return Err(AppError::new("CGD parameters exceed the size limit."));
        }
        validate_params(&value)?;
        // Reconstruct syntax rather than passing arbitrary punctuation accepted
        // by our tokenizer through to another parser.
        let value = normalize_generated(&value)?;
        let mut random = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|e| AppError::io("Failed to create private CGD parameter snapshot", e))?;
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let directory = PathBuf::from(format!(
            "/tmp/lurker-cgd-params-{}-{suffix}",
            std::process::id()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| AppError::io("Failed to create private CGD parameter directory", e))?;
        let snapshot = Self {
            path: directory.join("params"),
            directory,
        };
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&snapshot.path)
            .map_err(|e| AppError::io("Failed to create CGD parameter snapshot", e))?;
        out.write_all(value.as_bytes())
            .map_err(|e| AppError::io("Failed to write CGD parameter snapshot", e))?;
        Ok(snapshot)
    }
}
impl Drop for ParametersSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

fn parse_backing(listing: &str, n: u32) -> Option<(&str, u64)> {
    let line = listing.trim().strip_prefix(&format!("vnd{n}: "))?;
    let (filesystem, rest) = line.split_once(" (")?;
    if filesystem.is_empty() {
        return None;
    }
    let (device, inode) = rest.split_once(") inode ")?;
    if !device.starts_with("/dev/") || device.contains(['\n', '\r']) {
        return None;
    }
    Some((device, inode.trim().parse().ok()?))
}

fn device_number(device: &str, kind: &str) -> AppResult<u32> {
    let prefix = format!("/dev/{kind}");
    let digits = device
        .strip_prefix(&prefix)
        .and_then(|s| s.strip_suffix('d'))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| AppError::new("Invalid NetBSD device identity."))?;
    digits
        .parse()
        .map_err(|_| AppError::new("Invalid NetBSD device number."))
}

fn password_input(credential: &Credential, repeat: bool) -> AppResult<Vec<u8>> {
    let Credential::Password(value) = credential else {
        return Err(AppError::new("CGD currently accepts a password only; raw-key and shell-command key generators are excluded."));
    };
    if value.is_empty() || value.len() > 1023 || value.iter().any(|b| matches!(b, 0 | 10 | 13)) {
        return Err(AppError::new(
            "CGD password must contain 1–1023 bytes without NUL/newlines.",
        ));
    }
    let mut input = value.clone();
    input.push(b'\n');
    if repeat {
        input.extend(value);
        input.push(b'\n');
    }
    Ok(input)
}

fn free_cgd(listing: &str) -> Option<String> {
    listing
        .lines()
        .filter_map(|line| line.strip_suffix(": not in use"))
        .filter(|name| {
            name.strip_prefix("cgd")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .min()
        .map(String::from)
}

fn normalize_generated(value: &str) -> AppResult<String> {
    let tokens = tokens(value);
    if tokens.len() != 20
        || tokens[0] != "algorithm"
        || tokens[1] != "aes-xts"
        || tokens[8] != "keygen"
        || tokens[9] != "argon2id"
        || tokens[18] != "salt"
    {
        return Err(AppError::new("Unexpected native CGD parameter output."));
    }
    let result = format!("algorithm aes-xts;\niv-method encblkno1;\nkeylength 512;\nverify_method ffs;\nkeygen argon2id {{\n iterations 3;\n memory 262144;\n parallelism 2;\n version 19;\n salt {};\n}};\n", tokens[19]);
    validate_params(&result)?;
    Ok(result)
}

fn tokens(value: &str) -> Vec<&str> {
    value
        .split(|c: char| c.is_ascii_whitespace() || matches!(c, '{' | '}' | ';'))
        .filter(|s| !s.is_empty())
        .collect()
}

fn validate_params(value: &str) -> AppResult<()> {
    let parts = tokens(value);
    let expected = [
        "algorithm",
        "aes-xts",
        "iv-method",
        "encblkno1",
        "keylength",
        "512",
        "verify_method",
        "ffs",
        "keygen",
        "argon2id",
        "iterations",
        "3",
        "memory",
        "262144",
        "parallelism",
        "2",
        "version",
        "19",
        "salt",
    ];
    if parts.len() != 20
        || parts[..19] != expected
        || parts[19].len() != 28
        || !parts[19].starts_with("AAAAg")
        || !parts[19].ends_with('=')
        || !parts[19]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
    {
        return Err(AppError::new(
            "CGD parameters do not match Lurker's safe AES-XTS/Argon2id profile.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const PARAMS: &str = "algorithm aes-xts;\niv-method encblkno1;\nkeylength 512;\nverify_method ffs;\nkeygen argon2id {iterations 64; memory 42943; parallelism 2; version 19; salt AAAAgOI5SzGcFVKg64hWP3CRq2E=;};";
    #[test]
    fn native_output_is_normalized_and_executable_keygens_rejected() {
        let params = normalize_generated(PARAMS).unwrap();
        assert!(validate_params(&params).is_ok());
        assert!(validate_params(&format!("{params} keygen shell_cmd {{cmd \"id\";}};")).is_err());
        assert!(validate_params(&params.replace("memory 262144", "memory 1")).is_err());
        assert!(normalize_generated(&PARAMS.replace("argon2id", "shell_cmd")).is_err());
    }
    #[test]
    fn allocation_and_device_names_do_not_accept_unrelated_devices() {
        assert_eq!(
            free_cgd("cgd7: not in use\ncgd0: not in use\ncgd1: /dev/vnd0d"),
            Some("cgd0".into())
        );
        assert_eq!(free_cgd("wd0: not in use\ncgd: not in use"), None);
        assert_eq!(device_number("/dev/vnd3d", "vnd").unwrap(), 3);
        assert_eq!(
            parse_backing("vnd0: / (/dev/dk1) inode 332685\n", 0),
            Some(("/dev/dk1", 332685))
        );
        assert!(parse_backing("vnd0: not in use", 0).is_none());
        for name in ["/dev/wd0d", "/dev/vndd", "vnd0", "/dev/vnd0d/foo"] {
            assert!(device_number(name, "vnd").is_err());
        }
    }
    #[test]
    fn password_framing_and_native_limits() {
        assert_eq!(
            password_input(&Credential::Password(b"  test  ".to_vec()), true).unwrap(),
            b"  test  \n  test  \n"
        );
        assert!(password_input(&Credential::Password(vec![b'x'; 1024]), false).is_err());
        assert!(password_input(&Credential::Password(b"x\ny".to_vec()), false).is_err());
    }
}
