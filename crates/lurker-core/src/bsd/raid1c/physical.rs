//! Physical mirrors keep their member proofs and native UUID in persistent,
//! root-owned recovery metadata. Device nodes are never publication targets.
use super::*;
use crate::bsd::physical::{capture, ensure_idle, BackingIdentity};
use crate::SourceKind;

const DIRECTORY: &str = "/var/db/lurker-raid1c";

fn whole_disk(path: &Path) -> AppResult<(Identity, BackingIdentity)> {
    let (identity, proof) = capture(path)?;
    let unit = native::backing_unit(
        path.to_str()
            .ok_or_else(|| AppError::new("Non-UTF-8 physical member."))?,
    )?;
    if !unit.starts_with("sd") || path != Path::new(&format!("/dev/{unit}c")) {
        return Err(AppError::new(
            "Physical RAID1C members must be canonical whole /dev/sdNc disks.",
        ));
    }
    Ok((identity, proof))
}

pub(super) fn descriptor_path(source: &Path) -> AppResult<PathBuf> {
    whole_disk(source)?;
    let header = driver::probe(
        source
            .to_str()
            .ok_or_else(|| AppError::new("Non-UTF-8 physical member."))?,
    )?;
    if header.level != 0x1c || header.members != 2 {
        return Err(AppError::new("Not a native two-member RAID1C disk."));
    }
    Ok(Path::new(DIRECTORY).join(format!("{}.raid1c.json", header.uuid)))
}

