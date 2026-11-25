//! Native OpenZFS inside an authenticated VeraCrypt virtual disk, never an
//! encryption backend. The caller must prove that `device` still belongs to
//! its private decrypted image before every operation and journal PoolIdentity
//! before import. Targets must be private, canonical directories protected
//! against concurrent replacement. On any error keep the crypto device and
//! journal: a successful create/import may have preceded the failure.
use super::{args, execute};
use crate::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, OsStr, OsString};
use std::fs::File;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct PoolIdentity {
    pub name: String,
    pub guid: String,
    pub vdev_guid: String,
}

fn tool(name: &str) -> AppResult<PathBuf> {
    let candidates = if name == "mount_zfs" {
        vec![
            PathBuf::from("/usr/local/zfs/bin/mount_zfs"),
            PathBuf::from("/Library/Filesystems/zfs.fs/Contents/Resources/mount_zfs"),
        ]
    } else {
        vec![Path::new("/usr/local/zfs/bin").join(name)]
    };
    for candidate in candidates {
        if candidate.exists() {
            return crate::trusted_privileged_executable(&candidate);
        }
    }
    Err(AppError::new(format!(
        "Native OpenZFS tool {name} is missing. Install and enable the free signed OpenZFSonOsX package."
    )))
}

fn output_path(program: &Path, arguments: &[OsString]) -> AppResult<std::process::Output> {
    let program = crate::trusted_privileged_executable(program)?;
    if crate::effective_uid() == 0 {
        execute(&program, arguments, None)
    } else {
        let sudo = crate::trusted_privileged_executable(Path::new("/usr/bin/sudo"))?;
        let mut elevated = args(&["-n", "--"]);
        elevated.push(program.into_os_string());
        elevated.extend_from_slice(arguments);
        execute(&sudo, &elevated, None)
    }
}

fn command_error(program: &Path, output: &std::process::Output) -> AppError {
    let detail = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    AppError::new(format!(
        "{} failed ({}): {}",
        program.display(),
        output.status,
        String::from_utf8_lossy(detail).trim()
    ))
}

fn run_path(program: &Path, arguments: &[OsString]) -> AppResult<String> {
    let output = output_path(program, arguments)?;
    if !output.status.success() {
        return Err(command_error(program, &output));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| AppError::new("Native OpenZFS returned non-UTF-8 metadata."))
}

fn run(name: &str, arguments: &[OsString]) -> AppResult<String> {
    run_path(&tool(name)?, arguments)
}

pub(super) fn available() -> AppResult<()> {
    for name in ["zpool", "zfs", "zdb", "mount_zfs"] {
        tool(name)?;
    }
    // This also diagnoses an installed but unapproved/unloaded kernel module.
    run("zpool", &args(&["list", "-H", "-o", "name,guid"]))?;
    Ok(())
}

fn decimal(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|number| number != 0)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
        && !matches!(
            name,
            "mirror" | "raidz" | "draid" | "spare" | "log" | "logs" | "cache" | "special" | "dedup"
        )
}

fn validate_identity(identity: &PoolIdentity) -> AppResult<()> {
    if !valid_name(&identity.name) || !decimal(&identity.guid) || !decimal(&identity.vdev_guid) {
        return Err(AppError::new("Invalid single-device ZFS identity."));
    }
    Ok(())
}

