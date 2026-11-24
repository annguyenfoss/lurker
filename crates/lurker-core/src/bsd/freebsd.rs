//! FreeBSD's native file-backed GELI adapter.
//!
//! The shared BSD lifecycle owns staging, publication, mount identity and
//! recovery. This module only operates on md devices returned by mdconfig;
//! it deliberately has no physical-device, forced-detach or boot integration.
use super::{Credential, Registry, Session, args, exec};
use crate::{AppError, AppResult, CreateCipher, VolumeType};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;
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

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DetachIntent {
    Geli,
    Md,
}

/// Reserve storage before the kernel creates a new GELI provider.
pub(super) fn allocate_image(file: &fs::File, size: u64) -> AppResult<()> {
    let length = libc::off_t::try_from(size)
        .map_err(|_| AppError::new("GELI container size is too large."))?;
    loop {
        // SAFETY: The staging descriptor remains open and writable throughout.
        let error = unsafe { libc::posix_fallocate(file.as_raw_fd(), 0, length) };
        match error {
            0 => {
                return file
                    .sync_all()
                    .map_err(|e| AppError::io("Sync allocated GELI backing storage", e));
            }
            libc::EINTR => continue,
            // FreeBSD used EINVAL for unsupported file systems before 15.1.
            libc::EINVAL | libc::EOPNOTSUPP | libc::ENODEV => return Ok(()),
            _ => {
                return Err(AppError::io(
                    "Allocate GELI backing storage",
                    std::io::Error::from_raw_os_error(error),
                ));
            }
        }
    }
}

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
    md_unit(device)?;
    let metadata = fs::metadata(path)
        .map_err(|e| AppError::io("Failed to inspect the recorded md backing file", e))?;
    if !metadata.is_file() || backing_identity(device)? != Some((metadata.dev(), metadata.ino())) {
        return Err(AppError::new(
            "The FreeBSD md backing file changed; refusing cleanup.",
        ));
    }
    Ok(())
}

/// Query a vnode-backed md device without attaching, detaching or reading its
/// data. None means a successful native unit snapshot proved this unit absent;
/// query failures and unrecognized providers are errors, never absence proofs.
pub(super) fn backing_identity(device: &str) -> AppResult<Option<(u64, u64)>> {
    let unit = md_unit(device)?;
    let devices = exec(MDCONFIG, &args(&["-l"]), None)?;
    if !listed_unit(&devices, unit)? {
        return Ok(None);
    }
    let listing = exec("/sbin/geom", &args(&["md", "list", unit]), None)?;
    parse_backing_identity(&listing, unit).map(Some)
}

fn listed_unit(listing: &str, unit: &str) -> AppResult<bool> {
    let mut seen = std::collections::HashSet::new();
    for device in listing.split_whitespace() {
        if device.starts_with("/dev/") || md_unit(device)? != device || !seen.insert(device) {
            return Err(AppError::new("Ambiguous FreeBSD md unit listing."));
        }
    }
    Ok(seen.contains(unit))
}

fn parse_backing_identity(listing: &str, unit: &str) -> AppResult<(u64, u64)> {
    fn invalid() -> AppError {
        AppError::new("Unrecognized or ambiguous FreeBSD md backing identity.")
    }
    let geom = format!("Geom name: {unit}");
    let provider = format!("1. Name: {unit}");
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let valid = lines
        .iter()
        .filter(|line| line.starts_with("Geom name:"))
        .count()
        == 1
        && lines.contains(&geom.as_str())
        && lines
            .iter()
            .filter(|line| {
                line.split_once(". Name:")
                    .is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
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
        && lines.iter().filter(|line| **line == "Providers:").count() == 1;
    if !valid {
        return Err(invalid());
    }
    let ident = lines
        .iter()
        .find_map(|line| line.strip_prefix("ident: MD-DEV"))
        .ok_or_else(invalid)?;
    let (device, inode) = ident.split_once("-INO").ok_or_else(invalid)?;
    let device = device.parse::<u64>().map_err(|_| invalid())?;
    let inode = inode.parse::<u64>().map_err(|_| invalid())?;
    if ident != format!("{device}-INO{inode}") {
        return Err(invalid());
    }
    Ok((device, inode))
}

#[cfg(test)]
fn backing_matches(listing: &str, unit: &str, device: u64, inode: u64) -> bool {
    parse_backing_identity(listing, unit).ok() == Some((device, inode))
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    let unit = mapping_unit(mapped, backing)?;
    let listing = exec(GELI, &args(&["list", &format!("{unit}.eli")]), None)?;
    if !mapping_matches(&listing, unit) {
        return Err(AppError::new("The GELI mapping changed; refusing cleanup."));
    }
    Ok(())
}

fn mapping_unit<'a>(mapped: &str, backing: &'a str) -> AppResult<&'a str> {
    let unit = md_unit(backing)?;
    if mapped != format!("/dev/{unit}.eli") {
        return Err(AppError::new(
            "The GELI provider does not match the recorded md device.",
        ));
    }
    Ok(unit)
}