pub(super) fn existing_member(
    path: &Path,
    allow_missing: bool,
    descriptor: Option<&Descriptor>,
) -> AppResult<(PathBuf, Option<Identity>, Option<BackingIdentity>)> {
    let known = descriptor
        .filter(|d| d.version == 2)
        .map(|d| {
            d.member_paths
                .iter()
                .position(|p| p == path)
                .and_then(|i| d.physical.get(i))
                .cloned()
                .ok_or_else(|| {
                    AppError::new("Requested physical member is not in its persistent descriptor.")
                })
        })
        .transpose()?;
    if allow_missing
        && known.is_some()
        && native::disk_is_absent(
            path.to_str()
                .ok_or_else(|| AppError::new("Non-UTF-8 physical member."))?,
        )?
    {
        return Ok((path.to_owned(), None, known));
    }
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() => {
            let (identity, proof) = whole_disk(path)?;
            if descriptor.is_some_and(|d| d.version != 2) {
                return Err(AppError::new(
                    "A file mirror descriptor cannot identify a physical disk.",
                ));
            }
            if let Some(known) = &known {
                known.verify(path)?;
            }
            ensure_idle(path)?;
            Ok((path.to_owned(), Some(identity), Some(proof)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && allow_missing && known.is_some() => {
            Ok((path.to_owned(), None, known))
        }
        _ if known.is_some() => Err(AppError::new(
            "Physical mirror member was replaced by another object; preserve its descriptor.",
        )),
        _ => {
            let (path, identity) = existing_file(path, allow_missing)?;
            Ok((path, identity, None))
        }
    }
}

/// A one-member recovery may not race an active peer from the saved array.
pub(super) fn ensure_descriptor_idle(descriptor: &Descriptor) -> AppResult<()> {
    for path in &descriptor.member_paths {
        existing_member(path, true, Some(descriptor))?;
    }
    Ok(())
}

fn refresh_member(member: &mut Member) -> AppResult<()> {
    if !member
        .identity
        .as_ref()
        .is_some_and(|id| id.matches(&member.path))
    {
        return Err(AppError::new(
            "Physical mirror device node changed during initialization.",
        ));
    }
    let before = member
        .physical
        .as_ref()
        .ok_or_else(|| AppError::new("Missing physical mirror proof."))?;
    member.physical = Some(before.after_initialization(&member.path)?);
    Ok(())
}

fn save_descriptor(registry: &Registry, session: &mut Session) -> AppResult<()> {
    match fs::DirBuilder::new().mode(0o700).create(DIRECTORY) {
        Ok(()) => sync_directory(Path::new("/var/db"))?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => {
            return Err(AppError::io(
                "Create persistent RAID1C metadata directory",
                e,
            ))
        }
    }
    private_owned(Path::new(DIRECTORY), true)?;
    crate::trust::trusted_runtime_directory(Path::new(DIRECTORY))?;
    let mirror = state(session)?;
    let uuid = mirror
        .uuid
        .clone()
        .ok_or_else(|| AppError::new("Missing physical mirror UUID."))?;
    let descriptor = Descriptor {
        version: 2,
        volume_type: VolumeType::Raid1c,
        array_uuid: uuid.clone(),
        member_paths: mirror.members.iter().map(|m| m.path.clone()).collect(),
        physical: mirror
            .members
            .iter()
            .map(|m| {
                m.physical
                    .clone()
                    .ok_or_else(|| AppError::new("Missing physical mirror proof."))
            })
            .collect::<AppResult<_>>()?,
    };
    let path = Path::new(DIRECTORY).join(format!("{uuid}.raid1c.json"));
    state_mut(session)?.descriptor = path.clone();
    registry.save(session)?;
    let bytes = serde_json::to_vec_pretty(&descriptor)
        .map_err(|e| AppError::new(format!("Encode physical mirror descriptor: {e}")))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| {
            AppError::io(
                "Create persistent physical mirror descriptor without overwriting",
                e,
            )
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| AppError::io("Sync physical mirror descriptor", e))?;
    sync_directory(Path::new(DIRECTORY))?;
    let mirror = state_mut(session)?;
    mirror.descriptor_identity = Some(Identity::read(&path)?);
    mirror.descriptor_published = true;
    mirror.committed = true;
    registry.save(session)
}

pub(super) fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    if c.source_kind != SourceKind::Block || !c.force || c.size_gb.is_some() {
        return Err(AppError::new(
            "Physical RAID1C creation requires --force and no file size.",
        ));
    }
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    let mut members = Vec::new();
    for path in [&c.target, &c.peer_sources[0]] {
        let (identity, proof) = whole_disk(path)?;
        registry.ensure_unused(path)?;
        ensure_idle(path)?;
        members.push(Member {
            path: path.clone(),
            target: path.clone(),
            identity: Some(identity),
            physical: Some(proof),
            backing: None,
            stage_dir: None,
            stage_identity: None,
            published: false,
        });
    }
    ensure_distinct(&members)?;
    let credential = credential(c.passphrase.as_deref(), None, c.passphrase.is_none())?;
    let mut session = empty_session(
        members[0].path.clone(),
        members[0].identity.clone().unwrap(),
        members,
        Path::new(DIRECTORY).join(format!(".pending-{}.raid1c.json", unique_id())),
        false,
        false,
    );
    registry.save(&session)?;
    let initialized = (|| {
        for index in 0..2 {
            verify_members(&session, None)?;
            let path = state(&session)?.members[index].path.clone();
            ensure_idle(&path)?;
            native::prepare_physical(&path)?;
            let member = &mut state_mut(&mut session)?.members[index];
            refresh_member(member)?;
            member.backing = Some(path.to_string_lossy().into_owned());
            registry.save(&session)?;
        }
        verify_members(&session, None)?;
        session.mapped = Some(driver::format_physical(
            &state(&session)?.backings(),
            &credential,
        )?);
        registry.save(&session)?;
        set_uuid_from_members(&mut session, None)?;
        registry.save(&session)?;
        verify(&session)?;
        save_descriptor(&registry, &mut session)?;
        native::make_selected_filesystem(session.mapped.as_deref().unwrap(), c.filesystem)?;
        super::super::initialize_new_filesystem_owner(&registry, &mut session, owner)
    })();
    cleanup(&registry, &mut session).map_err(|error| {
        AppError::new(format!(
            "Physical RAID1C resources remain recorded for recovery: {error}"
        ))
    })?;
    initialized?;
    output.success(&format!(
        "Created physical RAID1C mirror: {} + {}",
        c.target.display(),
        c.peer_sources[0].display()
    ));
    output.msg2(&format!(
        "Back up the persistent membership descriptor: {}",
        state(&session)?.descriptor.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_character_node_is_not_authority_to_access_a_physical_disk() {
        assert!(whole_disk(Path::new("/dev/null")).is_err());
    }
}
