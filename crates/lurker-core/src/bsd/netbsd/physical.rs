//! Physical CGD sources are GPT wedges with independently checked UUIDs.
//! Wedge names are allocated dynamically and must never key recovery metadata.
use super::{args, exec};
use crate::{AppError, AppResult};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

fn wedge_name(value: &str) -> bool {
    value.strip_prefix("dk").is_some_and(digits)
}

fn parent_name(value: &str) -> bool {
    ["ld", "sd", "wd"]
        .iter()
        .any(|prefix| value.strip_prefix(prefix).is_some_and(digits))
}

fn wedge_info<'a>(listing: &'a str, wedge: &str) -> AppResult<(&'a str, u64, u64)> {
    let invalid = || AppError::new("Invalid native NetBSD GPT wedge information.");
    let mut lines = listing.lines();
    let first = lines.next().ok_or_else(invalid)?;
    let parent = first
        .strip_prefix(&format!("{wedge} at "))
        .and_then(|rest| rest.split_once(':'))
        .map(|(parent, _)| parent)
        .filter(|parent| parent_name(parent))
        .ok_or_else(invalid)?;
    let words: Vec<_> = lines
        .next()
        .ok_or_else(invalid)?
        .split_whitespace()
        .collect();
    if lines.next().is_some()
        || words.len() != 7
        || words[0] != format!("{wedge}:")
        || words[2] != "blocks"
        || words[3] != "at"
        || words[5] != "type:"
    {
        return Err(invalid());
    }
    let size = words[1].parse::<u64>().map_err(|_| invalid())?;
    let offset = words[4]
        .strip_suffix(',')
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(invalid)?;
    if size == 0 || offset == 0 || offset.checked_add(size).is_none() {
        return Err(invalid());
    }
    Ok((parent, offset, size))
}

fn canonical_uuid(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || bytes.iter().enumerate().any(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                *b != b'-'
            } else {
                !b.is_ascii_hexdigit()
            }
        })
        || !bytes
            .iter()
            .any(|b| matches!(b, b'1'..=b'9' | b'a'..=b'f' | b'A'..=b'F'))
    {
        None
    } else {
        Some(value.to_ascii_lowercase())
    }
}

fn gpt_uuid(listing: &str, offset: u64, size: u64) -> AppResult<String> {
    let invalid = || AppError::new("GPT partition UUID does not uniquely match the NetBSD wedge.");
    let rows: Vec<_> = listing
        .lines()
        .filter_map(|line| {
            let words: Vec<_> = line.split_whitespace().collect();
            if words.len() != 7 || words[3..6] != ["GPT", "part", "-"] {
                return None;
            }
            Some((
                words[0].parse::<u64>().ok(),
                words[1].parse::<u64>().ok(),
                words[2].parse::<u32>().ok().filter(|n| *n > 0),
                canonical_uuid(words[6]),
            ))
        })
        .collect();
    let matches: Vec<_> = rows
        .iter()
        .filter(|(start, blocks, _, _)| *start == Some(offset) && *blocks == Some(size))
        .collect();
    if matches.len() != 1 || matches[0].2.is_none() {
        return Err(invalid());
    }
    let uuid = matches[0].3.as_ref().ok_or_else(invalid)?;
    if rows
        .iter()
        .filter(|row| row.3.as_ref() == Some(uuid))
        .count()
        != 1
    {
        return Err(invalid());
    }
    Ok(uuid.to_owned())
}

fn geometry_bytes(listing: &str, raw: &str, blocks: u64) -> AppResult<u64> {
    let invalid = || AppError::new("NetBSD physical wedge geometry changed or exceeds its bounds.");
    let words: Vec<_> = listing.split_whitespace().collect();
    if words.len() != 9
        || words[0] != format!("{raw}:")
        || words[2] != "bytes"
        || words[3] != "in"
        || words[5] != "blocks"
        || words[6] != "of"
        || words[8] != "bytes"
        || words[4].parse::<u64>().ok() != Some(blocks)
    {
        return Err(invalid());
    }
    let sector = words[7].parse::<u64>().map_err(|_| invalid())?;
    if !(512..=65536).contains(&sector) || !sector.is_power_of_two() {
        return Err(invalid());
    }
    let bytes = blocks.checked_mul(sector).ok_or_else(invalid)?;
    if words[1].parse::<u64>().ok() != Some(bytes) || bytes < 64 * 1024 * 1024 {
        return Err(invalid());
    }
    Ok(bytes)
}

