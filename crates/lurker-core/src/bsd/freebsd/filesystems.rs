//! FreeBSD inner filesystems. Native kernel drivers are preferred; FUSE engines
//! are system packages and receive an explicit source name for mount identity.
use super::super::{Mounted, args, exec};
use crate::{AppError, AppResult, CreateFilesystem, VolumeType};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub(super) fn validate(selected: Option<CreateFilesystem>, backend: VolumeType) -> AppResult<()> {
    if selected == Some(CreateFilesystem::Ntfs) && backend == VolumeType::Geli {
        return Err(AppError::new(
            "FreeBSD's packaged mkntfs does not complete on GELI's fixed 4096-byte providers; NTFS creation requires VeraCrypt.",
        ));
    }
    if selected == Some(CreateFilesystem::Xfs) {
        return Err(AppError::new(
            "The packaged FreeBSD LKL driver supports read-only XFS imports only; its append writes fail, so XFS creation is unavailable.",
        ));
    }
    match selected {
        None
        | Some(
            CreateFilesystem::Ufs1
            | CreateFilesystem::Ufs2
            | CreateFilesystem::Ext4
            | CreateFilesystem::Xfs
            | CreateFilesystem::Ntfs
            | CreateFilesystem::Exfat
            | CreateFilesystem::Hammer2
            | CreateFilesystem::Zfs,
        ) => Ok(()),
        Some(fs) => Err(AppError::new(format!(
            "FreeBSD cannot create the selected {} filesystem with an available native formatter.",
            fs.as_str()
        ))),
    }
}

pub(super) fn format(mapped: &str, selected: Option<CreateFilesystem>) -> AppResult<()> {
    match selected.unwrap_or(CreateFilesystem::Ufs2) {
        CreateFilesystem::Ufs1 => exec("/sbin/newfs", &args(&["-O", "1", "-U", mapped]), None)?,
        CreateFilesystem::Ufs2 => exec("/sbin/newfs", &args(&["-O", "2", "-U", mapped]), None)?,
        CreateFilesystem::Ext4 => exec("/usr/local/sbin/mkfs.ext4", &args(&["-F", mapped]), None)?,
        CreateFilesystem::Xfs => exec("/usr/local/sbin/mkfs.xfs", &args(&["-f", mapped]), None)?,
        CreateFilesystem::Ntfs => {
            // ntfs-3g's FreeBSD formatter otherwise assumes 512-byte sectors,
            // which cannot be written through the fixed 4096-byte GELI profile.
            let info = exec("/usr/sbin/diskinfo", &args(&[mapped]), None)?;
            let sector = sector_size(mapped, &info)?;
            let sectors = info
                .split_whitespace()
                .nth(3)
                .and_then(|s| s.parse::<u64>().ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| AppError::new("Missing decrypted device sector count."))?;
            exec(
                "/usr/local/sbin/mkntfs",
                &args(&[
                    "-F",
                    "-Q",
                    "-s",
                    &sector.to_string(),
                    mapped,
                    &sectors.to_string(),
                ]),
                None,
            )?
        }
        CreateFilesystem::Exfat => exec("/usr/local/sbin/mkexfatfs", &args(&[mapped]), None)?,
        CreateFilesystem::Hammer2 => exec(
            "/usr/local/sbin/newfs_hammer2",
            &args(&["-L", "DATA", mapped]),
            None,
        )?,
        CreateFilesystem::Zfs => {
            let pool = format!("lurker_{}", super::super::unique_id().replace('-', "_"));
            exec(
                "/sbin/zpool",
                &args(&[
                    "create",
                    "-m",
                    "legacy",
                    "-o",
                    "cachefile=none",
                    &pool,
                    mapped,
                ]),
                None,
            )?;
            exec("/sbin/zpool", &args(&["export", &pool]), None)?
        }
        fs => {
            return Err(AppError::new(format!(
                "FreeBSD cannot format {}.",
                fs.as_str()
            )));
        }
    };
    Ok(())
}

