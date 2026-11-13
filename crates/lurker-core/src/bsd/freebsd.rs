//! FreeBSD's native file-backed GELI adapter.
//!
//! The shared BSD lifecycle owns staging, publication, mount identity and
//! recovery. This module only operates on md devices returned by mdconfig;
//! it deliberately has no physical-device, forced-detach or boot integration.
use super::{Credential, args, exec};
use crate::{AppError, AppResult, CreateCipher};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

const MDCONFIG: &str = "/sbin/mdconfig";
const GELI: &str = "/sbin/geli";
const NEWFS: &str = "/sbin/newfs";

/// Unlike cryptsetup, GELI names the AES component key size here: AES-XTS
/// accepts 128 or 256, not cryptsetup's combined XTS key size of 512.
const FORMAT_ARGS: &[&str] = &[
    "init", "-B", "none", "-e", "AES-XTS", "-l", "256", "-s", "4096",
];
const PASSPHRASE_ITERATIONS: &str = "600000";
const BACKING_SECTORS: &[u32] = &[512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];

/// Inspect at most 64 KiB without attaching a device. GELI metadata starts at
/// the final *provider* sector, which differs from its encrypted sector size.
/// This identifies the format and geometry only; native GELI must still verify
/// the metadata checksum and authenticate the credential before exposing data.
/// Layout: https://github.com/freebsd/freebsd-src/blob/releng/15.1/sys/geom/eli/g_eli.h
pub(super) fn probe_image(path: &Path) -> AppResult<Option<u32>> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| AppError::io("Failed to inspect image format", e))?;
    let metadata = file
        .metadata()
        .map_err(|e| AppError::io("Failed to inspect image metadata", e))?;
    if !metadata.is_file() {
        return Err(AppError::new(
            "GELI image detection requires a regular file.",
        ));
    }
    let length = metadata.len();
    let count = length.min(65536) as usize;
    let mut tail = vec![0; count];
    file.seek(SeekFrom::Start(length - count as u64))
        .and_then(|_| file.read_exact(&mut tail))
        .map_err(|e| AppError::io("Failed to read image metadata", e))?;
    probe_tail(&tail, length)
}

fn probe_tail(tail: &[u8], image_length: u64) -> AppResult<Option<u32>> {
    if image_length < 512 {
        return if tail.starts_with(b"GEOM::ELI\0") {
            Err(AppError::new("Truncated GELI image metadata."))
        } else {
            Ok(None)
        };
    }
    let mut matched = None;
    for &sector in BACKING_SECTORS {
        let offset = sector as usize;
        if offset > tail.len() {
            continue;
        }
        let header = &tail[tail.len() - offset..];
        if !header.starts_with(b"GEOM::ELI\0") {
            continue;
        }
        validate_metadata(header, image_length, sector)?;
        if matched.replace(sector).is_some() {
            return Err(AppError::new(
                "Ambiguous GELI backing-sector metadata; specify a qualified image instead.",
            ));
        }
    }
    Ok(matched)
}

