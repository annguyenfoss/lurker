//! Physical OpenBSD operations reuse the shared identity-checked lifecycle.
use super::super::{
    cleanup, credential, filesystems, initialize_acquired_mapping, initialize_new_filesystem_owner,
    native, verify_session, Registry, Session,
};
use super::{capture, ensure_idle};
use crate::{AppError, AppResult, CreateCipher, CreateCommand, Output, SourceKind, VolumeType};
use std::fs;

pub(super) fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    if c.source_kind != SourceKind::Block || !c.force || c.size_gb.is_some() {
        return Err(AppError::new(
            "Physical OpenBSD creation requires --force and no file size.",
        ));
    }
    if !matches!(c.volume_type, VolumeType::Auto | VolumeType::Softraid) {
        return Err(AppError::new(
            "This OpenBSD physical creation route uses softraid CRYPTO.",
        ));
    }
    if c.cipher != CreateCipher::Aes || c.keyfile.is_some() {
        return Err(AppError::new(
            "Softraid CRYPTO uses native AES and password authentication only.",
        ));
    }
    if fs::symlink_metadata(&c.target)
        .map_err(|e| AppError::io("Inspect physical OpenBSD target", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new(
            "Refusing destructive creation through a device symlink.",
        ));
    }
    filesystems::validate(c.filesystem, VolumeType::Softraid)?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    let source = fs::canonicalize(&c.target)
        .map_err(|e| AppError::io("Resolve physical OpenBSD disk", e))?;
    let (identity, proof) = capture(&source)?;
    if !source
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with('c'))
    {
        return Err(AppError::new(
            "Physical softraid CRYPTO creation requires a whole /dev/sdNc disk.",
        ));
    }
    registry.ensure_unused(&source)?;
    ensure_idle(&source)?;
    let credential = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        c.passphrase.is_none() && c.keyfile.is_none(),
    )?;
    let mut session = Session {
        version: 1,
        id: super::super::unique_id(),
        backend: VolumeType::Softraid,
        source,
        create_target: None,
        peer_sources: Vec::new(),
        identity,
        physical: Some(proof),
        backing: None,
        openbsd_detach: None,
        veracrypt_proof: None,
        mirror: None,
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
        session.mapped = Some(native::format(
            session.backing.as_deref().unwrap(),
            c.cipher,
            &credential,
        )?);
        // Journal acquired crypto before identity refresh or filesystem work.
        registry.save(&session)?;
        refresh_identity(&mut session)?;
        registry.save(&session)?;
        initialize_acquired_mapping(&registry, &session, |mapped| {
            filesystems::format(mapped, c.filesystem)
        })?;
        initialize_new_filesystem_owner(&registry, &mut session, owner)
    })();
    if let Err(error) = cleanup(&registry, &mut session) {
        return Err(AppError::new(format!(
            "Physical OpenBSD resources remain recorded for recovery: {error}"
        )));
    }
    initialized?;
    output.success(&format!(
        "Created softraid encrypted disk: {}",
        session.source.display()
    ));
    Ok(())
}

fn refresh_identity(session: &mut Session) -> AppResult<()> {
    if !session.identity.matches(&session.source) {
        return Err(AppError::new(
            "Physical OpenBSD device node changed during initialization.",
        ));
    }
    let (_, now) = capture(&session.source)?;
    let before = session
        .physical
        .as_ref()
        .ok_or_else(|| AppError::new("Physical backing proof is missing."))?;
    if before.rdev != now.rdev
        || before.bytes != now.bytes
        || (before.metadata_id.is_some() && before.metadata_id != now.metadata_id)
        || now.metadata_id.is_none()
    {
        return Err(AppError::new("Physical OpenBSD identity changed unexpectedly during initialization; preserve recorded resources."));
    }
    session.physical = Some(now);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_force_before_any_native_operation() {
        let command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/does/not/exist", "source_kind": "block", "force": false,
            "size_gb": null, "volume_type": "softraid", "passphrase": null
        }))
        .unwrap();
        assert!(create(command, Output::silent())
            .unwrap_err()
            .message
            .contains("requires --force"));
    }
    #[test]
    fn physical_filesystem_requires_the_privileged_native_route() {
        let mut command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/dev/test", "source_kind": "block", "force": true,
            "size_gb": null, "volume_type": "softraid", "passphrase": null,
            "filesystem": "ufs2"
        }))
        .unwrap();
        assert!(command.validate_filesystem("openbsd").is_ok());
        command.privilege_mode = crate::PrivilegeMode::Unprivileged;
        assert!(command.validate_filesystem("openbsd").is_err());
    }
}
