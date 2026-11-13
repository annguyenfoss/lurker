//! DragonFly's native LUKS1/dm-crypt adapter. The base cryptsetup 1.1.2
//! interface deliberately receives neither LUKS2 nor Argon2 arguments.
use super::{args, exec, Credential};
use crate::{AppError, AppResult, CreateCipher};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

const VNCONFIG: &str = "/usr/sbin/vnconfig";
const CRYPTSETUP: &str = "/sbin/cryptsetup";

fn vnode_name(device: &str) -> AppResult<&str> {
    let name = device.strip_prefix("/dev/").unwrap_or(device);
    let unit = name.strip_prefix("vn").unwrap_or("");
    if unit.is_empty() || !unit.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::new("Refusing an invalid DragonFly vnode device."));
    }
    Ok(name)
}

fn mapping_name(device: &str) -> AppResult<&str> {
    let name = device.strip_prefix("/dev/mapper/").unwrap_or(device);
    if !name.starts_with("lurker_")
        || name.len() > 100
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(AppError::new("Refusing a non-Lurker DragonFly mapping."));
    }
    Ok(name)
}

pub(super) fn attach_file(path: &Path, _readonly: bool) -> AppResult<String> {
    // Autocloning reserves a new unit in-kernel. Never choose an apparently
    // unused unit using a racy list-then-attach sequence. vnconfig has no
    // read-only backing option; the crypto mapping enforces read-only access.
    let output = exec(VNCONFIG, &["vn".into(), path.as_os_str().into()], None)?;
    let name = vnode_name(output.trim())?;
    Ok(format!("/dev/{name}"))
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let name = vnode_name(device)?;
    exec(VNCONFIG, &args(&["-u", name]), None)?;
    Ok(())
}

fn credential_args(credential: &Credential) -> (Vec<OsString>, Option<Vec<u8>>) {
    match credential {
        Credential::Password(password) => {
            let mut input = password.clone();
            input.push(b'\n');
            (Vec::new(), Some(input))
        }
        Credential::Keyfile(path) => (vec!["--key-file".into(), path.as_os_str().into()], None),
    }
}

pub(super) fn format(device: &str, cipher: CreateCipher, credential: &Credential) -> AppResult<()> {
    vnode_name(device)?;
    let cipher = match cipher {
        CreateCipher::Aes => "aes-xts-plain64",
        CreateCipher::Serpent => "serpent-xts-plain64",
        CreateCipher::Twofish => "twofish-xts-plain64",
    };
    let mut command = args(&[
        "--batch-mode",
        "--cipher",
        cipher,
        "--key-size",
        "512",
        "--hash",
        "sha256",
        "--iter-time",
        "5000",
    ]);
    let (auth, input) = credential_args(credential);
    command.extend(auth);
    command.extend(args(&["luksFormat", device]));
    exec(CRYPTSETUP, &command, input.as_deref())?;
    Ok(())
}

pub(super) fn unlock(device: &str, readonly: bool, credential: &Credential) -> AppResult<String> {
    vnode_name(device)?;
    let mut random = [0u8; 8];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|e| AppError::io("Failed to name DragonFly crypto mapping", e))?;
    let name = format!(
        "lurker_{}_{:016x}",
        std::process::id(),
        u64::from_ne_bytes(random)
    );
    let mut command = args(&["--batch-mode"]);
    if readonly {
        command.push("--readonly".into());
    }
    let (auth, input) = credential_args(credential);
    command.extend(auth);
    command.extend(args(&["luksOpen", device, &name]));
    // cryptsetup itself refuses an existing mapping. Cleanup ownership is
    // transferred only after a successful open, never after authentication failure.
    exec(CRYPTSETUP, &command, input.as_deref())?;
    Ok(format!("/dev/mapper/{name}"))
}

pub(super) fn close(device: &str) -> AppResult<()> {
    let name = mapping_name(device)?;
    exec(CRYPTSETUP, &args(&["luksClose", name]), None)?;
    Ok(())
}

pub(super) fn make_filesystem(device: &str) -> AppResult<()> {
    mapping_name(device)?;
    exec("/sbin/newfs", &args(&[device]), None)?;
    Ok(())
}

