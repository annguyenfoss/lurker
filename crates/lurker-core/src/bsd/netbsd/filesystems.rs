//! Native NetBSD inner filesystems. CGD keeps its whole-device identity while
//! non-FFS formats occupy a disklabel partition, retaining password verification.
use super::super::Mounted;
use super::{args, device_number, exec};
use crate::{AppError, AppResult, CreateFilesystem, VolumeType};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

pub(in super::super) fn validate_filesystem(
    fs: Option<CreateFilesystem>,
    backend: VolumeType,
) -> AppResult<()> {
    if backend != VolumeType::Cgd {
        return Err(AppError::new("NetBSD filesystem selection requires CGD."));
    }
    match fs.unwrap_or(CreateFilesystem::Ufs2) {
        CreateFilesystem::Ufs1
        | CreateFilesystem::Ufs2
        | CreateFilesystem::Ext4
        | CreateFilesystem::Exfat
        | CreateFilesystem::Ntfs
        | CreateFilesystem::Fat32
        | CreateFilesystem::Hammer2
        | CreateFilesystem::Zfs => Ok(()),
        other => Err(AppError::new(format!(
            "{} has no qualified NetBSD formatter/mount route.",
            other.as_str()
        ))),
    }
}
fn raw(device: &str) -> String {
    format!("/dev/r{}", device.trim_start_matches("/dev/"))
}
fn partition(device: &str) -> AppResult<String> {
    Ok(format!("/dev/cgd{}a", device_number(device, "cgd")?))
}
fn label_partition(device: &str, fs: CreateFilesystem) -> AppResult<String> {
    let r = raw(device);
    let geometry = exec("/sbin/dkctl", &args(&[&r, "getgeometry"]), None)?;
    let words: Vec<_> = geometry.split_whitespace().collect();
    let sectors = words
        .get(4)
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or_else(|| AppError::new("Native disk geometry exceeds the disklabel limit."))?;
    if sectors <= 4096 || words.get(7) != Some(&"512") {
        return Err(AppError::new(
            "CGD inner partitions require 512-byte sectors and at least 2 MiB.",
        ));
    }
    let kind = match fs {
        CreateFilesystem::Ext4 => "Linux Ext2",
        CreateFilesystem::Ntfs => "NTFS",
        CreateFilesystem::Fat32 => "MSDOS",
        CreateFilesystem::Zfs => "ZFS",
        _ => "unknown",
    };
    let label = format!("lurker|Lurker CGD inner filesystem:dt=SCSI:ty=fixed:se#512:ns#32:nt#64:nc#{}:su#{sectors}:pa#{}:oa#2048:ta={kind}:pd#{sectors}:od#0:td=unused:bs#8192:sb#65536:\n", sectors.div_ceil(2048), sectors - 2048);
    exec(
        "/sbin/disklabel",
        &args(&["-w", "-f", "/dev/stdin", &r, "lurker"]),
        Some(label.as_bytes()),
    )?;
    partition(device)
}
pub(in super::super) fn make_selected_filesystem(
    device: &str,
    fs: Option<CreateFilesystem>,
) -> AppResult<()> {
    let fs = fs.unwrap_or(CreateFilesystem::Ufs2);
    validate_filesystem(Some(fs), VolumeType::Cgd)?;
    if matches!(fs, CreateFilesystem::Ufs1 | CreateFilesystem::Ufs2) {
        exec(
            "/sbin/newfs",
            &args(&[
                "-I",
                "-O",
                if fs == CreateFilesystem::Ufs1 {
                    "1"
                } else {
                    "2"
                },
                &raw(device),
            ]),
            None,
        )?;
        return Ok(());
    }
    let inner = label_partition(device, fs)?;
    let r = raw(&inner);
    match fs {
        // fuse-ext2 supports only a restricted ext4 profile; do not format the
        // default modern Linux feature set its driver cannot safely handle.
        CreateFilesystem::Ext4 => {
            exec(
                "/usr/pkg/sbin/mke2fs",
                &args(&[
                    "-t",
                    "ext4",
                    "-O",
                    "^64bit,^metadata_csum,^orphan_file",
                    "-F",
                    &inner,
                ]),
                None,
            )?;
        }
        CreateFilesystem::Exfat => {
            exec("/usr/pkg/sbin/mkexfatfs", &args(&[&inner]), None)?;
        }
        CreateFilesystem::Ntfs => {
            exec("/usr/pkg/sbin/mkntfs", &args(&["-F", "-Q", &inner]), None)?;
        }
        CreateFilesystem::Fat32 => {
            exec("/sbin/newfs_msdos", &args(&["-F", "32", &r]), None)?;
        }
        CreateFilesystem::Hammer2 => {
            // NetBSD DIOCGMEDIASIZE describes the whole disk even for an a
            // slice. Supply the bounded slice size instead of writing past it.
            let geometry = exec("/sbin/dkctl", &args(&[&r, "getgeometry"]), None)?;
            let sectors = geometry
                .split_whitespace()
                .nth(4)
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| AppError::new("Missing HAMMER2 device geometry."))?;
            let mib = sectors.saturating_sub(2048) / 16384 * 8;
            if mib < 1024 {
                return Err(AppError::new(
                    "NetBSD HAMMER2 requires at least 1025 MiB for its verified inner partition.",
                ));
            }
            exec(
                "/usr/local/sbin/newfs_hammer2",
                &args(&["-L", "DATA", "-s", &format!("{mib}M"), &inner]),
                None,
            )?;
        }
        CreateFilesystem::Zfs => {
            let pool = format!("lurker_{}", super::super::unique_id());
            exec(
                "/sbin/zpool",
                &args(&[
                    "create",
                    "-f",
                    "-o",
                    "cachefile=none",
                    "-O",
                    "mountpoint=legacy",
                    &pool,
                    &inner,
                ]),
                None,
            )?;
            exec("/sbin/zpool", &args(&["export", &pool]), None)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
fn probe(device: &str) -> AppResult<(String, String)> {
    device_number(device, "cgd")?;
    for candidate in [device.to_owned(), partition(device)?] {
        // Base fstyp on NetBSD 11.1 fails its disk ioctl before recognizing
        // this upstream port. Probe only the public magic; native mount still
        // validates the complete on-disk structure and checksums.
        let mut header = [0u8; 512];
        if std::fs::File::open(raw(&candidate))
            .and_then(|mut f| f.read_exact(&mut header))
            .is_ok()
            && (header[..8] == 0x4841_4d32_0517_2011u64.to_le_bytes()
                || header[..8] == 0x4841_4d32_0517_2011u64.to_be_bytes())
        {
            return Ok((candidate, "hammer2".into()));
        }
        if let Ok(kind) = exec("/usr/sbin/fstyp", &args(&["-u", &raw(&candidate)]), None) {
            let kind = kind.trim();
            if matches!(
                kind,
                "ufs" | "ffs" | "exfat" | "ntfs" | "ext2fs" | "msdosfs" | "hammer2" | "zfs"
            ) {
                return Ok((candidate, kind.to_owned()));
            }
        }
    }
    Err(AppError::new(
        "Unrecognized or unsupported NetBSD inner filesystem.",
    ))
}
fn pool_name(device: &str) -> AppResult<String> {
    let listing = exec("/usr/sbin/zdb", &args(&["-l", &raw(device)]), None)?;
    let names: Vec<_> = listing
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("name: '")
                .and_then(|s| s.strip_suffix('\''))
        })
        .collect();
    let name = names
        .first()
        .copied()
        .filter(|s| {
            s.starts_with("lurker_")
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        })
        .ok_or_else(|| AppError::new("ZFS import requires a Lurker-owned pool name."))?;
    if names.iter().any(|n| *n != name) {
        return Err(AppError::new("Conflicting ZFS pool labels."));
    }
    Ok(name.to_owned())
}
fn pool_directory(pool: &str) -> PathBuf {
    PathBuf::from("/var/run/lurker-bsd").join(format!("zfs-{pool}"))
}
fn prepare_pool_directory(pool: &str, device: &str) -> AppResult<PathBuf> {
    let directory = pool_directory(pool);
    match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(AppError::io("Create private ZFS import directory", e)),
    }
    let metadata = std::fs::symlink_metadata(&directory)
        .map_err(|e| AppError::io("Inspect ZFS import directory", e))?;
    if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err(AppError::new("Unsafe ZFS import directory."));
    }
    let link = directory.join("device");
    match std::os::unix::fs::symlink(device, &link) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if std::fs::read_link(&link).ok().as_deref() != Some(Path::new(device)) {
                return Err(AppError::new("ZFS import link identity changed."));
            }
        }
        Err(e) => return Err(AppError::io("Create scoped ZFS import link", e)),
    }
    Ok(directory)
}
fn remove_pool_directory(pool: &str, device: &str) -> AppResult<()> {
    let directory = pool_directory(pool);
    let link = directory.join("device");
    if !directory.exists() {
        return Ok(());
    }
    if std::fs::read_link(&link).ok().as_deref() != Some(Path::new(device)) {
        return Err(AppError::new(
            "ZFS import link identity changed; preserving directory.",
        ));
    }
    std::fs::remove_file(link)
        .and_then(|_| std::fs::remove_dir(directory))
        .map_err(|e| AppError::io("Remove scoped ZFS import directory", e))
}
fn verify_pool(pool: &str, device: &str) -> AppResult<()> {
    let status = exec("/sbin/zpool", &args(&["status", pool]), None)?;
    let paths: Vec<_> = status
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|p| p.starts_with('/'))
        .collect();
    if paths.len() != 1
        || std::fs::canonicalize(paths[0]).ok().as_deref() != Some(Path::new(device))
    {
        return Err(AppError::new(
            "ZFS pool device identity differs; preserving pool.",
        ));
    }
    Ok(())
}
pub(in super::super) fn mount_filesystem(
    mapped: &str,
    target: &Path,
    readonly: bool,
) -> AppResult<()> {
    let (device, kind) = probe(mapped)?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let mode = if readonly { "ro" } else { "rw" };
    let options = format!("{mode},uid={},gid={},umask=077", owner.uid, owner.gid);
    let mut argv;
    let tool = match kind.as_str() {
        "ufs" | "ffs" => {
            argv = args(&["-o", if readonly { "ro" } else { "rw,log" }]);
            "/sbin/mount_ffs"
        }
        "exfat" => {
            argv = args(&["-o", &options]);
            "/usr/pkg/sbin/mount.exfat"
        }
        "ntfs" => {
            argv = args(&["-o", &options]);
            "/usr/pkg/bin/ntfs-3g"
        }
        "ext2fs" => {
            argv = args(&["-o", if readonly { "ro" } else { "rw+" }]);
            "/usr/pkg/bin/fuse-ext2"
        }
        "msdosfs" => {
            argv = args(&[
                "-o",
                mode,
                "-u",
                &owner.uid.to_string(),
                "-g",
                &owner.gid.to_string(),
                "-m",
                "700",
            ]);
            "/sbin/mount_msdos"
        }
        "hammer2" => {
            argv = args(&["-o", mode]);
            argv.extend([format!("{device}@DATA").into(), target.as_os_str().into()]);
            exec("/usr/local/sbin/mount_hammer2", &argv, None)?;
            return Ok(());
        }
        "zfs" => {
            let pool = pool_name(&device)?;
            let directory = prepare_pool_directory(&pool, &device)?;
            let directory = directory
                .to_str()
                .ok_or_else(|| AppError::new("Invalid ZFS import path."))?;
            // Import must fail if this name already exists; never reuse a pool
            // imported by another session, even when its name is identical.
            exec(
                "/sbin/zpool",
                &args(&[
                    "import",
                    "-d",
                    directory,
                    "-N",
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
            verify_pool(&pool, &device)?;
            exec(
                "/sbin/mount_zfs",
                &[
                    "-o".into(),
                    mode.into(),
                    pool.into(),
                    target.as_os_str().into(),
                ],
                None,
            )?;
            return Ok(());
        }
        _ => return Err(AppError::new("Unsupported NetBSD inner filesystem.")),
    };
    argv.extend([device.as_str().into(), target.as_os_str().into()]);
    if kind == "ntfs" {
        // NetBSD's libperfuse NTFS daemon retains inherited output descriptors.
        // Pipes would keep exec's drain threads blocked until filesystem close.
        let executable = crate::trusted_privileged_executable(Path::new(tool))?;
        let mut command = std::process::Command::new(executable);
        // The daemon must outlive the short-lived CLI's controlling terminal.
        // SAFETY: setsid is async-signal-safe and touches no shared Rust state.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let status = command
            .args(&argv)
            .env_clear()
            .env(
                "PATH",
                "/sbin:/bin:/usr/sbin:/usr/bin:/usr/pkg/sbin:/usr/pkg/bin",
            )
            .env("LC_ALL", "C")
            .env("HOME", "/root")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| AppError::io("Launch NetBSD NTFS mount", e))?;
        if !status.success() {
            return Err(AppError::new(format!(
                "NetBSD NTFS mount failed ({status})."
            )));
        }
        // perfuse completes its mount asynchronously after ntfs-3g's parent
        // exits. Wait for the exact native mount before the shared journal reads it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if super::super::mounts()?.iter().any(|m| {
                m.target == target
                    && m.source == Path::new(&device)
                    && m.filesystem == "puffs|perfuse|fuseblk"
            }) {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(AppError::new("NetBSD NTFS mount did not become ready."));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    } else {
        exec(tool, &argv, None)?;
    }
    Ok(())
}
pub(in super::super) fn matches_filesystem_mount(mapped: &str, mounted: &Mounted) -> bool {
    let Ok((device, kind)) = probe(mapped) else {
        return false;
    };
    if kind == "zfs" {
        return pool_name(&device).is_ok_and(|pool| {
            mounted.filesystem == "zfs"
                && mounted.source == Path::new(&pool)
                && verify_pool(&pool, &device).is_ok()
        });
    }
    if matches!(kind.as_str(), "exfat" | "ntfs" | "ext2fs") {
        // librefuse records the source in f_fstypename; f_mntfromname is
        // always /dev/puffs. Match both fields, including the exact CGD slice.
        return (mounted.source == Path::new("/dev/puffs")
            && mounted.filesystem == format!("puffs|{device}"))
            || (kind == "ntfs"
                && mounted.source == Path::new(&device)
                && mounted.filesystem == "puffs|perfuse|fuseblk");
    }
    let source_matches = mounted.source == Path::new(&device)
        || mounted.source == Path::new(&raw(&device))
        || (kind == "hammer2" && mounted.source == Path::new(&format!("{device}@DATA")));
    let expected = match kind.as_str() {
        "ufs" | "ffs" => "ffs",
        "msdosfs" => "msdos",
        other => other,
    };
    source_matches && mounted.filesystem == expected
}
pub(in super::super) fn filesystem_has_posix_ownership(mapped: &str) -> AppResult<bool> {
    Ok(!matches!(
        probe(mapped)?.1.as_str(),
        "exfat" | "ntfs" | "msdosfs"
    ))
}
pub(in super::super) fn close_filesystem(mapped: &str) -> AppResult<()> {
    let Ok((device, kind)) = probe(mapped) else {
        return Ok(());
    };
    if kind == "zfs" {
        let pool = pool_name(&device)?;
        // An already exported pool is the normal completed state after create.
        if exec(
            "/sbin/zpool",
            &args(&["list", "-H", "-o", "name", &pool]),
            None,
        )
        .is_err()
        {
            return remove_pool_directory(&pool, &device);
        }
        verify_pool(&pool, &device)?;
        let datasets = exec(
            "/sbin/zfs",
            &args(&["get", "-H", "-r", "-o", "value", "mounted", &pool]),
            None,
        )?;
        if datasets.lines().any(|s| s != "no") {
            return Err(AppError::new(
                "ZFS datasets remain mounted; refusing export.",
            ));
        }
        exec("/sbin/zpool", &args(&["export", &pool]), None)?;
        remove_pool_directory(&pool, &device)?;
    }
    Ok(())
}
