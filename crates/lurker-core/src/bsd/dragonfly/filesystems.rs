//! Stock DragonFly filesystem adapters. Foreign filesystems remain read-only;
//! the absence of a compatible FUSE kernel module is not a writable route.
use super::{args, exec, mapping_name};
use crate::bsd::Mounted;
use crate::{AppError, AppResult, CreateFilesystem, VolumeType};
use std::path::Path;

pub(in crate::bsd) fn filesystem_has_posix_ownership(_mapped: &str) -> AppResult<bool> {
    Ok(true)
}
pub(in crate::bsd) fn close_filesystem(_mapped: &str) -> AppResult<()> {
    Ok(())
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
    if matches!(selected, CreateFilesystem::Ufs1 | CreateFilesystem::Hammer2)
        || (backend == VolumeType::Veracrypt && selected == CreateFilesystem::Fat32)
    {
        return Ok(());
    }
    Err(AppError::new(format!(
        "DragonFly cannot create {} with this encryption route; use UFS1 or HAMMER2 (VeraCrypt also supports FAT32).",
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
        _ => Err(AppError::new("Unsupported writable DragonFly filesystem.")),
    }
}

pub(super) fn probe(mapped: &str) -> AppResult<CreateFilesystem> {
    mapping_name(mapped)?;
    match exec("/usr/sbin/fstyp", &args(&[mapped]), None)?.trim() {
        "ufs" => Ok(CreateFilesystem::Ufs1),
        "hammer2" => Ok(CreateFilesystem::Hammer2),
        "ext2fs" => Ok(CreateFilesystem::Ext4),
        "ntfs" => Ok(CreateFilesystem::Ntfs),
        _ => Err(AppError::new(
            "No supported stock DragonFly filesystem was detected.",
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
        filesystem => {
            let kind = match filesystem {
                CreateFilesystem::Ufs1 => "ufs",
                CreateFilesystem::Ext4 if readonly => "ext2fs",
                CreateFilesystem::Ntfs if readonly => "ntfs",
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
        "ext2fs" | "ntfs" => mounted.source == Path::new(mapped),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writable_selection_preserves_native_defaults_and_refuses_foreign_filesystems() {
        for backend in [VolumeType::Luks1, VolumeType::Veracrypt] {
            assert!(validate_filesystem(None, backend).is_ok());
            assert!(validate_filesystem(Some(CreateFilesystem::Ufs1), backend).is_ok());
            assert!(validate_filesystem(Some(CreateFilesystem::Hammer2), backend).is_ok());
            for fs in [
                CreateFilesystem::Ext4,
                CreateFilesystem::Btrfs,
                CreateFilesystem::Xfs,
                CreateFilesystem::Apfs,
                CreateFilesystem::Ufs2,
                CreateFilesystem::Ntfs,
                CreateFilesystem::Exfat,
                CreateFilesystem::Zfs,
            ] {
                assert!(validate_filesystem(Some(fs), backend).is_err());
            }
        }
        assert!(validate_filesystem(Some(CreateFilesystem::Fat32), VolumeType::Veracrypt).is_ok());
        assert!(validate_filesystem(Some(CreateFilesystem::Fat32), VolumeType::Luks1).is_err());
    }
}
