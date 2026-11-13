//! PEFS directory adapter. The common BSD layer owns directory reservation,
//! session records and mount identity. Never flush keys before a normal unmount:
//! flushing them would invalidate open files even if the filesystem is busy.
use super::{Credential, args, exec};
use crate::{AppError, AppResult, CreateCipher};
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

const PEFS: &str = "/usr/local/sbin/pefs";
const MODULE: &str = "/boot/modules/pefs.ko";
const PKG: &str = "/usr/local/sbin/pkg";
const ITERATIONS: &str = "600000";
const ALGORITHM: &str = "aes256-xts";
const CONFIG: &str = "aes256-xts:600000";

pub(super) const REQUIRED_TOOLS: &[&str] =
    &[PEFS, PKG, "/sbin/sysctl", "/sbin/mount", "/sbin/umount"];

/// PEFS is a kernel filesystem: a usable userspace executable alone is not a
/// capability check. Require the official package's kernel build identifier to
/// match the running kernel before allowing the utility to autoload its module.
/// This intentionally rejects unqualified, manually installed module builds.
pub(super) fn check_runtime() -> AppResult<()> {
    for path in [PEFS, MODULE, PKG] {
        let metadata = fs::symlink_metadata(path)
            .map_err(|e| AppError::io("Install a matching official pefs-kmod package", e))?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(AppError::new(format!(
                "Untrusted PEFS runtime file: {path}"
            )));
        }
    }
    let kernel = exec("/sbin/sysctl", &args(&["-n", "kern.osreldate"]), None)?;
    let package = exec(PKG, &args(&["query", "%v", "pefs-kmod"]), None)?;
    if !kernel_package_match(kernel.trim(), package.trim()) {
        return Err(AppError::new(
            "The installed pefs-kmod package does not match the running FreeBSD kernel; install the matching binary package before using PEFS.",
        ));
    }
    Ok(())
}

fn kernel_package_match(kernel: &str, package: &str) -> bool {
    let Some(build) = package.split(',').next().and_then(|v| v.rsplit('.').next()) else {
        return false;
    };
    kernel
        .parse::<u32>()
        .ok()
        .is_some_and(|value| value != 0 && build.parse::<u32>().ok() == Some(value))
}

pub(super) fn initialize(
    source: &Path,
    cipher: CreateCipher,
    credential: &Credential,
) -> AppResult<()> {
    check_runtime()?;
    if cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "PEFS uses the AES-256-XTS profile; Serpent and Twofish are unavailable.",
        ));
    }
    let metadata = fs::symlink_metadata(source)
        .map_err(|e| AppError::io("Failed to inspect new PEFS directory", e))?;
    if !metadata.is_dir()
        || fs::read_dir(source)
            .map_err(|e| AppError::io("Failed to inspect new PEFS directory", e))?
            .next()
            .is_some()
    {
        return Err(AppError::new(
            "PEFS initialization requires a newly reserved empty directory.",
        ));
    }
    symlink(CONFIG, source.join(".pefs.conf"))
        .map_err(|e| AppError::io("Failed to save PEFS profile", e))?;
    // -f here only permits creation of a keychain on the unmounted underlying
    // directory. It never appears in an unmount command. -Z creates a verifier
    // chain; addkey -c below must find it before any key is accepted.
    let mut argv = args(&["addchain", "-fZ", "-a", ALGORITHM, "-i", ITERATIONS]);
    let input = credential_arguments(&mut argv, credential);
    argv.push(source.as_os_str().to_owned());
    exec(PEFS, &argv, input.as_deref())?;
    Ok(())
}

pub(super) fn attach(
    source: &Path,
    mountpoint: &Path,
    readonly: bool,
    credential: &Credential,
) -> AppResult<()> {
    check_runtime()?;
    validate_profile(source)?;
    let mut argv = args(&["mount"]);
    if readonly {
        argv.extend(args(&["-o", "ro"]));
    }
    argv.push(source.as_os_str().to_owned());
    argv.push(mountpoint.as_os_str().to_owned());
    exec(PEFS, &argv, None)?;
    let mut argv = args(&["addkey", "-c", "-a", ALGORITHM, "-i", ITERATIONS]);
    let input = credential_arguments(&mut argv, credential);
    argv.push(mountpoint.as_os_str().to_owned());
    if let Err(error) = exec(PEFS, &argv, input.as_deref()) {
        // An invalid password must not leave an apparently unlocked filesystem.
        // If another process made the mount busy, preserve it and report the
        // cleanup failure; never flush keys, force or detach a replacement mount.
        let cleanup = verify_mapping(source, mountpoint).and_then(|()| detach(mountpoint));
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup) => Err(AppError::new(format!(
                "{}; PEFS mount cleanup failed at {}: {}. Preserve this session for recovery.",
                error.message,
                mountpoint.display(),
                cleanup.message
            ))),
        };
    }
    verify_mapping(source, mountpoint)
}

