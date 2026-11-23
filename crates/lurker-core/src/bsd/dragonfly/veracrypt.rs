//! VeraCrypt headers are handled in the private tcplay process. Only a
//! validated native mapper, identified by its journaled UUID, is mounted.
use super::{args, exec, mapping_name, verify_backing, vnode_name, AppError, AppResult};
use crate::bsd::{exec_dragonfly_locked, mounts, Credential, Registry, Session};
use crate::new_filesystem::Owner;
use crate::{CreateCipher, VolumeType};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DMSETUP: &str = "/sbin/dmsetup";
const SECTOR: u64 = 512;
const MIB: u64 = 1024 * 1024;
const MIN_CREATE_MIB: u64 = 260;
const ALIGNED_SECTORS: u64 = 8;
const RESERVED: u64 = 128 * 1024;
const MAX_KEYFILE: u64 = 1024 * 1024;
const MAX_PASSWORD: usize = 64;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Serialize, Deserialize)]
pub(in crate::bsd) struct Mapping {
    sectors: u64,
    #[serde(default)]
    removing: bool,
}

pub(in crate::bsd) fn classify(source: &Path) -> AppResult<VolumeType> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(source)
        .map_err(|e| AppError::io("Open DragonFly container for format detection", e))?;
    if !file
        .metadata()
        .map_err(|e| AppError::io("Inspect DragonFly container", e))?
        .is_file()
    {
        return Err(AppError::new("DragonFly containers must be regular files."));
    }
    let mut magic = [0; 6];
    match file.read_exact(&mut magic) {
        Ok(()) if magic == *b"LUKS\xba\xbe" => Ok(VolumeType::Luks1),
        Ok(()) => Ok(VolumeType::Veracrypt),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Ok(VolumeType::Veracrypt),
        Err(e) => Err(AppError::io("Read DragonFly container signature", e)),
    }
}

pub(in crate::bsd) fn backend() -> AppResult<PathBuf> {
    let executable = std::env::current_exe()
        .map_err(|e| AppError::io("Locate Lurker's private backend directory", e))?;
    let directory = executable
        .parent()
        .ok_or_else(|| AppError::new("Missing Lurker executable directory."))?;
    let wrapper = directory.join("../libexec/lurker/backends/lurker-tcplay");
    if !wrapper.is_file() {
        return Err(AppError::new(
            "DragonFly VeraCrypt requires the packaged lurker-tcplay wrapper.",
        ));
    }
    crate::trusted_privileged_executable(&wrapper)
}

pub(in crate::bsd) fn validate(
    credential: &Credential,
    source: Option<&Path>,
) -> AppResult<PathBuf> {
    if let Credential::Password(password) = credential {
        if password.len() > MAX_PASSWORD {
            return Err(AppError::new(
                "DragonFly VeraCrypt passwords must fit in 64 UTF-8 bytes.",
            ));
        }
    }
    if let Some(source) = source.filter(|path| path.exists()) {
        if super::attached_private_copy(source)?.is_some() {
            return Err(AppError::new(
                "Container is already attached to a DragonFly vnode.",
            ));
        }
    }
    let program = backend()?;
    if exec(
        program
            .to_str()
            .ok_or_else(|| AppError::new("Invalid private wrapper path."))?,
        &args(&["protocol"]),
        None,
    )?
    .trim()
        != "lurker-tcplay 1"
    {
        return Err(AppError::new(
            "Incompatible DragonFly VeraCrypt wrapper. Install a matching Lurker package.",
        ));
    }
    Ok(program)
}

fn run(
    registry: &Registry,
    program: &Path,
    operation: &str,
    backing: &str,
    parameters: &[&str],
    credential: &Credential,
    timeout: Duration,
) -> AppResult<String> {
    vnode_name(backing)?;
    let mut keyfile = Vec::new();
    let (kind, bytes) = match credential {
        Credential::Password(password) => ("password", password.as_slice()),
        Credential::Keyfile(path) => {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)
                .map_err(|e| AppError::io("Open VeraCrypt keyfile", e))?;
            if !file
                .metadata()
                .map_err(|e| AppError::io("Inspect VeraCrypt keyfile", e))?
                .is_file()
            {
                return Err(AppError::new(
                    "VeraCrypt requires a regular binary keyfile.",
                ));
            }
            if let Err(error) = file.take(MAX_KEYFILE).read_to_end(&mut keyfile) {
                keyfile.fill(0);
                return Err(AppError::io("Read VeraCrypt keyfile", error));
            }
            if keyfile.is_empty() {
                return Err(AppError::new("VeraCrypt requires a non-empty keyfile."));
            }
            ("keyfile", keyfile.as_slice())
        }
    };
    let mut command = args(&[operation, backing, kind]);
    command.extend(args(parameters));
    let result = exec_dragonfly_locked(registry, program, &command, Some(bytes), timeout);
    keyfile.fill(0);
    result
}