/// zdb may print all four labels, or one configuration and a label summary.
/// Every printed configuration must agree; stale device paths are deliberately
/// ignored because native import remaps the one recorded vdev GUID.
fn parse_labels(labels: &str) -> AppResult<Option<PoolIdentity>> {
    #[derive(Default)]
    struct Label<'a> {
        name: Option<&'a str>,
        pool: Option<&'a str>,
        top: Option<&'a str>,
        device: Option<&'a str>,
        leaf: Option<&'a str>,
        children: bool,
        disk: bool,
        tree_indent: Option<usize>,
        name_indent: usize,
    }
    impl Label<'_> {
        fn identity(&self) -> AppResult<PoolIdentity> {
            match (self.name, self.pool, self.top, self.device, self.leaf) {
                (Some(name), Some(pool), Some(top), Some(device), Some(leaf))
                    if self.children && self.disk && top == device && device == leaf =>
                {
                    let identity = PoolIdentity {
                        name: name.into(),
                        guid: pool.into(),
                        vdev_guid: leaf.into(),
                    };
                    validate_identity(&identity)?;
                    Ok(identity)
                }
                _ => Err(AppError::new(
                    "Incomplete or conflicting single-device ZFS labels.",
                )),
            }
        }
    }
    let duplicate = || AppError::new("Duplicate or misplaced ZFS label fields.");
    let mut current = Label::default();
    let mut identity = None;
    let finish = |label: &Label<'_>, expected: &mut Option<PoolIdentity>| -> AppResult<()> {
        if label.name.is_none() {
            return Ok(());
        }
        let found = label.identity()?;
        if expected.as_ref().is_some_and(|expected| expected != &found) {
            return Err(AppError::new("Ambiguous single-device ZFS labels."));
        }
        *expected = Some(found);
        Ok(())
    };
    for raw in labels.lines() {
        let line = raw.trim();
        let indent = raw.len() - raw.trim_start().len();
        if line.starts_with("LABEL ") {
            finish(&current, &mut identity)?;
            current = Label::default();
            continue;
        }
        if ["children", "spares", "l2cache"].iter().any(|key| {
            line.strip_prefix(key)
                .is_some_and(|rest| rest.starts_with('[') || rest.starts_with(':'))
        }) {
            return Err(AppError::new(
                "ZFS children, spare and cache devices are unsupported.",
            ));
        }
        if let Some(value) = line.strip_prefix("name:") {
            // Also accept zdb variants that print repeated configurations
            // without separate LABEL headings.
            finish(&current, &mut identity)?;
            current = Label::default();
            let value = value
                .trim()
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .ok_or_else(|| AppError::new("Invalid ZFS pool name label."))?;
            current.name = Some(value);
            current.name_indent = indent;
        }
        if let Some(value) = line.strip_prefix("pool_guid:") {
            if current.name.is_none()
                || indent != current.name_indent
                || current.pool.replace(value.trim()).is_some()
            {
                return Err(duplicate());
            }
        }
        if let Some(value) = line.strip_prefix("top_guid:") {
            if current.name.is_none()
                || indent != current.name_indent
                || current.top.replace(value.trim()).is_some()
            {
                return Err(duplicate());
            }
        }
        if let Some(value) = line.strip_prefix("guid:") {
            let slot = match current.tree_indent {
                Some(tree) if indent > tree => &mut current.leaf,
                None if current.name.is_some() && indent == current.name_indent => {
                    &mut current.device
                }
                _ => return Err(duplicate()),
            };
            if slot.replace(value.trim()).is_some() {
                return Err(duplicate());
            }
        }
        if let Some(value) = line.strip_prefix("vdev_children:") {
            if current.name.is_none()
                || indent != current.name_indent
                || value.trim() != "1"
                || current.children
            {
                return Err(AppError::new("ZFS requires exactly one vdev."));
            }
            current.children = true;
        }
        if line == "vdev_tree:" {
            if current.name.is_none()
                || indent != current.name_indent
                || current.tree_indent.replace(indent).is_some()
            {
                return Err(duplicate());
            }
        }
        if let Some(value) = line.strip_prefix("type:") {
            if value.trim() != "'disk'"
                || current.disk
                || !current.tree_indent.is_some_and(|tree| indent > tree)
            {
                return Err(AppError::new("ZFS requires one disk leaf."));
            }
            current.disk = true;
        }
    }
    finish(&current, &mut identity)?;
    Ok(identity)
}

pub(super) fn probe(device: &Path) -> AppResult<Option<PoolIdentity>> {
    let program = tool("zdb")?;
    let output = output_path(&program, &["-l".into(), device.as_os_str().into()])?;
    if !output.status.success() && output.status.code() != Some(2) {
        return Err(command_error(&program, &output));
    }
    let labels = String::from_utf8_lossy(&output.stdout);
    let identity = parse_labels(&labels)?;
    if identity.is_none() && !output.status.success() && !labels.contains("failed to unpack label")
    {
        return Err(command_error(&program, &output));
    }
    Ok(identity)
}

