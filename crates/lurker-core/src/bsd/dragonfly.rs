//! Native DragonFly vnode and device-mapper support. LUKS1 uses the base
//! cryptsetup 1.1.2 interface, while VeraCrypt uses the private tcplay adapter.
use super::{Credential, Identity, Registry, Session, args, exec, exec_interruptible};
use crate::{AppError, AppResult, CreateCipher};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString, OsString};
use std::fs::{File, Metadata};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_ulong};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;

mod filesystems;
pub(super) use filesystems::{
    close_filesystem, filesystem_has_posix_ownership, make_selected_filesystem,
    matches_filesystem_mount, mount_filesystem, validate_filesystem,
};
pub(super) mod linear;
mod metadata;
pub(super) mod veracrypt;

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

#[cfg(test)]
pub(super) fn attach_file(path: &Path, _readonly: bool) -> AppResult<String> {
    // Autocloning reserves a new unit in-kernel. Never choose an apparently
    // unused unit using a racy list-then-attach sequence. vnconfig has no
    // read-only backing option; the mapper and filesystem request read-only access.
    let output = exec(VNCONFIG, &["vn".into(), path.as_os_str().into()], None)?;
    let name = vnode_name(output.trim())?;
    Ok(format!("/dev/{name}"))
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct BackingReservation {
    unit: c_int,
    node: Identity,
    changed_at: (i64, i64),
    #[serde(default)]
    empty_release_started: bool,
}

impl BackingReservation {
    fn reserve() -> AppResult<(Self, File)> {
        let descriptor = open_vnode("/dev/vn")
            .map_err(|e| AppError::io("Reserve DragonFly vnode clone", e))?;
        let node = device_metadata(&descriptor)?;
        let (unit, state) = descriptor_state(&descriptor)?;
        if unit < 0 || state != VnodeState::Idle {
            return Err(AppError::new("DragonFly vnode reservation was not idle."));
        }
        Ok((
            Self {
                unit,
                node: Identity {
                    dev: node.dev(),
                    ino: node.ino(),
                },
                changed_at: (node.ctime(), node.ctime_nsec()),
                empty_release_started: false,
            },
            descriptor,
        ))
    }

    fn device(&self) -> String {
        format!("/dev/vn{}", self.unit)
    }

    fn matches(&self, node: &Metadata) -> bool {
        self.node.dev == node.dev()
            && self.node.ino == node.ino()
            && self.changed_at == (node.ctime(), node.ctime_nsec())
    }

    fn attach(&self, descriptor: &File, source: &Path, identity: &Identity) -> AppResult<()> {
        let path = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| AppError::new("DragonFly backing path contains a NUL byte."))?;
        change_attachment(descriptor, VnodeChange::AttachFile(&path))?;
        let expected = VnodeState::File(identity.dev, identity.ino);
        if descriptor_state(descriptor)? != (self.unit, expected) {
            return Err(AppError::new(
                "DragonFly attached a different backing file.",
            ));
        }
        Ok(())
    }

    fn release(
        &self,
        identity: &Identity,
        record_empty_release: impl FnOnce() -> AppResult<()>,
    ) -> AppResult<()> {
        let descriptor = match open_vnode(&self.device()) {
            Ok(descriptor) => descriptor,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(AppError::io("Open recorded DragonFly vnode reservation", e)),
        };
        let (unit, state) = descriptor_state(&descriptor)?;
        if unit != self.unit || !self.matches(&device_metadata(&descriptor)?) {
            return Err(AppError::new(
                "DragonFly vnode reservation changed; preserving the journal.",
            ));
        }
        match state {
            VnodeState::Idle => {
                // Native detach expects an active clone. Journal the one-page
                // anonymous object so a retry can finish releasing it.
                record_empty_release()?;
                change_attachment(&descriptor, VnodeChange::AttachPage)?;
            }
            VnodeState::Swap(1) if self.empty_release_started => (),
            VnodeState::File(dev, ino)
                if !self.empty_release_started && dev == identity.dev && ino == identity.ino =>
            {
                ()
            }
            _ => {
                return Err(AppError::new(
                    "DragonFly vnode reservation has a different backing; preserving the journal.",
                ))
            }
        }
        change_attachment(&descriptor, VnodeChange::Detach)
    }
}

