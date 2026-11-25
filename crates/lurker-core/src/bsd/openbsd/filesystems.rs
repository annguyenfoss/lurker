//! OpenBSD filesystem dispatch. Native labels stay outside non-FFS payloads.
use super::super::{args, exec, Mounted};
use crate::{AppError, AppResult, CreateFilesystem, VolumeType};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Filesystem {
    Ffs,
    Ntfs,
    Exfat,
    Ext,
    Xfs,
    Apfs,
}

pub(in crate::bsd) fn validate_filesystem(
    selection: Option<CreateFilesystem>,
    _: VolumeType,
) -> AppResult<()> {
    match selection {
        None | Some(CreateFilesystem::Ufs1 | CreateFilesystem::Ufs2 | CreateFilesystem::Ntfs | CreateFilesystem::Exfat) => Ok(()),
        Some(fs) => Err(AppError::new(format!("OpenBSD cannot create {} as an inner filesystem; select ufs1, ufs2, ntfs or exfat. Ext, XFS and APFS imports are read-only.", fs.as_str()))),
    }
}

fn unit(mapped: &str) -> AppResult<&str> {
    super::device_name(mapped, "sd").or_else(|_| super::device_name(mapped, "vnd"))
}

pub(in crate::bsd) fn make_selected_filesystem(
    mapped: &str,
    selection: Option<CreateFilesystem>,
) -> AppResult<()> {
    validate_filesystem(selection, VolumeType::Softraid)?;
    let selected = selection.unwrap_or(CreateFilesystem::Ufs2);
    let name = unit(mapped)?;
    let mut disk = OpenOptions::new()
        .write(true)
        .open(format!("/dev/r{name}c"))
        .map_err(|e| AppError::io("Open new OpenBSD virtual disk", e))?;
    disk.write_all(&vec![0; 1024 * 1024])
        .map_err(|e| AppError::io("Initialize new OpenBSD label area", e))?;
    disk.sync_all()
        .map_err(|e| AppError::io("Sync new OpenBSD label area", e))?;
    drop(disk);
    exec("/sbin/disklabel", &args(&["-wAT-", name]), Some(b"/ *\n"))?;
    match selected {
        CreateFilesystem::Ufs1 | CreateFilesystem::Ufs2 => {
            let version = if selected == CreateFilesystem::Ufs1 {
                "1"
            } else {
                "2"
            };
            exec(
                "/sbin/newfs",
                &args(&["-q", "-O", version, &format!("/dev/r{name}a")]),
                None,
            )?;
        }
        CreateFilesystem::Ntfs | CreateFilesystem::Exfat => {
            // Both formatters overwrite sector 1. Leave the native disklabel
            // outside partition a so its geometry survives close and reopen.
            exec(
                "/sbin/disklabel",
                &args(&["-E", name]),
                Some(b"d a\na a\n64\n*\nMSDOS\nw\nq\n"),
            )?;
            let label = exec("/sbin/disklabel", &args(&[name]), None)?;
            if !label.lines().any(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                fields.len() >= 4 && fields[0] == "a:" && fields[2] == "64" && fields[3] == "MSDOS"
            }) {
                return Err(AppError::new(
                    "New OpenBSD exchange partition has unexpected geometry; refusing format.",
                ));
            }
            let device = format!("/dev/{name}a");
            if selected == CreateFilesystem::Ntfs {
                exec(
                    "/usr/local/sbin/mkntfs",
                    &args(&["-F", "-Q", &device]),
                    None,
                )?;
            } else {
                exec("/usr/local/sbin/mkexfatfs", &args(&[&device]), None)?;
            }
        }
        _ => unreachable!("validated OpenBSD filesystem"),
    }
    Ok(())
}

fn probe(mapped: &str) -> AppResult<Filesystem> {
    let name = unit(mapped)?;
    let mut disk = File::open(format!("/dev/r{name}a"))
        .map_err(|e| AppError::io("Read OpenBSD filesystem signature", e))?;
    let mut bytes = vec![0; 128 * 1024];
    disk.seek(SeekFrom::Start(0))
        .map_err(|e| AppError::io("Seek OpenBSD filesystem", e))?;
    disk.read_exact(&mut bytes)
        .map_err(|e| AppError::io("Read OpenBSD filesystem", e))?;
    signature(&bytes).ok_or_else(|| AppError::new("Unsupported OpenBSD inner filesystem signature; refusing an implicit filesystem fallback."))
}

fn signature(bytes: &[u8]) -> Option<Filesystem> {
    if bytes.get(3..11) == Some(b"NTFS    ") {
        return Some(Filesystem::Ntfs);
    }
    if bytes.get(3..11) == Some(b"EXFAT   ") {
        return Some(Filesystem::Exfat);
    }
    if bytes.get(..4) == Some(b"XFSB") {
        return Some(Filesystem::Xfs);
    }
    if bytes.get(32..36) == Some(b"NXSB") {
        return Some(Filesystem::Apfs);
    }
    if bytes.get(1080..1082) == Some(&[0x53, 0xef]) {
        return Some(Filesystem::Ext);
    }
    for offset in [8192 + 1372, 65536 + 1372] {
        if let Some(value) = bytes.get(offset..offset + 4) {
            let magic = u32::from_le_bytes(value.try_into().unwrap());
            if matches!(magic, 0x00011954 | 0x19540119) {
                return Some(Filesystem::Ffs);
            }
        }
    }
    None
}