/// Select only the system entities explicitly attached to the expected private
/// image. Device-name prefixes never establish an image or partition identity.
fn associated_devices(
    attached: &[super::Image],
    parent: &Path,
    image: &Path,
) -> AppResult<Vec<PathBuf>> {
    let mut matches = attached
        .iter()
        .filter(|attached| super::same_path(&attached.path, image));
    let selected = matches
        .next()
        .ok_or_else(|| AppError::new("The expected VeraCrypt image is no longer attached."))?;
    if matches.next().is_some()
        || !selected
            .entities
            .iter()
            .any(|entity| entity.device == parent)
    {
        return Err(AppError::new(
            "Ambiguous image or missing authenticated virtual parent device.",
        ));
    }
    let mut devices: Vec<_> = selected
        .entities
        .iter()
        .map(|entity| entity.device.clone())
        .collect();
    devices.sort();
    if devices.is_empty() || devices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(AppError::new("Duplicate or missing virtual disk entities."));
    }
    for device in &devices {
        if device.parent() != Some(Path::new("/dev"))
            || attached.iter().any(|other| {
                !std::ptr::eq(other, selected)
                    && other.entities.iter().any(|entity| &entity.device == device)
            })
        {
            return Err(AppError::new(
                "A virtual device is ambiguous or outside the expected image.",
            ));
        }
    }
    Ok(devices)
}

#[derive(Deserialize)]
struct DiskInfo {
    #[serde(rename = "DeviceIdentifier")]
    identifier: String,
    #[serde(rename = "DeviceNode")]
    node: PathBuf,
    #[serde(rename = "ParentWholeDisk")]
    parent: Option<String>,
    #[serde(rename = "WholeDisk")]
    whole: bool,
}

fn disk_info(device: &Path) -> AppResult<DiskInfo> {
    let metadata = std::fs::symlink_metadata(device)
        .map_err(|error| AppError::io("Inspect associated virtual device", error))?;
    if !metadata.file_type().is_block_device() {
        return Err(AppError::new(
            "An associated virtual device is not a block device.",
        ));
    }
    let program = crate::trusted_privileged_executable(Path::new("/usr/sbin/diskutil"))?;
    let output = super::checked(
        &program,
        &["info".into(), "-plist".into(), device.as_os_str().into()],
        None,
    )?;
    let info: DiskInfo = super::decode(&output.stdout)?;
    if info.node != device || device.file_name() != Some(OsStr::new(&info.identifier)) {
        return Err(AppError::new(
            "Native virtual disk metadata identifies a different device.",
        ));
    }
    Ok(info)
}

fn verify_device_family(parent: &Path, devices: &[PathBuf]) -> AppResult<()> {
    let info = disk_info(parent)?;
    if !info.whole {
        return Err(AppError::new(
            "The authenticated virtual parent is not a whole disk.",
        ));
    }
    for device in devices.iter().filter(|device| device.as_path() != parent) {
        let child = disk_info(device)?;
        if child.whole || child.parent.as_deref() != Some(info.identifier.as_str()) {
            return Err(AppError::new(
                "An image entity does not belong to its authenticated parent disk.",
            ));
        }
    }
    Ok(())
}

/// Resolve OpenZFS's GPT data partition using only the expected image's native
/// attachment metadata. The caller must authenticate `image` and `parent` first
/// and retain the recorded image inode while calling this resolver.
pub(super) fn resolve_device(
    parent: &Path,
    image: &Path,
) -> AppResult<Option<(PathBuf, PoolIdentity)>> {
    let devices = associated_devices(&super::images()?, parent, image)?;
    verify_device_family(parent, &devices)?;
    let mut selected = None;
    for device in &devices {
        if let Some(identity) = probe(device)? {
            if selected.replace((device.clone(), identity)).is_some() {
                return Err(AppError::new(
                    "More than one ZFS device is associated with this image; refusing an ambiguous pool.",
                ));
            }
        }
    }
    // Recheck native associations after probing so a reattachment or partition
    // change cannot silently turn an earlier device number into our session.
    if associated_devices(&super::images()?, parent, image)? != devices {
        return Err(AppError::new(
            "The authenticated image's virtual devices changed during ZFS discovery.",
        ));
    }
    verify_device_family(parent, &devices)?;
    Ok(selected)
}

