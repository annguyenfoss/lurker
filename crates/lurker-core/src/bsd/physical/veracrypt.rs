//! Identity-checked native VeraCrypt partition creation on FreeBSD and OpenBSD.
use super::{capture, ensure_idle};
use crate::bsd::{
    capture_veracrypt_recovery, cleanup, credential, filesystems, initialize_acquired_mapping,
    initialize_new_filesystem_owner, managed_veracrypt, unique_id, veracrypt, verify_session,
    Registry, Session,
};
use crate::{AppError, AppResult, CreateCommand, Output, SourceKind, VolumeType};
use std::fs;

pub(super) fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    if c.source_kind != SourceKind::Block || !c.force || c.size_gb.is_some() {
        return Err(AppError::new(
            "Physical VeraCrypt creation requires --force and no file size.",
        ));
    }
    if c.volume_type != VolumeType::Veracrypt {
        return Err(AppError::new(
            "This physical creation route requires VeraCrypt.",
        ));
    }
    if fs::symlink_metadata(&c.target)
        .map_err(|e| AppError::io("Inspect physical VeraCrypt target", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new(
            "Refusing destructive creation through a device symlink.",
        ));
    }
    filesystems::validate(c.filesystem, VolumeType::Veracrypt)?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    let source = fs::canonicalize(&c.target)
        .map_err(|e| AppError::io("Resolve physical VeraCrypt partition", e))?;
    let (identity, physical) = capture(&source)?;
    registry.ensure_unused(&source)?;
    ensure_idle(&source)?;
    veracrypt::ensure_source_available_with_managed(&source, &managed_veracrypt(&registry)?)?;
    let credential = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        c.passphrase.is_none() && c.keyfile.is_none(),
    )?;
    let mut session = Session {
        version: 1,
        id: unique_id(),
        backend: VolumeType::Veracrypt,
        source,
        create_target: None,
        peer_sources: Vec::new(),
        veracrypt_proof: None,
        identity,
        physical: Some(physical),
        backing: None,
        #[cfg(target_os = "freebsd")]
        freebsd_detach: None,
        #[cfg(target_os = "openbsd")]
        openbsd_detach: None,
        #[cfg(target_os = "openbsd")]
        mirror: None,
        mapped: None,
        mounted: None,
        intended_mountpoint: None,
        initialization_directory: None,
        readonly: false,
    };
    registry.save(&session)?;
    let initialized = (|| {
        verify_session(&session)?;
        ensure_idle(&session.source)?;
        veracrypt::format_physical_with_managed(
            &session.source,
            c.cipher,
            &credential,
            &managed_veracrypt(&registry)?,
        )?;
        verify_session(&session)?;
        session.mapped = Some(veracrypt::unlock_with_managed(
            &session.source,
            false,
            &credential,
            &managed_veracrypt(&registry)?,
        )?);
        capture_veracrypt_recovery(&registry, &mut session)?;
        initialize_acquired_mapping(&registry, &session, |mapped| {
            filesystems::format(mapped, c.filesystem)
        })?;
        initialize_new_filesystem_owner(&registry, &mut session, owner)
    })();
    cleanup(&registry, &mut session).map_err(|error| {
        AppError::new(format!(
            "Physical VeraCrypt resources remain recorded for recovery: {error}"
        ))
    })?;
    initialized?;
    output.success(&format!(
        "Created VeraCrypt encrypted partition: {}",
        session.source.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_force_before_any_native_operation() {
        let command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/does/not/exist", "source_kind": "block", "force": false,
            "size_gb": null, "volume_type": "veracrypt", "passphrase": null
        }))
        .unwrap();
        assert!(create(command, Output::silent())
            .unwrap_err()
            .message
            .contains("requires --force"));
    }
}