fn probe(mapped: &str) -> AppResult<String> {
    let mut file = File::open(mapped).map_err(|e| AppError::io("Open decrypted filesystem", e))?;
    let mut bytes = [0; 4096];
    file.read_exact(&mut bytes)
        .map_err(|e| AppError::io("Probe decrypted filesystem", e))?;
    if bytes.starts_with(b"XFSB") {
        return Ok("xfs".into());
    }
    if &bytes[3..11] == b"EXFAT   " {
        return Ok("exfat".into());
    }
    if &bytes[32..36] == b"NXSB" {
        return Ok("apfs".into());
    }
    if &bytes[3..11] == b"NTFS    " {
        return Ok("ntfs".into());
    }
    if bytes[1080..1082] == [0x53, 0xef] {
        return Ok("ext2fs".into());
    }
    if bytes[..8] == [0x11, 0x20, 0x17, 0x05, 0x32, 0x4d, 0x41, 0x48] {
        return Ok("hammer2".into());
    }
    // GELI enforces its 4096-byte sector geometry, including probe reads.
    file.seek(SeekFrom::Start(65536))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|e| AppError::io("Probe Btrfs superblock", e))?;
    if &bytes[64..72] == b"_BHRfS_M" {
        return Ok("btrfs".into());
    }
    let value = match exec("/usr/sbin/fstyp", &args(&[mapped]), None) {
        Ok(value) => value,
        // fstyp's ZFS recognizer issues 512-byte reads on FreeBSD. zdb uses
        // native aligned label I/O, including GELI's 4096-byte providers.
        Err(_) if pool_name(mapped).is_ok() => return Ok("zfs".into()),
        Err(error) => return Err(error),
    };
    match value.trim() {
        "ufs" | "zfs" | "hammer2" => Ok(value.trim().into()),
        _ => Err(AppError::new("Unsupported FreeBSD inner filesystem.")),
    }
}

fn pool_name(mapped: &str) -> AppResult<String> {
    let labels = exec("/usr/sbin/zdb", &args(&["-l", mapped]), None)?;
    if !single_device_labels(&labels) {
        return Err(AppError::new(
            "Only single-device ZFS labels without child, spare or cache devices are accepted.",
        ));
    }
    let names: std::collections::HashSet<&str> = labels
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("name: '")
                .and_then(|s| s.strip_suffix('\''))
        })
        .collect();
    if names.len() != 1 {
        return Err(AppError::new("Ambiguous ZFS pool labels."));
    }
    let name = names.into_iter().next().unwrap();
    if !name.starts_with("lurker_") || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(AppError::new(
            "Only private Lurker single-device ZFS pools are accepted.",
        ));
    }
    Ok(name.into())
}

fn verify_pool(mapped: &str, pool: &str) -> AppResult<()> {
    let status = exec("/sbin/zpool", &args(&["status", "-P", pool]), None)?;
    let devices: Vec<_> = status
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|s| s.starts_with('/'))
        .collect();
    if devices != [mapped] {
        return Err(AppError::new(
            "ZFS pool is not the recorded single decrypted device.",
        ));
    }
    Ok(())
}

