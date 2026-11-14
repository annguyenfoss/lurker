//! OpenBSD native RAID1C: two encrypted mirror members, no stacked disciplines.
//!
//! The shared lifecycle owns files, publication, journal and per-member vnode
//! identities. This adapter never creates missing members during unlock,
//! adopts an existing attachment, rebuilds peers or forces a detach.
use super::{args, exec, native, Credential};
use crate::{AppError, AppResult};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Header {
    pub uuid: String,
    pub index: u32,
    pub members: u32,
    pub level: u32,
}

fn name<'a>(device: &'a str, prefix: &str) -> AppResult<&'a str> {
    let value = device.strip_prefix("/dev/").unwrap_or(device);
    let value = value.strip_prefix('r').unwrap_or(value);
    let value = value
        .strip_suffix('a')
        .or_else(|| value.strip_suffix('c'))
        .unwrap_or(value);
    if value
        .strip_prefix(prefix)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        Ok(value)
    } else {
        Err(AppError::new("Unexpected RAID1C native device identity."))
    }
}

fn password(credential: &Credential) -> AppResult<Vec<u8>> {
    match credential {
        Credential::Password(bytes)
            if !bytes.is_empty() && !bytes.iter().any(|b| matches!(b, 0 | 10 | 13)) =>
        {
            let mut input = bytes.clone();
            input.push(b'\n');
            Ok(input)
        }
        Credential::Password(_) => Err(AppError::new(
            "RAID1C needs a non-empty password without NUL or line breaks.",
        )),
        Credential::Keyfile(_) => Err(AppError::new(
            "RAID1C accepts a password; native keydisks are not regular keyfiles.",
        )),
    }
}

/// Reject an already attached source even when it has no Lurker journal entry.
/// Enumerated display paths are not authority: direct VNDIOCGET dev/inode
/// identities also catch symlinks, hard links, rename and raw foreign vnodes.
pub(super) fn ensure_unattached(paths: &[PathBuf]) -> AppResult<()> {
    if paths.is_empty() || paths.len() > 2 {
        return Err(AppError::new(
            "RAID1C requires one or two existing member files.",
        ));
    }
    let mut identities = BTreeSet::new();
    for path in paths {
        let metadata = fs::metadata(path).map_err(|e| AppError::io("Inspect RAID1C member", e))?;
        if !metadata.is_file() || !identities.insert((metadata.dev(), metadata.ino())) {
            return Err(AppError::new(
                "RAID1C members must be distinct regular files, including through aliases.",
            ));
        }
    }
    let listing = exec("/sbin/vnconfig", &args(&["-l"]), None)?;
    for line in listing.lines() {
        let unit = line.split_once(':').map(|(unit, _)| unit).ok_or_else(|| {
            AppError::new("Unrecognized native vnode inventory; refusing acquisition.")
        })?;
        let unit = name(unit, "vnd")?;
        if native::vnode_identity(unit)?.is_some_and(|identity| identities.contains(&identity)) {
            return Err(AppError::new("A RAID1C member is already attached to a native vnode. Close its owning session before retrying; foreign attachments are never adopted."));
        }
    }
    Ok(())
}

pub(super) fn probe(backing: &str) -> AppResult<Header> {
    let unit = name(backing, "vnd")?;
    let mut file = File::open(format!("/dev/r{unit}a"))
        .map_err(|e| AppError::io("Read native RAID1C member metadata", e))?;
    file.seek(SeekFrom::Start(8192))
        .map_err(|e| AppError::io("Seek native RAID1C metadata", e))?;
    let mut bytes = [0u8; 56];
    file.read_exact(&mut bytes)
        .map_err(|e| AppError::io("Read native RAID1C metadata", e))?;
    parse_header(&bytes)
}

