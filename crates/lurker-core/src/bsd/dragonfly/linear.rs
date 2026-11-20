//! Bind native activation to a private copy of its LUKS1 metadata.
use super::metadata::{self, HEADER_BYTES, SECTOR_BYTES};
use super::{
    AppError, AppResult, args, attach_file, detach_file, exec, mapping_name, verify_backing,
};
use crate::bsd::{Identity, Registry, Session, private_owned, sync_directory};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const DMSETUP: &str = "/sbin/dmsetup";

#[derive(Clone, Serialize, Deserialize)]
pub(in crate::bsd) struct Layer {
    identity: Option<Identity>,
    backing: Option<String>,
    #[serde(default)]
    detach_requested: bool,
    composite_requested: bool,
    activation_requested: bool,
    metadata_sectors: u64,
    total_sectors: u64,
}

fn metadata_path(directory: &Path, session: &Session) -> AppResult<PathBuf> {
    if session.id.is_empty() || !session.id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return Err(AppError::new(
            "Invalid DragonFly metadata session identity.",
        ));
    }
    Ok(directory.join(format!("{}.luks1-metadata", session.id)))
}

fn device(session: &Session) -> String {
    format!("/dev/mapper/lurker_linear_{}", session.id.replace('-', "_"))
}

fn uuid(session: &Session) -> String {
    format!("LURKER-DFLY-LUKS1-{}", session.id)
}

fn available() -> AppResult<bool> {
    Ok(exec(DMSETUP, &args(&["targets"]), None)?
        .lines()
        .any(|line| line.split_whitespace().next() == Some("linear")))
}

fn require_linear() -> AppResult<()> {
    if !available()? {
        let load = exec("/sbin/kldload", &args(&["dm_target_linear"]), None);
        if !available()? {
            return Err(AppError::new(format!(
                "DragonFly LUKS1 requires the stock dm_target_linear module. {}",
                load.err().map(|error| error.message).unwrap_or_default()
            )));
        }
    }
    Ok(())
}

fn read_header(source: &str, image_size: u64) -> AppResult<(metadata::Geometry, Vec<u8>)> {
    let mut file =
        File::open(source).map_err(|e| AppError::io("Open attached LUKS1 backing", e))?;
    let mut block = [0; (2 * SECTOR_BYTES) as usize];
    file.read_exact(&mut block)
        .map_err(|e| AppError::io("Read attached LUKS1 header", e))?;
    let geometry = metadata::inspect(block[..HEADER_BYTES].try_into().unwrap(), image_size)?;
    let mut prefix = vec![0; (geometry.metadata * SECTOR_BYTES) as usize];
    prefix[..block.len()].copy_from_slice(&block);
    file.read_exact(&mut prefix[block.len()..])
        .map_err(|e| AppError::io("Read LUKS1 metadata and keyslots", e))?;
    Ok((geometry, prefix))
}

fn has_mapping(path: &str) -> AppResult<bool> {
    let name = mapping_name(path)?;
    Ok(exec(DMSETUP, &args(&["ls"]), None)?
        .lines()
        .any(|line| line.split_whitespace().next() == Some(name)))
}

pub(in crate::bsd) fn bind(registry: &Registry, session: &mut Session) -> AppResult<String> {
    require_linear()?;
    let source = session
        .backing
        .as_deref()
        .ok_or_else(|| {
            AppError::new("DragonFly LUKS1 backing was not attached before metadata capture.")
        })?
        .to_owned();
    verify_backing(&source, &session.source)?;
    let metadata = fs::metadata(&session.source)
        .map_err(|e| AppError::io("Inspect attached LUKS1 image", e))?;
    if metadata.dev() != session.identity.dev || metadata.ino() != session.identity.ino {
        return Err(AppError::new(
            "LUKS1 image changed before metadata capture.",
        ));
    }
    let (geometry, prefix) = read_header(&source, metadata.len())?;
    let path = metadata_path(&registry.directory, session)?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(AppError::new(
            "Private LUKS1 metadata destination already exists.",
        ));
    }
    session.dragonfly_layer = Some(Layer {
        identity: None,
        backing: None,
        detach_requested: false,
        composite_requested: false,
        activation_requested: false,
        metadata_sectors: geometry.metadata,
        total_sectors: geometry.total,
    });
    registry.save(session)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|e| AppError::io("Create private LUKS1 metadata copy", e))?;
    session.dragonfly_layer.as_mut().unwrap().identity = Some(Identity::read(&path)?);
    registry.save(session)?;
    file.write_all(&prefix)
        .and_then(|()| file.sync_all())
        .map_err(|e| AppError::io("Sync private LUKS1 metadata copy", e))?;
    drop(file);
    private_owned(&path, false)?;
    let backing = attach_file(&path, false)?;
    session.dragonfly_layer.as_mut().unwrap().backing = Some(backing.clone());
    registry.save(session)?;
    verify_backing(&backing, &path)?;
    verify_backing(&source, &session.source)?;

    let composite = device(session);
    if has_mapping(&composite)? {
        return Err(AppError::new(
            "DragonFly composite mapping name is already in use.",
        ));
    }
    session
        .dragonfly_layer
        .as_mut()
        .unwrap()
        .composite_requested = true;
    registry.save(session)?;
    let layout = format!(
        "0 {} linear {backing} 0\n{} {} linear {source} {}\n",
        geometry.metadata,
        geometry.metadata,
        geometry.total - geometry.metadata,
        geometry.metadata
    );
    exec(
        DMSETUP,
        &args(&[
            "create",
            mapping_name(&composite)?,
            "--uuid",
            &uuid(session),
        ]),
        Some(layout.as_bytes()),
    )?;
    verify(registry.directory.as_path(), session)?;
    let mut observed = vec![0; prefix.len()];
    File::open(&composite)
        .and_then(|mut file| file.read_exact(&mut observed))
        .map_err(|e| AppError::io("Verify private LUKS1 metadata through composite device", e))?;
    if observed != prefix {
        return Err(AppError::new(
            "DragonFly composite did not expose the checked metadata.",
        ));
    }
    Ok(composite)
}

