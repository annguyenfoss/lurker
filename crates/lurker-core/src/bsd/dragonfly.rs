//! DragonFly's native LUKS1/dm-crypt adapter. The base cryptsetup 1.1.2
//! interface deliberately receives neither LUKS2 nor Argon2 arguments.
use super::{Credential, args, exec};
use crate::{AppError, AppResult, CreateCipher};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
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

fn backing_record(output: &str, device: &str) -> Option<(String, u64)> {
    let name = vnode_name(device).ok()?;
    let prefix = format!("{name}: covering ");
    let mut lines = output.lines().filter_map(|line| line.strip_prefix(&prefix));
    let rest = lines.next()?;
    if lines.next().is_some() {
        return None;
    }
    // vnconfig reconstructs the filename from the vnode's namecache. A
    // later hard-link lookup can change that display name without changing
    // the attached file. Filesystem device plus inode is the authority.
    let (_, location) = rest.rsplit_once(" on ")?;
    let (filesystem, inode) = location.rsplit_once(", inode ")?;
    if filesystem.is_empty()
        || !filesystem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        || filesystem == "."
        || filesystem == ".."
    {
        return None;
    }
    Some((filesystem.to_owned(), inode.trim().parse::<u64>().ok()?))
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let name = vnode_name(device)?;
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::io("Failed to inspect DragonFly backing file", e))?;
    let output = exec(VNCONFIG, &args(&["-l", name]), None)?;
    let matches = backing_record(&output, device).is_some_and(|(filesystem, inode)| {
        inode == metadata.ino()
            && std::fs::metadata(Path::new("/dev").join(filesystem)).is_ok_and(|fs_device| {
                (fs_device.file_type().is_char_device() || fs_device.file_type().is_block_device())
                    && fs_device.rdev() == metadata.dev()
            })
    });
    if !matches {
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
    fn kernel_backing_identity_uses_filesystem_and_inode_not_cached_alias() {
        let original = "vn4: covering /tmp/path on disk/container on ad0s1, inode 1234\n";
        let alias = "vn4: covering /tmp/different hard link on ad0s1, inode 1234\n";
        assert_eq!(
            backing_record(original, "/dev/vn4"),
            Some(("ad0s1".into(), 1234))
        );
        assert_eq!(
            backing_record(original, "/dev/vn4"),
            backing_record(alias, "/dev/vn4")
        );
        assert!(backing_record(original, "/dev/vn5").is_none());
        assert!(
            backing_record("vn4: covering /tmp/x on ../ad0s1, inode 1234", "/dev/vn4").is_none()
        );
        assert!(
            backing_record("vn4: covering /tmp/x on /dev/ad0s1, inode 1234", "/dev/vn4").is_none()
        );
        assert!(backing_record(&format!("{original}{alias}"), "/dev/vn4").is_none());
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

    #[test]
    #[ignore = "requires root on DragonFly; creates only disposable /tmp vnode files"]
    fn native_vnode_hardlinks_retain_backing_identity() {
        use std::os::unix::fs::DirBuilderExt;
        let root = std::env::temp_dir().join(format!(
            "lurker-vnode-alias-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let source = root.join("original container");
        let alias = root.join("source alias");
        let other = root.join("different container");
        File::create(&source).unwrap().set_len(1024 * 1024).unwrap();
        File::create(&other).unwrap().set_len(1024 * 1024).unwrap();
        std::fs::hard_link(&source, &alias).unwrap();
        let device = attach_file(&source, false).unwrap();
        let checked = (|| -> AppResult<()> {
            verify_backing(&device, &source)?;
            // Namecache lookups through the alias must not invalidate the
            // original session, and an unrelated inode must still be rejected.
            std::fs::metadata(&alias).unwrap();
            verify_backing(&device, &alias)?;
            verify_backing(&device, &source)?;
            if verify_backing(&device, &other).is_ok() {
                return Err(AppError::new("Accepted a different vnode backing inode."));
            }
            Ok(())
        })();
        let detached = detach_file(&device);
        if detached.is_ok() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        detached.unwrap();
        checked.unwrap();
    }
}
