//! OpenBSD's base-system softraid CRYPTO adapter.
//!
//! Lurker-owned vnode files and verified idle physical disks use one CRYPTO
//! chunk; `raid1c_native` composes the same identity and filesystem helpers for
//! explicitly identified two-member native RAID1C arrays.
use super::{args, exec, Credential, Registry, Session};
use crate::{AppError, AppResult, CreateCipher, VolumeType};
use serde::{Deserialize, Serialize};

mod filesystems;
mod physical;
pub(super) use filesystems::{
    close_filesystem, filesystem_has_posix_ownership, make_selected_filesystem,
    matches_filesystem_mount, mount_filesystem, validate_filesystem,
};
pub(super) use physical::{ensure_physical_idle, physical_device, prepare_physical};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DetachIntent {
    Volume,
    Backing,
}

fn device_name<'a>(value: &'a str, prefix: &str) -> AppResult<&'a str> {
    let value = value.strip_prefix("/dev/").unwrap_or(value);
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
        Err(AppError::new(
            "Unexpected OpenBSD device identity; refusing device operation.",
        ))
    }
}

pub(super) fn backing_unit(device: &str) -> AppResult<&str> {
    device_name(device, "vnd").or_else(|_| device_name(device, "sd"))
}

fn password(credential: &Credential) -> AppResult<Vec<u8>> {
    match credential {
        Credential::Password(bytes) if !bytes.is_empty() && !bytes.iter().any(|b| matches!(b, 0 | 10 | 13)) => {
            let mut input = bytes.clone();
            input.push(b'\n');
            Ok(input)
        }
        Credential::Password(_) => Err(AppError::new("Softraid passphrases must be non-empty and contain no NUL or newline.")),
        Credential::Keyfile(_) => Err(AppError::new("Softraid requires a password. Its native keydisk is not a regular keyfile; keyfile authentication is unavailable.")),
    }
}

fn require_writable(readonly: bool) -> AppResult<()> {
    if readonly {
        Err(AppError::new("Read-only softraid mappings are unavailable in the qualified OpenBSD base tools. Refusing a writable crypto fallback."))
    } else {
        Ok(())
    }
}

pub(super) fn attach_file(path: &Path, readonly: bool) -> AppResult<String> {
    require_writable(readonly)?;
    // vnconfig asks the kernel for an unused unit; do not inspect and then
    // overwrite a guessed unit, which would race another storage operation.
    let output = exec("/sbin/vnconfig", &[path.as_os_str().to_owned()], None)?;
    let name = device_name(output.trim(), "vnd")?;
    let device = format!("/dev/{name}c");
    verify_backing(&device, path)?;
    Ok(device)
}

// OpenBSD dev/vndioctl.h. This is an OS ABI binding, not a cryptographic
// implementation. Keep native scalar aliases: dev_t is signed on OpenBSD.
#[repr(C)]
struct VndUser {
    file: [libc::c_char; 1024],
    unit: libc::c_int,
    device: libc::dev_t,
    inode: libc::ino_t,
}

const VNDIOCGET: libc::c_ulong = 0xc0000000
    | (((std::mem::size_of::<VndUser>() as libc::c_ulong) & 0x1fff) << 16)
    | ((b'F' as libc::c_ulong) << 8)
    | 3;

fn queried_backing_identity(query: &VndUser, unit: i32) -> AppResult<Option<(u64, u64)>> {
    if query.unit != unit {
        return Err(AppError::new(
            "Unexpected vnode unit returned by the kernel.",
        ));
    }
    if query.device == 0 && query.inode == 0 {
        Ok(None)
    } else {
        Ok(Some((query.device as u64, query.inode as u64)))
    }
}