pub(in super::super) fn physical_device(source: &Path) -> AppResult<super::super::PhysicalDevice> {
    let source = fs::canonicalize(source)
        .map_err(|e| AppError::io("Resolve NetBSD physical partition", e))?;
    let name = source
        .strip_prefix("/dev")
        .ok()
        .and_then(|path| path.to_str())
        .filter(|name| wedge_name(name))
        .ok_or_else(|| AppError::new("NetBSD physical CGD requires a GPT /dev/dkN partition."))?;
    let before = fs::symlink_metadata(&source)
        .map_err(|e| AppError::io("Inspect NetBSD physical partition", e))?;
    if !before.file_type().is_block_device() || before.uid() != 0 {
        return Err(AppError::new(
            "NetBSD physical partition must be a root-owned block device.",
        ));
    }
    let raw = format!("/dev/r{name}");
    let listing = exec("/sbin/dkctl", &args(&[&raw, "getwedgeinfo"]), None)?;
    let (parent, offset, size) = wedge_info(&listing, name)?;
    let geometry = exec("/sbin/dkctl", &args(&[&raw, "getgeometry"]), None)?;
    let bytes = geometry_bytes(&geometry, &raw, size)?;
    let gpt = exec(
        "/sbin/gpt",
        &args(&["show", "-g", &format!("/dev/r{parent}d")]),
        None,
    )?;
    let uuid = gpt_uuid(&gpt, offset, size)?;
    let after = fs::symlink_metadata(&source)
        .map_err(|e| AppError::io("Recheck NetBSD physical partition", e))?;
    let confirmed = exec("/sbin/dkctl", &args(&[&raw, "getwedgeinfo"]), None)?;
    if (before.dev(), before.ino(), before.rdev()) != (after.dev(), after.ino(), after.rdev())
        || wedge_info(&confirmed, name)? != (parent, offset, size)
    {
        return Err(AppError::new(
            "NetBSD physical partition identity changed during inspection.",
        ));
    }
    Ok(super::super::PhysicalDevice {
        source,
        bytes,
        metadata_id: Some(uuid),
    })
}

pub(in super::super) fn ensure_physical_idle(source: &Path) -> AppResult<()> {
    let physical = physical_device(source)?;
    let name = physical
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap();
    let raw = format!("/dev/r{name}");
    if super::super::mounts()?
        .iter()
        .any(|mount| mount.source == physical.source || mount.source == Path::new(&raw))
    {
        return Err(AppError::new(
            "The NetBSD physical partition is already mounted.",
        ));
    }
    let cgd = exec("/sbin/cgdconfig", &args(&["-lv"]), None)?;
    if cgd
        .split_whitespace()
        .any(|word| word.trim_end_matches(',') == name)
    {
        return Err(AppError::new(
            "The NetBSD physical partition already backs a CGD mapping.",
        ));
    }
    // NetBSD returns exit 1 for `-l` when there are no configured swaps;
    // `-s` provides an unambiguous successful empty-state query.
    let summary = exec("/sbin/swapctl", &args(&["-s"]), None)?;
    let swaps = if summary.trim() == "no swap devices configured" {
        String::new()
    } else {
        exec("/sbin/swapctl", &args(&["-l"]), None)?
    };
    if swaps
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|path| path == physical.source.to_str().unwrap() || path == raw)
    {
        return Err(AppError::new(
            "The NetBSD physical partition is active swap.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const UUID: &str = "480a1c02-d6db-11f1-8000-000000000000";

    #[test]
    fn wedge_requires_canonical_physical_parent_and_checked_ranges() {
        assert_eq!(
            wedge_info(
                "dk1 at ld0: a label with spaces\ndk1: 73136095 blocks at 264192, type: ffs\n",
                "dk1"
            )
            .unwrap(),
            ("ld0", 264192, 73136095)
        );
        for input in [
            "dk1 at vnd0: a\ndk1: 42 blocks at 1, type: ffs\n",
            "dk1 at ld0: a\ndk2: 42 blocks at 1, type: ffs\n",
            "dk1 at ld0: a\ndk1: 1 blocks at 18446744073709551615, type: ffs\n",
        ] {
            assert!(wedge_info(input, "dk1").is_err());
        }
    }

    #[test]
    fn gpt_uuid_matches_exact_geometry_and_normalizes_case() {
        let text = format!("start size index contents\n1 1 Pri GPT header - {UUID}\n264192 73136095 2 GPT part - {}\n", UUID.to_ascii_uppercase());
        assert_eq!(gpt_uuid(&text, 264192, 73136095).unwrap(), UUID);
        assert!(gpt_uuid(&text, 264193, 73136095).is_err());
        assert!(gpt_uuid(&text, 264192, 73136094).is_err());
    }

    #[test]
    fn duplicate_geometry_or_uuid_never_selects_metadata() {
        let row = format!("2048 131072 1 GPT part - {UUID}\n");
        assert!(gpt_uuid(&(row.clone() + &row), 2048, 131072).is_err());
        assert!(gpt_uuid(
            &(row + &format!("200000 131072 2 GPT part - {UUID}\n")),
            2048,
            131072
        )
        .is_err());
    }

    #[test]
    fn malformed_uuid_or_partition_index_is_rejected() {
        for value in [
            "../../params",
            "00000000-0000-0000-0000-000000000000",
            "480a1c02-d6db-11f1-8000-00000000000z",
        ] {
            assert!(canonical_uuid(value).is_none());
        }
        assert!(gpt_uuid(&format!("2048 131072 0 GPT part - {UUID}\n"), 2048, 131072).is_err());
    }

    #[test]
    fn physical_geometry_is_checked_without_assuming_512_byte_sectors() {
        assert_eq!(
            geometry_bytes(
                "/dev/rdk2: 134217728 bytes in 32768 blocks of 4096 bytes\n",
                "/dev/rdk2",
                32768
            )
            .unwrap(),
            134217728
        );
        assert!(geometry_bytes(
            "/dev/rdk2: 1 bytes in 32768 blocks of 4096 bytes",
            "/dev/rdk2",
            32768
        )
        .is_err());
        assert!(geometry_bytes(
            "/dev/rdk2: 134217728 bytes in 32768 blocks of 4096 bytes",
            "/dev/rdk2",
            32769
        )
        .is_err());
        assert!(geometry_bytes(
            "/dev/rdk2: 1 bytes in 18446744073709551615 blocks of 4096 bytes",
            "/dev/rdk2",
            u64::MAX
        )
        .is_err());
    }
}