fn activation_name(session: &Session) -> String {
    format!("lurker_luks1_{}", session.id.replace('-', "_"))
}

pub(in crate::bsd) fn activate(
    registry: &Registry,
    session: &mut Session,
    readonly: bool,
    credential: &super::Credential,
) -> AppResult<String> {
    let composite = bind(registry, session)?;
    let name = activation_name(session);
    let mapped = format!("/dev/mapper/{name}");
    if has_mapping(&mapped)? {
        return Err(AppError::new(
            "DragonFly LUKS1 activation name is already in use.",
        ));
    }
    session
        .dragonfly_layer
        .as_mut()
        .unwrap()
        .activation_requested = true;
    registry.save(session)?;
    super::unlock_named(&composite, &name, readonly, credential)?;
    Ok(mapped)
}

pub(in crate::bsd) fn reconcile_activation(
    registry: &Registry,
    session: &mut Session,
) -> AppResult<()> {
    reconcile_private_backing(registry, session)?;
    if !session
        .dragonfly_layer
        .as_ref()
        .is_some_and(|layer| layer.activation_requested)
    {
        return Ok(());
    }
    let mapped = format!("/dev/mapper/{}", activation_name(session));
    if has_mapping(&mapped)? {
        if session.mapped.is_none() {
            verify(&registry.directory, session)?;
            super::verify_mapping(&mapped, &device(session))?;
            session.mapped = Some(mapped);
            registry.save(session)?;
        }
    } else if session.mapped.as_deref() == Some(&mapped)
        && session.mounted.is_none()
        && !crate::bsd::mounts()?
            .iter()
            .any(|mount| mount.source == Path::new(&mapped))
    {
        session.mapped = None;
        registry.save(session)?;
    }
    Ok(())
}

fn reconcile_private_backing(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(layer) = &session.dragonfly_layer else {
        return Ok(());
    };
    let Some(identity) = &layer.identity else {
        return Ok(());
    };
    let path = metadata_path(&registry.directory, session)?;
    if !identity.matches(&path) {
        return Ok(());
    }
    private_owned(&path, false)?;
    let next = match &layer.backing {
        Some(backing)
            if layer.detach_requested
                && !layer.composite_requested
                && session.mapped.is_none()
                && session.mounted.is_none()
                && super::backing_inactive(backing)? =>
        {
            None
        }
        None if !layer.detach_requested => {
            let Some(backing) = super::attached_private_copy(&path)? else {
                return Ok(());
            };
            Some(backing)
        }
        _ => return Ok(()),
    };
    session.dragonfly_layer.as_mut().unwrap().backing = next;
    registry.save(session)
}

pub(in crate::bsd) fn crypto_backing<'a>(
    session: &Session,
    backing: &'a str,
) -> std::borrow::Cow<'a, str> {
    if session
        .dragonfly_layer
        .as_ref()
        .is_some_and(|layer| layer.composite_requested)
    {
        std::borrow::Cow::Owned(device(session))
    } else {
        std::borrow::Cow::Borrowed(backing)
    }
}