pub(super) fn detach(mountpoint: &Path) -> AppResult<()> {
    exec("/sbin/umount", &[mountpoint.as_os_str().to_owned()], None)?;
    Ok(())
}

pub(super) fn verify_mapping(source: &Path, mountpoint: &Path) -> AppResult<()> {
    let mounts = exec("/sbin/mount", &args(&["-p"]), None)?;
    if !mount_matches(&mounts, source, mountpoint) {
        return Err(AppError::new(
            "The recorded PEFS mount changed; refusing cleanup.",
        ));
    }
    Ok(())
}

fn validate_profile(source: &Path) -> AppResult<()> {
    if fs::read_link(source.join(".pefs.conf")).ok().as_deref() != Some(Path::new(CONFIG)) {
        return Err(AppError::new(
            "This PEFS directory does not declare Lurker's AES-256-XTS/600000 profile.",
        ));
    }
    let database = fs::symlink_metadata(source.join(".pefs.db"))
        .map_err(|e| AppError::io("The PEFS credential verification database is missing", e))?;
    if !database.is_file() || database.len() == 0 {
        return Err(AppError::new(
            "PEFS requires a regular credential verification database.",
        ));
    }
    Ok(())
}

fn credential_arguments(argv: &mut Vec<OsString>, credential: &Credential) -> Option<Vec<u8>> {
    match credential {
        Credential::Password(password) => {
            argv.extend(args(&["-j", "-"]));
            let mut input = password.clone();
            input.push(b'\n');
            Some(input)
        }
        Credential::Keyfile(path) => {
            argv.extend(args(&["-p", "-k"]));
            argv.push(path.as_os_str().to_owned());
            None
        }
    }
}

fn mount_matches(mounts: &str, source: &Path, mountpoint: &Path) -> bool {
    mounts.lines().any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields.len() >= 3
            && fields[2] == "pefs"
            && unescape_mount(fields[0]).as_deref() == Some(source)
            && unescape_mount(fields[1]).as_deref() == Some(mountpoint)
    })
}

fn unescape_mount(value: &str) -> Option<PathBuf> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            let octal = bytes.get(index + 1..index + 4)?;
            if !octal.iter().all(|b| (b'0'..=b'7').contains(b)) {
                return None;
            }
            let value = (octal[0] - b'0') as u16 * 64
                + (octal[1] - b'0') as u16 * 8
                + (octal[2] - b'0') as u16;
            if value == 0 || value > 255 {
                return None;
            }
            decoded.push(value as u8);
            index += 4;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    Some(PathBuf::from(OsString::from_vec(decoded)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_kernel_build_is_required() {
        assert!(kernel_package_match("1501000", "g20260829.1501000,1"));
        for package in ["g20260829.1500000,1", "g20260829", "1501000\n1500000", ""] {
            assert!(!kernel_package_match("1501000", package));
        }
        assert!(!kernel_package_match("", "g20260829.1501000,1"));
    }

    #[test]
    fn mount_identity_includes_type_and_both_escaped_paths() {
        let line = "/tmp/cipher\\040dir /tmp/plain\\040dir pefs rw 0 0\n";
        assert!(mount_matches(
            line,
            Path::new("/tmp/cipher dir"),
            Path::new("/tmp/plain dir")
        ));
        assert!(!mount_matches(
            &line.replace("pefs", "nullfs"),
            Path::new("/tmp/cipher dir"),
            Path::new("/tmp/plain dir")
        ));
        assert!(!mount_matches(
            line,
            Path::new("/tmp/cipher"),
            Path::new("/tmp/plain dir")
        ));
        assert!(unescape_mount("/tmp/bad\\777").is_none());
        assert!(unescape_mount("/tmp/bad\\000").is_none());
    }
}
