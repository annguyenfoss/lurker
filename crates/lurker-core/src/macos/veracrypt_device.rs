//! VeraCrypt on an identified, unmounted external GPT data partition.
//! The native engine performs its own authorization; its executable must be
//! administrator-controlled before it can launch the elevated core service.
use super::*;
use std::os::unix::fs::FileTypeExt;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct Disk {
    #[serde(rename = "DeviceIdentifier")]
    id: String,
    #[serde(rename = "DeviceNode")]
    node: PathBuf,
    #[serde(rename = "ParentWholeDisk")]
    parent: Option<String>,
    #[serde(rename = "WholeDisk")]
    whole: bool,
    #[serde(rename = "Internal")]
    internal: bool,
    #[serde(rename = "OSInternalMedia", default)]
    os_internal: bool,
    #[serde(rename = "SystemImage", default)]
    system: bool,
    #[serde(rename = "WritableMedia")]
    writable: bool,
    #[serde(rename = "TotalSize")]
    size: u64,
    #[serde(rename = "Content")]
    content: String,
    #[serde(rename = "MountPoint", default)]
    mountpoint: Option<String>,
    #[serde(rename = "DiskUUID")]
    uuid: Option<String>,
    #[serde(rename = "PartitionMapPartition", default)]
    partition: bool,
    #[serde(rename = "PartitionMapPartitionOffset", default)]
    offset: u64,
    #[serde(rename = "BusProtocol", default)]
    transport: String,
    #[serde(rename = "DeviceTreePath", default)]
    tree: String,
    #[serde(rename = "MediaName", default)]
    media: String,
    #[serde(rename = "VirtualOrPhysical", default)]
    location: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Identity {
    partition: Disk,
    parent: Disk,
    node_inode: u64,
    node_device: u64,
    virtual_backing: Option<(PathBuf, u64, u64)>,
    hardware_serial: Option<String>,
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOBSDNameMatching(port: u32, options: u32, name: *const libc::c_char)
    -> *const libc::c_void;
    fn IOServiceGetMatchingService(port: u32, matching: *const libc::c_void) -> u32;
    fn IORegistryEntrySearchCFProperty(
        entry: u32,
        plane: *const libc::c_char,
        key: *const libc::c_void,
        allocator: *const libc::c_void,
        options: u32,
    ) -> *const libc::c_void;
    fn IOObjectRelease(object: u32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const libc::c_void,
        value: *const libc::c_char,
        encoding: u32,
    ) -> *const libc::c_void;
    fn CFGetTypeID(value: *const libc::c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetCString(
        value: *const libc::c_void,
        buffer: *mut libc::c_char,
        size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(value: *const libc::c_void);
}
struct RegistryEntry(u32);
impl Drop for RegistryEntry {
    fn drop(&mut self) {
        unsafe {
            IOObjectRelease(self.0);
        }
    }
}
struct CFValue(*const libc::c_void);
impl Drop for CFValue {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}

fn hardware_serial(device: &str) -> Option<String> {
    let name = CString::new(device).ok()?;
    let matching = unsafe { IOBSDNameMatching(0, 0, name.as_ptr()) };
    if matching.is_null() {
        return None;
    }
    // IOServiceGetMatchingService consumes the matching dictionary.
    let entry = RegistryEntry(unsafe { IOServiceGetMatchingService(0, matching) });
    if entry.0 == 0 {
        return None;
    }
    for property in ["USB Serial Number", "Serial Number", "SerialNumber"] {
        let property = CString::new(property).ok()?;
        let key = CFValue(unsafe {
            CFStringCreateWithCString(std::ptr::null(), property.as_ptr(), 0x08000100)
        });
        if key.0.is_null() {
            return None;
        }
        // Search this media's ancestors, never sibling USB devices. IOKit uses
        // iterate-recursively (1) | iterate-parents (2) in the IOService plane.
        let value = CFValue(unsafe {
            IORegistryEntrySearchCFProperty(
                entry.0,
                c"IOService".as_ptr(),
                key.0,
                std::ptr::null(),
                3,
            )
        });
        if value.0.is_null() || unsafe { CFGetTypeID(value.0) != CFStringGetTypeID() } {
            continue;
        }
        let mut buffer = [0 as libc::c_char; 512];
        if unsafe {
            CFStringGetCString(
                value.0,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                0x08000100,
            )
        } != 0
        {
            let serial = unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
                .to_str()
                .ok()?
                .trim();
            if !serial.is_empty() {
                return Some(serial.to_owned());
            }
        }
    }
    None
}

fn info(path: &Path) -> AppResult<Disk> {
    decode(
        &checked(
            Path::new(DISKUTIL),
            &["info".into(), "-plist".into(), path.into()],
            None,
        )?
        .stdout,
    )
}

fn unsafe_target() -> AppError {
    AppError::new(
        "Select an unmounted, writable external GPT data partition. Whole disks, internal/system storage, reserved partitions and existing APFS containers are refused.",
    )
}

fn validate(part: &Disk, parent: &Disk) -> AppResult<()> {
    if part.whole
        || !part.partition
        || part.internal
        || part.os_internal
        || part.system
        || !parent.whole
        || parent.internal
        || parent.os_internal
        || parent.system
        || !part.writable
        || !parent.writable
        || parent.content != "GUID_partition_scheme"
        || part.parent.as_deref() != Some(parent.id.as_str())
        || part.node != Path::new("/dev").join(&part.id)
        || parent.node != Path::new("/dev").join(&parent.id)
        || !part
            .id
            .strip_prefix(&parent.id)
            .and_then(|s| s.strip_prefix('s'))
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        || part.uuid.as_ref().is_none_or(|id| id.is_empty())
        || part.offset == 0
        || part.size < 32 * 1024 * 1024
        || part.size % 512 != 0
        || part
            .offset
            .checked_add(part.size)
            .is_none_or(|end| end > parent.size)
        || part.mountpoint.as_ref().is_some_and(|p| !p.is_empty())
        || !matches!(
            part.content.as_str(),
            "Microsoft Basic Data"
                | "Linux Filesystem"
                | "Apple_HFS"
                | "FreeBSD UFS"
                | "FreeBSD ZFS"
        )
    {
        return Err(unsafe_target());
    }
    Ok(())
}

fn on_disk(part: &str, parent: &str) -> bool {
    part.strip_prefix(parent)
        .and_then(|value| value.strip_prefix('s'))
        .is_some_and(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
}

pub(super) fn identity(path: &Path) -> AppResult<Identity> {
    let metadata =
        fs::symlink_metadata(path).map_err(|e| AppError::io("Inspect data partition", e))?;
    if !metadata.file_type().is_block_device() {
        return Err(unsafe_target());
    }
    let partition = info(path)?;
    let parent =
        info(&Path::new("/dev").join(partition.parent.as_deref().ok_or_else(unsafe_target)?))?;
    validate(&partition, &parent)?;
    if partition.node != path {
        return Err(unsafe_target());
    }
    let tree: plist::Value = decode(
        &checked(
            Path::new(DISKUTIL),
            &args(&["apfs", "list", "-plist"]),
            None,
        )?
        .stdout,
    )?;
    if let Some(containers) = tree
        .as_dictionary()
        .and_then(|d| d.get("Containers"))
        .and_then(|v| v.as_array())
    {
        for container in containers {
            let Some(container) = container.as_dictionary() else {
                return Err(unsafe_target());
            };
            let stores = container
                .get("PhysicalStores")
                .and_then(|v| v.as_array())
                .ok_or_else(unsafe_target)?;
            let names: Vec<_> = stores
                .iter()
                .filter_map(|v| v.as_dictionary()?.get("DeviceIdentifier")?.as_string())
                .collect();
            if names.contains(&partition.id.as_str()) {
                return Err(unsafe_target());
            }
            if names.iter().any(|name| on_disk(name, &parent.id))
                && container
                    .get("Volumes")
                    .and_then(|v| v.as_array())
                    .is_some_and(|volumes| {
                        volumes.iter().any(|volume| {
                            volume
                                .as_dictionary()
                                .and_then(|v| v.get("Roles"))
                                .and_then(|v| v.as_array())
                                .is_some_and(|roles| !roles.is_empty())
                        })
                    })
            {
                return Err(unsafe_target());
            }
        }
    } else {
        return Err(unsafe_target());
    }
    let virtual_backing = if parent.location == "Virtual" {
        let owned: Vec<_> = images()?
            .into_iter()
            .filter(|image| image.entities.iter().any(|e| e.device == partition.node))
            .collect();
        if owned.len() != 1
            || owned[0]
                .owner
                .is_some_and(|uid| uid != crate::effective_uid())
            || owned[0].encrypted
            || !owned[0].entities.iter().any(|e| e.device == parent.node)
        {
            return Err(unsafe_target());
        }
        let source = fs::symlink_metadata(&owned[0].path)
            .map_err(|e| AppError::io("Inspect virtual partition backing", e))?;
        if !source.is_file() || source.uid() != crate::effective_uid() {
            return Err(unsafe_target());
        }
        Some((owned[0].path.clone(), source.dev(), source.ino()))
    } else if parent.location == "Physical"
        && !parent.transport.is_empty()
        && !parent.tree.is_empty()
        && !parent.media.is_empty()
    {
        None
    } else {
        return Err(unsafe_target());
    };
    let hardware_serial = if virtual_backing.is_none() {
        Some(hardware_serial(&parent.id).ok_or_else(|| AppError::new("External physical media lacks a native hardware serial identity; refusing device setup."))?)
    } else {
        None
    };
    Ok(Identity {
        partition,
        parent,
        node_inode: metadata.ino(),
        node_device: metadata.rdev(),
        virtual_backing,
        hardware_serial,
    })
}

impl Identity {
    pub(super) fn revalidate(&self) -> AppResult<()> {
        if identity(&self.partition.node)? != *self {
            return Err(AppError::new(
                "External partition identity changed; refusing the device operation.",
            ));
        }
        Ok(())
    }
}

pub(super) fn backend() -> AppResult<PathBuf> {
    let program = require_veracrypt()?;
    crate::trusted_privileged_executable(&program).map_err(|error| AppError::new(format!(
        "Physical VeraCrypt needs an administrator-owned complete Lurker installation before its native authorization service can run. {}", error.message
    )))
}

pub(super) fn validate_mount(path: &Path) -> AppResult<()> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AppError::new("Invalid partition mountpoint."))?;
    let mut native = std::mem::MaybeUninit::uninit();
    if unsafe { libc::statfs(path.as_ptr(), native.as_mut_ptr()) } != 0 {
        return Err(AppError::io(
            "Inspect native partition mount",
            std::io::Error::last_os_error(),
        ));
    }
    let native = unsafe { native.assume_init() };
    let kind = unsafe { std::ffi::CStr::from_ptr(native.f_fstypename.as_ptr()) };
    if kind.to_bytes() != b"exfat" || native.f_flags & libc::MNT_RDONLY as u32 != 0 {
        return Err(AppError::new(
            "macOS VeraCrypt partitions currently require a native writable exFAT mount; file-only external filesystem routes are not qualified for physical devices.",
        ));
    }
    Ok(())
}

pub(super) fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    if !c.force
        || c.size_gb.is_some()
        || !matches!(c.filesystem, None | Some(crate::CreateFilesystem::Exfat))
    {
        return Err(AppError::new(
            "macOS VeraCrypt partition creation requires --force, no file size, and the qualified exFAT filesystem.",
        ));
    }
    let target = existing(&c.target)?;
    let identity = identity(&target)?;
    let program = backend()?;
    if vera_volumes()?
        .iter()
        .any(|v| same_path(&v.source, &target))
    {
        return Err(AppError::new(
            "Refusing to overwrite an active VeraCrypt partition.",
        ));
    }
    if let Some(keyfile) = c.keyfile.as_deref() {
        crate::credentials::ensure_distinct_target(&target, keyfile)?;
    }
    let mut pass = if c.keyfile.is_some() {
        Vec::new()
    } else {
        secret(c.passphrase.as_deref(), true)?
    };
    pass.push(b'\n');
    let mut arguments =
        crate::workflow::macos_veracrypt_device_create_args(c.cipher, c.keyfile.as_deref())?;
    arguments.push(target.as_os_str().into());
    identity.revalidate()?;
    output.msg2(&format!(
        "Creating VeraCrypt on external partition {}",
        target.display()
    ));
    checked(&program, &arguments, Some(&pass))?;
    identity.revalidate()?;
    output.success(&format!("Created VeraCrypt partition {}", target.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an explicitly identified native external device and serial"]
    fn native_serial_follows_the_selected_media_ancestry() {
        let device =
            std::env::var("LURKER_NATIVE_SERIAL_DEVICE").expect("explicit BSD device name");
        let expected = std::env::var("LURKER_NATIVE_SERIAL_EXPECTED")
            .expect("independently inventoried hardware serial");
        assert_eq!(hardware_serial(&device), Some(expected));
    }
    fn facts() -> (Disk, Disk) {
        let parent: Disk = serde_json::from_value(serde_json::json!({"DeviceIdentifier":"disk42","DeviceNode":"/dev/disk42","WholeDisk":true,"Internal":false,"WritableMedia":true,"TotalSize":2147483648u64,"Content":"GUID_partition_scheme","VirtualOrPhysical":"Physical","BusProtocol":"USB","DeviceTreePath":"usb/owned","MediaName":"test"})).unwrap();
        let part: Disk = serde_json::from_value(serde_json::json!({"DeviceIdentifier":"disk42s2","DeviceNode":"/dev/disk42s2","ParentWholeDisk":"disk42","WholeDisk":false,"Internal":false,"WritableMedia":true,"TotalSize":1073741824u64,"Content":"Microsoft Basic Data","DiskUUID":"fresh-partition-uuid","PartitionMapPartition":true,"PartitionMapPartitionOffset":1048576})).unwrap();
        (part, parent)
    }
    #[test]
    fn refuses_system_whole_mounted_reserved_and_unidentified_targets() {
        let (part, parent) = facts();
        validate(&part, &parent).unwrap();
        let mut value = part.clone();
        value.whole = true;
        assert!(validate(&value, &parent).is_err());
        let mut value = parent.clone();
        value.internal = true;
        assert!(validate(&part, &value).is_err());
        let mut value = parent.clone();
        value.system = true;
        assert!(validate(&part, &value).is_err());
        let mut value = part.clone();
        value.mountpoint = Some("/Volumes/in-use".into());
        assert!(validate(&value, &parent).is_err());
        let mut value = part.clone();
        value.content = "EFI".into();
        assert!(validate(&value, &parent).is_err());
        let mut value = part.clone();
        value.uuid = None;
        assert!(validate(&value, &parent).is_err());
        let mut value = part.clone();
        value.parent = Some("disk4".into());
        assert!(validate(&value, &parent).is_err());
        let mut value = part.clone();
        value.offset = u64::MAX;
        assert!(validate(&value, &parent).is_err());
        assert!(!on_disk("disk420s2", "disk42"));
    }
}