pub(super) fn attach_backing(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let (reservation, descriptor) = BackingReservation::reserve()?;
    // Save the exact clone before attaching to it on the same descriptor.
    session.dragonfly_backing_reservation = Some(reservation.clone());
    registry.save(session)?;
    reservation.attach(&descriptor, &session.source, &session.identity)?;
    session.backing = Some(reservation.device());
    session.dragonfly_backing_reservation = None;
    registry.save(session)
}

pub(super) fn reconcile_backing_reservation(
    registry: &Registry,
    session: &mut Session,
) -> AppResult<()> {
    let Some(reservation) = session.dragonfly_backing_reservation.clone() else {
        return Ok(());
    };
    if reservation.unit < 0
        || session.backing.is_some()
        || session.mapped.is_some()
        || session.mounted.is_some()
        || session.dragonfly_layer.is_some()
        || !session.identity.matches(&session.source)
    {
        return Err(AppError::new(
            "DragonFly vnode reservation conflicts with its session; preserving the journal.",
        ));
    }
    let identity = session.identity.clone();
    reservation.release(&identity, || {
        session
            .dragonfly_backing_reservation
            .as_mut()
            .unwrap()
            .empty_release_started = true;
        registry.save(session)
    })?;
    session.dragonfly_backing_reservation = None;
    registry.save(session)
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let name = vnode_name(device)?;
    exec(VNCONFIG, &args(&["-u", name]), None)?;
    Ok(())
}

fn backing_inactive(device: &str) -> AppResult<bool> {
    let name = vnode_name(device)?;
    // The kernel removes autoclones when their last descriptor closes.
    // Preallocated units still have a device node when they are idle.
    match std::fs::symlink_metadata(format!("/dev/{name}")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(AppError::io("Inspect recorded DragonFly vnode", error)),
        Ok(_) => Ok(matches!(vnode_state(name)?, VnodeState::Idle)),
    }
}

pub(super) fn reconcile_backing_detach(
    registry: &super::Registry,
    session: &mut super::Session,
) -> AppResult<()> {
    let Some(backing) = &session.backing else {
        return Ok(());
    };
    if session.dragonfly_backing_detach_requested
        && session.dragonfly_layer.is_none()
        && session.mapped.is_none()
        && session.mounted.is_none()
        && session.identity.matches(&session.source)
        && backing_inactive(backing)?
    {
        session.backing = None;
        registry.save(session)?;
    }
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
    // The native LUKS1 keyslot code handles SIGINT to remove its temporary map.
    exec_interruptible(CRYPTSETUP, &command, input.as_deref())?;
    Ok(())
}

#[cfg(test)]
pub(super) fn unlock(device: &str, readonly: bool, credential: &Credential) -> AppResult<String> {
    vnode_name(device)?;
    let name = format!("lurker_{}", super::unique_id().replace('-', "_"));
    unlock_named(device, &name, readonly, credential)?;
    Ok(format!("/dev/mapper/{name}"))
}