fn validate_metadata(header: &[u8], image_length: u64, backing_sector: u32) -> AppResult<()> {
    fn invalid() -> AppError {
        AppError::new("Invalid or unsupported GELI image metadata.")
    }
    if header.len() < 20 || &header[..16] != b"GEOM::ELI\0\0\0\0\0\0\0" {
        return Err(invalid());
    }
    let version = u32::from_le_bytes(header[16..20].try_into().unwrap());
    if version > 7 {
        return Err(invalid());
    }
    // Version 0 predates md_aalgo; versions 1 through 7 share one disk layout.
    let shift = if version == 0 { 0 } else { 2 };
    if header.len() < 509 + shift {
        return Err(invalid());
    }
    let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());
    let algorithm = u16::from_le_bytes(header[24..26].try_into().unwrap());
    let key_bits = u16::from_le_bytes(header[26..28].try_into().unwrap());
    let provider = u64::from_le_bytes(header[28 + shift..36 + shift].try_into().unwrap());
    let encrypted_sector = u32::from_le_bytes(header[36 + shift..40 + shift].try_into().unwrap());
    let keys = header[40 + shift];
    let iterations = i32::from_le_bytes(header[41 + shift..45 + shift].try_into().unwrap());
    // Values are the stable on-disk opencrypto identifiers, not CLI options.
    let encryption_valid = match algorithm {
        11 | 21 => matches!(key_bits, 128 | 192 | 256), // AES/Camellia CBC
        22 => version >= 5 && matches!(key_bits, 128 | 256), // AES-XTS
        _ => false, // In particular, never treat NULL encryption as supported.
    };
    if flags & !0x3ff != 0
        || flags & 1 != 0
        || !encryption_valid
        || provider != image_length
        || image_length % backing_sector as u64 != 0
        || !encrypted_sector.is_power_of_two()
        || encrypted_sector < backing_sector
        || encrypted_sector > 1024 * 1024
        || image_length <= encrypted_sector as u64 + backing_sector as u64
        || keys == 0
        || keys & !3 != 0
        || iterations < -1
    {
        return Err(invalid());
    }
    if flags & 0x10 != 0 {
        if version == 0 {
            return Err(invalid());
        }
        let authentication = u16::from_le_bytes(header[28..30].try_into().unwrap());
        if !matches!(authentication, 7 | 8 | 18 | 19 | 20) {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn attach_file(path: &Path, readonly: bool) -> AppResult<String> {
    attach_file_with_sector(path, readonly, 512)
}

pub(super) fn attach_file_with_sector(
    path: &Path,
    readonly: bool,
    sector: u32,
) -> AppResult<String> {
    if !BACKING_SECTORS.contains(&sector) {
        return Err(AppError::new(
            "Unsupported FreeBSD image backing-sector size.",
        ));
    }
    let mut argv = args(&["-a", "-t", "vnode", "-S", &sector.to_string()]);
    if readonly {
        argv.extend(args(&["-o", "readonly"]));
    }
    argv.push("-f".into());
    argv.push(path.as_os_str().to_owned());
    let value = exec(MDCONFIG, &argv, None)?;
    let name = value.trim();
    md_unit(name)?;
    Ok(format!("/dev/{name}"))
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let unit = md_unit(device)?;
    exec(MDCONFIG, &args(&["-d", "-u", unit]), None)?;
    Ok(())
}

/// md unit numbers may be reused after another process detaches a device.
/// Match the kernel's backing vnode identity, not GEOM's printed filename:
/// libgeom's XML conversion can corrupt Unicode/spaces in that display field.
pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let unit = md_unit(device)?;
    let metadata = fs::metadata(path)
        .map_err(|e| AppError::io("Failed to inspect the recorded md backing file", e))?;
    let listing = exec("/sbin/geom", &args(&["md", "list", unit]), None)?;
    if !metadata.is_file() || !backing_matches(&listing, unit, metadata.dev(), metadata.ino()) {
        return Err(AppError::new(
            "The FreeBSD md backing file changed; refusing cleanup.",
        ));
    }
    Ok(())
}

fn backing_matches(listing: &str, unit: &str, device: u64, inode: u64) -> bool {
    let geom = format!("Geom name: {unit}");
    let provider = format!("1. Name: {unit}");
    let ident = format!("ident: MD-DEV{device}-INO{inode}");
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    lines
        .iter()
        .filter(|line| line.starts_with("Geom name:"))
        .count()
        == 1
        && lines.contains(&geom.as_str())
        && lines
            .iter()
            .filter(|line| line.starts_with("1. Name:"))
            .count()
            == 1
        && lines.contains(&provider.as_str())
        && lines
            .iter()
            .filter(|line| line.starts_with("type:"))
            .count()
            == 1
        && lines.contains(&"type: vnode")
        && lines
            .iter()
            .filter(|line| line.starts_with("ident:"))
            .count()
            == 1
        && lines.contains(&ident.as_str())
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    let unit = md_unit(backing)?;
    if mapped != format!("/dev/{unit}.eli") {
        return Err(AppError::new(
            "The GELI provider does not match the recorded md device.",
        ));
    }
    let listing = exec(GELI, &args(&["list", &format!("{unit}.eli")]), None)?;
    if !mapping_matches(&listing, unit) {
        return Err(AppError::new("The GELI mapping changed; refusing cleanup."));
    }
    Ok(())
}

fn mapping_matches(listing: &str, unit: &str) -> bool {
    let geom = format!("Geom name: {unit}.eli");
    let consumer = format!("1. Name: {unit}");
    let Some((header, consumers)) = listing.split_once("Consumers:\n") else {
        return false;
    };
    header.lines().any(|line| line == geom)
        && header.lines().any(|line| line == "State: ACTIVE")
        && consumers
            .lines()
            .next()
            .is_some_and(|line| line == consumer)
}

pub(super) fn format(device: &str, cipher: CreateCipher, credential: &Credential) -> AppResult<()> {
    md_unit(device)?;
    if cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "FreeBSD GELI uses the AES-XTS profile; Serpent and Twofish are unavailable.",
        ));
    }
    let mut argv = args(FORMAT_ARGS);
    let input = match credential {
        Credential::Password(password) => {
            argv.extend(args(&["-i", PASSPHRASE_ITERATIONS, "-J", "-"]));
            Some(password_line(password))
        }
        Credential::Keyfile(path) => {
            argv.extend(args(&["-P", "-K"]));
            argv.push(path.as_os_str().to_owned());
            None
        }
    };
    argv.push(device.into());
    exec(GELI, &argv, input.as_deref())?;
    Ok(())
}

