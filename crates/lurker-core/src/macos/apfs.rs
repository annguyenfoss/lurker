//! Native APFS data volumes, optionally carried by a plain disk image.
use super::*;

#[derive(Deserialize)]
struct Containers {
    #[serde(rename = "Containers", default)]
    containers: Vec<Container>,
}
#[derive(Deserialize)]
struct Container {
    #[serde(rename = "ContainerReference")]
    reference: String,
    #[serde(rename = "PhysicalStores", default)]
    stores: Vec<Store>,
    #[serde(rename = "Volumes", default)]
    volumes: Vec<ApfsVolume>,
}
#[derive(Deserialize)]
struct Store {
    #[serde(rename = "DeviceIdentifier")]
    device: String,
}
#[derive(Deserialize)]
struct ApfsVolume {
    #[serde(rename = "DeviceIdentifier")]
    device: String,
    #[serde(rename = "APFSVolumeUUID")]
    uuid: String,
    #[serde(rename = "FileVault", default)]
    encrypted: bool,
    #[serde(rename = "Locked", default)]
    locked: bool,
    #[serde(rename = "Roles", default)]
    roles: Vec<String>,
}
#[derive(Deserialize)]
struct DiskInfo {
    #[serde(rename = "DeviceIdentifier")]
    device: String,
    #[serde(rename = "WholeDisk")]
    whole: bool,
    #[serde(rename = "Internal", default)]
    internal: bool,
    #[serde(rename = "MountPoint")]
    mountpoint: Option<PathBuf>,
}
fn disk(arguments: &[OsString], input: Option<&[u8]>) -> AppResult<ProcessOutput> {
    checked(Path::new(DISKUTIL), arguments, input)
}
fn info(path: &Path) -> AppResult<DiskInfo> {
    decode(&disk(&["info".into(), "-plist".into(), path.into()], None)?.stdout)
}
fn containers() -> AppResult<Vec<Container>> {
    Ok(decode::<Containers>(&disk(&args(&["apfs", "list", "-plist"]), None)?.stdout)?.containers)
}
fn device(name: &str) -> PathBuf {
    PathBuf::from(format!("/dev/{name}"))
}
fn image_owns(image: &Image, container: &Container) -> bool {
    container.stores.iter().any(|store| {
        image
            .entities
            .iter()
            .any(|e| e.device == device(&store.device))
    })
}
fn attach(source: &Path, readonly: bool) -> AppResult<Image> {
    if images()?.iter().any(|i| same_path(&i.path, source)) {
        return Err(AppError::new(
            "Disk image is already attached. Close it before opening again.",
        ));
    }
    let mut a = args(&["attach", "-nomount", "-nobrowse", "-plist"]);
    if readonly {
        a.push("-readonly".into());
    }
    a.push(source.into());
    checked(Path::new(HDIUTIL), &a, None)?;
    images()?
        .into_iter()
        .find(|i| same_path(&i.path, source))
        .ok_or_else(|| AppError::new("Attached APFS image was not found."))
}
fn detach_image(image: &Image) -> AppResult<()> {
    let target = image
        .entities
        .first()
        .ok_or_else(|| AppError::new("Image has no device."))?;
    checked(
        Path::new(HDIUTIL),
        &["detach".into(), target.device.clone().into()],
        None,
    )?;
    Ok(())
}

pub(super) fn detects(source: &Path) -> AppResult<bool> {
    if source.starts_with("/dev") {
        return Ok(true);
    }
    let result = execute(
        Path::new(HDIUTIL),
        &[
            "imageinfo".into(),
            "-plist".into(),
            "-stdinpass".into(),
            source.into(),
        ],
        None,
    )?;
    if !result.status.success() {
        return Ok(false);
    }
    let tree: plist::Value = decode(&result.stdout)?;
    Ok(tree
        .as_dictionary()
        .and_then(|d| d.get("partitions"))
        .and_then(|v| v.as_dictionary())
        .and_then(|d| d.get("partitions"))
        .and_then(|v| v.as_array())
        .is_some_and(|parts| {
            parts.iter().any(|p| {
                p.as_dictionary()
                    .and_then(|d| d.get("partition-hint"))
                    .and_then(|v| v.as_string())
                    == Some("Apple_APFS")
            })
        }))
}