fn parse_header(bytes: &[u8]) -> AppResult<Header> {
    let bad = || {
        AppError::new(
            "Unrecognized native softraid metadata; refusing initialization during unlock.",
        )
    };
    if bytes.len() < 56 {
        return Err(bad());
    }
    let number = |offset| u32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
    if u64::from_ne_bytes(bytes[..8].try_into().unwrap()) != 0x4d4152436372616d
        || number(8) != 6
        || number(44) != 512
    {
        return Err(bad());
    }
    let members = number(32);
    let index = number(36);
    let level = number(52);
    if !((level == 0x1c && members == 2 && index < 2)
        || (level == u32::from(b'C') && members == 1 && index == 0))
        || bytes[16..32].iter().all(|b| *b == 0)
    {
        return Err(bad());
    }
    let uuid = bytes[16..32]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(Header {
        uuid,
        index,
        members,
        level,
    })
}

fn backing_names(backings: &[String]) -> AppResult<Vec<String>> {
    if backings.is_empty() || backings.len() > 2 {
        return Err(AppError::new("RAID1C accepts exactly two members, or one existing member with explicit degraded recovery."));
    }
    let names: Vec<String> = backings
        .iter()
        .map(|b| name(b, "vnd").map(str::to_owned))
        .collect::<AppResult<_>>()?;
    let mut devices = BTreeSet::new();
    let mut objects = BTreeSet::new();
    for unit in &names {
        let identity = native::vnode_identity(unit)?
            .ok_or_else(|| AppError::new("RAID1C vnode backing is no longer attached."))?;
        if !devices.insert(unit) || !objects.insert(identity) {
            return Err(AppError::new(
                "RAID1C cannot use the same backing member twice.",
            ));
        }
    }
    Ok(names)
}

fn headers(
    backings: &[String],
    expected_uuid: &str,
    allow_degraded: bool,
) -> AppResult<Vec<Header>> {
    if expected_uuid.len() != 32
        || !expected_uuid
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(AppError::new(
            "Invalid RAID1C array UUID in recovery metadata.",
        ));
    }
    if backings.len() != 2 && !(allow_degraded && backings.len() == 1) {
        return Err(AppError::new(
            "Both RAID1C members are required unless degraded recovery was explicitly requested.",
        ));
    }
    let values = backings
        .iter()
        .map(|b| probe(b))
        .collect::<AppResult<Vec<_>>>()?;
    validate_headers(&values, expected_uuid)?;
    Ok(values)
}

fn validate_headers(values: &[Header], expected_uuid: &str) -> AppResult<()> {
    let mut indices = BTreeSet::new();
    if values.is_empty()
        || values.len() > 2
        || values.iter().any(|h| {
            h.level != 0x1c
                || h.members != 2
                || h.index > 1
                || h.uuid != expected_uuid
                || !indices.insert(h.index)
        })
    {
        return Err(AppError::new(
            "RAID1C member UUID, index or array profile does not match its peer and descriptor.",
        ));
    }
    Ok(())
}

fn ensure_backings_unassembled(names: &[String]) -> AppResult<()> {
    let active = exec("/sbin/bioctl", &args(&["softraid0"]), None)?;
    if names.iter().any(|n| active.contains(&format!("<{n}a>"))) {
        return Err(AppError::new(
            "A RAID1C backing already belongs to an active array; refusing duplicate acquisition.",
        ));
    }
    Ok(())
}

fn assemble(names: &[String], credential: &Credential, creating: bool) -> AppResult<String> {
    let chunks = names
        .iter()
        .map(|unit| format!("/dev/{unit}a"))
        .collect::<Vec<_>>()
        .join(",");
    let mut argv = args(&["-s", "-C", "noauto", "-c", "1C"]);
    if creating {
        argv.extend(args(&["-r", "auto"]));
    }
    argv.extend(args(&["-l", &chunks, "softraid0"]));
    let mut input = password(credential)?;
    let result = exec("/sbin/bioctl", &argv, Some(&input));
    input.fill(0);
    let output = result?;
    let unit = parse_attached(&output)?;
    // Return immediately so the caller can journal this acquisition before any
    // subsequent identity probe, filesystem operation or cleanup can fail.
    Ok(format!("/dev/{unit}a"))
}