pub(in crate::bsd) fn mount_filesystem(
    mapped: &str,
    target: &Path,
    readonly: bool,
) -> AppResult<()> {
    let fs = probe(mapped)?;
    if !readonly && matches!(fs, Filesystem::Ext | Filesystem::Xfs | Filesystem::Apfs) {
        return Err(AppError::new(
            "The installed OpenBSD driver cannot provide writable access to this filesystem.",
        ));
    }
    let mode = if readonly { "ro" } else { "rw" };
    let owner = crate::new_filesystem::invoking_owner()?;
    let private_options = format!("{mode},uid={},gid={},umask=077", owner.uid, owner.gid);
    let (command, mut argv) = match fs {
        Filesystem::Ffs => (
            "/sbin/mount",
            args(&["-t", "ffs", "-o", if readonly { "ro" } else { "rw,sync" }]),
        ),
        Filesystem::Ntfs => ("/usr/local/bin/ntfs-3g", args(&["-o", &private_options])),
        Filesystem::Exfat => (
            "/usr/local/sbin/mount.exfat-fuse",
            args(&["-o", &private_options]),
        ),
        Filesystem::Ext => ("/sbin/mount", args(&["-t", "ext2fs", "-o", "ro"])),
        Filesystem::Xfs => ("/usr/local/bin/fsxfsmount", args(&["-X", "ro"])),
        Filesystem::Apfs => ("/usr/local/bin/fsapfsmount", args(&["-X", "ro"])),
    };
    argv.push(mapped.into());
    argv.push(target.as_os_str().to_owned());
    exec(command, &argv, None)?;
    Ok(())
}

// OpenBSD's FUSE layer labels every source `fusefs`, and its packaged
// ntfs-3g rejects fsname=. Require the expected root daemon to hold exactly
// this mapped block device, in addition to the journal's exact mount fsid.
fn fuse_holds_mapping(mapped: &str, fs: Filesystem) -> bool {
    let expected = match fs {
        Filesystem::Ntfs => "ntfs-3g",
        Filesystem::Exfat => "mount.exfat-fuse",
        Filesystem::Xfs => "fsxfsmount",
        Filesystem::Apfs => "fsapfsmount",
        _ => return false,
    };
    let Ok(output) = exec("/usr/bin/fstat", &args(&[mapped]), None) else {
        return false;
    };
    output
        .lines()
        .skip(1)
        .filter(|line| {
            let columns: Vec<_> = line.split_whitespace().collect();
            columns.len() >= 10
                && columns[0] == "root"
                && columns[1] == expected
                && columns.last() == Some(&mapped)
        })
        .count()
        == 1
}

pub(in crate::bsd) fn matches_filesystem_mount(mapped: &str, mount: &Mounted) -> bool {
    let Ok(fs) = probe(mapped) else {
        return false;
    };
    match fs {
        Filesystem::Ffs => mount.source == Path::new(mapped) && mount.filesystem == "ffs",
        Filesystem::Ext => {
            mount.source == Path::new(mapped) && mount.filesystem == "ext2fs" && mount.readonly
        }
        _ => {
            mount.source == Path::new("fusefs")
                && mount.filesystem == "fuse"
                && fuse_holds_mapping(mapped, fs)
        }
    }
}

pub(in crate::bsd) fn filesystem_has_posix_ownership(mapped: &str) -> AppResult<bool> {
    Ok(matches!(probe(mapped)?, Filesystem::Ffs))
}
pub(in crate::bsd) fn close_filesystem(_: &str) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signatures_are_explicit_and_unknown_data_is_rejected() {
        let mut bytes = vec![0; 128 * 1024];
        assert_eq!(signature(&bytes), None);
        bytes[65536 + 1372..65536 + 1376].copy_from_slice(&0x19540119u32.to_le_bytes());
        assert_eq!(signature(&bytes), Some(Filesystem::Ffs));
        bytes[3..11].copy_from_slice(b"EXFAT   ");
        assert_eq!(signature(&bytes), Some(Filesystem::Exfat));
        bytes[3..11].copy_from_slice(b"NTFS    ");
        assert_eq!(signature(&bytes), Some(Filesystem::Ntfs));
    }
    #[test]
    fn unsupported_create_selections_fail_before_allocation() {
        assert!(validate_filesystem(Some(CreateFilesystem::Btrfs), VolumeType::Softraid).is_err());
        assert!(validate_filesystem(Some(CreateFilesystem::Ufs1), VolumeType::Raid1c).is_ok());
    }
}