fn mapping_is_absent(mapped: &str, backing: &str) -> AppResult<bool> {
    let provider = format!("{}.eli", mapping_unit(mapped, backing)?);
    let listing = exec("/sbin/geom", &args(&["eli", "status", "-s"]), None)?;
    // A capped subprocess transcript cannot prove a provider absent.
    if listing.len() >= 1024 * 1024 {
        return Err(AppError::new(
            "FreeBSD GELI inventory is too large; preserving the journal.",
        ));
    }
    let mut found = false;
    for line in listing.trim().lines() {
        let row = line.split_whitespace().collect::<Vec<_>>();
        if row.len() != 3 || !row[0].ends_with(".eli") {
            return Err(AppError::new(
                "Unrecognized FreeBSD GELI inventory; preserving the journal.",
            ));
        }
        found |= row[0] == provider;
    }
    Ok(!found)
}

pub(super) fn reconcile_detach(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(intent) = session.freebsd_detach else {
        return Ok(());
    };
    let conflict = || {
        AppError::new("FreeBSD detach intent conflicts with its session; preserving the journal.")
    };
    if session.backend != VolumeType::Geli
        || session.mounted.is_some()
        || !session.identity.matches(&session.source)
    {
        return Err(conflict());
    }
    let backing = session.backing.as_deref().ok_or_else(conflict)?;
    match intent {
        DetachIntent::Geli => {
            let mapped = session.mapped.as_deref().ok_or_else(conflict)?;
            verify_backing(backing, &session.source)?;
            if !mapping_is_absent(mapped, backing)? {
                return Ok(());
            }
            if super::mounts()?
                .iter()
                .any(|mount| mount.source == Path::new(mapped))
            {
                return Err(conflict());
            }
            session.mapped = None;
        }
        DetachIntent::Md => {
            if session.mapped.is_some() {
                return Err(conflict());
            }
            if backing_identity(backing)?.is_some() {
                return Ok(());
            }
            session.backing = None;
        }
    }
    session.freebsd_detach = None;
    registry.save(session)
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
    if name.strip_prefix("md").is_some_and(|n| {
        n.parse::<u32>()
            .ok()
            .is_some_and(|value| value <= i32::MAX as u32 && n == value.to_string())
    }) {
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

    #[test]
    #[ignore = "requires root and real disposable FreeBSD GELI and md attachments"]
    fn cleanup_reconciles_completed_native_detaches_with_saved_intent() {
        use super::super::{Identity, cleanup, require_root, unique_id};
        use std::os::unix::fs::DirBuilderExt;

        require_root().unwrap();
        let work = std::env::temp_dir().join(format!("lurker-freebsd-detach-{}", unique_id()));
        fs::DirBuilder::new().mode(0o700).create(&work).unwrap();
        let source = work.join("owned.img");
        fs::File::create(&source)
            .unwrap()
            .set_len(128 * 1024 * 1024)
            .unwrap();
        let keyfile = work.join("owned.key");
        fs::write(&keyfile, [0xa3; 64]).unwrap();
        let credential = Credential::Keyfile(keyfile);
        let registry = Registry {
            directory: work.clone(),
            _lock: fs::File::create(work.join("lock")).unwrap(),
        };
        let backing = attach_file(&source, false).unwrap();
        format(&backing, CreateCipher::Aes, &credential).unwrap();
        let mapped = unlock(&backing, false, &credential).unwrap();
        let mut session = Session {
            version: 1,
            id: unique_id(),
            backend: VolumeType::Geli,
            identity: Identity::read(&source).unwrap(),
            source: source.clone(),
            create_target: None,
            peer_sources: Vec::new(),
            backing: Some(backing.clone()),
            freebsd_detach: Some(DetachIntent::Geli),
            mapped: Some(mapped.clone()),
            mounted: None,
            intended_mountpoint: None,
            initialization_directory: None,
            readonly: false,
            veracrypt_proof: None,
        };
        registry.save(&session).unwrap();
        reconcile_detach(&registry, &mut session).unwrap();
        assert_eq!(session.mapped.as_deref(), Some(mapped.as_str()));
        verify_mapping(&mapped, &backing).unwrap();
        close(&mapped).unwrap();

        let mut saved = registry.records().unwrap().pop().unwrap();
        let mut without_intent = saved.clone();
        without_intent.freebsd_detach = None;
        assert!(cleanup(&registry, &mut without_intent).is_err());
        reconcile_detach(&registry, &mut saved).unwrap();
        assert!(saved.mapped.is_none() && saved.freebsd_detach.is_none());
        verify_backing(&backing, &source).unwrap();

        saved.freebsd_detach = Some(DetachIntent::Md);
        registry.save(&saved).unwrap();
        reconcile_detach(&registry, &mut saved).unwrap();
        assert_eq!(saved.backing.as_deref(), Some(backing.as_str()));
        detach_file(&backing).unwrap();

        let mut saved = registry.records().unwrap().pop().unwrap();
        let mut without_intent = saved.clone();
        without_intent.freebsd_detach = None;
        assert!(cleanup(&registry, &mut without_intent).is_err());

        let replacement = work.join("replacement.img");
        fs::File::create(&replacement)
            .unwrap()
            .set_len(128 * 1024 * 1024)
            .unwrap();
        let mut argv = args(&["-a", "-t", "vnode", "-u", md_unit(&backing).unwrap(), "-f"]);
        argv.push(replacement.as_os_str().to_owned());
        exec(MDCONFIG, &argv, None).unwrap();
        let refused = cleanup(&registry, &mut saved).is_err();
        verify_backing(&backing, &replacement).unwrap();
        detach_file(&backing).unwrap();
        assert!(refused);

        cleanup(&registry, &mut saved).unwrap();
        assert!(registry.records().unwrap().is_empty());
        assert!(saved.identity.matches(&source) && replacement.is_file());
        fs::remove_dir_all(work).unwrap();
    }

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
            "md00",
            "md+1",
            "md2147483648",
        ] {
            assert!(md_unit(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn absence_requires_a_complete_unambiguous_unit_snapshot() {
        assert!(!listed_unit("", "md42").unwrap());
        assert!(!listed_unit("md0 md1\n", "md42").unwrap());
        assert!(listed_unit("md0 md42\n", "md42").unwrap());
        for listing in ["md0 md0", "md000", "md0 unknown", "/dev/md0"] {
            assert!(listed_unit(listing, "md42").is_err(), "accepted {listing}");
        }
    }

    #[test]
    fn backing_parser_requires_canonical_vnode_identifiers() {
        let listing = "Geom name: md42\nProviders:\n1. Name: md42\n type: vnode\n ident: MD-DEV64-INO240775\n";
        assert_eq!(
            parse_backing_identity(listing, "md42").unwrap(),
            (64, 240775)
        );
        for invalid in [
            listing.replace("MD-DEV64", "MD-DEV064"),
            listing.replace("INO240775", "INO+240775"),
            listing.replace("INO240775", "INO18446744073709551616"),
            listing.replace("vnode", "swap"),
            listing.replace("ident: MD-DEV64-INO240775", "ident: UNKNOWN"),
            format!("{listing}2. Name: md43\n"),
        ] {
            assert!(
                parse_backing_identity(&invalid, "md42").is_err(),
                "accepted {invalid}"
            );
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

mod filesystems;
pub(super) fn validate_filesystem(
    selected: Option<crate::CreateFilesystem>,
    backend: VolumeType,
) -> AppResult<()> {
    filesystems::validate(selected, backend)
}
pub(super) fn make_selected_filesystem(
    mapped: &str,
    selected: Option<crate::CreateFilesystem>,
) -> AppResult<()> {
    filesystems::format(mapped, selected)
}
pub(super) fn mount_filesystem(mapped: &str, target: &Path, readonly: bool) -> AppResult<()> {
    filesystems::mount(mapped, target, readonly)
}
pub(super) fn matches_filesystem_mount(mapped: &str, mounted: &super::Mounted) -> bool {
    filesystems::matches_mount(mapped, mounted)
}
pub(super) fn filesystem_has_posix_ownership(mapped: &str) -> AppResult<bool> {
    filesystems::posix(mapped)
}
pub(super) fn close_filesystem(mapped: &str) -> AppResult<()> {
    filesystems::close(mapped)
}