pub(super) fn mount(mapped: &str, target: &Path, readonly: bool) -> AppResult<()> {
    let fs = probe(mapped)?;
    let mode = if readonly { "ro" } else { "rw" };
    let owner = crate::new_filesystem::invoking_owner()?;
    let ownership = format!(
        "{mode},uid={},gid={},umask=077,fsname={mapped}",
        owner.uid, owner.gid
    );
    let (program, mut argv) = match fs.as_str() {
        "apfs" if readonly => {
            let input = apfs_input(mapped)?;
            let mut argv = args(&["-X", &format!("ro,fsname={mapped}")]);
            argv.push(input.as_os_str().into());
            ("/usr/local/bin/fsapfsmount", argv)
        }
        "apfs" => {
            return Err(AppError::new(
                "The FreeBSD APFS driver supports read-only mounts only.",
            ));
        }
        "ufs" | "ext2fs" => ("/sbin/mount", args(&["-t", &fs, "-o", mode, mapped])),
        "hammer2" => (
            "/usr/local/sbin/mount_hammer2",
            args(&["-o", mode, &format!("{mapped}@DATA")]),
        ),
        "exfat" => (
            "/usr/local/sbin/mount.exfat",
            args(&["-o", &ownership, mapped]),
        ),
        "ntfs" => (
            "/usr/local/bin/ntfs-3g",
            args(&[
                "-o",
                &format!("{mode},uid={},gid={},umask=077", owner.uid, owner.gid),
                mapped,
            ]),
        ),
        "xfs" | "btrfs" => {
            if !readonly {
                return Err(AppError::new(
                    "Writable XFS/Btrfs mounts are unavailable with the packaged FreeBSD LKL driver because append writes fail; use --readonly.",
                ));
            }
            let info = exec("/usr/sbin/diskinfo", &args(&[mapped]), None)?;
            if sector_size(mapped, &info)? != 512 {
                return Err(AppError::new(
                    "FreeBSD's packaged LKL driver cannot mount XFS/Btrfs on a provider with sectors larger than 512 bytes.",
                ));
            }
            (
                "/usr/local/bin/lklfuse",
                args(&[
                    "-s",
                    "-o",
                    &format!(
                        "{mode},type={fs},fsname={mapped}{}",
                        if fs == "xfs" { ",opts=norecovery" } else { "" }
                    ),
                    mapped,
                ]),
            )
        }
        "zfs" => {
            let pool = pool_name(mapped)?;
            let imported = exec("/sbin/zpool", &args(&["list", "-H", "-o", "name"]), None)?;
            if imported.lines().any(|n| n == pool) {
                verify_pool(mapped, &pool)?;
            } else {
                exec(
                    "/sbin/zpool",
                    &args(&[
                        "import",
                        "-N",
                        "-d",
                        mapped,
                        "-o",
                        "cachefile=none",
                        "-o",
                        if readonly {
                            "readonly=on"
                        } else {
                            "readonly=off"
                        },
                        &pool,
                    ]),
                    None,
                )?;
                verify_pool(mapped, &pool)?;
            }
            let mut argv = args(&["-t", "zfs", "-o", mode, &pool]);
            argv.push(target.as_os_str().into());
            exec("/sbin/mount", &argv, None)?;
            return Ok(());
        }
        _ => return Err(AppError::new("Unsupported FreeBSD inner filesystem.")),
    };
    argv.push(target.as_os_str().into());
    exec(program, &argv, None)?;
    if program == "/usr/local/bin/lklfuse" {
        // LKL forks before starting its kernel/filesystem. A bounded native
        // metadata request must succeed before the CLI reports a usable mount.
        let mut check = args(&["-f", "%d"]);
        check.push(target.as_os_str().into());
        super::super::exec_in_locale(
            "/usr/bin/stat",
            &check,
            None,
            "C",
            None,
            std::time::Duration::from_secs(30),
            None,
        )
        .map_err(|e| {
            AppError::new(format!(
                "The FreeBSD filesystem driver did not become ready: {e}"
            ))
        })?;
    }
    Ok(())
}

pub(super) fn matches_mount(mapped: &str, mounted: &Mounted) -> bool {
    if mounted.source == Path::new(mapped) {
        return true;
    }
    if mounted.filesystem == "hammer2" && mounted.source == Path::new(&format!("{mapped}@DATA")) {
        return true;
    }
    mounted.filesystem == "zfs"
        && pool_name(mapped).is_ok_and(|pool| {
            mounted.source == Path::new(&pool) && verify_pool(mapped, &pool).is_ok()
        })
}
pub(super) fn posix(mapped: &str) -> AppResult<bool> {
    Ok(!matches!(probe(mapped)?.as_str(), "ntfs" | "exfat"))
}
pub(super) fn close(mapped: &str) -> AppResult<()> {
    if probe(mapped).is_ok_and(|fs| fs == "zfs") {
        let pool = pool_name(mapped)?;
        let imported = exec("/sbin/zpool", &args(&["list", "-H", "-o", "name"]), None)?;
        if imported.lines().any(|n| n == pool) {
            verify_pool(mapped, &pool)?;
            // Export normally unmounts every dataset. The shared lifecycle has
            // authority over its recorded root mount only; retain other mounts.
            if super::super::mounts()?.iter().any(|mount| {
                mount.filesystem == "zfs"
                    && mount.source.to_str().is_some_and(|source| {
                        source == pool
                            || source.starts_with(&format!("{pool}/"))
                            || source.starts_with(&format!("{pool}@"))
                    })
            }) {
                return Err(AppError::new(
                    "The private ZFS pool still has mounted datasets; refusing export and retaining its encrypted mapping.",
                ));
            }
            exec("/sbin/zpool", &args(&["export", &pool]), None)?;
        }
    }
    Ok(())
}