pub(super) fn list() -> AppResult<Vec<Volume>> {
    let attached = images()?;
    let mut result = Vec::new();
    for container in containers()? {
        let image = attached.iter().find(|i| image_owns(i, &container));
        if image.is_some_and(|i| i.owner.is_some_and(|uid| uid != crate::effective_uid())) {
            continue;
        }
        for v in container
            .volumes
            .iter()
            .filter(|v| v.encrypted && !v.locked && v.roles.is_empty())
        {
            let path = device(&v.device);
            let details = info(&path)?;
            result.push(Volume {
                source: image
                    .map(|i| i.path.clone())
                    .unwrap_or_else(|| path.clone()),
                devices: std::iter::once(path)
                    .chain(container.stores.iter().map(|s| device(&s.device)))
                    .collect(),
                mountpoint: details.mountpoint.filter(|p| !p.as_os_str().is_empty()),
                kind: VolumeType::Apfs,
                readonly: image.is_some_and(|i| !i.writable),
                filesystem: Some("apfs".into()),
            });
        }
    }
    Ok(result)
}

pub(super) fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    if c.cipher != CreateCipher::Aes || c.keyfile.is_some() {
        return Err(AppError::new(
            "APFS uses its native encryption profile and accepts a password only.",
        ));
    }
    let mut pass = secret(c.passphrase.as_deref(), true)?;
    pass.push(b'\n');
    if c.source_kind == SourceKind::Block {
        if !c.force {
            return Err(AppError::new(
                "APFS partition creation erases data and requires --force.",
            ));
        }
        initialize_partition(&c.target, &pass)?;
    } else if c.source_kind == SourceKind::File {
        let size = size_gb_to_mib(
            c.size_gb
                .as_deref()
                .ok_or_else(|| AppError::new("File container size is required."))?,
        )?;
        if size < 32 {
            return Err(AppError::new("APFS containers must be at least 32 MiB."));
        }
        let target = target_path(&c.target)?;
        assert_target(&target, c.force)?;
        if images()?.iter().any(|i| same_path(&i.path, &target)) {
            return Err(AppError::new("Refusing to overwrite an attached image."));
        }
        let staging = Staging::new(target.parent().unwrap())?;
        let temp = staging.0.join("container.dmg");
        let mut a = args(&[
            "create",
            "-size",
            &format!("{size}m"),
            "-type",
            "UDIF",
            "-layout",
            "GPTSPUD",
            "-fs",
            "APFS",
        ]);
        a.push(temp.clone().into());
        checked(Path::new(HDIUTIL), &a, None)?;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
            .map_err(|e| AppError::io("Protect APFS image", e))?;
        let image = attach(&temp, false)?;
        let initialized = (|| {
            let container = containers()?
                .into_iter()
                .find(|c| image_owns(&image, c))
                .ok_or_else(|| AppError::new("Image has no APFS container."))?;
            // Remove the empty formatter-created volume, then create encrypted from birth.
            for v in &container.volumes {
                disk(
                    &[
                        "apfs".into(),
                        "deleteVolume".into(),
                        device(&v.device).into(),
                    ],
                    None,
                )?;
            }
            add_volume(&container.reference, &pass)
        })();
        let detached = detach_image(&image);
        if let Err(error) = detached {
            // Keep an attached staging file addressable for an explicit close.
            std::mem::forget(staging);
            return Err(error);
        }
        initialized?;
        OpenOptions::new()
            .write(true)
            .open(&temp)
            .and_then(|f| f.sync_all())
            .map_err(|e| AppError::io("Sync APFS image", e))?;
        assert_target(&target, c.force)?;
        publish(&temp, &target, c.force)?;
    } else {
        return Err(AppError::new(
            "APFS creation requires a file or data partition.",
        ));
    }
    output.msg2("Created APFS encrypted volume");
    Ok(())
}
fn add_volume(container: &str, pass: &[u8]) -> AppResult<()> {
    disk(
        &args(&[
            "apfs",
            "addVolume",
            container,
            "APFS",
            "Lurker",
            "-stdinpassphrase",
            "-nomount",
        ]),
        Some(pass),
    )?;
    Ok(())
}
fn initialize_partition(target: &Path, pass: &[u8]) -> AppResult<()> {
    let details = info(target)?;
    if details.whole
        || details.internal
        || details
            .mountpoint
            .as_ref()
            .is_some_and(|p| !p.as_os_str().is_empty())
    {
        return Err(AppError::new("Select an unmounted external data partition. Whole disks and internal disks are refused."));
    }
    // Never initialize an existing APFS container whose other volumes may be mounted.
    if containers()?
        .iter()
        .any(|c| c.stores.iter().any(|s| s.device == details.device))
    {
        return Err(AppError::new(
            "Partition already contains an APFS container.",
        ));
    }
    disk(
        &["apfs".into(), "createContainer".into(), target.into()],
        None,
    )?;
    let container = containers()?
        .into_iter()
        .find(|c| c.stores.iter().any(|s| s.device == details.device))
        .ok_or_else(|| AppError::new("New APFS container was not found."))?;
    add_volume(&container.reference, pass)?;
    let created = containers()?
        .into_iter()
        .find(|c| c.reference == container.reference)
        .and_then(|c| c.volumes.into_iter().find(|v| v.encrypted))
        .ok_or_else(|| AppError::new("New encrypted volume was not found."))?;
    if !created.locked {
        disk(&args(&["apfs", "lockVolume", &created.uuid]), None)?;
    }
    Ok(())
}

