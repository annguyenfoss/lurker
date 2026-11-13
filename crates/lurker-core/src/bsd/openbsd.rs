//! OpenBSD's base-system softraid CRYPTO adapter.
//!
//! Only Lurker-owned vnode files are accepted. A single RAID chunk is used;
//! RAID1C needs multiple independently identified backing files and is not
//! represented by the current single-source request protocol.
use super::{args, exec, Credential};
use crate::{AppError, AppResult, CreateCipher};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

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

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let name = device_name(device, "vnd")?;
    let canonical = fs::canonicalize(path).map_err(|e| AppError::io("Resolve vnode source", e))?;
    let metadata = fs::metadata(&canonical).map_err(|e| AppError::io("Inspect vnode source", e))?;
    let output = exec("/sbin/vnconfig", &args(&["-l", name]), None)?;
    let (covered, inode) = parse_backing(&output, name)?;
    if covered != canonical || inode != metadata.ino() || !metadata.is_file() {
        return Err(AppError::new(
            "Vnode backing identity changed; refusing storage operation.",
        ));
    }
    Ok(())
}

fn parse_backing(output: &str, name: &str) -> AppResult<(std::path::PathBuf, u64)> {
    let prefix = format!("{name}: covering ");
    let line = output.trim_end().strip_prefix(&prefix).ok_or_else(|| {
        AppError::new("Vnode is absent or its backing identity cannot be verified.")
    })?;
    let (path, rest) = line
        .rsplit_once(" on ")
        .ok_or_else(|| AppError::new("Unrecognized vnode identity response."))?;
    let inode = rest
        .rsplit_once(", inode ")
        .and_then(|(_, n)| n.parse().ok())
        .ok_or_else(|| AppError::new("Unrecognized vnode inode response."))?;
    Ok((path.into(), inode))
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let name = device_name(device, "vnd")?;
    exec("/sbin/vnconfig", &args(&["-u", name]), None)?;
    Ok(())
}

pub(super) fn format(device: &str, cipher: CreateCipher, credential: &Credential) -> AppResult<()> {
    if cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "OpenBSD softraid CRYPTO supports Lurker's AES profile only.",
        ));
    }
    // Validate credentials before writing any label.
    let _ = password(credential)?;
    let name = device_name(device, "vnd")?;
    exec(
        "/sbin/disklabel",
        &args(&["-wAT-", name]),
        Some(b"RAID *\n"),
    )?;
    let mapped = assemble(device, credential, true)?;
    // No filesystem exists yet. Closing here gives the shared lifecycle the
    // same format -> unlock -> make_filesystem contract as GELI and LUKS.
    verify_mapping(&mapped, device)?;
    close(&mapped)
}

/// A mount must never turn a damaged or unrelated image into a new volume.
/// bioctl's create/assemble command does both, so qualify native metadata first.
fn require_crypto_header(device: &str) -> AppResult<()> {
    let name = device_name(device, "vnd")?;
    let mut file = File::open(format!("/dev/r{name}a"))
        .map_err(|e| AppError::io("Read softraid metadata", e))?;
    file.seek(SeekFrom::Start(8192))
        .map_err(|e| AppError::io("Seek softraid metadata", e))?;
    let mut bytes = [0u8; 56];
    file.read_exact(&mut bytes)
        .map_err(|e| AppError::io("Read softraid metadata", e))?;
    if !is_crypto_header(&bytes) {
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

fn assemble(device: &str, credential: &Credential, creating: bool) -> AppResult<String> {
    let name = device_name(device, "vnd")?;
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
    let vnode = device_name(backing, "vnd")?;
    let output = exec("/sbin/bioctl", &args(&[name]), None)?;
    if !mapping_matches(&output, name, vnode) {
        return Err(AppError::new(
            "Softraid mapping no longer belongs to the recorded vnode; refusing operation.",
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
    fn device_names_cannot_escape_the_native_device_namespace() {
        assert_eq!(device_name("/dev/rvnd3c", "vnd").unwrap(), "vnd3");
        assert_eq!(device_name("/dev/sd12a", "sd").unwrap(), "sd12");
        for bad in ["sd", "/dev/sd1/../sd0", "sd1b", "sd1\n", "sd1;echo bad"] {
            assert!(device_name(bad, "sd").is_err());
        }
    }
    #[test]
    fn identity_parsers_preserve_spaces_and_reject_other_crypto_volumes() {
        let line = "vnd1: covering /tmp/lurker with on spaces/file.img on sd0a, inode 42\n";
        assert_eq!(
            parse_backing(line, "vnd1").unwrap(),
            ("/tmp/lurker with on spaces/file.img".into(), 42)
        );
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
        assert!(unlock(&device, false, &credential).is_err());
        format(&device, CreateCipher::Aes, &credential).unwrap();
        let mapped = unlock(&device, false, &credential).unwrap();
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
