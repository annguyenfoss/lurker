//! DragonFly's native LUKS1/dm-crypt adapter. The base cryptsetup 1.1.2
//! interface deliberately receives neither LUKS2 nor Argon2 arguments.
use super::{Credential, args, exec};
use crate::{AppError, AppResult, CreateCipher};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_ulong};
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

// Native ABI from DragonFly sys/vnioctl.h: dev_t is u32, ino_t is u64,
// PATH_MAX is 1024, and both unions have u64 size/alignment. Keep these
// narrow definitions here; they do not model the encryption format.
#[repr(C)]
union VnodeDevice {
    dev: u32,
    size: u64,
}

#[repr(C)]
union VnodeInode {
    ino: u64,
    sector_size: c_int,
}

#[repr(C)]
struct VnodeUser {
    unit: c_int,
    file: [u8; 1024],
    device: VnodeDevice,
    inode: VnodeInode,
}

// _IOWR('F', 6, struct vn_user), sys/ioccom.h. Layout and request were
// qualified against the installed DragonFly headers; regression tests retain
// those expected ABI values.
const VNIOCGET: c_ulong = 0xc0000000
    | (((std::mem::size_of::<VnodeUser>() as c_ulong) & 0x1fff) << 16)
    | ((b'F' as c_ulong) << 8)
    | 6;

unsafe extern "C" {
    #[link_name = "ioctl"]
    fn vnode_ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

fn vnode_identity(device: &str) -> AppResult<(u64, u64)> {
    let name = vnode_name(device)?;
    let expected_unit = name[2..]
        .parse::<c_int>()
        .map_err(|_| AppError::new("DragonFly vnode unit is out of range."))?;
    let descriptor = File::open(format!("/dev/{name}"))
        .map_err(|e| AppError::io("Failed to open DragonFly vnode identity", e))?;
    // All members are integer or byte-array values; zero initializes the unions
    // as well as padding before the kernel copies the complete structure back.
    let mut info: VnodeUser = unsafe { std::mem::zeroed() };
    info.unit = -1; // query this exact descriptor's unit, never another unit
    // SAFETY: descriptor is live, request encodes the repr(C) buffer's size,
    // and the writable buffer remains valid for the duration of the ioctl.
    let result = unsafe { vnode_ioctl(descriptor.as_raw_fd(), VNIOCGET, &mut info) };
    if result != 0 {
        return Err(AppError::io(
            "Failed to query DragonFly vnode identity",
            std::io::Error::last_os_error(),
        ));
    }
    if info.unit != expected_unit || info.file[0] == 0 || info.file.starts_with(b"swap\0") {
        return Err(AppError::new(
            "DragonFly vnode is not this attached file; preserving the session.",
        ));
    }
    // SAFETY: VNIOCGET succeeded for a file-backed vnode; sys/vnioctl.h
    // specifies the dev/ino arms for this configuration (not size/secsize).
    Ok(unsafe { (u64::from(info.device.dev), info.inode.ino) })
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::io("Failed to inspect DragonFly backing file", e))?;
    // Kernel device/inode identity is independent of reconstructed pathnames
    // and does not require the backing filesystem to have a /dev entry.
    let (dev, inode) = vnode_identity(device)?;
    if !metadata.is_file() || dev != metadata.dev() || inode != metadata.ino() {
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
    fn native_vnode_abi_and_key_free_mapping_dependencies() {
        assert_eq!(std::mem::size_of::<VnodeUser>(), 1048);
        assert_eq!(std::mem::align_of::<VnodeUser>(), 8);
        assert_eq!(std::mem::offset_of!(VnodeUser, device), 1032);
        assert_eq!(std::mem::offset_of!(VnodeUser, inode), 1040);
        assert_eq!(VNIOCGET, 0xc4184606);
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
