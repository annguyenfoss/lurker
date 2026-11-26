//! Native GPT data partitions retain their kernel identity without a vnode lease.
use super::{args, exec};
use crate::{AppError, AppResult};
use std::fs::{self, File, Metadata};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

// Installed sys/diskslice.h and sys/ioccom.h, stock DragonFly 6.5.
#[repr(C)]
struct PartInfo {
    media_offset: u64,
    media_size: u64,
    media_blocks: u64,
    media_blksize: libc::c_int,
    reserved_blocks: u64,
    fstype: libc::c_int,
    fsreserved: [u8; 16],
    heads: u32,
    cylinders: u32,
    secpertrack: u32,
    secpercyl: u32,
    reserved: [u32; 8],
    fstype_uuid: [u8; 16],
    storage_uuid: [u8; 16],
}
const DIOCGPART: libc::c_ulong = 0x4090_6468;
const UNIT_MASK: u64 = 0x01e0_00f8; // sys/diskslice.h dkunitmask()

fn invalid() -> AppError {
    AppError::new(
        "Select a canonical root-owned DragonFly GPT data partition with 512-byte sectors.",
    )
}

fn parent_name(name: &str) -> Option<&str> {
    let prefix = ["vbd", "nvme", "ad", "da"]
        .into_iter()
        .find(|prefix| name.starts_with(prefix))?;
    let rest = &name[prefix.len()..];
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || rest[..digits].parse::<u32>().is_err() {
        return None;
    }
    let partition = rest[digits..].strip_prefix('s')?;
    let number = partition.parse::<u32>().ok()?;
    if partition.is_empty()
        || !partition.bytes().all(|b| b.is_ascii_digit())
        || !(1..=128).contains(&number)
    {
        return None;
    }
    Some(&name[..prefix.len() + digits])
}

fn node(metadata: &Metadata) -> (u64, u64, u64) {
    (metadata.dev(), metadata.ino(), metadata.rdev())
}

fn geometry(file: &File) -> AppResult<PartInfo> {
    // All fields are integers/byte arrays; zero is valid for every member.
    let mut info: PartInfo = unsafe { std::mem::zeroed() };
    if unsafe { super::vnode_ioctl(file.as_raw_fd(), DIOCGPART, &mut info) } != 0 {
        return Err(AppError::io(
            "Read native DragonFly partition geometry",
            std::io::Error::last_os_error(),
        ));
    }
    if info.media_blksize != 512
        || info.media_size == 0
        || info.media_blocks.checked_mul(512) != Some(info.media_size)
        || info.media_offset % 512 != 0
        || info.media_offset.checked_add(info.media_size).is_none()
        || info.reserved_blocks != 0
        || info.storage_uuid == [0; 16]
    {
        return Err(invalid());
    }
    Ok(info)
}

fn geometry_identity(info: &PartInfo) -> (u64, u64, [u8; 16]) {
    (info.media_offset, info.media_size, info.storage_uuid)
}

fn verify_gpt(text: &str, partition: u32, offset: u64, blocks: u64) -> AppResult<()> {
    if !text.contains("Primary GPT header") || !text.contains("Secondary GPT header") {
        return Err(invalid());
    }
    let mut matches = 0;
    for line in text.lines() {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() < 4 || columns[3].parse::<u32>().ok() != Some(partition) {
            continue;
        }
        if columns.len() < 7
            || columns[4..7] != ["GPT", "part", "-"]
            || columns[0].parse::<u64>().ok() != Some(offset / 512)
            || columns[1].parse::<u64>().ok() != Some(blocks)
        {
            return Err(invalid());
        }
        matches += 1;
    }
    if matches != 1 {
        return Err(invalid());
    }
    Ok(())
}

