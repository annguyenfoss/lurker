//! Native DragonFly filesystems and host-installed FUSE filesystem drivers.
use super::{args, exec, mapping_name};
use crate::bsd::Mounted;
use crate::{AppError, AppResult, CreateFilesystem, VolumeType};
use std::{fs::File, io::Read, path::Path};

mod fuse;

pub(in crate::bsd) fn filesystem_has_posix_ownership(mapped: &str) -> AppResult<bool> {
    Ok(!matches!(
        probe(mapped)?,
        CreateFilesystem::Ntfs | CreateFilesystem::Exfat | CreateFilesystem::Fat32
    ))
}
pub(in crate::bsd) fn close_filesystem(mapped: &str) -> AppResult<()> {
    fuse::close(mapped)
}

pub(super) fn fuse_mount_uses_mapper(mapped: &str, mounted: &Mounted) -> bool {
    fuse::matches(mapped, mounted)
}

pub(in crate::bsd) fn validate_filesystem(
    selected: Option<CreateFilesystem>,
    backend: VolumeType,
) -> AppResult<()> {
    let selected = selected.unwrap_or(if backend == VolumeType::Veracrypt {
        CreateFilesystem::Hammer2
    } else {
        CreateFilesystem::Ufs1
    });
    if matches!(
        selected,
        CreateFilesystem::Ufs1
            | CreateFilesystem::Hammer2
            | CreateFilesystem::Ext4
            | CreateFilesystem::Ntfs
            | CreateFilesystem::Exfat
    ) || (backend == VolumeType::Veracrypt && selected == CreateFilesystem::Fat32)
    {
        return Ok(());
    }
    Err(AppError::new(format!(
        "DragonFly cannot create {} with this encryption route; use UFS1, HAMMER2, ext4, NTFS or exFAT with the required native tools (VeraCrypt also supports FAT32).",
        selected.as_str()
    )))
}

pub(in crate::bsd) fn make_selected_filesystem(
    mapped: &str,
    selected: Option<CreateFilesystem>,
) -> AppResult<()> {
    mapping_name(mapped)?;
    match selected.unwrap_or(CreateFilesystem::Ufs1) {
        CreateFilesystem::Ufs1 => super::make_filesystem(mapped),
        CreateFilesystem::Hammer2 => {
            exec("/sbin/newfs_hammer2", &args(&["-L", "DATA", mapped]), None)?;
            Ok(())
        }
        CreateFilesystem::Ext4 => {
            let sectors = decrypted_sectors(mapped)?;
            let blocks = sectors / 8;
            if blocks == 0 {
                return Err(AppError::new(
                    "Decrypted DragonFly device is too small for ext4.",
                ));
            }
            exec(
                "/usr/local/sbin/mkfs.ext4",
                &args(&["-F", "-b", "4096", mapped, &blocks.to_string()]),
                None,
            )?;
            Ok(())
        }
        CreateFilesystem::Exfat => {
            exec("/usr/local/sbin/mkfs.exfat", &args(&[mapped]), None)?;
            Ok(())
        }
        CreateFilesystem::Ntfs => {
            // Both native encryption profiles expose fixed 512-byte sectors.
            // Give mkntfs an explicit key-free mapper length rather than relying
            // on Linux/FreeBSD geometry ioctls against a DragonFly raw device.
            let sectors = decrypted_sectors(mapped)?;
            exec(
                "/usr/local/sbin/mkntfs",
                &args(&["-F", "-Q", "-s", "512", mapped, &sectors.to_string()]),
                None,
            )?;
            Ok(())
        }
        _ => Err(AppError::new("Unsupported writable DragonFly filesystem.")),
    }
}

pub(super) fn probe(mapped: &str) -> AppResult<CreateFilesystem> {
    mapping_name(mapped)?;
    let mut bytes = [0; 4096];
    File::open(mapped)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| AppError::io("Probe decrypted DragonFly filesystem", error))?;
    if &bytes[3..11] == b"EXFAT   " {
        return Ok(CreateFilesystem::Exfat);
    }
    if &bytes[3..11] == b"NTFS    " {
        return Ok(CreateFilesystem::Ntfs);
    }
    if &bytes[82..90] == b"FAT32   " {
        return Ok(CreateFilesystem::Fat32);
    }
    if bytes[1080..1082] == [0x53, 0xef] {
        return Ok(CreateFilesystem::Ext4);
    }
    match exec("/usr/sbin/fstyp", &args(&[mapped]), None)?.trim() {
        "ufs" => Ok(CreateFilesystem::Ufs1),
        "hammer2" => Ok(CreateFilesystem::Hammer2),
        "ext2fs" => Ok(CreateFilesystem::Ext4),
        "ntfs" => Ok(CreateFilesystem::Ntfs),
        _ => Err(AppError::new(
            "No supported native DragonFly filesystem was detected.",
        )),
    }
}