fn parse_attached(output: &str) -> AppResult<&str> {
    let unit = output.trim().strip_prefix("softraid0: RAID 1C volume attached as ")
        .ok_or_else(|| AppError::new("RAID1C attached but its device identity is unknown; preserve the journal for recovery."))?;
    name(unit, "sd")
}

/// Only newly staged, identity-recorded files may reach format. The returned
/// mapping stays attached: the shared lifecycle must journal it before verify
/// and mkfs. Initialization uses native AES-XTS and calibrated bcrypt_pbkdf.
pub(super) fn format(backings: &[String], credential: &Credential) -> AppResult<String> {
    if backings.len() != 2 {
        return Err(AppError::new(
            "New RAID1C arrays require exactly two independent members.",
        ));
    }
    let mut checked = password(credential)?;
    checked.fill(0);
    let names = backing_names(backings)?;
    ensure_backings_unassembled(&names)?;
    for unit in &names {
        exec(
            "/sbin/disklabel",
            &args(&["-wAT-", unit]),
            Some(b"RAID *\n"),
        )?;
    }
    assemble(&names, credential, true)
}

pub(super) fn unlock(
    backings: &[String],
    credential: &Credential,
    expected_uuid: &str,
    allow_degraded: bool,
) -> AppResult<String> {
    let names = backing_names(backings)?;
    headers(backings, expected_uuid, allow_degraded)?;
    ensure_backings_unassembled(&names)?;
    // Never call disklabel or pass a force flag here. Recognizable metadata
    // with a bad checksum is an assembly error in the native kernel, not create.
    assemble(&names, credential, false)
}

pub(super) fn verify_mapping(
    mapped: &str,
    backings: &[String],
    expected_uuid: &str,
    allow_degraded: bool,
) -> AppResult<()> {
    let names = backing_names(backings)?;
    let values = headers(backings, expected_uuid, allow_degraded)?;
    let expected = names
        .iter()
        .zip(&values)
        .map(|(name, h)| (h.index, name.as_str()))
        .collect();
    let unit = name(mapped, "sd")?;
    let listing = exec("/sbin/bioctl", &args(&[unit]), None)?;
    verify_listing(&listing, unit, &expected, allow_degraded)
}

fn verify_listing(
    listing: &str,
    mapped: &str,
    expected: &BTreeMap<u32, &str>,
    allow_degraded: bool,
) -> AppResult<()> {
    let bad = || {
        AppError::new(
            "RAID1C native mapping no longer matches its recorded members, array or health state.",
        )
    };
    let mut volume = None;
    let mut rows = BTreeMap::new();
    for line in listing.lines().filter(|line| !line.trim().is_empty()) {
        let row: Vec<_> = line.split_whitespace().collect();
        if row.first() == Some(&"Volume") {
            continue;
        }
        if row.first() == Some(&"softraid0") {
            if volume.is_some() || row.len() != 6 || row[4] != mapped || row[5] != "RAID1C" {
                return Err(bad());
            }
            volume = Some(row[2]);
            continue;
        }
        if row.len() < 6 {
            return Err(bad());
        }
        let index = row[0].parse::<u32>().map_err(|_| bad())?;
        let chunk = row
            .last()
            .unwrap()
            .strip_prefix('<')
            .and_then(|n| n.strip_suffix('>'))
            .ok_or_else(bad)?;
        if index > 1 || rows.insert(index, (row[1], chunk)).is_some() {
            return Err(bad());
        }
    }
    if rows.len() != 2 || expected.is_empty() || expected.len() > 2 {
        return Err(bad());
    }
    let mut online = 0;
    for (index, (status, chunk)) in &rows {
        match expected.get(index) {
            Some(unit) if *chunk == format!("{unit}a") => {
                if *status == "Online" {
                    online += 1;
                } else if !allow_degraded || *status != "Offline" {
                    return Err(bad());
                }
            }
            None if allow_degraded && *status == "Offline" && chunk.is_empty() => (),
            _ => return Err(bad()),
        }
    }
    match volume {
        Some("Online") if online == 2 && expected.len() == 2 => Ok(()),
        Some("Degraded") if allow_degraded && online == 1 => Ok(()),
        _ => Err(bad()),
    }
}