pub(super) fn mount(c: MountCommand, output: Output) -> AppResult<()> {
    require_no_tag(c.tag.as_deref())?;
    if c.keyfile.is_some() {
        return Err(AppError::new("APFS accepts a password only."));
    }
    let source = existing(&c.source)?;
    let mountpoint = target_path(&c.mountpoint)?;
    if mountpoint.exists()
        && (!mountpoint.is_dir()
            || fs::read_dir(&mountpoint)
                .map_err(|e| AppError::io("Inspect mountpoint", e))?
                .next()
                .is_some())
    {
        return Err(AppError::new("Mountpoint must be an empty directory."));
    }
    let mut pass = secret(c.passphrase.as_deref(), false)?;
    pass.push(b'\n');
    let image = if source.is_file() {
        Some(attach(&source, c.readonly)?)
    } else {
        None
    };
    let opened = (|| {
        let native = if image.is_none() {
            Some(info(&source)?.device)
        } else {
            None
        };
        let image_selected = image.is_some();
        let native_ref = native.as_deref();
        let matches: Vec<_> = containers()?
            .into_iter()
            .filter(|container| {
                image.as_ref().is_some_and(|i| image_owns(i, container))
                    || native.as_ref().is_some_and(|n| {
                        container.stores.iter().any(|s| &s.device == n)
                            || container.volumes.iter().any(|v| &v.device == n)
                    })
            })
            .flat_map(|container| {
                let store_selected =
                    native_ref.is_some_and(|n| container.stores.iter().any(|s| s.device == n));
                container.volumes.into_iter().filter(move |v| {
                    image_selected || store_selected || native_ref.is_some_and(|n| v.device == n)
                })
            })
            .filter(|v| v.encrypted && v.roles.is_empty())
            .collect();
        if matches.len() != 1 {
            return Err(AppError::new(
                "Source is unencrypted or must identify exactly one encrypted APFS volume.",
            ));
        }
        let volume = &matches[0];
        if !volume.locked {
            return Err(AppError::new(
                "APFS volume is already unlocked. Close it first.",
            ));
        }
        if !mountpoint.exists() {
            fs::create_dir(&mountpoint).map_err(|e| AppError::io("Create APFS mountpoint", e))?;
        }
        disk(
            &args(&[
                "apfs",
                "unlockVolume",
                &volume.uuid,
                "-stdinpassphrase",
                "-nomount",
            ]),
            Some(&pass),
        )?;
        let mut a = args(&["mount"]);
        if c.readonly {
            a.push("readOnly".into());
        }
        a.extend([
            "nobrowse".into(),
            "-mountPoint".into(),
            mountpoint.clone().into(),
            volume.uuid.clone().into(),
        ]);
        if let Err(error) = disk(&a, None) {
            disk(&args(&["apfs", "lockVolume", &volume.uuid]), None)?;
            return Err(error);
        }
        Ok(())
    })();
    if opened.is_err() {
        if let Some(image) = &image {
            detach_image(image)?;
        }
    }
    opened?;
    output.msg2(&format!("Mounted APFS at {}", mountpoint.display()));
    Ok(())
}

pub(super) fn unmount(c: UnmountCommand, output: Output) -> AppResult<()> {
    require_no_tag(c.tag.as_deref())?;
    let target = existing(&c.target)?;
    let attached = images()?;
    let matches: Vec<_> = list()?
        .into_iter()
        .filter(|v| {
            same_path(&v.source, &target)
                || v.mountpoint.as_ref().is_some_and(|p| same_path(p, &target))
                || v.devices.iter().any(|p| same_path(p, &target))
        })
        .collect();
    if matches.len() != 1 {
        return Err(AppError::new(
            "Target must identify exactly one unlocked APFS volume.",
        ));
    }
    let volume = &matches[0];
    disk(
        &[
            "apfs".into(),
            "lockVolume".into(),
            volume.devices[0].clone().into(),
        ],
        None,
    )?;
    if let Some(image) = attached.iter().find(|i| same_path(&i.path, &volume.source)) {
        detach_image(image)?;
    }
    output.msg2("Locked APFS volume");
    Ok(())
}