pub(in crate::bsd) fn verify(directory: &Path, session: &Session) -> AppResult<()> {
    let Some(layer) = &session.dragonfly_layer else {
        return Ok(());
    };
    let path = metadata_path(directory, session)?;
    if path.exists() {
        private_owned(&path, false)?;
        if layer
            .identity
            .as_ref()
            .is_some_and(|identity| !identity.matches(&path))
        {
            return Err(AppError::new("Private LUKS1 metadata identity changed."));
        }
    }
    if let Some(backing) = &layer.backing {
        verify_backing(backing, &path)?;
    }
    if !layer.composite_requested {
        return Ok(());
    }
    let composite = device(session);
    if !has_mapping(&composite)? {
        if session.mapped.is_some() {
            return Err(AppError::new("DragonFly composite backing disappeared."));
        }
        return Ok(());
    }
    let name = mapping_name(&composite)?;
    let reported_uuid = exec(
        DMSETUP,
        &args(&["info", "-c", "--noheadings", "-o", "uuid", name]),
        None,
    )?;
    if reported_uuid.trim() != uuid(session) {
        return Err(AppError::new(
            "DragonFly composite mapping identity changed.",
        ));
    }
    let Some((header, source)) = layer.backing.as_deref().zip(session.backing.as_deref()) else {
        return Err(AppError::new(
            "DragonFly composite backing identities are missing.",
        ));
    };
    super::verify_dependencies(&composite, &[header, source])?;
    let status = exec(DMSETUP, &args(&["status", name]), None)?;
    let expected = [
        (0, layer.metadata_sectors),
        (
            layer.metadata_sectors,
            layer.total_sectors - layer.metadata_sectors,
        ),
    ];
    let mut rows = status.lines();
    for (start, length) in expected {
        let mut columns = rows.next().unwrap_or_default().split_whitespace();
        if columns.next().and_then(|v| v.parse().ok()) != Some(start)
            || columns.next().and_then(|v| v.parse().ok()) != Some(length)
            || columns.next() != Some("linear")
        {
            return Err(AppError::new("DragonFly composite mapping layout changed."));
        }
    }
    if rows.next().is_some() {
        return Err(AppError::new(
            "DragonFly composite mapping contains unexpected targets.",
        ));
    }
    Ok(())
}

pub(in crate::bsd) fn release(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(layer) = &session.dragonfly_layer else {
        return Ok(());
    };
    let path = metadata_path(&registry.directory, session)?;
    verify(&registry.directory, session)?;
    let composite = device(session);
    if layer.composite_requested {
        if has_mapping(&composite)? {
            exec(DMSETUP, &args(&["remove", mapping_name(&composite)?]), None)?;
        }
        session
            .dragonfly_layer
            .as_mut()
            .unwrap()
            .composite_requested = false;
        registry.save(session)?;
    }
    if let Some(backing) = session.dragonfly_layer.as_ref().unwrap().backing.clone() {
        session.dragonfly_layer.as_mut().unwrap().detach_requested = true;
        registry.save(session)?;
        detach_file(&backing)?;
        session.dragonfly_layer.as_mut().unwrap().backing = None;
        registry.save(session)?;
    }
    if path.exists() {
        private_owned(&path, false)?;
        fs::remove_file(&path)
            .map_err(|e| AppError::io("Remove private LUKS1 metadata copy", e))?;
        sync_directory(&registry.directory)?;
    }
    session.dragonfly_layer = None;
    registry.save(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;

    #[test]
    #[ignore = "requires root on DragonFly; allocates a disposable vnode and journal"]
    fn native_metadata_detach_retry_requires_intent_and_inactive_unit() {
        crate::bsd::require_root().unwrap();
        let id = crate::bsd::unique_id();
        let directory = std::env::temp_dir().join(format!("lurker-metadata-close-{id}"));
        fs::DirBuilder::new().mode(0o700).create(&directory).unwrap();
        let source = directory.join("image");
        File::create(&source).unwrap().set_len(4096).unwrap();
        let registry = Registry {
            directory: directory.clone(),
            _lock: File::create(directory.join("lock")).unwrap(),
        };
        let mut session = Session {
            version: 1,
            id,
            backend: crate::VolumeType::Luks1,
            source: source.clone(),
            peer_sources: Vec::new(),
            identity: Identity::read(&source).unwrap(),
            backing: None,
            dragonfly_layer: None,
            mapped: None,
            mounted: None,
            intended_mountpoint: None,
            initialization_directory: None,
            readonly: false,
        };
        let private = metadata_path(&directory, &session).unwrap();
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&private)
            .unwrap()
            .set_len(1024)
            .unwrap();
        let identity = Identity::read(&private).unwrap();
        let device = attach_file(&source, false).unwrap();
        session.dragonfly_layer = Some(Layer {
            identity: Some(identity),
            backing: Some(device.clone()),
            detach_requested: true,
            composite_requested: false,
            activation_requested: false,
            metadata_sectors: 2,
            total_sectors: 8,
        });
        let live = reconcile_activation(&registry, &mut session)
            .and_then(|()| verify(&directory, &session));
        detach_file(&device).unwrap();
        assert!(live.is_err(), "an unrelated live vnode must be preserved");

        // Model a native detach that completed just before its journal write.
        let device = attach_file(&private, false).unwrap();
        detach_file(&device).unwrap();
        session.dragonfly_layer.as_mut().unwrap().backing = Some(device);
        session.dragonfly_layer.as_mut().unwrap().detach_requested = false;
        registry.save(&session).unwrap();
        reconcile_activation(&registry, &mut session).unwrap();
        assert!(verify(&directory, &session).is_err());
        assert!(private.exists());
        session.dragonfly_layer.as_mut().unwrap().detach_requested = true;
        registry.save(&session).unwrap();
        let mut restarted = registry.records().unwrap().pop().unwrap();
        reconcile_activation(&registry, &mut restarted).unwrap();
        release(&registry, &mut restarted).unwrap();
        assert!(restarted.dragonfly_layer.is_none());
        assert!(!private.exists());
        drop(registry);
        fs::remove_dir_all(directory).unwrap();
    }
}