fn coordinates(session: &Session) -> AppResult<(String, String)> {
    let id = session.id.replace('-', "_");
    let name = format!("lurker_vc_{id}");
    mapping_name(&name)?;
    Ok((name, format!("LURKER-VERACRYPT-{id}")))
}

fn backing(session: &Session) -> AppResult<&str> {
    session
        .backing
        .as_deref()
        .ok_or_else(|| AppError::new("Missing DragonFly VeraCrypt vnode."))
}

fn require_crypt() -> AppResult<()> {
    let available = || -> AppResult<bool> {
        Ok(exec(DMSETUP, &args(&["targets"]), None)?
            .lines()
            .any(|line| line.split_whitespace().next() == Some("crypt")))
    };
    if !available()? {
        let loaded = exec("/sbin/kldload", &args(&["dm_target_crypt"]), None);
        if !available()? {
            return Err(AppError::new(format!(
                "DragonFly VeraCrypt requires the native dm_target_crypt module. {}",
                loaded.err().map(|error| error.message).unwrap_or_default()
            )));
        }
    }
    Ok(())
}

fn payload_sectors(bytes: u64) -> AppResult<u64> {
    bytes.checked_sub(2 * RESERVED)
        .filter(|payload| payload % SECTOR == 0)
        .map(|payload| payload / SECTOR)
        .filter(|&sectors| (1..=u64::from(u32::MAX)).contains(&sectors))
        .ok_or_else(|| AppError::new("DragonFly VeraCrypt requires a sector-aligned FAT32 image within the 32-bit sector limit."))
}

pub(in crate::bsd) fn validate_create_size(bytes: u64) -> AppResult<()> {
    if bytes < MIN_CREATE_MIB * MIB {
        return Err(AppError::new(format!(
            "DragonFly VeraCrypt requires at least {MIN_CREATE_MIB} MiB for aligned FAT32."
        )));
    }
    payload_sectors(bytes).map(|_| ())
}

pub(in crate::bsd) fn format(
    registry: &Registry,
    session: &Session,
    program: &Path,
    size: u64,
    cipher: CreateCipher,
    credential: &Credential,
) -> AppResult<()> {
    require_crypt()?;
    let cipher = crate::workflow::dragonfly_veracrypt_cipher(cipher);
    // Full initialization is proportional to the size of the image.
    let timeout = COMMAND_TIMEOUT + Duration::from_secs(size / MIB);
    if run(
        registry,
        program,
        "create",
        backing(session)?,
        &[&cipher],
        credential,
        timeout,
    )?
    .trim()
        != "created"
    {
        return Err(AppError::new(
            "Unexpected DragonFly VeraCrypt create response.",
        ));
    }
    Ok(())
}

pub(in crate::bsd) fn activate(
    registry: &Registry,
    session: &mut Session,
    program: &Path,
    credential: &Credential,
) -> AppResult<String> {
    require_crypt()?;
    let (name, uuid) = coordinates(session)?;
    let device = format!("/dev/mapper/{name}");
    if super::linear::has_mapping(&device)? {
        return Err(AppError::new(
            "DragonFly VeraCrypt mapper name is already in use.",
        ));
    }
    let bytes = fs::metadata(&session.source)
        .map_err(|e| AppError::io("Inspect VeraCrypt container size", e))?
        .len();
    let sectors = payload_sectors(bytes)?;
    session.dragonfly_veracrypt = Some(Mapping {
        sectors,
        removing: false,
    });
    registry.save(session)?;
    let response = run(
        registry,
        program,
        "map",
        backing(session)?,
        &[&name, &uuid],
        credential,
        COMMAND_TIMEOUT,
    )?;
    if response.trim() != format!("mapped {RESERVED} {}", sectors * SECTOR) {
        return Err(AppError::new(
            "Unexpected authenticated VeraCrypt payload geometry.",
        ));
    }
    verify_device(session, &device, sectors)?;
    Ok(device)
}