/// The caller verifies its journaled mapping and all vnode identities first.
pub(super) fn close(mapped: &str) -> AppResult<()> {
    native::close(mapped)
}
pub(super) fn make_filesystem(mapped: &str) -> AppResult<()> {
    native::make_filesystem(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(level: u32, count: u32, index: u32, uuid: u8) -> [u8; 56] {
        let mut b = [0u8; 56];
        b[..8].copy_from_slice(&0x4d4152436372616du64.to_ne_bytes());
        for (at, value) in [(8, 6u32), (32, count), (36, index), (44, 512), (52, level)] {
            b[at..at + 4].copy_from_slice(&value.to_ne_bytes());
        }
        b[16..32].fill(uuid);
        b
    }
    #[test]
    fn metadata_distinguishes_crypto_from_two_member_mirrors() {
        let a = parse_header(&bytes(0x1c, 2, 0, 7)).unwrap();
        let b = parse_header(&bytes(0x1c, 2, 1, 7)).unwrap();
        assert_eq!(a.uuid, "07".repeat(16));
        validate_headers(&[a.clone(), b.clone()], &a.uuid).unwrap();
        assert!(validate_headers(&[a.clone(), a.clone()], &a.uuid).is_err());
        assert!(validate_headers(
            &[a.clone(), parse_header(&bytes(0x1c, 2, 1, 8)).unwrap()],
            &a.uuid
        )
        .is_err());
        let c = parse_header(&bytes(u32::from(b'C'), 1, 0, 7)).unwrap();
        assert_eq!(c.level, u32::from(b'C'));
        assert!(validate_headers(&[c], &a.uuid).is_err());
        for bad in [
            bytes(0x1c, 1, 0, 7),
            bytes(0x1c, 2, 2, 7),
            bytes(0x1c, 3, 0, 7),
            bytes(0x1c, 2, 0, 0),
            bytes(1, 2, 0, 7),
        ] {
            assert!(parse_header(&bad).is_err());
        }
        assert!(parse_header(&bytes(0x1c, 2, 0, 7)[..55]).is_err());
    }
    #[test]
    fn native_stdout_and_membership_require_exact_mirror_identity() {
        assert_eq!(
            parse_attached("softraid0: RAID 1C volume attached as sd1\n").unwrap(),
            "sd1"
        );
        assert!(parse_attached("softraid0: CRYPTO volume attached as sd1").is_err());
        let online="Volume Status Size Device\nsoftraid0 0 Online 1000 sd1 RAID1C\n 0 Online 1000 0:0.0 noencl <vnd0a>\n 1 Online 1000 0:1.0 noencl <vnd1a>\n";
        let expected = BTreeMap::from([(0, "vnd0"), (1, "vnd1")]);
        verify_listing(online, "sd1", &expected, false).unwrap();
        for bad in [
            online.replace("sd1 RAID1C", "sd2 RAID1C"),
            online.replace("RAID1C", "CRYPTO"),
            online.replace("<vnd1a>", "<vnd2a>"),
            online.replace("1 Online", "0 Online"),
        ] {
            assert!(verify_listing(&bad, "sd1", &expected, false).is_err());
        }
        let degraded="Volume Status Size Device\nsoftraid0 0 Degraded 1000 sd1 RAID1C\n 0 Online 1000 0:0.0 noencl <vnd0a>\n 1 Offline key disk 0:1.0 noencl <>\n";
        verify_listing(degraded, "sd1", &BTreeMap::from([(0, "vnd0")]), true).unwrap();
        assert!(verify_listing(degraded, "sd1", &BTreeMap::from([(0, "vnd0")]), false).is_err());
        assert!(verify_listing(degraded, "sd1", &BTreeMap::from([(1, "vnd1")]), true).is_err());
        let returned = degraded.replace("<>", "<vnd1a>");
        verify_listing(&returned, "sd1", &expected, true).unwrap();
        assert!(verify_listing(&returned, "sd1", &expected, false).is_err());
    }
}

#[cfg(all(test, target_os = "openbsd"))]
mod native_tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

    fn fixture(label: &str, count: usize) -> (PathBuf, Vec<PathBuf>) {
        assert_eq!(crate::effective_uid(), 0);
        let root =
            std::env::temp_dir().join(format!("lurker-raid1c-{label}-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let paths = (0..count)
            .map(|i| {
                let path = root.join(format!("member {i}.img"));
                let file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&path)
                    .unwrap();
                file.set_len(128 * 1024 * 1024).unwrap();
                path
            })
            .collect();
        (root, paths)
    }
    fn attach(paths: &[PathBuf]) -> Vec<String> {
        ensure_unattached(paths).unwrap();
        paths
            .iter()
            .map(|p| native::attach_file(p, false).unwrap())
            .collect()
    }
    fn hash(paths: &[PathBuf]) -> String {
        exec(
            "/bin/sha256",
            &paths
                .iter()
                .map(|p| p.as_os_str().to_owned())
                .collect::<Vec<_>>(),
            None,
        )
        .unwrap()
    }
    fn detach(backings: &[String], paths: &[PathBuf]) {
        for (device, path) in backings.iter().zip(paths).rev() {
            native::verify_backing(device, path).unwrap();
            native::detach_file(device).unwrap();
        }
    }

    #[test]
    #[ignore = "root, disposable OpenBSD host, two free vnodes and sqlite3 required"]
    fn native_mirror_lifecycle_and_explicit_degraded_recovery() {
        let (root, paths) = fixture("lifecycle", 2);
        let credential = Credential::Password(b"native temporary mirrored test secret".to_vec());
        assert!(native::attach_file(&paths[0], true).is_err());
        let mut backings = attach(&paths);
        let mapped = format(&backings, &credential).unwrap();
        let uuid = probe(&backings[0]).unwrap().uuid;
        verify_mapping(&mapped, &backings, &uuid, false).unwrap();
        make_filesystem(&mapped).unwrap();
        let mount = root.join("mounted");
        fs::create_dir(&mount).unwrap();
        let mount_args = |m: &str| {
            vec![
                "-t".into(),
                "ffs".into(),
                m.into(),
                mount.as_os_str().to_owned(),
            ]
        };
        exec("/sbin/mount", &mount_args(&mapped), None).unwrap();
        let mut data = File::create(mount.join("sentinel")).unwrap();
        data.write_all(b"durably committed mirrored data").unwrap();
        data.sync_all().unwrap();
        drop(data);
        let db = mount.join("state.sqlite");
        exec("/usr/local/bin/sqlite3", &[db.as_os_str().to_owned()], Some(b"PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE state(value TEXT); INSERT INTO state VALUES('committed'); PRAGMA integrity_check;\n")).unwrap();
        exec("/sbin/umount", &[mount.as_os_str().to_owned()], None).unwrap();
        verify_mapping(&mapped, &backings, &uuid, false).unwrap();
        close(&mapped).unwrap();
        detach(&backings, &paths);
        backings = attach(&paths);
        let before = hash(&paths);
        assert!(unlock(
            &backings,
            &Credential::Password(b"incorrect".to_vec()),
            &uuid,
            false
        )
        .is_err());
        assert_eq!(hash(&paths), before);
        let mapped = unlock(&backings, &credential, &uuid, false).unwrap();
        verify_mapping(&mapped, &backings, &uuid, false).unwrap();
        exec("/sbin/mount", &mount_args(&mapped), None).unwrap();
        assert_eq!(
            fs::read(mount.join("sentinel")).unwrap(),
            b"durably committed mirrored data"
        );
        assert_eq!(
            exec(
                "/usr/local/bin/sqlite3",
                &[db.as_os_str().to_owned()],
                Some(b"PRAGMA integrity_check; SELECT value FROM state;\n")
            )
            .unwrap()
            .trim(),
            "ok\ncommitted"
        );
        exec("/sbin/umount", &[mount.as_os_str().to_owned()], None).unwrap();
        verify_mapping(&mapped, &backings, &uuid, false).unwrap();
        close(&mapped).unwrap();
        // Existing one-member recovery only. The other member's file stays
        // untouched and present; it is intentionally not attached to a vnode.
        native::verify_backing(&backings[1], &paths[1]).unwrap();
        native::detach_file(&backings[1]).unwrap();
        backings.truncate(1);
        let before = hash(&paths);
        assert!(unlock(&backings, &credential, &uuid, false).is_err());
        assert!(unlock(
            &backings,
            &Credential::Password(b"incorrect".to_vec()),
            &uuid,
            true
        )
        .is_err());
        assert_eq!(hash(&paths), before);
        let mapped = unlock(&backings, &credential, &uuid, true).unwrap();
        verify_mapping(&mapped, &backings, &uuid, true).unwrap();
        assert!(verify_mapping(&mapped, &backings, &uuid, false).is_err());
        exec("/sbin/mount", &mount_args(&mapped), None).unwrap();
        assert_eq!(
            fs::read(mount.join("sentinel")).unwrap(),
            b"durably committed mirrored data"
        );
        assert_eq!(
            exec(
                "/usr/local/bin/sqlite3",
                &[db.as_os_str().to_owned()],
                Some(b"PRAGMA integrity_check; SELECT value FROM state;\n")
            )
            .unwrap()
            .trim(),
            "ok\ncommitted"
        );
        exec("/sbin/umount", &[mount.as_os_str().to_owned()], None).unwrap();
        verify_mapping(&mapped, &backings, &uuid, true).unwrap();
        close(&mapped).unwrap();
        detach(&backings, &paths[..1]);
        // Supplying the old peer again does not repair it: the native array
        // remains degraded. Require explicit acknowledgement, never auto-rebuild.
        let backings = attach(&paths);
        let mapped = unlock(&backings, &credential, &uuid, true).unwrap();
        assert!(verify_mapping(&mapped, &backings, &uuid, false).is_err());
        verify_mapping(&mapped, &backings, &uuid, true).unwrap();
        close(&mapped).unwrap();
        detach(&backings, &paths);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "root, disposable OpenBSD host and four free vnodes required"]
    fn native_foreign_attachments_aliases_mixed_uuids_and_duplicates_are_rejected() {
        let (root, paths) = fixture("ownership", 4);
        let alias = root.join("alias.img");
        fs::hard_link(&paths[0], &alias).unwrap();
        assert!(ensure_unattached(&[paths[0].clone(), alias]).is_err());
        let credential = Credential::Password(b"native temporary mixed-array test secret".to_vec());
        let first = attach(&paths[..2]);
        assert!(ensure_unattached(&paths[..2]).is_err()); // raw foreign vnode
        let mapped = format(&first, &credential).unwrap();
        let uuid = probe(&first[0]).unwrap().uuid;
        assert!(ensure_unattached(&paths[..2]).is_err()); // foreign active array
        verify_mapping(&mapped, &first, &uuid, false).unwrap();
        close(&mapped).unwrap();
        let second = attach(&paths[2..]);
        let mapped = format(&second, &credential).unwrap();
        let other_uuid = probe(&second[0]).unwrap().uuid;
        assert_ne!(uuid, other_uuid);
        verify_mapping(&mapped, &second, &other_uuid, false).unwrap();
        close(&mapped).unwrap();
        let before = hash(&paths);
        assert!(unlock(
            &[first[0].clone(), second[1].clone()],
            &credential,
            &uuid,
            false
        )
        .is_err());
        assert!(unlock(
            &[first[0].clone(), first[0].clone()],
            &credential,
            &uuid,
            false
        )
        .is_err());
        assert!(unlock(&first, &credential, &other_uuid, false).is_err());
        assert_eq!(hash(&paths), before);
        detach(&second, &paths[2..]);
        detach(&first, &paths[..2]);
        fs::remove_dir_all(root).unwrap();
    }
}