pub(in crate::bsd) fn physical_device(source: &Path) -> AppResult<crate::bsd::PhysicalDevice> {
    if source.parent() != Some(Path::new("/dev")) {
        return Err(invalid());
    }
    let parent = source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(parent_name)
        .ok_or_else(invalid)?;
    let disks = || exec("/sbin/sysctl", &args(&["-n", "kern.disks"]), None);
    if !disks()?.split_whitespace().any(|name| name == parent) {
        return Err(invalid());
    }
    let before = fs::symlink_metadata(source)
        .map_err(|e| AppError::io("Inspect DragonFly physical partition", e))?;
    if before.uid() != 0 || !before.file_type().is_char_device() {
        return Err(invalid());
    }
    let file = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(source)
        .map_err(|e| AppError::io("Open DragonFly physical partition", e))?;
    let opened = file
        .metadata()
        .map_err(|e| AppError::io("Inspect open DragonFly partition", e))?;
    if node(&before) != node(&opened) || !opened.file_type().is_char_device() {
        return Err(invalid());
    }
    let info = geometry(&file)?;
    let partition = source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once('s'))
        .and_then(|(_, number)| number.parse().ok())
        .ok_or_else(invalid)?;
    let disk = format!("/dev/{parent}");
    let layout = exec("/sbin/gpt", &args(&["show", &disk]), None)?;
    verify_gpt(&layout, partition, info.media_offset, info.media_blocks)?;
    let confirmed = geometry(&file)?;
    let after = fs::symlink_metadata(source)
        .map_err(|e| AppError::io("Recheck DragonFly physical partition", e))?;
    if node(&before) != node(&after)
        || geometry_identity(&info) != geometry_identity(&confirmed)
        || !disks()?.split_whitespace().any(|name| name == parent)
    {
        return Err(AppError::new(
            "DragonFly physical partition changed during inspection.",
        ));
    }
    let uuid = info
        .storage_uuid
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(crate::bsd::PhysicalDevice {
        source: source.to_path_buf(),
        bytes: info.media_size,
        metadata_id: Some(format!(
            "dragonfly-gpt:{uuid}:{}:{}",
            info.media_offset, info.media_size
        )),
    })
}

fn same_disk(left: u64, right: u64) -> bool {
    (left >> 8) & 0xff == (right >> 8) & 0xff && left & UNIT_MASK == right & UNIT_MASK
}

fn busy() -> AppError {
    AppError::new(
        "DragonFly physical disk is mounted, swapped, mapped or open; preserve its resources.",
    )
}

fn check_path(path: &str, selected: u64) -> AppResult<()> {
    let path = path.split('@').next().unwrap_or(path);
    let path = if path.starts_with("/dev/") {
        PathBuf::from(path)
    } else if path.starts_with("serno/") || path.starts_with("part-by-") {
        Path::new("/dev").join(path)
    } else {
        return Ok(());
    };
    let metadata = fs::metadata(path)
        .map_err(|e| AppError::io("Inspect mounted/swapped DragonFly source", e))?;
    if metadata.file_type().is_char_device() && same_disk(metadata.rdev(), selected) {
        return Err(busy());
    }
    Ok(())
}

fn dependencies(text: &str) -> AppResult<Vec<(u64, u64)>> {
    let (count, mut rest) = text.trim().split_once(':').ok_or_else(invalid)?;
    let count: Vec<_> = count.split_whitespace().collect();
    if count.len() != 2 || count[1] != "dependencies" {
        return Err(invalid());
    }
    let count = count[0].parse::<usize>().map_err(|_| invalid())?;
    if count > 64 {
        return Err(invalid());
    }
    let mut result = Vec::new();
    while let Some(pair) = rest.trim().strip_prefix('(') {
        let (pair, next) = pair.split_once(')').ok_or_else(invalid)?;
        let (major, minor) = pair.split_once(',').ok_or_else(invalid)?;
        result.push((
            major.trim().parse().map_err(|_| invalid())?,
            minor.trim().parse().map_err(|_| invalid())?,
        ));
        rest = next;
    }
    if !rest.trim().is_empty() || result.len() != count {
        return Err(invalid());
    }
    Ok(result)
}