fn verify_device(session: &Session, device: &str, sectors: u64) -> AppResult<()> {
    let (name, uuid) = coordinates(session)?;
    let path = format!("/dev/mapper/{name}");
    let backing = backing(session)?;
    verify_backing(backing, &session.source)?;
    let info = exec(DMSETUP, &args(&["info", &name]), None)?;
    let uuid_matches = info
        .lines()
        .filter_map(|line| line.strip_prefix("UUID:").map(str::trim))
        .eq([uuid.as_str()]);
    // Only query identity, status and dependencies. dmsetup table can return keys.
    if device != path || !uuid_matches || super::mapping_sectors(device, backing)? != sectors {
        return Err(AppError::new(
            "DragonFly VeraCrypt mapping identity changed; preserving the session.",
        ));
    }
    Ok(())
}

pub(in crate::bsd) fn verify(session: &Session) -> AppResult<()> {
    if let Some(mapped) = &session.mapped {
        let mapping = session
            .dragonfly_veracrypt
            .as_ref()
            .ok_or_else(|| AppError::new("Missing DragonFly VeraCrypt mapper identity."))?;
        verify_device(session, mapped, mapping.sectors)?;
    }
    Ok(())
}

pub(in crate::bsd) fn reconcile(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(mapping) = &session.dragonfly_veracrypt else {
        return Ok(());
    };
    let (name, _) = coordinates(session)?;
    let device = format!("/dev/mapper/{name}");
    if super::linear::has_mapping(&device)? {
        verify_device(session, &device, mapping.sectors)?;
        if session.mapped.is_none() {
            session.mapped = Some(device.clone());
            registry.save(session)?;
        }
    } else if mapping.removing && session.mounted.is_none() {
        session.mapped = None;
        session.dragonfly_veracrypt = None;
        registry.save(session)?;
        return Ok(());
    } else if session.mapped.is_some() {
        return Err(AppError::new(
            "Recorded DragonFly VeraCrypt mapper disappeared; preserving the session.",
        ));
    }
    if session.mounted.is_none() && session.mapped.as_deref() == Some(device.as_str()) {
        if let Some(mounted) = mounts()?.into_iter().find(|mount| {
            session.intended_mountpoint.as_ref() == Some(&mount.target)
                && mount.source == Path::new(&device)
                && mount.filesystem == "msdos"
                && mount.readonly == session.readonly
        }) {
            session.mounted = Some(mounted);
            registry.save(session)?;
        }
    }
    Ok(())
}

pub(in crate::bsd) fn remove(registry: &Registry, session: &mut Session) -> AppResult<()> {
    verify(session)?;
    let (name, _) = coordinates(session)?;
    session.dragonfly_veracrypt.as_mut().unwrap().removing = true;
    registry.save(session)?;
    exec_dragonfly_locked(
        registry,
        Path::new(DMSETUP),
        &args(&["remove", &name]),
        None,
        COMMAND_TIMEOUT,
    )?;
    session.dragonfly_veracrypt = None;
    Ok(())
}

pub(in crate::bsd) fn make_filesystem(
    registry: &Registry,
    mapped: &str,
    size: u64,
) -> AppResult<()> {
    mapping_name(mapped)?;
    // Stock tcplay lost data on 512-byte clusters after a normal reopen.
    // Keep the data region and every cluster aligned to 4 KiB or larger.
    let cluster = match size / MIB {
        0..=8191 => "8",
        8192..=16383 => "16",
        16384..=32767 => "32",
        _ => "64",
    };
    let mut options = args(&["-A", "-F", "32", "-c", cluster, "-u", "63", "-h", "255"]);
    options.push(mapped.into());
    exec_dragonfly_locked(
        registry,
        Path::new("/sbin/newfs_msdos"),
        &options,
        None,
        COMMAND_TIMEOUT,
    )?;
    Ok(())
}

