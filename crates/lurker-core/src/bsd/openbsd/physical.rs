//! Native whole-disk sources retain disklabel and kernel-device identities.
use super::{args, exec};
use crate::{AppError, AppResult};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

fn disk_unit(path: &Path) -> AppResult<&str> {
    let name = path
        .strip_prefix("/dev")
        .ok()
        .and_then(Path::to_str)
        .and_then(|name| name.strip_suffix('c'))
        .filter(|name| {
            name.strip_prefix("sd")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .ok_or_else(|| {
            AppError::new("OpenBSD physical encryption requires a whole /dev/sdNc block device.")
        })?;
    Ok(name)
}

fn field<'a>(label: &'a str, key: &str) -> AppResult<&'a str> {
    let values: Vec<_> = label
        .lines()
        .filter_map(|line| line.strip_prefix(key))
        .collect();
    if values.len() != 1 || values[0].trim().is_empty() {
        return Err(AppError::new(
            "Ambiguous OpenBSD physical disklabel geometry.",
        ));
    }
    Ok(values[0].trim())
}

fn geometry(label: &str) -> AppResult<(u64, Option<String>)> {
    let invalid = || AppError::new("Invalid OpenBSD whole-disk geometry or disklabel identity.");
    if field(label, "disk:")? == "softraid" {
        return Err(AppError::new(
            "A softraid virtual volume cannot be used as a physical backing disk.",
        ));
    }
    let sector = field(label, "bytes/sector:")?
        .parse::<u64>()
        .map_err(|_| invalid())?;
    let sectors = field(label, "total sectors:")?
        .parse::<u64>()
        .map_err(|_| invalid())?;
    if !(512..=65536).contains(&sector) || !sector.is_power_of_two() {
        return Err(invalid());
    }
    let bytes = sector
        .checked_mul(sectors)
        .filter(|n| *n >= 64 * 1024 * 1024)
        .ok_or_else(invalid)?;
    let duid = field(label, "duid:")?;
    if duid.len() != 16 || !duid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let id = (duid != "0000000000000000").then(|| duid.to_ascii_lowercase());
    Ok((bytes, id))
}

fn partition_on_disk(value: &str, unit: &str) -> bool {
    let value = value.strip_prefix("/dev/").unwrap_or(value);
    let value = value.strip_prefix('r').unwrap_or(value);
    value
        .strip_prefix(unit)
        .is_some_and(|rest| rest.len() == 1 && matches!(rest.as_bytes()[0], b'a'..=b'p'))
}

pub(in super::super) fn physical_device(source: &Path) -> AppResult<super::super::PhysicalDevice> {
    let source =
        fs::canonicalize(source).map_err(|e| AppError::io("Resolve OpenBSD physical disk", e))?;
    let unit = disk_unit(&source)?;
    let before = fs::symlink_metadata(&source)
        .map_err(|e| AppError::io("Inspect OpenBSD physical disk", e))?;
    if before.uid() != 0 || !before.file_type().is_block_device() {
        return Err(AppError::new(
            "OpenBSD physical disk must be a root-owned block device.",
        ));
    }
    let controller = exec("/sbin/bioctl", &args(&["softraid0"]), None)?;
    if controller.lines().any(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        fields.len() == 6 && fields[0] == "softraid0" && fields[4] == unit
    }) {
        return Err(AppError::new(
            "A softraid virtual volume cannot be used as a physical backing disk.",
        ));
    }
    let label = exec("/sbin/disklabel", &args(&[unit]), None)?;
    let (bytes, metadata_id) = geometry(&label)?;
    let after = fs::symlink_metadata(&source)
        .map_err(|e| AppError::io("Recheck OpenBSD physical disk", e))?;
    let confirmed = exec("/sbin/disklabel", &args(&[unit]), None)?;
    if (before.dev(), before.ino(), before.rdev()) != (after.dev(), after.ino(), after.rdev())
        || geometry(&confirmed)? != (bytes, metadata_id.clone())
    {
        return Err(AppError::new(
            "OpenBSD physical disk identity changed during inspection.",
        ));
    }
    Ok(super::super::PhysicalDevice {
        source,
        bytes,
        metadata_id,
    })
}

