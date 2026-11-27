//! Physical GELI uses the shared journal and native GEOM provider operations.
use super::{capture, ensure_idle};
use crate::bsd::{
    check_backend, cleanup, credential, filesystems, initialize_acquired_mapping,
    initialize_new_filesystem_owner, native, unique_id, verify_session, Registry, Session,
};
use crate::{AppError, AppResult, CreateCipher, CreateCommand, Output, SourceKind, VolumeType};
use std::fs;

pub(super) fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    if c.source_kind != SourceKind::Block || !c.force || c.size_gb.is_some() {
        return Err(AppError::new(
            "Physical GELI creation requires --force and no file size.",
        ));
    }
    let backend = check_backend(c.volume_type)?;
    if backend != VolumeType::Geli {
        return Err(AppError::new("This physical creation route requires GELI."));
    }
    if c.cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "FreeBSD GELI uses the fixed AES-XTS profile.",
        ));
    }
    if fs::symlink_metadata(&c.target)
        .map_err(|e| AppError::io("Inspect physical GELI target", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new(
            "Refusing destructive creation through a device symlink.",
        ));
    }
    filesystems::validate(c.filesystem, backend)?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    let source = fs::canonicalize(&c.target)
        .map_err(|e| AppError::io("Resolve physical GELI provider", e))?;
    registry.ensure_unused(&source)?;
    let (identity, physical) = capture(&source)?;
    ensure_idle(&source)?;
    let credential = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        c.passphrase.is_none() && c.keyfile.is_none(),
    )?;
    let mut session = Session {
        version: 1,
        id: unique_id(),
        backend,
        source,
        create_target: None,
        peer_sources: Vec::new(),
        veracrypt_proof: None,
        identity,
        physical: Some(physical),
        backing: None,
        freebsd_detach: None,
        mapped: None,
        mounted: None,
        intended_mountpoint: None,
        initialization_directory: None,
        readonly: false,
    };
    session.backing = Some(session.source.to_string_lossy().into_owned());
    registry.save(&session)?;
    let initialized = (|| {
        verify_session(&session)?;
        ensure_idle(&session.source)?;
        let backing = session.backing.as_deref().unwrap();
        native::format_with_filesystem(backing, c.cipher, &credential, c.filesystem)?;
        session.mapped = Some(native::unlock(backing, false, &credential)?);
        initialize_acquired_mapping(&registry, &session, |mapped| {
            filesystems::format(mapped, c.filesystem)
        })?;
        initialize_new_filesystem_owner(&registry, &mut session, owner)
    })();
    cleanup(&registry, &mut session).map_err(|error| {
        AppError::new(format!(
            "Physical GELI resources remain recorded for recovery: {error}"
        ))
    })?;
    initialized?;
    output.success(&format!(
        "Created GELI encrypted data provider: {}",
        session.source.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_create_requires_force_before_device_access() {
        let command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/does/not/exist", "source_kind": "block", "force": false,
            "size_gb": null, "volume_type": "geli", "passphrase": null
        }))
        .unwrap();
        assert!(create(command, Output::silent())
            .unwrap_err()
            .message
            .contains("requires --force"));
    }
}
