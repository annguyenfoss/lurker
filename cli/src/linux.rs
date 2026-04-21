use crate::error::{AppError, AppResult};
use crate::model::{ResolvedTargetKind, SourceKind};
use crate::sha256::hex_digest;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MountInfo {
    pub mount_point: PathBuf,
    pub source: Option<PathBuf>,
    pub fs_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUmountTarget {
    pub target_kind: ResolvedTargetKind,
    pub mapper_name: String,
    pub mapper_path: PathBuf,
    pub mountpoint: Option<PathBuf>,
    pub origin: PathBuf,
}

pub fn source_kind_for_path(path: &Path) -> Option<SourceKind> {
    let metadata = fs::metadata(path).ok()?;
    let file_type = metadata.file_type();
    if file_type.is_block_device() {
        Some(SourceKind::Block)
    } else if file_type.is_file() {
        Some(SourceKind::File)
    } else {
        None
    }
}

pub fn is_block_device_target(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.file_type().is_block_device())
        .unwrap_or(false)
}

pub fn absolute_path(input_path: &Path) -> AppResult<PathBuf> {
    if input_path.exists() {
        return fs::canonicalize(input_path).map_err(|err| {
            AppError::io(format!("Failed to resolve {}", input_path.display()), err)
        });
    }

    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    let base = input_path.file_name().ok_or_else(|| {
        AppError::new(format!(
            "Path is missing a usable file name: {}",
            input_path.display()
        ))
    })?;
    let parent = fs::canonicalize(parent)
        .map_err(|err| AppError::io(format!("Failed to resolve {}", parent.display()), err))?;
    Ok(parent.join(base))
}

pub fn canonical_device_path(device_path: &Path) -> PathBuf {
    fs::canonicalize(device_path).unwrap_or_else(|_| device_path.to_path_buf())
}

pub fn mapper_device_path(mapper_name: &str) -> PathBuf {
    Path::new("/dev/mapper").join(mapper_name)
}

pub fn normalize_mapper_name(mapper_reference: &Path) -> String {
    let reference = mapper_reference.as_os_str().as_bytes();
    let prefix = b"/dev/mapper/";
    if reference.starts_with(prefix) {
        String::from_utf8_lossy(&reference[prefix.len()..]).into_owned()
    } else {
        mapper_reference.as_os_str().to_string_lossy().into_owned()
    }
}

pub fn mapper_name_for_source(source_path: &Path, tag_override: Option<&str>) -> AppResult<String> {
    if let Some(tag_override) = tag_override {
        return mapper_name_for_tag(tag_override);
    }

    if is_block_device_target(source_path) {
        mapper_name_for_block_device(source_path)
    } else {
        mapper_name_for_path(source_path)
    }
}

pub fn mapper_name_for_path(path: &Path) -> AppResult<String> {
    let resolved_path = absolute_path(path)?;
    Ok(resolved_mapper_name_from_path(&resolved_path))
}

pub fn mapper_name_for_block_device(path: &Path) -> AppResult<String> {
    let resolved_path = canonical_device_path(path);
    Ok(resolved_mapper_name_from_path(&resolved_path))
}

pub fn mapper_name_for_tag(raw_tag: &str) -> AppResult<String> {
    let safe_tag = sanitize_component(raw_tag);
    if safe_tag.is_empty() {
        return Err(AppError::new(
            "Tag must contain at least one usable character.",
        ));
    }
    Ok(format!("lurker_{}", truncate_chars(&safe_tag, 56)))
}