fn tuple(text: &str) -> AppResult<(u64, u64)> {
    let (major, minor) = text.split_once(',').ok_or_else(invalid)?;
    Ok((
        major.parse().map_err(|_| invalid())?,
        minor.parse().map_err(|_| invalid())?,
    ))
}

fn open_device(text: &str, selected: u64) -> AppResult<bool> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let header: Vec<_> = lines
        .next()
        .ok_or_else(invalid)?
        .split_whitespace()
        .collect();
    if header
        != [
            "USER", "CMD", "PID", "FD", "DEV", "INUM", "MODE", "SZ|DV", "R/W",
        ]
    {
        return Err(invalid());
    }
    for line in lines {
        let columns: Vec<_> = line.split_whitespace().collect();
        for (index, value) in columns.iter().enumerate() {
            if !value.starts_with('0') || value.len() < 6 {
                continue;
            }
            let Ok(mode) = u32::from_str_radix(value, 8) else {
                continue;
            };
            if mode & 0o170000 != 0o020000 {
                continue;
            }
            // Native raw descriptors report the device cookie, not devfs's
            // filesystem ID. Filename-filtered fstat misses these descriptors.
            let field = columns.get(index + 1).ok_or_else(invalid)?;
            // fstat pads a short cookie before the colon ("14,2   :0").
            let (device, offset) = match field.split_once(':') {
                Some(pair) => pair,
                None => (
                    *field,
                    columns
                        .get(index + 2)
                        .and_then(|value| value.strip_prefix(':'))
                        .ok_or_else(invalid)?,
                ),
            };
            offset.parse::<i64>().map_err(|_| invalid())?;
            let (major, minor) = tuple(device)?;
            if major == (selected >> 8) & 0xff && minor & UNIT_MASK == selected & UNIT_MASK {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(in crate::bsd) fn ensure_physical_idle(source: &Path) -> AppResult<()> {
    let info = physical_device(source)?;
    let selected = fs::symlink_metadata(source)
        .map_err(|e| AppError::io("Inspect DragonFly physical identity", e))?
        .rdev();
    for mount in crate::bsd::mounts()? {
        check_path(&mount.source.to_string_lossy(), selected)?;
    }
    let swaps = exec("/usr/sbin/swapinfo", &args(&["-k"]), None)?;
    for line in swaps.lines() {
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        if name.starts_with("/dev/") {
            check_path(name, selected)?;
        } else if !matches!(name, "Device" | "Total") {
            return Err(invalid());
        }
    }
    let mappings = exec("/sbin/dmsetup", &args(&["ls"]), None)?;
    if mappings.trim() != "No devices found" {
        for line in mappings.lines() {
            let name = line.split_whitespace().next().ok_or_else(invalid)?;
            if name.starts_with('-')
                || name.len() > 127
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_+.-".contains(&b))
            {
                return Err(invalid());
            }
            let deps = exec("/sbin/dmsetup", &args(&["deps", name]), None)?;
            for (major, minor) in dependencies(&deps)? {
                if (major == 0 || major == (selected >> 8) & 0xff)
                    && minor & UNIT_MASK == selected & UNIT_MASK
                {
                    return Err(busy());
                }
            }
        }
    }
    if open_device(&exec("/usr/bin/fstat", &args(&["-n"]), None)?, selected)? {
        return Err(busy());
    }
    let confirmed = physical_device(source)?;
    if confirmed.bytes != info.bytes || confirmed.metadata_id != info.metadata_id {
        return Err(invalid());
    }
    Ok(())
}

pub(in crate::bsd) fn create_physical(
    command: crate::CreateCommand,
    output: crate::Output,
) -> AppResult<()> {
    use crate::bsd::{
        Registry, Session, cleanup, credential, initialize_acquired_mapping,
        initialize_new_filesystem_owner, verify_session,
    };
    use crate::{CreateFilesystem, SourceKind, VolumeType};
    if command.source_kind != SourceKind::Block || !command.force || command.size_gb.is_some() {
        return Err(AppError::new(
            "DragonFly physical creation requires --force and no file size.",
        ));
    }
    let backend = crate::bsd::check_backend(command.volume_type)?;
    super::validate_filesystem(command.filesystem, backend)?;
    let target_metadata = fs::symlink_metadata(&command.target)
        .map_err(|e| AppError::io("Inspect DragonFly physical target", e))?;
    if !target_metadata.file_type().is_char_device() || target_metadata.file_type().is_symlink() {
        return Err(invalid());
    }
    let source = fs::canonicalize(&command.target)
        .map_err(|e| AppError::io("Resolve DragonFly physical target", e))?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    registry.ensure_unused(&source)?;
    let (identity, proof) = crate::bsd::physical::capture(&source)?;
    ensure_physical_idle(&source)?;
    let bytes = physical_device(&source)?.bytes;
    let credential = credential(
        command.passphrase.as_deref(),
        command.keyfile.as_deref(),
        command.passphrase.is_none() && command.keyfile.is_none(),
    )?;
    let program = if backend == VolumeType::Veracrypt {
        super::veracrypt::validate_create_size(bytes)?;
        Some(super::veracrypt::validate(&credential, Some(&source))?)
    } else {
        None
    };
    let mut session = Session {
        version: 1,
        id: crate::bsd::unique_id(),
        backend,
        backing: Some(source.to_string_lossy().into_owned()),
        source,
        create_target: None,
        peer_sources: Vec::new(),
        identity,
        physical: Some(proof),
        dragonfly_backing_reservation: None,
        dragonfly_backing_detach_requested: false,
        dragonfly_layer: None,
        dragonfly_veracrypt: None,
        mapped: None,
        mounted: None,
        intended_mountpoint: None,
        initialization_directory: None,
        readonly: false,
    };
    registry.save(&session)?;
    let initialized = (|| {
        verify_session(&session)?;
        ensure_physical_idle(&session.source)?;
        let mapped = if let Some(program) = &program {
            let descriptor = super::open_source(&session, true)?;
            super::veracrypt::initialize_image(&descriptor, bytes)?;
            session.physical.as_ref().unwrap().verify(&session.source)?;
            drop(descriptor);
            super::veracrypt::create_header(
                &registry,
                &session,
                program,
                command.cipher,
                &credential,
            )?;
            super::veracrypt::activate(&registry, &mut session, program, &credential)?
        } else {
            super::format(
                session.backing.as_deref().unwrap(),
                command.cipher,
                &credential,
            )?;
            super::linear::activate(&registry, &mut session, false, &credential)?
        };
        session.mapped = Some(mapped);
        registry.save(&session)?;
        initialize_acquired_mapping(&registry, &session, |mapped| {
            if program.is_some() {
                super::veracrypt::make_filesystem(
                    &registry,
                    mapped,
                    bytes,
                    command.filesystem.unwrap_or(CreateFilesystem::Hammer2),
                )
            } else {
                super::make_selected_filesystem(mapped, command.filesystem)
            }
        })?;
        initialize_new_filesystem_owner(&registry, &mut session, owner)
    })();
    if let Err(error) = cleanup(&registry, &mut session) {
        return Err(AppError::new(format!(
            "DragonFly physical resources remain recorded for recovery at {}: {error}",
            session.source.display()
        )));
    }
    initialized.map_err(|error: AppError| {
        AppError::new(format!(
            "{error}; preserve the selected physical partition {}",
            session.source.display()
        ))
    })?;
    output.success(&format!(
        "Created {backend} data partition: {}",
        session.source.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_matches_native_abi_and_only_gpt_hardware_names_are_accepted() {
        assert_eq!(std::mem::size_of::<PartInfo>(), 144);
        assert_eq!(std::mem::offset_of!(PartInfo, storage_uuid), 124);
        for name in ["vbd0s1", "ad0s2", "da12s128", "nvme0s1"] {
            assert!(parent_name(name).is_some());
        }
        for name in [
            "vbd0",
            "vbd0s0",
            "vbd0s1a",
            "vn0s1",
            "md0s1",
            "mapper/lurker_x",
            "vbd0s129",
            "vbd0s-1",
            "vbd0s1/../ad0",
        ] {
            assert!(parent_name(name).is_none(), "{name}");
        }
    }
    #[test]
    fn raw_fstat_cookie_protects_the_whole_selected_disk() {
        let header = "USER CMD PID FD DEV INUM MODE SZ|DV R/W\n";
        let held = format!(
            "{header}root python3.12 1361 3 26,504496135 341 020640 26,504496135:1048576 r\n"
        );
        assert!(open_device(&held, 504502791).unwrap());
        assert!(!open_device(header, 504502791).unwrap());
        assert!(!open_device(&held, 504502279).unwrap());
        let short = format!("{header}root syslogd 535 0 14,2 190 020666 14,2   :0 rw\n");
        assert!(!open_device(&short, 504502791).unwrap());
        assert!(open_device("unexpected native output", 504502791).is_err());
        assert!(
            open_device(
                &format!("{header}root x 1 3 dev 1 020640 unavailable r\n"),
                504502791
            )
            .is_err()
        );
    }
    #[test]
    fn dependency_records_require_complete_native_counts() {
        assert_eq!(
            dependencies("1 dependencies : (0, 504496135)").unwrap(),
            [(0, 504496135)]
        );
        for text in [
            "1 dependencies :",
            "0 dependencies : (0, 1)",
            "1 dependencies : (0, bad)",
            "1 dependencies : (0, 1) extra",
        ] {
            assert!(dependencies(text).is_err());
        }
    }

    #[test]
    fn gpt_geometry_requires_one_matching_data_row_and_both_headers() {
        let text = "1 1 512B - Primary GPT header\n2048 2093056 1022MB 1 GPT part - Linux LUKS\n2097151 1 512B - Secondary GPT header\n";
        verify_gpt(text, 1, 1048576, 2093056).unwrap();
        assert!(verify_gpt(text, 2, 1048576, 2093056).is_err());
        assert!(verify_gpt(text, 1, 0, 2093056).is_err());
        assert!(verify_gpt(&text.replace("GPT part", "MBR part"), 1, 1048576, 2093056).is_err());
        assert!(
            verify_gpt(
                &format!("{text}2048 2093056 1022MB 1 GPT part - duplicate\n"),
                1,
                1048576,
                2093056
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires the independently verified disposable DragonFly GPT partition"]
    fn native_physical_identity_and_open_descriptor_guards_are_read_only() {
        let source =
            PathBuf::from(std::env::var_os("LURKER_DRAGONFLY_PHYSICAL_TEST_DEVICE").unwrap());
        let expected_bytes: u64 = std::env::var("LURKER_DRAGONFLY_PHYSICAL_TEST_BYTES")
            .unwrap()
            .parse()
            .unwrap();
        let expected_id = std::env::var("LURKER_DRAGONFLY_PHYSICAL_TEST_ID").unwrap();
        let info = physical_device(&source).unwrap();
        assert_eq!(info.bytes, expected_bytes);
        assert_eq!(info.metadata_id.as_deref(), Some(expected_id.as_str()));
        ensure_physical_idle(&source).unwrap();
        let held = File::options()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&source)
            .unwrap();
        assert!(ensure_physical_idle(&source).is_err());
        drop(held);
        ensure_physical_idle(&source).unwrap();
        for system in ["/dev/null", "/dev/ad0", "/dev/ad0s1d", "/dev/vbd0s0"] {
            assert!(physical_device(Path::new(system)).is_err());
        }
    }
}