fn sector_size(mapped: &str, info: &str) -> AppResult<u32> {
    let mut fields = info.split_whitespace();
    let source = fields.next();
    let sector = fields.next().and_then(|s| s.parse::<u32>().ok());
    match (source, sector) {
        (Some(source), Some(sector))
            if source == mapped && (512..=65536).contains(&sector) && sector.is_power_of_two() =>
        {
            Ok(sector)
        }
        _ => Err(AppError::new(
            "Unrecognized decrypted device sector geometry.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ntfs_sector_geometry_must_describe_the_exact_decrypted_device() {
        assert_eq!(
            sector_size("/dev/md0.eli", "/dev/md0.eli 4096 536866816 131071 0 0").unwrap(),
            4096
        );
        assert_eq!(
            sector_size("/dev/md0", "/dev/md0 512 536870912 1048576 0 0").unwrap(),
            512
        );
        assert!(sector_size("/dev/md0.eli", "/dev/md1.eli 4096 536866816").is_err());
        assert!(sector_size("/dev/md0", "/dev/md0 513 536870912").is_err());
    }
}

/// libfsapfs makes byte-sized metadata reads that FreeBSD raw providers reject.
/// VeraCrypt already exposes its decrypted regular FUSE image; use only the
/// file proven to be this recorded md provider's kernel backing vnode.
fn apfs_input(mapped: &str) -> AppResult<PathBuf> {
    if mapped.ends_with(".eli") {
        return Err(AppError::new(
            "The FreeBSD APFS reader requires a regular decrypted image; GELI exposes only a raw provider, so APFS imports are unavailable.",
        ));
    }
    let mut selected = None;
    for mount in super::super::mounts()? {
        let name = mount
            .target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let nonce = name.strip_prefix(".veracrypt_aux_mnt-").unwrap_or("");
        if mount.target.parent() != Some(Path::new("/tmp"))
            || nonce.len() != 32
            || !nonce.bytes().all(|b| b.is_ascii_hexdigit())
            || mount.source != Path::new("veracrypt")
            || !matches!(mount.filesystem.as_str(), "fuse" | "fusefs")
        {
            continue;
        }
        let path = mount.target.join("volume");
        if super::verify_backing(mapped, &path).is_ok() && selected.replace(path).is_some() {
            return Err(AppError::new(
                "Ambiguous VeraCrypt decrypted image identity.",
            ));
        }
    }
    selected.ok_or_else(|| {
        AppError::new("No verified VeraCrypt regular image is available for the APFS reader.")
    })
}

/// Each valid label must describe one disk leaf, with no other device arrays.
/// Label paths may name an earlier md unit; native import remaps the sole GUID.
fn single_device_labels(labels: &str) -> bool {
    let mut names = 0;
    let mut counts = 0;
    let mut leaves = 0;
    for line in labels.lines().map(str::trim) {
        if line.starts_with("name:") {
            names += 1;
        }
        if let Some(count) = line.strip_prefix("vdev_children:") {
            if count.trim() != "1" {
                return false;
            }
            counts += 1;
        }
        if let Some(kind) = line.strip_prefix("type:") {
            if kind.trim() != "'disk'" {
                return false;
            }
            leaves += 1;
        }
        if ["children", "spares", "l2cache"].iter().any(|key| {
            line.strip_prefix(key)
                .is_some_and(|rest| rest.starts_with('[') || rest.starts_with(':'))
        }) {
            return false;
        }
    }
    names > 0 && names == counts && counts == leaves
}

#[cfg(test)]
mod zfs_label_tests {
    use super::single_device_labels;
    #[test]
    fn labels_require_one_disk_without_additional_devices() {
        let label = "name: 'lurker_test'\nvdev_children: 1\nvdev_tree:\n type: 'disk'\n path: '/dev/md17.eli'\n";
        assert!(single_device_labels(label));
        assert!(single_device_labels(&format!("{label}{label}")));
        assert!(!single_device_labels(
            &label.replace("vdev_children: 1", "vdev_children: 2")
        ));
        assert!(!single_device_labels(&label.replace("'disk'", "'mirror'")));
        for extra in ["children[0]:", "spares[0]:", "l2cache[0]:"] {
            assert!(!single_device_labels(&format!("{label}{extra}\n")));
        }
        assert!(!single_device_labels(
            &label.replace("vdev_children: 1\n", "")
        ));
    }
}