/// Destructive preparation is separate from immutable identity inspection:
/// verified sessions must remain inspectable while their own mapping is active.
pub(in super::super) fn ensure_physical_idle(source: &Path) -> AppResult<()> {
    let physical = physical_device(source)?;
    let unit = disk_unit(&physical.source)?;
    if super::super::mounts()?.iter().any(|mount| {
        mount
            .source
            .to_str()
            .is_some_and(|value| partition_on_disk(value, unit))
    }) {
        return Err(AppError::new(
            "An OpenBSD physical disk partition is mounted; refusing destructive creation.",
        ));
    }
    let swap = exec("/sbin/swapctl", &args(&["-l"]), None)?;
    if swap
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|value| partition_on_disk(value, unit))
    {
        return Err(AppError::new(
            "An OpenBSD physical disk partition is active swap.",
        ));
    }
    let controller = exec("/sbin/bioctl", &args(&["softraid0"]), None)?;
    if controller.split_whitespace().any(|value| {
        value
            .strip_prefix('<')
            .and_then(|s| s.strip_suffix('>'))
            .is_some_and(|chunk| partition_on_disk(chunk, unit))
    }) {
        return Err(AppError::new(
            "The OpenBSD physical disk already backs a softraid volume.",
        ));
    }
    // FUSE mount rows hide their backing device. Include every existing raw
    // and block partition node so native daemons and foreign direct users are
    // detected without adopting or closing their resources.
    let mut nodes = Vec::new();
    for partition in b'a'..=b'p' {
        for raw in ["", "r"] {
            let path = format!("/dev/{raw}{unit}{}", char::from(partition));
            if Path::new(&path).exists() {
                nodes.push(path);
            }
        }
    }
    if nodes.is_empty() {
        return Err(AppError::new(
            "OpenBSD physical disk partition nodes are missing.",
        ));
    }
    let argv = nodes.into_iter().map(Into::into).collect::<Vec<_>>();
    let holders = exec("/usr/bin/fstat", &argv, None)?;
    if holders
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        != 1
        || !holders.starts_with("USER")
    {
        return Err(AppError::new(
            "An OpenBSD physical disk partition has an open native holder; refusing creation.",
        ));
    }
    // Recheck the same immutable identity after all foreign-resource queries.
    let confirmed = physical_device(&physical.source)?;
    if confirmed.bytes != physical.bytes || confirmed.metadata_id != physical.metadata_id {
        return Err(AppError::new(
            "OpenBSD physical disk changed during its idle check.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn label() -> String {
        "disk: SCSI disk\nduid: 480A1C02D6DB11F1\nbytes/sector: 512\ntotal sectors: 1048576\n"
            .to_owned()
    }
    #[test]
    fn whole_disk_names_never_accept_a_partition_or_vnode() {
        assert_eq!(disk_unit(Path::new("/dev/sd12c")).unwrap(), "sd12");
        for path in [
            "/dev/sd1a",
            "/dev/rsd1c",
            "/dev/vnd1c",
            "/tmp/sd1c",
            "/dev/sdc",
            "/dev/sd1/../sd2c",
        ] {
            assert!(disk_unit(Path::new(path)).is_err(), "{path}");
        }
    }
    #[test]
    fn physical_geometry_checks_overflow_and_normalizes_duid() {
        assert_eq!(
            geometry(&label()).unwrap(),
            (536870912, Some("480a1c02d6db11f1".to_owned()))
        );
        assert_eq!(
            geometry(&label().replace("480A1C02D6DB11F1", "0000000000000000"))
                .unwrap()
                .1,
            None
        );
        for text in [
            label().replace("512", "513"),
            label().replace("1048576", "18446744073709551615"),
            label().replace("1048576", "1"),
            label().replace("480A1C02D6DB11F1", "../../identity"),
            label() + "bytes/sector: 512\n",
            label().replace("SCSI disk", "softraid"),
        ] {
            assert!(geometry(&text).is_err(), "{text}");
        }
    }
    #[test]
    fn sibling_partition_matching_does_not_confuse_disk_numbers() {
        for path in ["sd1a", "sd1p", "/dev/sd1c", "/dev/rsd1b"] {
            assert!(partition_on_disk(path, "sd1"));
        }
        for path in ["sd10a", "sd1", "sd1aa", "/tmp/sd1a", "fusefs", "sd1q"] {
            assert!(!partition_on_disk(path, "sd1"));
        }
    }
}