fn fat32(boot: &[u8; 512], payload: u64) -> bool {
    let short = |i| u64::from(u16::from_le_bytes([boot[i], boot[i + 1]]));
    let long = |i| {
        u64::from(u32::from_le_bytes([
            boot[i],
            boot[i + 1],
            boot[i + 2],
            boot[i + 3],
        ]))
    };
    let cluster = u64::from(boot[13]);
    let total = long(32);
    let reserved = short(14);
    let fat = long(36);
    let overhead = reserved + fat * u64::from(boot[16]);
    if boot[510..] != [0x55, 0xaa]
        || short(11) != SECTOR
        || !cluster.is_power_of_two()
        || !(ALIGNED_SECTORS..=128).contains(&cluster)
        || overhead % ALIGNED_SECTORS != 0
        || reserved == 0
        || !matches!(boot[16], 1 | 2)
        || fat == 0
        || short(17) != 0
        || short(19) != 0
        || short(22) != 0
        || short(42) != 0
        || total > payload
        || total <= overhead
    {
        return false;
    }
    let clusters = (total - overhead) / cluster;
    (65525..=0x0ffffff5).contains(&clusters)
        && fat * SECTOR / 4 >= clusters + 2
        && (2..=clusters + 1).contains(&long(44))
}

pub(in crate::bsd) fn mount(
    registry: &Registry,
    session: &Session,
    target: &Path,
    owner: Owner,
) -> AppResult<()> {
    verify(session)?;
    let mapped = session
        .mapped
        .as_deref()
        .ok_or_else(|| AppError::new("Missing VeraCrypt mapper for FAT32 mount."))?;
    let sectors = session.dragonfly_veracrypt.as_ref().unwrap().sectors;
    let mut boot = [0; 512];
    File::open(mapped)
        .and_then(|mut file| file.read_exact(&mut boot))
        .map_err(|e| AppError::io("Read authenticated VeraCrypt filesystem boot sector", e))?;
    if !fat32(&boot, sectors) {
        return Err(AppError::new(
            "DragonFly VeraCrypt requires FAT32 with 4 KiB or larger clusters and a 4 KiB-aligned data region.",
        ));
    }
    let uid = owner.uid.to_string();
    let gid = owner.gid.to_string();
    let mut options = args(&[
        "-u",
        &uid,
        "-g",
        &gid,
        "-m",
        "600",
        "-M",
        "700",
        "-o",
        "rw,sync,nosuid,nodev,noexec",
        mapped,
    ]);
    options.push(target.as_os_str().to_owned());
    exec_dragonfly_locked(
        registry,
        Path::new("/sbin/mount_msdos"),
        &options,
        None,
        COMMAND_TIMEOUT,
    )?;
    Ok(())
}

pub(in crate::bsd) fn verify_access(target: &Path, owner: Owner) -> AppResult<()> {
    let metadata =
        fs::metadata(target).map_err(|e| AppError::io("Inspect mounted FAT32 root", e))?;
    if metadata.uid() != owner.uid
        || metadata.gid() != owner.gid
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(AppError::new(
            "Mounted FAT32 root does not have the invoking user's private access permissions.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fat32;

    #[test]
    fn fat32_requires_four_kibibyte_clusters_and_data_alignment() {
        // Geometry produced by the DragonFly formatter for a 260 MiB vault.
        let mut boot = [0; 512];
        boot[11..13].copy_from_slice(&512_u16.to_le_bytes());
        boot[13] = 8;
        boot[14..16].copy_from_slice(&34_u16.to_le_bytes());
        boot[16] = 2;
        boot[32..36].copy_from_slice(&531_968_u32.to_le_bytes());
        boot[36..40].copy_from_slice(&519_u32.to_le_bytes());
        boot[44..48].copy_from_slice(&2_u32.to_le_bytes());
        boot[510..].copy_from_slice(&[0x55, 0xaa]);
        assert!(fat32(&boot, 531_968));
        assert!(!fat32(&boot, 531_967));

        boot[14..16].copy_from_slice(&35_u16.to_le_bytes());
        assert!(!fat32(&boot, 531_968));
        boot[13] = 1;
        // Recalculate the FAT lengths and the alignment so only the cluster
        // size makes this otherwise valid FAT32 format unusable.
        boot[14..16].copy_from_slice(&32_u16.to_le_bytes());
        boot[36..40].copy_from_slice(&4_096_u32.to_le_bytes());
        assert!(!fat32(&boot, 531_968));
    }
}
