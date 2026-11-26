//! Physical partitions reuse the native session journal without a vnode lease.
use super::{Identity, PhysicalDevice, Registry, Session};
use crate::{AppError, AppResult, CreateCommand, Output};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct BackingIdentity {
    rdev: u64,
    bytes: u64,
    metadata_id: Option<String>,
}

pub(super) fn inspect(source: &Path) -> AppResult<PhysicalDevice> {
    #[cfg(target_os = "netbsd")]
    return super::native::physical_device(source);
    #[cfg(not(target_os = "netbsd"))]
    {
        let _ = source;
        Err(AppError::new(
            "This BSD physical-device adapter is not implemented yet.",
        ))
    }
}

pub(super) fn ensure_idle(source: &Path) -> AppResult<()> {
    #[cfg(target_os = "netbsd")]
    return super::native::ensure_physical_idle(source);
    #[cfg(not(target_os = "netbsd"))]
    {
        let _ = source;
        Err(AppError::new(
            "This BSD physical-device adapter is not implemented yet.",
        ))
    }
}

pub(super) fn capture(source: &Path) -> AppResult<(Identity, BackingIdentity)> {
    let info = inspect(source)?;
    let metadata = fs::symlink_metadata(source)
        .map_err(|e| AppError::io("Inspect physical BSD partition identity", e))?;
    if info.source != source
        || metadata.uid() != 0
        || !(metadata.file_type().is_block_device() || metadata.file_type().is_char_device())
    {
        return Err(AppError::new(
            "Physical BSD source must be its canonical root-owned device node.",
        ));
    }
    Ok((
        Identity {
            dev: metadata.dev(),
            ino: metadata.ino(),
        },
        BackingIdentity {
            rdev: metadata.rdev(),
            bytes: info.bytes,
            metadata_id: info.metadata_id,
        },
    ))
}

impl BackingIdentity {
    pub(super) fn verify(&self, source: &Path) -> AppResult<()> {
        let (_, now) = capture(source)?;
        if self.rdev != now.rdev || self.bytes != now.bytes || self.metadata_id != now.metadata_id {
            return Err(AppError::new(
                "Physical BSD partition identity changed; preserve its recorded resources.",
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "netbsd")]
pub(super) fn params_path(session: &Session) -> AppResult<std::path::PathBuf> {
    let Some(physical) = &session.physical else {
        return Ok(super::params_path(&session.source));
    };
    let id = physical
        .metadata_id
        .as_deref()
        .filter(|id| {
            id.len() == 36
                && id.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                    }
                })
        })
        .ok_or_else(|| {
            AppError::new("Physical CGD recovery metadata requires a canonical GPT partition UUID.")
        })?;
    Ok(Path::new("/var/db/lurker-cgd").join(format!("{id}.cgdparams")))
}

pub(super) fn create(command: CreateCommand, output: Output) -> AppResult<()> {
    #[cfg(target_os = "netbsd")]
    return create_cgd(command, output);
    #[cfg(not(target_os = "netbsd"))]
    {
        let _ = (command, output);
        Err(AppError::new(
            "This BSD physical-device creation adapter is not implemented yet.",
        ))
    }
}

#[cfg(target_os = "netbsd")]
fn create_cgd(c: CreateCommand, output: Output) -> AppResult<()> {
    use super::{
        cleanup, credential, filesystems, initialize_acquired_mapping,
        initialize_new_filesystem_owner, native, private_owned, sync_directory, verify_session,
    };
    use crate::{SourceKind, VolumeType};
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    if c.source_kind != SourceKind::Block || !c.force || c.size_gb.is_some() {
        return Err(AppError::new(
            "Physical CGD creation requires --force and no file size.",
        ));
    }
    if c.volume_type != VolumeType::Cgd {
        return Err(AppError::new(
            "NetBSD physical creation currently uses CGD.",
        ));
    }
    if fs::symlink_metadata(&c.target)
        .map_err(|e| AppError::io("Inspect physical CGD target", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new(
            "Refusing destructive creation through a device symlink.",
        ));
    }
    filesystems::validate(c.filesystem, c.volume_type)?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    let source = fs::canonicalize(&c.target)
        .map_err(|e| AppError::io("Resolve physical CGD partition", e))?;
    registry.ensure_unused(&source)?;
    let (identity, physical) = capture(&source)?;
    ensure_idle(&source)?;
    let mut session = Session {
        version: 1,
        id: super::unique_id(),
        backend: VolumeType::Cgd,
        source,
        create_target: None,
        peer_sources: Vec::new(),
        identity,
        physical: Some(physical),
        backing: None,
        netbsd_detach: None,
        mapped: None,
        mounted: None,
        intended_mountpoint: None,
        initialization_directory: None,
        readonly: false,
    };
    let params = params_path(&session)?;
    let parent = params.parent().unwrap();
    match fs::DirBuilder::new().mode(0o700).create(parent) {
        Ok(()) => sync_directory(parent.parent().unwrap())?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(AppError::io("Create persistent CGD metadata directory", e)),
    }
    private_owned(parent, true)?;
    crate::trust::trusted_runtime_directory(parent)?;
    if fs::symlink_metadata(&params).is_ok() {
        return Err(AppError::new(format!(
            "CGD recovery parameters already exist at {}; preserve them before reusing this GPT partition UUID.", params.display())));
    }
    let credential = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        c.passphrase.is_none() && c.keyfile.is_none(),
    )?;
    session.backing = Some(session.source.to_string_lossy().into_owned());
    registry.save(&session)?;
    let initialized = (|| {
        verify_session(&session)?;
        ensure_idle(&session.source)?;
        let backing = session.backing.as_deref().unwrap();
        native::format(backing, c.cipher, &credential, &params, c.filesystem)?;
        private_owned(&params, false)?;
        sync_directory(parent)?;
        session.mapped = Some(native::unlock_initial(backing, &credential, &params)?);
        initialize_acquired_mapping(&registry, &session, |mapped| {
            filesystems::format(mapped, c.filesystem)
        })?;
        initialize_new_filesystem_owner(&registry, &mut session, owner)
    })();
    if let Err(error) = cleanup(&registry, &mut session) {
        return Err(AppError::new(format!(
            "Physical CGD resources remain recorded for recovery; parameters at {}: {error}",
            params.display()
        )));
    }
    initialized.map_err(|error: AppError| {
        AppError::new(format!(
            "{}; preserve CGD recovery parameters at {}",
            error.message,
            params.display()
        ))
    })?;
    fs::set_permissions(&params, fs::Permissions::from_mode(0o600))
        .map_err(|e| AppError::io("Protect physical CGD recovery parameters", e))?;
    output.success(&format!(
        "Created CGD encrypted data partition: {}",
        session.source.display()
    ));
    output.msg2(&format!(
        "Back up the mandatory recovery parameters: {}",
        params.display()
    ));
    Ok(())
}

#[cfg(all(test, target_os = "netbsd"))]
mod tests {
    use super::*;

    #[test]
    fn destructive_create_requires_force_before_inspecting_a_device() {
        let command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/does/not/exist", "source_kind": "block", "force": false,
            "size_gb": null, "volume_type": "cgd", "passphrase": null
        }))
        .unwrap();
        let error = create(command, Output::silent()).unwrap_err();
        assert!(error.message.contains("requires --force"));
    }
}