fn unlock_named(
    device: &str,
    name: &str,
    readonly: bool,
    credential: &Credential,
) -> AppResult<()> {
    if device.starts_with("/dev/mapper/lurker_linear_") {
        mapping_name(device)?;
    } else {
        vnode_name(device)?;
    }
    mapping_name(name)?;
    let mut command = args(&["--batch-mode"]);
    if readonly {
        command.push("--readonly".into());
    }
    let (auth, input) = credential_args(credential);
    command.extend(auth);
    command.extend(args(&["luksOpen", device, name]));
    exec_interruptible(CRYPTSETUP, &command, input.as_deref())?;
    Ok(())
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

#[repr(C)]
struct VnodeAttachment {
    path: *const libc::c_char,
    size: i64,
}

// _IOWR('F', number, type), from the installed sys/vnioctl.h and sys/ioccom.h.
const fn vnode_request<T>(number: c_ulong) -> c_ulong {
    0xc0000000
        | (((std::mem::size_of::<T>() as c_ulong) & 0x1fff) << 16)
        | ((b'F' as c_ulong) << 8)
        | number
}
const VNIOCATTACH: c_ulong = vnode_request::<VnodeAttachment>(0);
const VNIOCDETACH: c_ulong = vnode_request::<VnodeAttachment>(1);
const VNIOCGET: c_ulong = vnode_request::<VnodeUser>(6);

unsafe extern "C" {
    #[link_name = "ioctl"]
    fn vnode_ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

#[derive(PartialEq, Eq)]
enum VnodeState {
    Idle,
    Swap(u64),
    File(u64, u64),
}

enum VnodeChange<'a> {
    AttachFile(&'a CStr),
    AttachPage,
    Detach,
}

fn open_vnode(path: &str) -> io::Result<File> {
    File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

fn device_metadata(descriptor: &File) -> AppResult<Metadata> {
    let metadata = descriptor
        .metadata()
        .map_err(|e| AppError::io("Inspect DragonFly vnode descriptor", e))?;
    if !metadata.file_type().is_char_device() {
        return Err(AppError::new("DragonFly vnode is not a character device."));
    }
    Ok(metadata)
}

fn change_attachment(descriptor: &File, change: VnodeChange<'_>) -> AppResult<()> {
    let (operation, path, size) = match change {
        VnodeChange::AttachFile(path) => (VNIOCATTACH, path.as_ptr(), 0),
        VnodeChange::AttachPage => (VNIOCATTACH, std::ptr::null(), 1),
        VnodeChange::Detach => (VNIOCDETACH, std::ptr::null(), 0),
    };
    let mut request = VnodeAttachment { path, size };
    // SAFETY: the request matches the kernel ABI. The live descriptor and
    // optional NUL-terminated pathname remain valid for the synchronous ioctl.
    if unsafe { vnode_ioctl(descriptor.as_raw_fd(), operation, &mut request as *mut VnodeAttachment) }
        != 0
    {
        return Err(AppError::io(
            "Change DragonFly vnode attachment",
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

fn descriptor_state(descriptor: &File) -> AppResult<(c_int, VnodeState)> {
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
    let state = if info.file[0] == 0 {
        VnodeState::Idle
    } else if info.file.starts_with(b"swap\0") {
        // SAFETY: VNIOCGET marks anonymous backing and returns its page count.
        unsafe { VnodeState::Swap(info.device.size) }
    } else {
        // SAFETY: VNIOCGET succeeded for a file-backed vnode; sys/vnioctl.h
        // specifies the dev/ino arms for this configuration (not size/secsize).
        unsafe { VnodeState::File(u64::from(info.device.dev), info.inode.ino) }
    };
    Ok((info.unit, state))
}

fn vnode_state(device: &str) -> AppResult<VnodeState> {
    let name = vnode_name(device)?;
    let expected_unit = name[2..]
        .parse::<c_int>()
        .map_err(|_| AppError::new("DragonFly vnode unit is out of range."))?;
    let descriptor = open_vnode(&format!("/dev/{name}"))
        .map_err(|e| AppError::io("Failed to open DragonFly vnode identity", e))?;
    device_metadata(&descriptor)?;
    let (unit, state) = descriptor_state(&descriptor)?;
    if unit != expected_unit {
        return Err(AppError::new(
            "DragonFly vnode unit changed; preserving the session.",
        ));
    }
    Ok(state)
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::io("Failed to inspect DragonFly backing file", e))?;
    // Kernel device/inode identity is independent of reconstructed pathnames
    // and does not require the backing filesystem to have a /dev entry.
    if !metadata.is_file()
        || vnode_state(device)? != VnodeState::File(metadata.dev(), metadata.ino())
    {
        return Err(AppError::new(
            "DragonFly vnode no longer identifies this container; preserving the session.",
        ));
    }
    Ok(())
}

fn attached_private_copy(path: &Path) -> AppResult<Option<String>> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::io("Inspect DragonFly vnode backing file", e))?;
    let expected = VnodeState::File(metadata.dev(), metadata.ino());
    // vnconfig's bulk listing stops at the first missing unit and continues
    // past failed kernel queries. Inspect every existing device by descriptor.
    let devices = std::fs::read_dir("/dev")
        .map_err(|e| AppError::io("List DragonFly vnode devices", e))?;
    let mut found = None;
    for device in devices {
        let name = device
            .map_err(|e| AppError::io("Read DragonFly device entry", e))?
            .file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if vnode_name(name).is_ok() && vnode_state(name)? == expected {
            if found.is_some() {
                return Err(AppError::new(
                    "Multiple DragonFly vnodes cover the same backing file.",
                ));
            }
            found = Some(format!("/dev/{name}"));
        }
    }
    Ok(found)
}

fn dependency_matches(output: &str, rdevs: &[u64]) -> bool {
    let Some((count, mut rest)) = output.trim().split_once(':') else {
        return false;
    };
    let mut count = count.split_whitespace();
    if count.next().and_then(|v| v.parse().ok()) != Some(rdevs.len())
        || count.next() != Some("dependencies")
        || count.next().is_some()
    {
        return false;
    }
    let mut remaining = rdevs.to_vec();
    while let Some(pair) = rest.trim().strip_prefix('(') {
        let Some((pair, next)) = pair.split_once(')') else {
            return false;
        };
        let Some((major, minor)) = pair.split_once(',') else {
            return false;
        };
        let (Ok(major), Ok(minor)) = (major.trim().parse::<u64>(), minor.trim().parse::<u64>())
        else {
            return false;
        };
        // The native libdm can present major zero. The kernel minor cookie
        // still identifies the backing device in DragonFly's dev_t layout.
        let Some(index) = remaining.iter().position(|rdev| {
            (major == 0 || major == (rdev >> 8) & 0xff) && minor == rdev & 0xffff00ff
        }) else {
            return false;
        };
        remaining.swap_remove(index);
        rest = next;
    }
    remaining.is_empty() && rest.trim().is_empty()
}

fn verify_dependencies(mapped: &str, backing: &[&str]) -> AppResult<()> {
    let name = mapping_name(mapped)?;
    let rdevs = backing
        .iter()
        .map(|path| {
            std::fs::metadata(path)
                .map(|metadata| metadata.rdev())
                .map_err(|e| AppError::io("Failed to inspect DragonFly device identity", e))
        })
        .collect::<AppResult<Vec<_>>>()?;
    // Dependency queries and status are key-free. Table queries can disclose
    // a live encryption key on older implementations.
    let output = exec("/sbin/dmsetup", &args(&["deps", name]), None)?;
    if !dependency_matches(&output, &rdevs) {
        return Err(AppError::new(
            "DragonFly mapping dependencies changed; preserving the session.",
        ));
    }
    Ok(())
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    mapping_sectors(mapped, backing).map(|_| ())
}

fn mapping_sectors(mapped: &str, backing: &str) -> AppResult<u64> {
    if backing.starts_with("/dev/mapper/lurker_linear_") {
        mapping_name(backing)?;
    } else {
        vnode_name(backing)?;
    }
    verify_dependencies(mapped, &[backing])?;
    let status = exec(
        "/sbin/dmsetup",
        &args(&["status", mapping_name(mapped)?]),
        None,
    )?;
    let mut entries = status.lines();
    let mut first = entries.next().unwrap_or_default().split_whitespace();
    let start = first.next();
    let size = first.next().and_then(|value| value.parse().ok());
    size.filter(|&size| {
        start == Some("0") && size > 0 && first.next() == Some("crypt") && entries.next().is_none()
    })
    .ok_or_else(|| AppError::new("DragonFly crypto mapping type changed."))
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
        assert_eq!(std::mem::size_of::<VnodeAttachment>(), 16);
        assert_eq!(VNIOCATTACH, 0xc0104600);
        assert_eq!(VNIOCDETACH, 0xc0104601);
        assert!(dependency_matches(
            "1 dependencies\t: (0, 504430631)\n",
            &[504432679]
        ));
        assert!(!dependency_matches(
            "1 dependencies\t: (0, 504430632)\n",
            &[504432679]
        ));
        assert!(!dependency_matches(
            "2 dependencies: (0, 504430631) (0, 42)\n",
            &[504432679]
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