pub(super) fn unlock(device: &str, readonly: bool, credential: &Credential) -> AppResult<String> {
    md_unit(device)?;
    let mut argv = args(&["attach"]);
    if readonly {
        argv.push("-r".into());
    }
    let input = match credential {
        Credential::Password(password) => {
            argv.extend(args(&["-j", "-"]));
            Some(password_line(password))
        }
        Credential::Keyfile(path) => {
            argv.extend(args(&["-p", "-k"]));
            argv.push(path.as_os_str().to_owned());
            None
        }
    };
    argv.push(device.into());
    exec(GELI, &argv, input.as_deref())?;
    Ok(format!("{device}.eli"))
}

pub(super) fn close(device: &str) -> AppResult<()> {
    let backing = device
        .strip_suffix(".eli")
        .ok_or_else(|| AppError::new("Expected a GELI md provider."))?;
    md_unit(backing)?;
    exec(GELI, &args(&["detach", backing]), None)?;
    Ok(())
}

pub(super) fn make_filesystem(device: &str) -> AppResult<()> {
    let backing = device
        .strip_suffix(".eli")
        .ok_or_else(|| AppError::new("Expected a GELI md provider."))?;
    md_unit(backing)?;
    exec(NEWFS, &args(&["-U", device]), None)?;
    Ok(())
}

fn md_unit(value: &str) -> AppResult<&str> {
    let name = value.strip_prefix("/dev/").unwrap_or(value);
    if name
        .strip_prefix("md")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        Ok(name)
    } else {
        Err(AppError::new("Expected a file-backed FreeBSD md device."))
    }
}