fn backing_matches(output: &str, device: &str, path: &Path, inode: u64) -> bool {
    let Ok(name) = vnode_name(device) else {
        return false;
    };
    let Some(path) = path.to_str() else {
        return false;
    };
    if path.contains(['\n', '\r']) {
        return false;
    }
    // The filesystem device name is the final " on " field; splitting at
    // the final delimiter preserves a filename that itself contains " on ".
    let prefix = format!("{name}: covering ");
    output.lines().any(|line| {
        let Some(rest) = line.strip_prefix(&prefix) else {
            return false;
        };
        let Some((reported_path, location)) = rest.rsplit_once(" on ") else {
            return false;
        };
        reported_path == path
            && location
                .rsplit_once(", inode ")
                .and_then(|(_, value)| value.trim().parse::<u64>().ok())
                == Some(inode)
    })
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let name = vnode_name(device)?;
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::io("Failed to inspect DragonFly backing file", e))?;
    let output = exec(VNCONFIG, &args(&["-l", name]), None)?;
    if !backing_matches(&output, device, path, metadata.ino()) {
        return Err(AppError::new(
            "DragonFly vnode no longer identifies this container; preserving the session.",
        ));
    }
    Ok(())
}

fn dependency_matches(output: &str, rdev: u64) -> bool {
    let Some((count, rest)) = output.trim().split_once(':') else {
        return false;
    };
    if count.split_whitespace().collect::<Vec<_>>() != ["1", "dependencies"] {
        return false;
    }
    let Some(pair) = rest
        .trim()
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
    else {
        return false;
    };
    let Some((major, minor)) = pair.split_once(',') else {
        return false;
    };
    let (Ok(major), Ok(minor)) = (major.trim().parse::<u64>(), minor.trim().parse::<u64>()) else {
        return false;
    };
    // DragonFly sys/types.h encodes the major in bits 8..15, with a
    // non-contiguous minor cookie. The native 6.4 libdevmapper presents major
    // zero for vnode dependencies; the kernel-reported minor cookie still
    // identifies the attached vnode. Reject extra dependencies or another cookie.
    let native_major = (rdev >> 8) & 0xff;
    (major == 0 || major == native_major) && minor == (rdev & 0xffff00ff)
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    vnode_name(backing)?;
    let name = mapping_name(mapped)?;
    let metadata = std::fs::metadata(backing)
        .map_err(|e| AppError::io("Failed to inspect DragonFly vnode identity", e))?;
    // The base cryptsetup 1.1.2 status command leaves its device field blank
    // on DragonFly. deps is key-free; never inspect dmsetup table output,
    // which can contain the live encryption key on older implementations.
    let output = exec("/sbin/dmsetup", &args(&["deps", name]), None)?;
    if !dependency_matches(&output, metadata.rdev()) {
        return Err(AppError::new(
            "DragonFly crypto mapping was replaced; preserving the session.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_rejects_devices_outside_owned_namespaces() {
        for invalid in ["/dev/ad0", "vn", "vn4s0", "/dev/vn4/../ad0", "-f"] {
            assert!(vnode_name(invalid).is_err());
        }
        for invalid in ["/dev/mapper/root", "root", "lurker_x/../root", "lurker_x-y"] {
            assert!(mapping_name(invalid).is_err());
        }
        assert_eq!(vnode_name("/dev/vn42").unwrap(), "vn42");
        assert!(mapping_name("/dev/mapper/lurker_123_deadbeef").is_ok());
    }

    #[test]
    fn kernel_backing_identity_requires_exact_path_unit_and_inode() {
        let output = "vn4: covering /tmp/path on disk/container on ad0s1, inode 1234\n";
        let path = Path::new("/tmp/path on disk/container");
        assert!(backing_matches(output, "/dev/vn4", path, 1234));
        assert!(!backing_matches(output, "/dev/vn5", path, 1234));
        assert!(!backing_matches(output, "/dev/vn4", path, 1235));
        assert!(!backing_matches(
            output,
            "/dev/vn4",
            Path::new("/tmp/path"),
            1234
        ));
        assert!(dependency_matches(
            "1 dependencies\t: (0, 504430631)\n",
            504432679
        ));
        assert!(!dependency_matches(
            "1 dependencies\t: (0, 504430632)\n",
            504432679
        ));
        assert!(!dependency_matches(
            "2 dependencies: (0, 504430631) (0, 42)\n",
            504432679
        ));
    }
}