pub fn size_gb_to_mib(raw: &str) -> AppResult<u64> {
    if raw.is_empty() {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    let mut parts = raw.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some() {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    if whole.is_empty() && fraction.is_none() {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    let whole_valid = !whole.is_empty() && whole.chars().all(|ch| ch.is_ascii_digit());
    let fraction_valid = fraction
        .map(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or(true);

    let valid = match (whole.is_empty(), fraction) {
        (false, None) => whole_valid,
        (false, Some(_)) => whole_valid && fraction_valid,
        (true, Some(_)) => fraction_valid,
        (true, None) => false,
    };

    if !valid {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    let whole_value = if whole.is_empty() {
        0u128
    } else {
        whole
            .parse::<u128>()
            .map_err(|_| AppError::new("Size must be a positive number of GB."))?
    };
    let mut size_mib = whole_value
        .checked_mul(1024)
        .ok_or_else(|| AppError::new("Size is too large."))?;

    if let Some(fraction) = fraction {
        let denominator = 10u128
            .checked_pow(fraction.len() as u32)
            .ok_or_else(|| AppError::new("Size is too large."))?;
        let fraction_value = fraction
            .parse::<u128>()
            .map_err(|_| AppError::new("Size must be a positive number of GB."))?;
        size_mib = size_mib
            .checked_add(
                fraction_value
                    .checked_mul(1024)
                    .ok_or_else(|| AppError::new("Size is too large."))?
                    / denominator,
            )
            .ok_or_else(|| AppError::new("Size is too large."))?;
    }

    if size_mib == 0 {
        return Err(AppError::new("Size must be greater than zero."));
    }

    u64::try_from(size_mib).map_err(|_| AppError::new("Size is too large."))
}

pub fn read_mountinfo() -> AppResult<Vec<MountInfo>> {
    let content = fs::read_to_string("/proc/self/mountinfo")
        .map_err(|err| AppError::io("Failed to read /proc/self/mountinfo", err))?;
    let mut mounts = Vec::new();
    for line in content.lines() {
        if let Some(entry) = parse_mountinfo_line(line) {
            mounts.push(entry);
        }
    }
    Ok(mounts)
}

pub fn find_mount_source_for_mountpoint(target: &Path) -> AppResult<Option<PathBuf>> {
    let mounts = read_mountinfo()?;
    for mount in mounts {
        if mount.mount_point == target {
            return Ok(mount.source);
        }
    }
    Ok(None)
}

pub fn find_mountpoint_for_source(source: &Path) -> AppResult<Option<PathBuf>> {
    let mounts = read_mountinfo()?;
    for mount in &mounts {
        if mount.source.as_deref() == Some(source) {
            return Ok(Some(mount.mount_point.clone()));
        }
    }

    let canonical_source = canonical_device_path(source);
    if canonical_source != source {
        for mount in mounts {
            if let Some(candidate) = mount.source {
                if canonical_device_path(&candidate) == canonical_source {
                    return Ok(Some(mount.mount_point));
                }
            }
        }
    }

    Ok(None)
}

pub fn mapper_name_from_device_path(device_path: &Path) -> AppResult<String> {
    if device_path.starts_with("/dev/mapper") {
        return Ok(normalize_mapper_name(device_path));
    }

    let canonical_device = canonical_device_path(device_path);
    let entries = fs::read_dir("/dev/mapper")
        .map_err(|err| AppError::io("Failed to inspect /dev/mapper", err))?;
    for entry in entries {
        let entry = entry.map_err(|err| AppError::io("Failed to inspect /dev/mapper", err))?;
        let path = entry.path();
        if path == Path::new("/dev/mapper/control") {
            continue;
        }
        if canonical_device_path(&path) == canonical_device {
            return Ok(normalize_mapper_name(&path));
        }
    }

    Err(AppError::new(format!(
        "Mapper is not active: {}",
        device_path.display()
    )))
}

pub fn active_lurker_mappers_for_block_device(target_path: &Path) -> AppResult<Vec<String>> {
    let target_identity = match device_identity_for_path(target_path)? {
        Some(identity) => identity,
        None => return Ok(Vec::new()),
    };

    let mut matches = Vec::new();
    let dm_entries = fs::read_dir("/sys/class/block")
        .map_err(|err| AppError::io("Failed to inspect /sys/class/block", err))?;
    for entry in dm_entries {
        let entry = entry.map_err(|err| AppError::io("Failed to inspect /sys/class/block", err))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("dm-") {
            continue;
        }

        let mapper_name = match mapper_name_from_dm_sysfs_path(&path)? {
            Some(value) if value.starts_with("lurker_") => value,
            _ => continue,
        };

        let slaves = path.join("slaves");
        let slave_entries = match fs::read_dir(&slaves) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for slave in slave_entries {
            let slave = match slave {
                Ok(value) => value.path(),
                Err(_) => continue,
            };
            let identity = device_identity_for_sysfs_block_path(&slave)?;
            if identity.as_deref() == Some(target_identity.as_str()) {
                matches.push(mapper_name.clone());
                break;
            }
        }
    }

    Ok(matches)
}

pub fn resolve_umount_target(
    target: &Path,
    tag_override: Option<&str>,
) -> AppResult<ResolvedUmountTarget> {
    if target.starts_with("/dev/mapper") {
        let mapper_name = mapper_name_from_device_path(target)?;
        let mapper_path = mapper_device_path(&mapper_name);
        if !mapper_path.exists() {
            return Err(AppError::new(format!(
                "Mapper is not active: {}",
                target.display()
            )));
        }
        let mountpoint = find_mountpoint_for_source(&mapper_path)?;
        return Ok(ResolvedUmountTarget {
            target_kind: ResolvedTargetKind::Mapper,
            mapper_name,
            mapper_path,
            mountpoint,
            origin: target.to_path_buf(),
        });
    }

    if target.is_dir() {
        let canonical_target = absolute_path(target)?;
        let source = find_mount_source_for_mountpoint(&canonical_target)?
            .ok_or_else(|| AppError::new(format!("Not a mounted path: {}", target.display())))?;
        let mapper_name = mapper_name_from_device_path(&source).map_err(|_| {
            AppError::new(format!(
                "Mounted path is not backed by a device-mapper mapper: {}",
                target.display()
            ))
        })?;
        let mapper_path = mapper_device_path(&mapper_name);
        return Ok(ResolvedUmountTarget {
            target_kind: ResolvedTargetKind::Mountpoint,
            mapper_name,
            mapper_path,
            mountpoint: Some(canonical_target.clone()),
            origin: canonical_target,
        });
    }

    if is_block_device_target(target) {
        let canonical_target = canonical_device_path(target);
        let mapper_name = if let Some(tag_override) = tag_override {
            mapper_name_for_source(&canonical_target, Some(tag_override))?
        } else {
            let matching_mappers = active_lurker_mappers_for_block_device(&canonical_target)?;
            if matching_mappers.is_empty() {
                return Err(AppError::new(format!(
                    "Block device is not open via an active lurker mapper: {}",
                    target.display()
                )));
            }
            if matching_mappers.len() > 1 {
                return Err(AppError::new(format!(
                    "Block device matches multiple active lurker mappers: {}. Use a mountpoint or explicit /dev/mapper path.",
                    matching_mappers.join(" ")
                )));
            }
            matching_mappers[0].clone()
        };

        let mapper_path = mapper_device_path(&mapper_name);
        let mountpoint = find_mountpoint_for_source(&mapper_path)?;
        if mountpoint.is_none() && !mapper_path.exists() {
            if tag_override.is_some() {
                return Err(AppError::new(format!(
                    "Block device is not open: {}",
                    target.display()
                )));
            }
            return Err(AppError::new(format!(
                "Block device is not open via an active lurker mapper: {}",
                target.display()
            )));
        }

        return Ok(ResolvedUmountTarget {
            target_kind: ResolvedTargetKind::Block,
            mapper_name,
            mapper_path,
            mountpoint,
            origin: canonical_target,
        });
    }

    match source_kind_for_path(target) {
        Some(SourceKind::File) => {}
        _ => {
            return Err(AppError::new(format!(
                "File does not exist or is not a regular file: {}",
                target.display()
            )));
        }
    }

    let canonical_target = absolute_path(target)?;
    let mapper_name = mapper_name_for_source(&canonical_target, tag_override)?;
    let mapper_path = mapper_device_path(&mapper_name);
    let mountpoint = find_mountpoint_for_source(&mapper_path)?;
    if mountpoint.is_none() && !mapper_path.exists() {
        return Err(AppError::new(format!(
            "Container is not open: {}",
            target.display()
        )));
    }

    Ok(ResolvedUmountTarget {
        target_kind: ResolvedTargetKind::File,
        mapper_name,
        mapper_path,
        mountpoint,
        origin: canonical_target,
    })
}

fn resolved_mapper_name_from_path(resolved_path: &Path) -> String {
    let base_name = resolved_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "volume".into());
    let safe_base = sanitize_component(&base_name);
    let safe_base = if safe_base.is_empty() {
        "volume".into()
    } else {
        safe_base
    };
    let checksum = hex_digest(resolved_path.as_os_str().as_bytes());
    format!(
        "lurker_{}_{}",
        truncate_chars(&safe_base, 48),
        &checksum[..12]
    )
}

fn sanitize_component(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    output.trim_matches('_').chars().collect::<String>()
}

fn truncate_chars(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn sysfs_block_path_for_device(device_path: &Path) -> AppResult<Option<PathBuf>> {
    let canonical_device = canonical_device_path(device_path);
    let block_name = match canonical_device.file_name() {
        Some(name) => name,
        None => return Ok(None),
    };
    let path = Path::new("/sys/class/block").join(block_name);
    if path.exists() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

fn device_identity_for_sysfs_block_path(sysfs_block_path: &Path) -> AppResult<Option<String>> {
    let path = sysfs_block_path.join("dev");
    match fs::read_to_string(&path) {
        Ok(value) => Ok(Some(value.trim().to_string())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(AppError::io(
            format!("Failed to read {}", path.display()),
            err,
        )),
    }
}

fn device_identity_for_path(path: &Path) -> AppResult<Option<String>> {
    let Some(sysfs_path) = sysfs_block_path_for_device(path)? else {
        return Ok(None);
    };
    device_identity_for_sysfs_block_path(&sysfs_path)
}

fn mapper_name_from_dm_sysfs_path(dm_sysfs_path: &Path) -> AppResult<Option<String>> {
    let name_path = dm_sysfs_path.join("dm/name");
    match fs::read_to_string(&name_path) {
        Ok(value) => Ok(Some(value.trim().to_string())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(AppError::io(
            format!("Failed to read {}", name_path.display()),
            err,
        )),
    }
}

fn parse_mountinfo_line(line: &str) -> Option<MountInfo> {
    let (left, right) = line.split_once(" - ")?;
    let left_parts: Vec<&str> = left.split_whitespace().collect();
    if left_parts.len() < 5 {
        return None;
    }

    let right_parts: Vec<&str> = right.split_whitespace().collect();
    if right_parts.len() < 2 {
        return None;
    }

    Some(MountInfo {
        mount_point: PathBuf::from(unescape_mount_field(left_parts[4])),
        source: Some(PathBuf::from(unescape_mount_field(right_parts[1]))),
        fs_type: right_parts[0].to_string(),
    })
}

fn unescape_mount_field(value: &str) -> OsString {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let octal = &bytes[index + 1..index + 4];
            if octal.iter().all(|digit| (b'0'..=b'7').contains(digit)) {
                let decoded = (octal[0] - b'0') * 64 + (octal[1] - b'0') * 8 + (octal[2] - b'0');
                output.push(decoded);
                index += 4;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    OsString::from_vec(output)
}

#[cfg(test)]
mod tests {
    use super::{mapper_name_for_tag, parse_mountinfo_line, size_gb_to_mib, unescape_mount_field};

    #[test]
    fn size_parser_accepts_decimals() {
        assert_eq!(size_gb_to_mib("1").unwrap(), 1024);
        assert_eq!(size_gb_to_mib("1.5").unwrap(), 1536);
        assert_eq!(size_gb_to_mib(".5").unwrap(), 512);
    }

    #[test]
    fn tag_sanitization_matches_expected_shape() {
        assert_eq!(mapper_name_for_tag("work").unwrap(), "lurker_work");
        assert_eq!(
            mapper_name_for_tag(" weird tag ").unwrap(),
            "lurker_weird_tag"
        );
    }

    #[test]
    fn mountinfo_unescapes_paths() {
        assert_eq!(
            unescape_mount_field("/mnt/my\\040vault"),
            std::ffi::OsString::from("/mnt/my vault")
        );
    }

    #[test]
    fn mountinfo_parsing_extracts_mountpoint_and_source() {
        let line =
            "123 456 0:45 / /mnt/my\\040vault rw,relatime - btrfs /dev/mapper/lurker_test rw";
        let entry = parse_mountinfo_line(line).unwrap();
        assert_eq!(entry.mount_point, std::path::Path::new("/mnt/my vault"));
        assert_eq!(
            entry.source.unwrap(),
            std::path::Path::new("/dev/mapper/lurker_test")
        );
        assert_eq!(entry.fs_type, "btrfs");
    }
}