/// Read the kernel's backing identity without configuring or detaching a vnode.
pub(super) fn vnode_identity(device: &str) -> AppResult<Option<(u64, u64)>> {
    let name = device_name(device, "vnd")?;
    let unit = name[3..]
        .parse::<i32>()
        .map_err(|_| AppError::new("OpenBSD vnode unit is outside its native range."))?;
    let control = File::open(format!("/dev/r{name}c"))
        .map_err(|e| AppError::io("Open vnode identity control", e))?;
    let mut query = VndUser {
        file: [0; 1024],
        unit: -1,
        device: 0,
        inode: 0,
    };
    // SAFETY: VNDIOCGET receives the exact repr(C) native structure with valid
    // initialized scalar fields and writable storage. The live descriptor was
    // opened read-only on the named vnode; this GET command does not configure
    // or detach storage. -1 asks the kernel for this descriptor's actual unit.
    if unsafe { libc::ioctl(control.as_raw_fd(), VNDIOCGET, &mut query) } == -1 {
        return Err(AppError::io(
            "Read vnode backing identity",
            std::io::Error::last_os_error(),
        ));
    }
    queried_backing_identity(&query, unit)
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let metadata = fs::metadata(path).map_err(|e| AppError::io("Inspect vnode source", e))?;
    if metadata.file_type().is_block_device() {
        let source = physical_device(path)?;
        let backing = physical_device(Path::new(device))?;
        if source.source != backing.source
            || source.bytes != backing.bytes
            || source.metadata_id != backing.metadata_id
        {
            return Err(AppError::new(
                "OpenBSD physical backing identity changed; refusing operation.",
            ));
        }
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(AppError::new(
            "Backing must be a regular vnode file or verified physical disk.",
        ));
    }
    // Do not trust vnconfig's cached display pathname or a /dev name for the
    // backing filesystem: hard links can change the displayed path, and FUSE
    // filesystems do not necessarily have a corresponding block-device node.
    if vnode_identity(device)? != Some((metadata.dev(), metadata.ino())) {
        return Err(AppError::new(
            "Vnode backing identity changed; refusing storage operation.",
        ));
    }
    Ok(())
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    if backing_unit(device)?.starts_with("sd") {
        // Physical storage has no acquired vnode lease to detach.
        physical_device(Path::new(device))?;
        return Ok(());
    }
    let name = device_name(device, "vnd")?;
    exec("/sbin/vnconfig", &args(&["-u", name]), None)?;
    Ok(())
}

pub(super) fn detach_backing(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let backing = session.backing.as_deref().ok_or_else(|| {
        AppError::new("OpenBSD native backing is missing from the detach journal.")
    })?;
    verify_backing(backing, &session.source)?;
    session.openbsd_detach = Some(DetachIntent::Backing);
    registry.save(session)?;
    detach_file(backing)?;
    session.backing = None;
    session.openbsd_detach = None;
    registry.save(session)
}

pub(super) fn format(
    device: &str,
    cipher: CreateCipher,
    credential: &Credential,
) -> AppResult<String> {
    if cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "OpenBSD softraid CRYPTO supports Lurker's AES profile only.",
        ));
    }
    // Validate credentials before writing any label.
    let _ = password(credential)?;
    let name = backing_unit(device)?;
    if name.starts_with("sd") {
        prepare_physical(Path::new(device))?;
    } else {
        exec(
            "/sbin/disklabel",
            &args(&["-wAT-", name]),
            Some(b"RAID *\n"),
        )?;
    }
    // Return the newly initialized mapping to the shared lifecycle without an
    // internal detach/reopen. The caller journals this resource before
    // filesystem formatting or cleanup, keeping interrupted creation recoverable.
    assemble(device, credential, true)
}

/// Probe only native CRYPTO metadata. This never calls bioctl or writes a
/// label: Auto dispatch may try VeraCrypt only when this returns false, never
/// after native authentication fails. Other storage I/O errors fail closed.
pub(super) fn probe_crypto_header(device: &str) -> AppResult<bool> {
    let name = backing_unit(device)?;
    let mut file = match File::open(format!("/dev/r{name}a")) {
        Ok(file) => file,
        // Unlabeled/random/VeraCrypt images need not expose partition 'a'.
        Err(error) if matches!(error.raw_os_error(), Some(libc::ENXIO | libc::ENOENT)) => {
            return Ok(false)
        }
        Err(error) => return Err(AppError::io("Read softraid metadata", error)),
    };
    probe_crypto_reader(&mut file)
}