fn imported(identity: &PoolIdentity) -> AppResult<bool> {
    validate_identity(identity)?;
    let listing = run("zpool", &args(&["list", "-H", "-p", "-o", "name,guid"]))?;
    let mut matched = false;
    for row in listing.lines().filter(|row| !row.trim().is_empty()) {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() != 2 || !decimal(fields[1]) {
            return Err(AppError::new("Invalid imported ZFS pool inventory."));
        }
        if fields[0] == identity.name || fields[1] == identity.guid {
            if fields[0] != identity.name || fields[1] != identity.guid || matched {
                return Err(AppError::new(
                    "ZFS pool name or GUID conflicts with an imported pool.",
                ));
            }
            matched = true;
        }
    }
    Ok(matched)
}

fn status_leaf(status: &str, pool: &str) -> AppResult<String> {
    let mut config = false;
    let mut entries = Vec::new();
    for line in status.lines().map(str::trim) {
        if line == "config:" {
            config = true;
            continue;
        }
        if line.starts_with("errors:") {
            break;
        }
        if !config || line.is_empty() || line.starts_with("NAME ") {
            continue;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 5 || fields[1] != "ONLINE" {
            return Err(AppError::new("ZFS pool has extra or unhealthy vdevs."));
        }
        entries.push(fields[0]);
    }
    if entries.len() != 2 || entries[0] != pool {
        return Err(AppError::new(
            "ZFS pool is not a strict single-device pool.",
        ));
    }
    Ok(entries[1].into())
}

fn verify_pool(device: &Path, identity: &PoolIdentity, readonly: Option<bool>) -> AppResult<()> {
    if !imported(identity)? {
        return Err(AppError::new("The recorded ZFS pool is not imported."));
    }
    let status = run("zpool", &args(&["status", "-P", &identity.name]))?;
    if Path::new(&status_leaf(&status, &identity.name)?) != device {
        return Err(AppError::new(
            "ZFS vdev is not the authenticated decrypted device.",
        ));
    }
    let status = run("zpool", &args(&["status", "-g", &identity.name]))?;
    if status_leaf(&status, &identity.name)? != identity.vdev_guid {
        return Err(AppError::new("ZFS vdev GUID changed."));
    }
    let properties = run(
        "zpool",
        &args(&[
            "get",
            "-H",
            "-p",
            "-o",
            "property,value",
            "guid,cachefile,readonly",
            &identity.name,
        ]),
    )?;
    let mut values = HashMap::new();
    for line in properties.lines() {
        let (key, value) = line
            .split_once('\t')
            .ok_or_else(|| AppError::new("Invalid ZFS properties."))?;
        if values.insert(key, value).is_some() {
            return Err(AppError::new("Duplicate ZFS properties."));
        }
    }
    if values.get("guid") != Some(&identity.guid.as_str())
        || values.get("cachefile") != Some(&"none")
        || !matches!(values.get("readonly"), Some(&"on") | Some(&"off"))
        || readonly.is_some_and(|ro| values.get("readonly") != Some(&if ro { "on" } else { "off" }))
    {
        return Err(AppError::new(
            "ZFS import identity, cache or read-only policy changed.",
        ));
    }
    let datasets = run(
        "zfs",
        &args(&[
            "list",
            "-H",
            "-o",
            "name,type,mountpoint",
            "-r",
            "-t",
            "filesystem,volume",
            &identity.name,
        ]),
    )?;
    let expected = format!("{}\tfilesystem\tlegacy", identity.name);
    if datasets.trim_end() != expected {
        return Err(AppError::new(
            "Only a private ZFS root dataset with mountpoint=legacy is supported.",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct Mounted {
    source: String,
    target: PathBuf,
    readonly: bool,
    zfs: bool,
}

fn mount_bindings(listing: &str) -> AppResult<HashMap<PathBuf, String>> {
    let mut bindings = HashMap::new();
    let mut datasets = HashSet::new();
    for line in listing.lines().filter(|line| !line.is_empty()) {
        let (dataset, target) = line
            .split_once(char::is_whitespace)
            .ok_or_else(|| AppError::new("Invalid native ZFS mount binding."))?;
        // zfs mount pads the first column, but does not split or escape spaces
        // within the absolute target. Preserve those bytes, including trailing
        // spaces, rather than tokenizing or trimming the whole output line.
        let target = PathBuf::from(target.trim_start_matches(char::is_whitespace));
        if dataset.is_empty()
            || !target.is_absolute()
            || !datasets.insert(dataset)
            || bindings.insert(target, dataset.to_owned()).is_some()
        {
            return Err(AppError::new(
                "Duplicate or ambiguous native ZFS mount bindings.",
            ));
        }
    }
    Ok(bindings)
}

fn mounts() -> AppResult<Vec<Mounted>> {
    // macOS exposes synthetic /dev/diskNsM dataset devices in getmntinfo, not
    // dataset names. Bind the native table to OpenZFS's own dataset inventory
    // through exact mount targets before making any pool cleanup decision.
    let bindings = mount_bindings(&run("zfs", &args(&["mount"]))?)?;
    let mut seen = HashSet::new();
    let mut table: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo supplies count initialized statfs entries. Copy every
    // string before making another native mount-table call.
    let count = unsafe { libc::getmntinfo(&mut table, libc::MNT_NOWAIT) };
    if count <= 0 || table.is_null() {
        return Err(AppError::io(
            "Read native mount table",
            std::io::Error::last_os_error(),
        ));
    }
    let table = unsafe { std::slice::from_raw_parts(table, count as usize) };
    let mounted = table
        .iter()
        .map(|entry| {
            let mut source = unsafe { CStr::from_ptr(entry.f_mntfromname.as_ptr()) }
                .to_str()
                .map_err(|_| AppError::new("Non-UTF-8 filesystem source."))?
                .to_owned();
            let target = PathBuf::from(OsStr::from_bytes(
                unsafe { CStr::from_ptr(entry.f_mntonname.as_ptr()) }.to_bytes(),
            ));
            let zfs = unsafe { CStr::from_ptr(entry.f_fstypename.as_ptr()) }.to_bytes() == b"zfs";
            if zfs {
                let dataset = bindings.get(&target).ok_or_else(|| {
                    AppError::new(
                        "Native ZFS mount has no exact dataset binding; preserving all pools.",
                    )
                })?;
                let native_device = Path::new(&source).parent() == Some(Path::new("/dev"));
                if !seen.insert(target.clone()) || (!native_device && source != *dataset) {
                    return Err(AppError::new(
                        "Native ZFS mount identity is ambiguous; preserving all pools.",
                    ));
                }
                source.clone_from(dataset);
            }
            Ok(Mounted {
                source,
                target,
                readonly: entry.f_flags & libc::MNT_RDONLY as u32 != 0,
                zfs,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    if seen.len() != bindings.len() {
        return Err(AppError::new(
            "ZFS dataset bindings changed or are missing from the native mount table.",
        ));
    }
    Ok(mounted)
}

fn pool_source(source: &str, identity: &PoolIdentity) -> bool {
    source == identity.name
        || source
            .strip_prefix(&identity.name)
            .is_some_and(|suffix| suffix.starts_with('/') || suffix.starts_with('@'))
}

fn mounted_root(identity: &PoolIdentity, target: &Path, readonly: Option<bool>) -> AppResult<bool> {
    let mut found = false;
    for mount in mounts()? {
        let pool = mount.zfs && pool_source(&mount.source, identity);
        let target_tree = mount.target == target || mount.target.starts_with(target);
        if !pool && !target_tree {
            continue;
        }
        if mount.source != identity.name
            || mount.target != target
            || !mount.zfs
            || found
            || readonly.is_some_and(|readonly| readonly != mount.readonly)
        {
            return Err(AppError::new(
                "Unexpected or additional mounts; preserving the ZFS pool.",
            ));
        }
        found = true;
    }
    Ok(found)
}

/// Confirm both the recorded sole vdev/GUID and the exact native root mount.
/// This also rejects any additional dataset or mount within the target tree.
pub(super) fn verify_mounted(
    device: &Path,
    identity: &PoolIdentity,
    target: &Path,
    readonly: Option<bool>,
) -> AppResult<bool> {
    verify_pool(device, identity, readonly)?;
    mounted_root(identity, target, readonly)
}

fn export(device: &Path, identity: &PoolIdentity) -> AppResult<()> {
    verify_pool(device, identity, None)?;
    if mounts()?
        .iter()
        .any(|mount| mount.zfs && pool_source(&mount.source, identity))
    {
        return Err(AppError::new(
            "ZFS datasets remain mounted; refusing export.",
        ));
    }
    // zpool export accepts names. Resolve and recheck the recorded GUID directly
    // above; never export a name found only in mutable on-disk labels.
    run("zpool", &args(&["export", &identity.name]))?;
    if imported(identity)? {
        return Err(AppError::new("ZFS pool remains imported."));
    }
    Ok(())
}

/// Creates only the native inner filesystem. If a later step fails the caller
/// must recover its identity with resolve_device and retain the image's native
/// attachment until the exact pool has been exported.
pub(super) fn create(parent: &Path, image: &Path) -> AppResult<(PathBuf, PoolIdentity)> {
    available()?;
    let devices = associated_devices(&super::images()?, parent, image)?;
    verify_device_family(parent, &devices)?;
    let mut random = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|error| AppError::io("Generate private ZFS pool identity", error))?;
    let name = format!(
        "lurker_{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let mut arguments = args(&["create", "-m", "legacy", "-o", "cachefile=none", &name]);
    arguments.push(parent.as_os_str().into());
    run("zpool", &arguments)?;
    let (device, identity) = resolve_device(parent, image)?.ok_or_else(|| {
        AppError::new("Created ZFS pool has no recognizable labels; retain its virtual device.")
    })?;
    if identity.name != name {
        return Err(AppError::new(
            "Created ZFS pool identity differs; retain its virtual device.",
        ));
    }
    export(&device, &identity)?;
    Ok((device, identity))
}

pub(super) fn mount(
    device: &Path,
    identity: &PoolIdentity,
    target: &Path,
    readonly: bool,
) -> AppResult<()> {
    validate_identity(identity)?;
    if probe(device)?.as_ref() != Some(identity) {
        return Err(AppError::new("ZFS device labels changed."));
    }
    if imported(identity)? {
        return Err(AppError::new(
            "This ZFS pool is already imported; refusing a duplicate session.",
        ));
    }
    if mounted_root(identity, target, None)? {
        return Err(AppError::new("ZFS mount already exists."));
    }
    let mut arguments = args(&["import", "-N", "-d"]);
    arguments.push(device.as_os_str().into());
    arguments.extend(args(&[
        "-o",
        "cachefile=none",
        "-o",
        if readonly {
            "readonly=on"
        } else {
            "readonly=off"
        },
        &identity.guid,
    ]));
    run("zpool", &arguments)?;
    verify_pool(device, identity, Some(readonly))?;
    let mut arguments = args(&["-o", if readonly { "ro" } else { "rw" }, &identity.name]);
    arguments.push(target.as_os_str().into());
    run("mount_zfs", &arguments)?;
    if !mounted_root(identity, target, Some(readonly))? {
        return Err(AppError::new("ZFS root mount was not published."));
    }
    Ok(())
}

/// Call only for a newly created filesystem, after persisting its identity.
/// Existing filesystem ownership must never be changed on unlock.
pub(super) fn initialize(device: &Path, identity: &PoolIdentity, target: &Path) -> AppResult<()> {
    let uid = crate::effective_uid();
    if uid == 0 {
        return Err(AppError::new(
            "New macOS ZFS filesystems must belong to the login user; run Lurker without sudo.",
        ));
    }
    // Kernel group identity accompanies the login uid; environment variables
    // cannot select ownership for the scoped elevated chown below.
    let gid = unsafe { libc::getegid() };
    mount(device, identity, target, false)?;
    verify_pool(device, identity, Some(false))?;
    if !mounted_root(identity, target, Some(false))? {
        return Err(AppError::new("New ZFS root disappeared."));
    }
    run_path(
        Path::new("/usr/sbin/chown"),
        &[format!("{uid}:{gid}").into(), target.as_os_str().into()],
    )?;
    if !mounted_root(identity, target, Some(false))? {
        return Err(AppError::new("New ZFS root changed."));
    }
    run_path(
        Path::new("/bin/chmod"),
        &["0700".into(), target.as_os_str().into()],
    )?;
    if !mounted_root(identity, target, Some(false))? {
        return Err(AppError::new("New ZFS root changed during initialization."));
    }
    let metadata = std::fs::symlink_metadata(target)
        .map_err(|error| AppError::io("Verify new ZFS root ownership", error))?;
    if !metadata.is_dir()
        || metadata.uid() != uid
        || metadata.gid() != gid
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(AppError::new(
            "New ZFS root is not private to the invoking user.",
        ));
    }
    close(device, identity, target)
}

pub(super) fn close(device: &Path, identity: &PoolIdentity, target: &Path) -> AppResult<()> {
    if probe(device)?.as_ref() != Some(identity) {
        return Err(AppError::new("ZFS labels no longer match the session."));
    }
    let mounted = mounted_root(identity, target, None)?;
    if !imported(identity)? {
        return if mounted {
            Err(AppError::new("ZFS mount exists without its recorded pool."))
        } else {
            Ok(())
        };
    }
    verify_pool(device, identity, None)?;
    if mounted {
        run_path(
            Path::new("/usr/sbin/diskutil"),
            &["unmount".into(), target.as_os_str().into()],
        )?;
        if mounted_root(identity, target, None)? {
            return Err(AppError::new("ZFS filesystem remains mounted."));
        }
    }
    export(device, identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_dataset_bindings_preserve_paths_and_reject_ambiguity() {
        let bindings = mount_bindings("tank                 /private/zfs initialize/root with spaces\notherpool/data       /Volumes/another root \n").unwrap();
        assert_eq!(
            bindings
                .get(Path::new("/private/zfs initialize/root with spaces"))
                .map(String::as_str),
            Some("tank")
        );
        assert_eq!(
            bindings
                .get(Path::new("/Volumes/another root "))
                .map(String::as_str),
            Some("otherpool/data")
        );
        assert!(mount_bindings("tank /Volumes/root\notherpool /Volumes/root\n").is_err());
        assert!(mount_bindings("tank /Volumes/root\ntank /Volumes/another\n").is_err());
        assert!(mount_bindings("tank relative-target\n").is_err());
    }
    fn attached_image(path: &str, devices: &[&str]) -> super::super::Image {
        super::super::Image {
            path: path.into(),
            encrypted: false,
            entities: devices
                .iter()
                .map(|device| super::super::Entity {
                    device: (*device).into(),
                    ..Default::default()
                })
                .collect(),
            writable: true,
            owner: Some(501),
        }
    }
    #[test]
    fn discovery_candidates_are_scoped_to_one_exact_attached_image() {
        let expected = attached_image(
            "/private/owned/volume.dmg",
            &["/dev/disk4", "/dev/disk4s1", "/dev/disk4s9"],
        );
        let unrelated = attached_image(
            "/private/unrelated/volume.dmg",
            &["/dev/disk40", "/dev/disk40s1"],
        );
        let attached = [expected, unrelated];
        assert_eq!(
            associated_devices(
                &attached,
                Path::new("/dev/disk4"),
                Path::new("/private/owned/volume.dmg")
            )
            .unwrap(),
            vec![
                PathBuf::from("/dev/disk4"),
                PathBuf::from("/dev/disk4s1"),
                PathBuf::from("/dev/disk4s9")
            ]
        );
        assert!(
            associated_devices(
                &attached,
                Path::new("/dev/disk40"),
                Path::new("/private/owned/volume.dmg")
            )
            .is_err()
        );
        assert!(
            associated_devices(
                &attached,
                Path::new("/dev/disk4"),
                Path::new("/private/missing/volume.dmg")
            )
            .is_err()
        );
        let shared_device = [
            attached_image("/private/owned/volume.dmg", &["/dev/disk4", "/dev/disk4s1"]),
            attached_image("/private/unrelated/volume.dmg", &["/dev/disk4s1"]),
        ];
        assert!(
            associated_devices(
                &shared_device,
                Path::new("/dev/disk4"),
                Path::new("/private/owned/volume.dmg")
            )
            .is_err()
        );
    }
    fn label() -> &'static str {
        // Real zdb -l field layout, with synthetic host/device/identity values.
        // Both the config and its disk vdev carry a GUID; a single disk has
        // config guid == top_guid == vdev_tree.guid.
        // https://github.com/openzfs/zfs/issues/3425
        r#"--------------------------------------------
LABEL 0
--------------------------------------------
    version: 5000
    name: 'lurker_test'
    state: 0
    txg: 24
    pool_guid: 123
    errata: 0
    hostid: 42
    hostname: 'testhost'
    top_guid: 456
    guid: 456
    vdev_children: 1
    vdev_tree:
        type: 'disk'
        id: 0
        guid: 456
        path: '/dev/disk42'
        whole_disk: 0
        metaslab_array: 35
        metaslab_shift: 29
        ashift: 12
        asize: 536870912
        is_log: 0
        create_txg: 4
    features_for_read:
        com.delphix:hole_birth
        com.delphix:embedded_data
"#
    }
    #[test]
    fn label_identity_rejects_conflicting_or_multiple_devices() {
        let identity = parse_labels(label()).unwrap().unwrap();
        assert_eq!(identity.guid, "123");
        assert_eq!(
            parse_labels(&format!("{}{}", label(), label())).unwrap(),
            Some(identity)
        );
        assert!(parse_labels(&format!("{}{}", label(), label().replace("123", "789"))).is_err());
        for changed in [
            label().replace("children: 1", "children: 2"),
            label().replace("'disk'", "'mirror'"),
            label().replace("top_guid: 456", "top_guid: 789"),
            label().replace("pool_guid: 123\n", ""),
        ] {
            assert!(parse_labels(&changed).is_err());
        }
        for extra in ["children[0]:", "spares[0]:", "l2cache[0]:"] {
            assert!(parse_labels(&format!("{}{extra}\n", label())).is_err());
        }
        assert!(parse_labels("failed to unpack label 0").unwrap().is_none());
    }
    #[test]
    fn each_label_requires_both_device_and_leaf_guids() {
        // Each missing or duplicated field fails within its own label; another
        // complete label cannot fill its gap or hide its disagreement.
        for changed in [
            label().replace("\n    guid: 456\n", "\n"),
            label().replace("        guid: 456\n", ""),
            label().replace("        guid: 456\n", "        guid: 789\n"),
            label().replace("\n    guid: 456\n", "\n    guid: 456\n    guid: 456\n"),
        ] {
            assert!(parse_labels(&changed).is_err());
            assert!(parse_labels(&format!("{}{changed}", label())).is_err());
        }
    }
    #[test]
    fn imports_accept_safe_foreign_names_without_treating_names_as_identity() {
        for name in ["tank", "user-data", "Volume.2", "pool:backup"] {
            let identity = parse_labels(&label().replace("lurker_test", name))
                .unwrap()
                .unwrap();
            assert_eq!(identity.name, name);
            assert_eq!(identity.guid, "123");
            assert_eq!(identity.vdev_guid, "456");
        }
        for name in [
            "-a",
            "1tank",
            "pool/child",
            "tank@snapshot",
            "tank space",
            "mirror",
            "",
        ] {
            assert!(parse_labels(&label().replace("lurker_test", name)).is_err());
        }
    }
    #[test]
    fn status_requires_exactly_one_online_leaf() {
        let status = "pool: lurker_test\nconfig:\n NAME STATE READ WRITE CKSUM\n lurker_test ONLINE 0 0 0\n /dev/disk42 ONLINE 0 0 0\nerrors: No known data errors\n";
        assert_eq!(status_leaf(status, "lurker_test").unwrap(), "/dev/disk42");
        assert!(
            status_leaf(
                &status.replace("errors:", " /dev/disk43 ONLINE 0 0 0\nerrors:"),
                "lurker_test"
            )
            .is_err()
        );
        assert!(
            status_leaf(
                &status.replace("/dev/disk42 ONLINE", "/dev/disk42 OFFLINE"),
                "lurker_test"
            )
            .is_err()
        );
        assert!(status_leaf(&status.replace("errors:", "cache\nerrors:"), "lurker_test").is_err());
    }
}