pub(in crate::bsd) fn mount_filesystem(
    mapped: &str,
    target: &Path,
    readonly: bool,
) -> AppResult<()> {
    match probe(mapped)? {
        CreateFilesystem::Hammer2 => {
            super::veracrypt::mount_hammer2_local(mapped, target, readonly)
        }
        filesystem
        @ (CreateFilesystem::Ext4 | CreateFilesystem::Ntfs | CreateFilesystem::Exfat) => {
            fuse::mount(mapped, target, filesystem, readonly)
        }
        filesystem => {
            let kind = match filesystem {
                CreateFilesystem::Ufs1 => "ufs",
                _ => {
                    return Err(AppError::new(
                        "The stock DragonFly foreign filesystem driver cannot provide writable access.",
                    ));
                }
            };
            let mut options = args(&[
                "-t",
                kind,
                "-o",
                if readonly {
                    "ro,nosuid,nodev,noexec"
                } else {
                    "rw,nosuid,nodev,noexec"
                },
                mapped,
            ]);
            options.push(target.as_os_str().into());
            exec("/sbin/mount", &options, None)?;
            Ok(())
        }
    }
}

pub(in crate::bsd) fn matches_filesystem_mount(mapped: &str, mounted: &Mounted) -> bool {
    match mounted.filesystem.as_str() {
        "hammer2" => super::veracrypt::mount_uses_mapper(mounted, mapped),
        "ufs" => mounted.source == Path::new(mapped),
        "fuse" => fuse::matches(mapped, mounted),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writable_selection_preserves_defaults_and_uses_available_native_drivers() {
        for backend in [VolumeType::Luks1, VolumeType::Veracrypt] {
            assert!(validate_filesystem(None, backend).is_ok());
            assert!(validate_filesystem(Some(CreateFilesystem::Ufs1), backend).is_ok());
            assert!(validate_filesystem(Some(CreateFilesystem::Hammer2), backend).is_ok());
            for fs in [
                CreateFilesystem::Ext4,
                CreateFilesystem::Ntfs,
                CreateFilesystem::Exfat,
            ] {
                assert!(validate_filesystem(Some(fs), backend).is_ok());
            }
            for fs in [
                CreateFilesystem::Btrfs,
                CreateFilesystem::Xfs,
                CreateFilesystem::Apfs,
                CreateFilesystem::Ufs2,
                CreateFilesystem::Zfs,
            ] {
                assert!(validate_filesystem(Some(fs), backend).is_err());
            }
        }
        assert!(validate_filesystem(Some(CreateFilesystem::Fat32), VolumeType::Veracrypt).is_ok());
        assert!(validate_filesystem(Some(CreateFilesystem::Fat32), VolumeType::Luks1).is_err());
    }
}

fn decrypted_sectors(mapped: &str) -> AppResult<u64> {
    let status = exec(
        "/sbin/dmsetup",
        &args(&["status", mapping_name(mapped)?]),
        None,
    )?;
    crypto_sectors(&status)
}

fn crypto_sectors(status: &str) -> AppResult<u64> {
    let fields: Vec<_> = status.split_whitespace().collect();
    match fields.as_slice() {
        ["0", sectors, "crypt"] => sectors.parse::<u64>().ok().filter(|value| *value > 0),
        _ => None,
    }
    .ok_or_else(|| AppError::new("DragonFly formatter requires one verified crypt mapping extent."))
}

#[cfg(test)]
mod geometry_tests {
    use super::crypto_sectors;
    #[test]
    fn formatter_geometry_rejects_extra_extents_and_non_crypto_devices() {
        assert_eq!(crypto_sectors("0 655360 crypt\n").unwrap(), 655360);
        for status in [
            "0 0 crypt",
            "1 655360 crypt",
            "0 655360 linear",
            "0 655360 crypt\n655360 16 linear",
            "0 655360 crypt extra",
        ] {
            assert!(crypto_sectors(status).is_err());
        }
    }
}