fn probe_crypto_reader(reader: &mut (impl Read + Seek)) -> AppResult<bool> {
    reader
        .seek(SeekFrom::Start(8192))
        .map_err(|e| AppError::io("Seek softraid metadata", e))?;
    let mut bytes = [0u8; 56];
    match reader.read_exact(&mut bytes) {
        Ok(()) if is_raid1c_header(&bytes) => Err(AppError::new("This is a native RAID1C member. Use --type raid1c --member PATH, its .raid1c.json descriptor, or explicit --degraded recovery; a member is not a single CRYPTO volume.")),
        Ok(()) => Ok(is_crypto_header(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(AppError::io("Read softraid metadata", error)),
    }
}

/// bioctl -c C both creates and assembles volumes. Missing native metadata must
/// never reach that command during mount. Metadata with a valid native header
/// but a corrupt checksum is rejected by the kernel's sr_meta_read/validate;
/// it is not treated as a new volume, and no force flag is ever supplied.
fn require_crypto_header(device: &str) -> AppResult<()> {
    if !probe_crypto_header(device)? {
        return Err(AppError::new("No qualified single-chunk softraid CRYPTO metadata found; refusing to initialize during mount."));
    }
    Ok(())
}

fn is_crypto_header(bytes: &[u8; 56]) -> bool {
    // Native metadata constants and packed invariant layout from OpenBSD
    // sys/dev/softraidvar.h: SR_MAGIC, SR_META_VERSION, ssd_chunk_no/ssd_level.
    u64::from_ne_bytes(bytes[0..8].try_into().unwrap()) == 0x4d4152436372616d
        && u32::from_ne_bytes(bytes[8..12].try_into().unwrap()) == 6
        && u32::from_ne_bytes(bytes[32..36].try_into().unwrap()) == 1
        && u32::from_ne_bytes(bytes[52..56].try_into().unwrap()) == u32::from(b'C')
}

fn is_raid1c_header(bytes: &[u8; 56]) -> bool {
    u64::from_ne_bytes(bytes[0..8].try_into().unwrap()) == 0x4d4152436372616d
        && u32::from_ne_bytes(bytes[8..12].try_into().unwrap()) == 6
        && u32::from_ne_bytes(bytes[32..36].try_into().unwrap()) == 2
        && u32::from_ne_bytes(bytes[36..40].try_into().unwrap()) < 2
        && u32::from_ne_bytes(bytes[52..56].try_into().unwrap()) == 0x1c
}

fn assemble(device: &str, credential: &Credential, creating: bool) -> AppResult<String> {
    let name = backing_unit(device)?;
    let chunk = format!("/dev/{name}a");
    let input = password(credential)?;
    let mut argv = args(&["-s", "-C", "noauto", "-c", "C"]);
    if creating {
        argv.extend(args(&["-r", "auto"]));
    }
    argv.extend(args(&["-l", &chunk, "softraid0"]));
    let output = exec("/sbin/bioctl", &argv, Some(&input))?;
    let name = parse_attached(&output)?;
    let mapped = format!("/dev/{name}a");
    verify_mapping(&mapped, device)?;
    Ok(mapped)
}

fn parse_attached(output: &str) -> AppResult<&str> {
    let name = output.trim().strip_prefix("softraid0: CRYPTO volume attached as ")
        .ok_or_else(|| AppError::new("Softraid attached but its identity could not be determined; inspect bioctl before retrying."))?;
    device_name(name, "sd")
}

pub(super) fn unlock(device: &str, readonly: bool, credential: &Credential) -> AppResult<String> {
    require_writable(readonly)?;
    require_crypto_header(device)?;
    assemble(device, credential, false)
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    let name = device_name(mapped, "sd")?;
    let vnode = backing_unit(backing)?;
    let output = exec("/sbin/bioctl", &args(&[name]), None)?;
    if !mapping_matches(&output, name, vnode) {
        return Err(AppError::new(
            "Softraid mapping no longer belongs to its recorded backing; refusing operation.",
        ));
    }
    Ok(())
}

fn mapping_matches(output: &str, mapped: &str, vnode: &str) -> bool {
    let rows: Vec<Vec<&str>> = output
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    let volumes = rows
        .iter()
        .filter(|r| r.first() == Some(&"softraid0"))
        .collect::<Vec<_>>();
    let chunks = rows
        .iter()
        .filter(|r| r.last().is_some_and(|v| v.starts_with('<')))
        .collect::<Vec<_>>();
    volumes.len() == 1
        && volumes[0].len() == 6
        && volumes[0][4] == mapped
        && volumes[0][5] == "CRYPTO"
        && chunks.len() == 1
        && chunks[0].last() == Some(&format!("<{vnode}a>").as_str())
}

/// Caller must verify_mapping against its stored backing identity immediately
/// before closing. No force option is used: a busy filesystem keeps its mapping.
pub(super) fn close(device: &str) -> AppResult<()> {
    let name = device_name(device, "sd")?;
    exec("/sbin/bioctl", &args(&["-d", name]), None)?;
    Ok(())
}

pub(super) fn disk_is_absent(mapped: &str) -> AppResult<bool> {
    let recorded = device_name(mapped, "sd")?;
    let listing = exec("/sbin/sysctl", &args(&["-n", "hw.disknames"]), None)?;
    let listing = listing.trim();
    if listing.is_empty() {
        return Ok(true);
    }
    let mut found = false;
    for item in listing.split(',') {
        let Some((name, duid)) = item.split_once(':') else {
            return Err(AppError::new(
                "Unexpected OpenBSD disk inventory; preserving the journal.",
            ));
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric())
            || (!duid.is_empty()
                && (duid.len() != 16 || !duid.bytes().all(|b| b.is_ascii_hexdigit())))
        {
            return Err(AppError::new(
                "Unexpected OpenBSD disk inventory; preserving the journal.",
            ));
        }
        found |= name == recorded;
    }
    Ok(!found)
}

pub(super) fn reconcile_detach(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(intent) = session.openbsd_detach else {
        return Ok(());
    };
    let conflict = || {
        AppError::new("OpenBSD detach intent conflicts with its session; preserving the journal.")
    };
    if session.backend != VolumeType::Softraid
        || session.mounted.is_some()
        || !session.identity.matches(&session.source)
    {
        return Err(conflict());
    }
    let backing = session.backing.as_deref().ok_or_else(conflict)?;
    match intent {
        DetachIntent::Volume => {
            let mapped = session.mapped.as_deref().ok_or_else(conflict)?;
            verify_backing(backing, &session.source)?;
            if !disk_is_absent(mapped)? {
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
        DetachIntent::Backing => {
            if session.mapped.is_some() {
                return Err(conflict());
            }
            if backing_unit(backing)?.starts_with("sd") {
                verify_backing(backing, &session.source)?;
            } else if vnode_identity(backing)?.is_some() {
                return Ok(());
            }
            session.backing = None;
        }
    }
    session.openbsd_detach = None;
    registry.save(session)
}

pub(super) fn make_filesystem(device: &str) -> AppResult<()> {
    let name = device_name(device, "sd")?;
    // Freshly decrypted random data can spoof disk labels. Initialize only the
    // beginning of this newly created virtual disk before writing its FFS label.
    let mut disk = OpenOptions::new()
        .write(true)
        .open(format!("/dev/r{name}c"))
        .map_err(|e| AppError::io("Open newly created softraid volume", e))?;
    disk.write_all(&vec![0u8; 1024 * 1024])
        .map_err(|e| AppError::io("Initialize new softraid label area", e))?;
    disk.sync_all()
        .map_err(|e| AppError::io("Sync new softraid label area", e))?;
    drop(disk);
    exec("/sbin/disklabel", &args(&["-wAT-", name]), Some(b"/ *\n"))?;
    exec(
        "/sbin/newfs",
        &args(&["-q", "-O", "2", &format!("/dev/r{name}a")]),
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires root and real disposable OpenBSD CRYPTO and vnode attachments"]
    fn cleanup_reconciles_completed_native_detaches_with_saved_intent() {
        use super::super::{cleanup, require_root, unique_id, Identity};
        use std::os::unix::fs::DirBuilderExt;

        require_root().unwrap();
        let work = std::env::temp_dir().join(format!("lurker-openbsd-detach-{}", unique_id()));
        fs::DirBuilder::new().mode(0o700).create(&work).unwrap();
        let source = work.join("owned.img");
        File::create(&source)
            .unwrap()
            .set_len(128 * 1024 * 1024)
            .unwrap();
        let registry = Registry {
            directory: work.clone(),
            _lock: File::create(work.join("lock")).unwrap(),
        };
        let backing = attach_file(&source, false).unwrap();
        let credential = Credential::Password(b"disposable OpenBSD detach fixture".to_vec());
        let mapped = format(&backing, CreateCipher::Aes, &credential).unwrap();
        let mut session = Session {
            version: 1,
            id: unique_id(),
            backend: VolumeType::Softraid,
            identity: Identity::read(&source).unwrap(),
            source: source.clone(),
            create_target: None,
            peer_sources: Vec::new(),
            backing: Some(backing.clone()),
            physical: None,
            openbsd_detach: Some(DetachIntent::Volume),
            mapped: Some(mapped.clone()),
            mounted: None,
            intended_mountpoint: None,
            initialization_directory: None,
            readonly: false,
            veracrypt_proof: None,
            mirror: None,
        };
        registry.save(&session).unwrap();
        reconcile_detach(&registry, &mut session).unwrap();
        assert_eq!(session.mapped.as_deref(), Some(mapped.as_str()));
        verify_mapping(&mapped, &backing).unwrap();
        close(&mapped).unwrap();

        let mut saved = registry.records().unwrap().pop().unwrap();
        let mut without_intent = saved.clone();
        without_intent.openbsd_detach = None;
        assert!(cleanup(&registry, &mut without_intent).is_err());
        reconcile_detach(&registry, &mut saved).unwrap();
        assert!(saved.mapped.is_none() && saved.openbsd_detach.is_none());
        verify_backing(&backing, &source).unwrap();

        saved.openbsd_detach = Some(DetachIntent::Backing);
        registry.save(&saved).unwrap();
        reconcile_detach(&registry, &mut saved).unwrap();
        assert_eq!(saved.backing.as_deref(), Some(backing.as_str()));
        detach_file(&backing).unwrap();

        let mut saved = registry.records().unwrap().pop().unwrap();
        let mut without_intent = saved.clone();
        without_intent.openbsd_detach = None;
        assert!(cleanup(&registry, &mut without_intent).is_err());
        cleanup(&registry, &mut saved).unwrap();
        assert!(registry.records().unwrap().is_empty());
        assert!(saved.identity.matches(&source));
        fs::remove_dir_all(work).unwrap();
    }
    #[test]
    fn device_names_cannot_escape_the_native_device_namespace() {
        assert_eq!(device_name("/dev/rvnd3c", "vnd").unwrap(), "vnd3");
        assert_eq!(device_name("/dev/sd12a", "sd").unwrap(), "sd12");
        for bad in ["sd", "/dev/sd1/../sd0", "sd1b", "sd1\n", "sd1;echo bad"] {
            assert!(device_name(bad, "sd").is_err());
        }
    }
    #[test]
    fn mapping_parser_rejects_other_crypto_volumes() {
        let listing = "Volume Status Size Device\nsoftraid0 0 Online 1000 sd2 CRYPTO\n 0 Online 1000 0:0.0 noencl <vnd1a>\n";
        assert!(mapping_matches(listing, "sd2", "vnd1"));
        assert!(!mapping_matches(listing, "sd1", "vnd1"));
        assert!(!mapping_matches(listing, "sd2", "vnd0"));
        assert!(!mapping_matches(
            &listing.replace("CRYPTO", "RAID1"),
            "sd2",
            "vnd1"
        ));
        assert_eq!(
            parse_attached("softraid0: CRYPTO volume attached as sd2\n").unwrap(),
            "sd2"
        );
        assert!(parse_attached("softraid0: CRYPTO volume attached as sd2;evil").is_err());
    }
    #[test]
    fn native_vnode_query_requires_unit_device_and_inode_identity() {
        let mut query = VndUser {
            file: [0; 1024],
            unit: 2,
            device: 1024,
            inode: 42,
        };
        assert_eq!(
            queried_backing_identity(&query, 2).unwrap(),
            Some((1024, 42))
        );
        assert!(queried_backing_identity(&query, 1).is_err());
        query.device = 0;
        query.inode = 0;
        assert_eq!(queried_backing_identity(&query, 2).unwrap(), None);
    }

    #[cfg(all(target_os = "openbsd", target_pointer_width = "64"))]
    #[test]
    fn native_vnode_query_matches_qualified_openbsd_header_abi() {
        assert_eq!(std::mem::size_of::<VndUser>(), 1040);
        assert_eq!(std::mem::offset_of!(VndUser, unit), 1024);
        assert_eq!(std::mem::offset_of!(VndUser, device), 1028);
        assert_eq!(std::mem::offset_of!(VndUser, inode), 1032);
        assert_eq!(VNDIOCGET, 0xc4104603);
    }

    #[test]
    fn incomplete_or_other_metadata_cannot_trigger_initialization_on_unlock() {
        let mut header = [0u8; 56];
        assert!(!is_crypto_header(&header));
        header[..8].copy_from_slice(&0x4d4152436372616du64.to_ne_bytes());
        header[8..12].copy_from_slice(&6u32.to_ne_bytes());
        header[32..36].copy_from_slice(&1u32.to_ne_bytes());
        header[52..56].copy_from_slice(&u32::from(b'C').to_ne_bytes());
        assert!(is_crypto_header(&header));
        header[52] ^= 1;
        assert!(!is_crypto_header(&header));
    }

    #[test]
    fn metadata_probe_uses_native_offset_and_rejects_truncation() {
        let mut header = [0u8; 56];
        header[..8].copy_from_slice(&0x4d4152436372616du64.to_ne_bytes());
        header[8..12].copy_from_slice(&6u32.to_ne_bytes());
        header[32..36].copy_from_slice(&1u32.to_ne_bytes());
        header[52..56].copy_from_slice(&u32::from(b'C').to_ne_bytes());
        let mut bytes = vec![0u8; 8192 + header.len()];
        bytes[..header.len()].copy_from_slice(&header);
        assert!(!probe_crypto_reader(&mut std::io::Cursor::new(&bytes)).unwrap());
        bytes[8192..].copy_from_slice(&header);
        assert!(probe_crypto_reader(&mut std::io::Cursor::new(&bytes)).unwrap());
        assert!(
            !probe_crypto_reader(&mut std::io::Cursor::new(&bytes[..bytes.len() - 1])).unwrap()
        );
        assert!(!probe_crypto_reader(&mut std::io::Cursor::new(&[])).unwrap());
        for field in [0, 8, 32, 52] {
            let mut malformed = bytes.clone();
            malformed[8192 + field] ^= 1;
            assert!(!probe_crypto_reader(&mut std::io::Cursor::new(&malformed)).unwrap());
        }
    }

    #[test]
    fn recognized_mirror_member_needs_explicit_membership_not_veracrypt_fallback() {
        let mut bytes = vec![0u8; 8192 + 56];
        bytes[8192..8200].copy_from_slice(&0x4d4152436372616du64.to_ne_bytes());
        for (offset, value) in [(8, 6u32), (32, 2), (36, 1), (52, 0x1c)] {
            bytes[8192 + offset..8192 + offset + 4].copy_from_slice(&value.to_ne_bytes());
        }
        let error = probe_crypto_reader(&mut std::io::Cursor::new(bytes)).unwrap_err();
        assert!(error.message.contains("--type raid1c --member"));
        assert!(error.message.contains("--degraded"));
    }

    #[test]
    fn metadata_probe_does_not_hide_storage_read_errors() {
        struct Unreadable;
        impl Read for Unreadable {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            }
        }
        impl Seek for Unreadable {
            fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
                Ok(8192)
            }
        }
        assert!(probe_crypto_reader(&mut Unreadable).is_err());
    }

    /// A hard-link alias is the same object, while a reused vnode number is not.
    #[cfg(target_os = "openbsd")]
    #[test]
    #[ignore = "requires root and a disposable native OpenBSD test host"]
    fn native_backing_identity_accepts_hardlinks_and_rejects_reused_vnodes() {
        assert_eq!(crate::effective_uid(), 0);
        let root =
            std::env::temp_dir().join(format!("lurker-softraid-identity-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.img");
        let alias = root.join("hardlink alias.img");
        let replacement = root.join("replacement.img");
        for path in [&source, &replacement] {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            file.set_len(32 * 1024 * 1024).unwrap();
        }
        fs::hard_link(&source, &alias).unwrap();
        let device = attach_file(&source, false).unwrap();
        verify_backing(&device, &alias).unwrap();
        assert!(verify_backing(&device, &replacement).is_err());
        verify_backing(&device, &source).unwrap();
        detach_file(&device).unwrap();
        let next_device = attach_file(&replacement, false).unwrap();
        // Probe the actual newly allocated unit, which may reuse the old name.
        // Its new inode must fail the old source's ownership check either way.
        assert!(verify_backing(&next_device, &source).is_err());
        verify_backing(&next_device, &replacement).unwrap();
        detach_file(&next_device).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    /// Confirm that the kernel treats recognizable-but-corrupt metadata as an
    /// assembly failure, never as permission to initialize a new volume.
    #[cfg(target_os = "openbsd")]
    #[test]
    #[ignore = "requires root and a disposable native OpenBSD test host"]
    fn native_corrupt_metadata_cannot_initialize_or_rewrite_image() {
        assert_eq!(crate::effective_uid(), 0);
        let root =
            std::env::temp_dir().join(format!("lurker-softraid-checksum-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let image = root.join("disposable corrupt metadata.img");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&image)
            .unwrap();
        file.set_len(96 * 1024 * 1024).unwrap();
        drop(file);
        let credential =
            Credential::Password(b"native temporary checksum regression secret".to_vec());
        let device = attach_file(&image, false).unwrap();
        let mapped = format(&device, CreateCipher::Aes, &credential).unwrap();
        verify_mapping(&mapped, &device).unwrap();
        close(&mapped).unwrap();
        let name = device_name(&device, "vnd").unwrap();
        let mut native = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/r{name}a"))
            .unwrap();
        // OpenBSD sr_meta_invariant places ssd_uuid at byte 16. It participates
        // in the native metadata checksum but not our format-family probe.
        // Raw character devices require sector-aligned I/O. Preserve the rest
        // of the containing sector while flipping just one UUID byte.
        native.seek(SeekFrom::Start(8192)).unwrap();
        let mut sector = [0u8; 512];
        native.read_exact(&mut sector).unwrap();
        sector[16] ^= 1;
        native.seek(SeekFrom::Start(8192)).unwrap();
        native.write_all(&sector).unwrap();
        native.sync_all().unwrap();
        drop(native);
        assert!(probe_crypto_header(&device).unwrap());
        let before = exec("/bin/sha256", &[image.as_os_str().to_owned()], None).unwrap();
        let error = unlock(&device, false, &credential).unwrap_err();
        let after = exec("/bin/sha256", &[image.as_os_str().to_owned()], None).unwrap();
        assert_eq!(before, after, "Corrupt metadata unlock rewrote the disposable image; preserve its fixture for inspection.");
        let active = exec("/sbin/bioctl", &args(&["softraid0"]), None).unwrap();
        assert!(
            !active.contains(&format!("<{name}a>")),
            "Corrupt metadata unexpectedly assembled a mapping; preserve its fixture."
        );
        assert!(
            error.message.contains("corrupt metadata")
                || error.message.contains("invalid metadata checksum"),
            "Unexpected native error: {}",
            error.message
        );
        eprintln!(
            "Corrupt softraid metadata rejected without creating a mapping: {}",
            error.message
        );
        eprintln!("Unchanged image digest: {}", before.trim());
        verify_backing(&device, &image).unwrap();
        detach_file(&device).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    /// Opt in as root on the OpenBSD test guest. All storage belongs to this
    /// uniquely created temporary directory; no existing device is selected.
    #[cfg(target_os = "openbsd")]
    #[test]
    #[ignore = "requires root and a disposable native OpenBSD test host"]
    fn native_softraid_file_lifecycle() {
        assert_eq!(crate::effective_uid(), 0);
        let root =
            std::env::temp_dir().join(format!("lurker-softraid-test-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let image = root.join("test with spaces.img");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&image)
            .unwrap();
        file.set_len(96 * 1024 * 1024).unwrap();
        drop(file);
        let credential = Credential::Password(b"native disposable softraid test secret".to_vec());
        let device = attach_file(&image, false).unwrap();
        assert!(!probe_crypto_header(&device).unwrap());
        assert!(unlock(&device, false, &credential).is_err());
        let mapped = format(&device, CreateCipher::Aes, &credential).unwrap();
        assert!(probe_crypto_header(&device).unwrap());
        verify_mapping(&mapped, &device).unwrap();
        make_filesystem(&mapped).unwrap();
        let mountpoint = root.join("mounted");
        fs::create_dir(&mountpoint).unwrap();
        let mount_args = vec![
            "-t".into(),
            "ffs".into(),
            mapped.clone().into(),
            mountpoint.as_os_str().to_owned(),
        ];
        exec("/sbin/mount", &mount_args, None).unwrap();
        let mut data = File::create(mountpoint.join("sentinel")).unwrap();
        data.write_all(b"durably committed test content").unwrap();
        data.sync_all().unwrap();
        drop(data);
        exec("/sbin/umount", &[mountpoint.as_os_str().to_owned()], None).unwrap();
        verify_mapping(&mapped, &device).unwrap();
        close(&mapped).unwrap();
        assert!(unlock(&device, false, &Credential::Password(b"incorrect".to_vec())).is_err());
        let mapped = unlock(&device, false, &credential).unwrap();
        let mount_args = vec![
            "-t".into(),
            "ffs".into(),
            mapped.clone().into(),
            mountpoint.as_os_str().to_owned(),
        ];
        exec("/sbin/mount", &mount_args, None).unwrap();
        assert_eq!(
            fs::read(mountpoint.join("sentinel")).unwrap(),
            b"durably committed test content"
        );
        exec("/sbin/umount", &[mountpoint.as_os_str().to_owned()], None).unwrap();
        verify_mapping(&mapped, &device).unwrap();
        close(&mapped).unwrap();
        verify_backing(&device, &image).unwrap();
        detach_file(&device).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