fn password_line(password: &[u8]) -> Vec<u8> {
    let mut input = password.to_vec();
    input.push(b'\n');
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_tail(backing_sector: u32, logical_sector: u32, length: u64) -> Vec<u8> {
        let mut tail = vec![0; 65536];
        let offset = tail.len() - backing_sector as usize;
        let header = &mut tail[offset..];
        header[..16].copy_from_slice(b"GEOM::ELI\0\0\0\0\0\0\0");
        header[16..20].copy_from_slice(&7u32.to_le_bytes());
        header[20..24].copy_from_slice(&0x200u32.to_le_bytes());
        header[24..26].copy_from_slice(&22u16.to_le_bytes());
        header[26..28].copy_from_slice(&256u16.to_le_bytes());
        header[30..38].copy_from_slice(&length.to_le_bytes());
        header[38..42].copy_from_slice(&logical_sector.to_le_bytes());
        header[42] = 1;
        header[43..47].copy_from_slice(&600000i32.to_le_bytes());
        tail
    }

    #[test]
    fn detects_backing_geometry_without_confusing_encrypted_sector_size() {
        let length = 128 * 1024 * 1024;
        assert_eq!(
            probe_tail(&metadata_tail(512, 4096, length), length).unwrap(),
            Some(512)
        );
        assert_eq!(
            probe_tail(&metadata_tail(4096, 4096, length), length).unwrap(),
            Some(4096)
        );
        assert_eq!(probe_tail(&[0; 65536], length).unwrap(), None);
        assert_eq!(probe_tail(&[], 0).unwrap(), None);
    }

    #[test]
    fn rejects_truncated_invalid_and_ambiguous_metadata() {
        let length = 128 * 1024 * 1024;
        assert!(probe_tail(b"GEOM::ELI\0", 10).is_err());
        let tail = metadata_tail(512, 4096, length);
        assert!(validate_metadata(&tail[65536 - 512..65536 - 12], length, 512).is_err());
        assert!(probe_tail(&tail, length + 512).is_err());
        assert!(probe_tail(&metadata_tail(4096, 512, length), length).is_err());
        let mut unsupported = tail.clone();
        unsupported[65536 - 512 + 16..65536 - 512 + 20].copy_from_slice(&8u32.to_le_bytes());
        assert!(probe_tail(&unsupported, length).is_err());
        let mut null_encryption = tail.clone();
        null_encryption[65536 - 512 + 24..65536 - 512 + 26].copy_from_slice(&16u16.to_le_bytes());
        assert!(probe_tail(&null_encryption, length).is_err());
        let mut ambiguous = metadata_tail(4096, 4096, length);
        ambiguous[65536 - 512..].copy_from_slice(&tail[65536 - 512..]);
        assert!(probe_tail(&ambiguous, length).is_err());
    }

    #[test]
    fn image_probe_is_bounded_and_does_not_open_devices_as_images() {
        use std::io::Write;
        let root = std::env::temp_dir().join(format!("lurker-geli-probe-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("vault 密碼 with spaces");
        let mut file = fs::File::create(&path).unwrap();
        let length = 1024 * 1024 * 1024u64;
        file.set_len(length).unwrap();
        file.seek(SeekFrom::End(-65536)).unwrap();
        file.write_all(&metadata_tail(4096, 4096, length)).unwrap();
        drop(file);
        assert_eq!(probe_image(&path).unwrap(), Some(4096));
        assert!(probe_image(Path::new("/dev/null")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_md_device_names_reach_native_tools() {
        for value in ["md0", "/dev/md123"] {
            assert!(md_unit(value).is_ok());
        }
        for value in [
            "md",
            "ada0",
            "/dev/ada0",
            "md0.eli",
            "md0\nmd1",
            "md0/../ada0",
        ] {
            assert!(md_unit(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn password_transport_preserves_spaces_and_uses_one_line() {
        assert_eq!(password_line(b"  password  "), b"  password  \n");
    }

    #[test]
    fn backing_identity_uses_vnode_numbers_and_rejects_reused_or_ambiguous_devices() {
        let listing = "Geom name: md42\nProviders:\n1. Name: md42\n file: garbled Unicode display\n type: vnode\n ident: MD-DEV64-INO240775\n";
        assert!(backing_matches(listing, "md42", 64, 240775));
        assert!(!backing_matches(listing, "md42", 64, 240776));
        assert!(!backing_matches(listing, "md42", 65, 240775));
        assert!(!backing_matches(listing, "md4", 64, 240775));
        assert!(!backing_matches(
            &listing.replace("vnode", "swap"),
            "md42",
            64,
            240775
        ));
        assert!(!backing_matches(
            &format!("{listing} ident: MD-DEV64-INO240775\n"),
            "md42",
            64,
            240775
        ));
    }

    #[test]
    fn mapping_identity_requires_active_geom_and_matching_consumer() {
        let value = "Geom name: md42.eli\nState: ACTIVE\nProviders:\n1. Name: md42.eli\nConsumers:\n1. Name: md42\n";
        assert!(mapping_matches(value, "md42"));
        assert!(!mapping_matches(value, "md4"));
        assert!(!mapping_matches(
            &value.replace("ACTIVE", "SUSPENDED"),
            "md42"
        ));
        assert!(!mapping_matches(
            &value.replace("1. Name: md42\n", "1. Name: md43\n"),
            "md42"
        ));
    }
}
