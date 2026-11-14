//! Two-member native OpenBSD encrypted mirrors. Membership metadata contains
//! paths and an array UUID, never credentials or encryption parameters.
//! Every acquired vnode and every publication step is durably journaled.
use super::raid1c_native as driver;
use super::{
    absolute_destination, credential, exec, mounts, native, private_owned, require_root,
    sync_directory, unique_id, Identity, Registry, Session,
};
use crate::{AppError, AppResult, CommandAction, CreateCommand, MountCommand, Output, VolumeType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Member {
    path: PathBuf,
    target: PathBuf,
    identity: Option<Identity>,
    backing: Option<String>,
    stage_dir: Option<PathBuf>,
    stage_identity: Option<Identity>,
    published: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Mirror {
    uuid: Option<String>,
    members: Vec<Member>,
    descriptor: PathBuf,
    descriptor_stage: Option<PathBuf>,
    descriptor_identity: Option<Identity>,
    descriptor_published: bool,
    creating: bool,
    committed: bool,
    #[serde(default)]
    rolled_back: bool,
    allow_degraded: bool,
}

impl Mirror {
    pub(super) fn owns(&self, path: &Path, identity: Option<&fs::Metadata>) -> bool {
        self.descriptor == path
            || self.members.iter().any(|member| {
                member.path == path
                    || member.target == path
                    || identity.is_some_and(|m| {
                        member
                            .identity
                            .as_ref()
                            .is_some_and(|id| id.dev == m.dev() && id.ino == m.ino())
                    })
            })
    }
    fn backings(&self) -> Vec<String> {
        self.members
            .iter()
            .filter_map(|m| m.backing.clone())
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    version: u32,
    volume_type: VolumeType,
    array_uuid: String,
    member_paths: Vec<PathBuf>,
}

pub(super) fn descriptor_path(primary: &Path) -> PathBuf {
    let mut value = primary.as_os_str().to_owned();
    value.push(".raid1c.json");
    PathBuf::from(value)
}

pub(super) fn handles(command: &CommandAction) -> bool {
    command.volume_type() == VolumeType::Raid1c
        || matches!(command,
        CommandAction::Mount(c) if c.volume_type == VolumeType::Auto && fs::symlink_metadata(descriptor_path(&c.source)).is_ok())
}

pub(super) fn run(command: CommandAction, output: Output) -> AppResult<()> {
    match command {
        CommandAction::Create(c) => create(c, output),
        CommandAction::Mount(c) => mount(c, output),
        CommandAction::Unmount(c) => super::unmount(c, output),
        _ => Err(AppError::new("RAID1C supports create, mount and unmount.")),
    }
}

fn state(session: &Session) -> AppResult<&Mirror> {
    session.mirror.as_ref().ok_or_else(|| {
        AppError::new("RAID1C ownership journal is incomplete; preserve it for recovery.")
    })
}
fn state_mut(session: &mut Session) -> AppResult<&mut Mirror> {
    session.mirror.as_mut().ok_or_else(|| {
        AppError::new("RAID1C ownership journal is incomplete; preserve it for recovery.")
    })
}

fn empty_session(
    primary: PathBuf,
    identity: Identity,
    members: Vec<Member>,
    descriptor: PathBuf,
    creating: bool,
    degraded: bool,
) -> Session {
    Session {
        version: 1,
        id: unique_id(),
        backend: VolumeType::Raid1c,
        peer_sources: members.iter().skip(1).map(|m| m.target.clone()).collect(),
        source: primary,
        identity,
        backing: None,
        mapped: None,
        mounted: None,
        intended_mountpoint: None,
        readonly: false,
        mirror: Some(Mirror {
            uuid: None,
            members,
            descriptor,
            descriptor_stage: None,
            descriptor_identity: None,
            descriptor_published: false,
            creating,
            committed: false,
            rolled_back: false,
            allow_degraded: degraded,
        }),
    }
}

fn regular_matches(identity: &Identity, path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|m| m.is_file() && m.dev() == identity.dev && m.ino() == identity.ino)
}

fn existing_file(path: &Path, allow_missing: bool) -> AppResult<(PathBuf, Option<Identity>)> {
    let path = absolute_destination(path)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok((path.clone(), Some(Identity::read(&path)?))),
        Ok(_) => Err(AppError::new(
            "RAID1C members must be regular files, never devices, directories or symlinks.",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && allow_missing => Ok((path, None)),
        Err(e) => Err(AppError::io("Inspect RAID1C member", e)),
    }
}

fn ensure_distinct(members: &[Member]) -> AppResult<()> {
    let mut paths = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for member in members {
        if !paths.insert(&member.target)
            || member
                .identity
                .as_ref()
                .is_some_and(|id| !identities.insert((id.dev, id.ino)))
        {
            return Err(AppError::new("RAID1C needs two independent member files; aliases and repeated paths are refused."));
        }
    }
    Ok(())
}

fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    require_root()?;
    let owner = crate::new_filesystem::invoking_owner()?;
    CommandAction::Create(c.clone()).validate_raid1c_options("openbsd")?;
    let bytes = crate::size::size_gb_to_mib(
        c.size_gb
            .as_deref()
            .ok_or_else(|| AppError::new("RAID1C create requires a size in GB per member."))?,
    )?
    .checked_mul(1024 * 1024)
    .ok_or_else(|| AppError::new("Member size is too large."))?;
    if bytes < 64 * 1024 * 1024 {
        return Err(AppError::new("RAID1C needs at least 64 MiB per member."));
    }
    let targets = [
        absolute_destination(&c.target)?,
        absolute_destination(&c.peer_sources[0])?,
    ];
    let descriptor = descriptor_path(&targets[0]);
    if targets[0] == targets[1] || targets.iter().any(|p| p == &descriptor) {
        return Err(AppError::new(
            "Member files and the RAID1C descriptor need distinct paths.",
        ));
    }
    let registry = Registry::open()?;
    for path in targets.iter().chain([&descriptor]) {
        registry.ensure_unused(path)?;
        if fs::symlink_metadata(path).is_ok() {
            return Err(AppError::new(
                "RAID1C never replaces existing member files or descriptors; choose new paths.",
            ));
        }
    }
    let credential = credential(c.passphrase.as_deref(), None, c.passphrase.is_none())?;
    let members = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let directory = target
                .parent()
                .unwrap()
                .join(format!(".lurker-raid1c-{}-{index}", unique_id()));
            Member {
                path: directory.join("member.img"),
                target: target.clone(),
                identity: None,
                backing: None,
                stage_dir: Some(directory),
                stage_identity: None,
                published: false,
            }
        })
        .collect::<Vec<_>>();
    // Persist the transaction before allocating even the first staging directory.
    // A crash at an identity boundary preserves the private artifact for inspection.
    let mut session = empty_session(
        members[0].path.clone(),
        Identity { dev: 0, ino: 0 },
        members,
        descriptor,
        true,
        false,
    );
    session.peer_sources = targets[1..].to_vec();
    registry.save(&session)?;
    let result = (|| {
        for index in 0..state(&session)?.members.len() {
            let directory = state(&session)?.members[index].stage_dir.clone().unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .map_err(|e| AppError::io("Create private mirror staging directory", e))?;
            state_mut(&mut session)?.members[index].stage_identity =
                Some(Identity::read(&directory)?);
            registry.save(&session)?;
            sync_directory(directory.parent().unwrap())?;
            let image = state(&session)?.members[index].path.clone();
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&image)
                .map_err(|e| AppError::io("Create staged mirror member", e))?;
            state_mut(&mut session)?.members[index].identity = Some(Identity::read(&image)?);
            registry.save(&session)?;
            crate::new_filesystem::assign_file(&file, owner)?;
            file.set_len(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|e| AppError::io("Size and sync staged mirror member", e))?;
            sync_directory(&directory)?;
        }
        session.identity = state(&session)?.members[0].identity.clone().unwrap();
        registry.save(&session)?;
        attach_members(&registry, &mut session)?;
        session.mapped = Some(driver::format(&state(&session)?.backings(), &credential)?);
        registry.save(&session)?;
        set_uuid_from_members(&mut session, None)?;
        registry.save(&session)?;
        verify(&session)?;
        driver::make_filesystem(session.mapped.as_deref().unwrap())?;
        let target = state(&session)?.members[0]
            .stage_dir
            .as_ref()
            .unwrap()
            .join("root");
        super::initialize_new_filesystem_owner(&registry, &mut session, &target, owner)?;
        close_resources(&registry, &mut session)?;
        for member in &state(&session)?.members {
            File::open(&member.path)
                .and_then(|f| f.sync_all())
                .map_err(|e| AppError::io("Sync encrypted mirror member", e))?;
        }
        prepare_descriptor(&registry, &mut session, owner)?;
        publish(&registry, &mut session)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            remove_staging(&session)?;
            registry.remove(&session)?;
            output.success(&format!(
                "Created RAID1C mirror: {} + {} (size is per member)",
                targets[0].display(),
                targets[1].display()
            ));
            Ok(())
        }
        Err(error) => match cleanup(&registry, &mut session) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(AppError::new(format!(
                "{error}; RAID1C transaction {} retained for recovery: {cleanup}",
                session.id
            ))),
        },
    }
}

fn attach_members(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let present: Vec<_> = state(session)?
        .members
        .iter()
        .filter(|m| m.identity.is_some())
        .map(|m| m.path.clone())
        .collect();
    driver::ensure_unattached(&present)?;
    for index in 0..state(session)?.members.len() {
        if state(session)?.members[index].identity.is_none() {
            continue;
        }
        let path = state(session)?.members[index].path.clone();
        let backing = native::attach_file(&path, false)?;
        state_mut(session)?.members[index].backing = Some(backing);
        registry.save(session)?;
        verify_members(session)?;
    }
    Ok(())
}

fn set_uuid_from_members(session: &mut Session, expected: Option<&str>) -> AppResult<()> {
    let headers = state(session)?
        .backings()
        .iter()
        .map(|b| driver::probe(b))
        .collect::<AppResult<Vec<_>>>()?;
    let first = headers
        .first()
        .ok_or_else(|| AppError::new("No surviving RAID1C member."))?;
    let uuid = expected.unwrap_or(&first.uuid);
    let mut indices = BTreeSet::new();
    if headers
        .iter()
        .any(|h| h.level != 0x1c || h.members != 2 || h.uuid != uuid || !indices.insert(h.index))
    {
        return Err(AppError::new(
            "RAID1C members belong to different arrays or repeat a member index.",
        ));
    }
    state_mut(session)?.uuid = Some(uuid.to_owned());
    Ok(())
}

fn relative_path(base: &Path, target: &Path) -> PathBuf {
    let left: Vec<_> = base.components().collect();
    let right: Vec<_> = target.components().collect();
    let shared = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for component in &left[shared..] {
        if matches!(component, Component::Normal(_)) {
            result.push("..");
        }
    }
    for component in &right[shared..] {
        result.push(component.as_os_str());
    }
    result
}

fn prepare_descriptor(
    registry: &Registry,
    session: &mut Session,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    let mirror = state(session)?;
    let descriptor = Descriptor {
        version: 1,
        volume_type: VolumeType::Raid1c,
        array_uuid: mirror.uuid.clone().unwrap(),
        member_paths: mirror
            .members
            .iter()
            .map(|m| relative_path(mirror.descriptor.parent().unwrap(), &m.target))
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&descriptor)
        .map_err(|e| AppError::new(format!("Encode mirror descriptor: {e}")))?;
    let path = mirror.members[0]
        .stage_dir
        .as_ref()
        .unwrap()
        .join("descriptor.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| AppError::io("Create mirror descriptor", e))?;
    crate::new_filesystem::assign_file(&file, owner)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| AppError::io("Sync mirror descriptor", e))?;
    let identity = Identity::read(&path)?;
    let mirror = state_mut(session)?;
    mirror.descriptor_stage = Some(path);
    mirror.descriptor_identity = Some(identity);
    registry.save(session)
}

/// Files may have different backing filesystems, so use same-filesystem
/// no-clobber links individually and publish the descriptor last. The journal
/// remains until all parent directories are synced and commit is durable.
fn publish(registry: &Registry, session: &mut Session) -> AppResult<()> {
    for index in 0..state(session)?.members.len() {
        let member = &state(session)?.members[index];
        if !member
            .identity
            .as_ref()
            .is_some_and(|id| regular_matches(id, &member.path))
        {
            return Err(AppError::new(
                "Staged mirror member changed; refusing publication.",
            ));
        }
        fs::hard_link(&member.path, &member.target)
            .map_err(|e| AppError::io("Publish mirror member without overwriting", e))?;
        if !member
            .identity
            .as_ref()
            .is_some_and(|id| regular_matches(id, &member.target))
        {
            return Err(AppError::new(
                "Published mirror member changed; preserve transaction evidence.",
            ));
        }
        state_mut(session)?.members[index].published = true;
        registry.save(session)?;
        sync_directory(state(session)?.members[index].target.parent().unwrap())?;
    }
    let mirror = state(session)?;
    if !mirror
        .descriptor_identity
        .as_ref()
        .is_some_and(|id| regular_matches(id, mirror.descriptor_stage.as_ref().unwrap()))
    {
        return Err(AppError::new(
            "Staged mirror descriptor changed; refusing publication.",
        ));
    }
    fs::hard_link(
        mirror.descriptor_stage.as_ref().unwrap(),
        &mirror.descriptor,
    )
    .map_err(|e| AppError::io("Publish RAID1C descriptor without overwriting", e))?;
    state_mut(session)?.descriptor_published = true;
    registry.save(session)?;
    sync_directory(state(session)?.descriptor.parent().unwrap())?;
    state_mut(session)?.committed = true;
    registry.save(session)
}

fn load_descriptor(path: &Path) -> AppResult<Descriptor> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| AppError::io("Read RAID1C descriptor", e))?;
    if !file
        .metadata()
        .map_err(|e| AppError::io("Inspect RAID1C descriptor", e))?
        .is_file()
    {
        return Err(AppError::new("RAID1C descriptor must be a regular file."));
    }
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::io("Read RAID1C descriptor", e))?;
    if bytes.len() > 65536 {
        return Err(AppError::new("RAID1C descriptor is too large."));
    }
    let value: Descriptor =
        serde_json::from_slice(&bytes).map_err(|_| AppError::new("Invalid RAID1C descriptor."))?;
    if value.version != 1
        || value.volume_type != VolumeType::Raid1c
        || value.member_paths.len() != 2
        || value.member_paths.iter().any(|p| p.as_os_str().is_empty())
        || value.member_paths[0] == value.member_paths[1]
        || value.array_uuid.len() != 32
        || !value
            .array_uuid
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(AppError::new("Unsupported RAID1C membership descriptor."));
    }
    Ok(value)
}

fn mount(c: MountCommand, output: Output) -> AppResult<()> {
    require_root()?;
    if c.readonly {
        return Err(AppError::new("Read-only native OpenBSD RAID1C attachment is unavailable; refusing a writable fallback."));
    }
    if c.keyfile.is_some() || c.tag.is_some() {
        return Err(AppError::new(
            "RAID1C uses passwords and does not support tags.",
        ));
    }
    let primary = absolute_destination(&c.source)?;
    let descriptor = descriptor_path(&primary);
    let metadata = if fs::symlink_metadata(&descriptor).is_ok() {
        Some(load_descriptor(&descriptor)?)
    } else {
        None
    };
    let paths = if c.peer_sources.len() == 1 {
        vec![primary.clone(), absolute_destination(&c.peer_sources[0])?]
    } else if let Some(value) = &metadata {
        vec![
            primary.clone(),
            absolute_destination(&descriptor.parent().unwrap().join(&value.member_paths[1]))?,
        ]
    } else if c.allow_degraded {
        vec![primary.clone()]
    } else {
        return Err(AppError::new("RAID1C mount needs --member or its .raid1c.json descriptor; single-member recovery requires explicit --type raid1c --degraded."));
    };
    let registry = Registry::open()?;
    let mut members = Vec::new();
    for path in paths {
        registry.ensure_unused(&path)?;
        let (path, identity) = existing_file(&path, c.allow_degraded)?;
        members.push(Member {
            path: path.clone(),
            target: path,
            identity,
            backing: None,
            stage_dir: None,
            stage_identity: None,
            published: false,
        });
    }
    ensure_distinct(&members)?;
    let present = members.iter().filter(|m| m.identity.is_some()).count();
    if present == 0 || (!c.allow_degraded && present != 2) {
        return Err(AppError::new(
            "Both mirror members are required without explicit degraded recovery.",
        ));
    }
    let identity = members.iter().find_map(|m| m.identity.clone()).unwrap();
    let credential = credential(c.passphrase.as_deref(), None, false)?;
    let target = mountpoint(&c.mountpoint, &members)?;
    let mut session = empty_session(
        primary,
        identity,
        members,
        descriptor,
        false,
        c.allow_degraded,
    );
    session.intended_mountpoint = Some(target.clone());
    registry.save(&session)?;
    let result = (|| {
        attach_members(&registry, &mut session)?;
        set_uuid_from_members(
            &mut session,
            metadata.as_ref().map(|m| m.array_uuid.as_str()),
        )?;
        registry.save(&session)?;
        let mirror = state(&session)?;
        session.mapped = Some(driver::unlock(
            &mirror.backings(),
            &credential,
            mirror.uuid.as_deref().unwrap(),
            c.allow_degraded,
        )?);
        registry.save(&session)?;
        verify(&session)?;
        let mirror = state(&session)?;
        driver::verify_mapping(
            session.mapped.as_deref().unwrap(),
            &mirror.backings(),
            mirror.uuid.as_deref().unwrap(),
            c.allow_degraded,
        )?;
        let mut argv = super::args(&["-t", "ffs", "-o", "rw"]);
        argv.push(session.mapped.as_ref().unwrap().into());
        argv.push(target.as_os_str().to_owned());
        exec("/sbin/mount", &argv, None)?;
        session.mounted = mounts()?.into_iter().find(|m| m.target == target);
        let mounted = session.mounted.as_ref().ok_or_else(|| {
            AppError::new("RAID1C mount identity is missing; preserve the journal for recovery.")
        })?;
        if mounted.source != Path::new(session.mapped.as_deref().unwrap())
            || mounted.readonly
            || mounted.filesystem != "ffs"
        {
            return Err(AppError::new(
                "Unexpected RAID1C filesystem attachment; preserve its journal.",
            ));
        }
        registry.save(&session)?;
        Ok(())
    })();
    if let Err(error) = result {
        return match cleanup(&registry, &mut session) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(AppError::new(format!(
                "{error}; RAID1C session {} retained: {cleanup}",
                session.id
            ))),
        };
    }
    if c.allow_degraded {
        output.warn("RAID1C degraded recovery was explicitly allowed. Offline or missing members are not rebuilt automatically.");
    }
    output.success(&format!("Mounted RAID1C at {}", target.display()));
    Ok(())
}

fn mountpoint(path: &Path, members: &[Member]) -> AppResult<PathBuf> {
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(|e| AppError::io("Create mirror mountpoint", e))?;
    }
    if fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Inspect mirror mountpoint", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new("Mirror mountpoint must not be a symlink."));
    }
    let target =
        fs::canonicalize(path).map_err(|e| AppError::io("Resolve mirror mountpoint", e))?;
    if target.as_os_str().as_bytes().len() >= 90
        || members.iter().any(|m| m.path.starts_with(&target))
        || mounts()?.iter().any(|m| m.target == target)
        || fs::read_dir(&target)
            .map_err(|e| AppError::io("Inspect mirror mountpoint", e))?
            .next()
            .is_some()
    {
        return Err(AppError::new(
            "Mirror mountpoint must be empty, unmounted, disjoint and shorter than 90 bytes.",
        ));
    }
    Ok(target)
}

fn verify_members(session: &Session) -> AppResult<()> {
    for member in &state(session)?.members {
        if let Some(identity) = &member.identity {
            let finalized = state(session)?.committed
                && member.backing.is_none()
                && session.mapped.is_none()
                && regular_matches(identity, &member.target);
            let discarded = state(session)?.rolled_back
                && member.backing.is_none()
                && session.mapped.is_none()
                && fs::symlink_metadata(&member.path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
            if !regular_matches(identity, &member.path) && !finalized && !discarded {
                return Err(AppError::new(
                    "Mirror member identity changed; refusing cleanup.",
                ));
            }
            if let Some(backing) = &member.backing {
                native::verify_backing(backing, &member.path)?;
            }
        } else if member.backing.is_some() {
            return Err(AppError::new(
                "An unowned missing mirror member has a device; preserve this journal.",
            ));
        }
    }
    Ok(())
}

pub(super) fn verify(session: &Session) -> AppResult<()> {
    verify_members(session)?;
    if let Some(mapped) = &session.mapped {
        let mirror = state(session)?;
        driver::verify_mapping(
            mapped,
            &mirror.backings(),
            mirror.uuid.as_deref().ok_or_else(|| {
                AppError::new("Array UUID is missing; preserve the recorded mapping for recovery.")
            })?,
            true,
        )?;
    }
    Ok(())
}

fn close_resources(registry: &Registry, session: &mut Session) -> AppResult<()> {
    verify(session)?;
    if session.mounted.is_none()
        && session.intended_mountpoint.is_some()
        && mounts()?
            .iter()
            .any(|m| Some(&m.target) == session.intended_mountpoint.as_ref())
    {
        return Err(AppError::new(
            "An unrecorded mount occupies the mirror target; preserve the session.",
        ));
    }
    if let Some(recorded) = &session.mounted {
        if let Some(current) = mounts()?.into_iter().find(|m| m.target == recorded.target) {
            if current != *recorded {
                return Err(AppError::new(
                    "Mirror mount identity changed; refusing to detach a replacement.",
                ));
            }
            exec(
                "/sbin/umount",
                &[recorded.target.as_os_str().to_owned()],
                None,
            )?;
        }
        session.mounted = None;
        session.intended_mountpoint = None;
        registry.save(session)?;
    }
    if let Some(mapped) = &session.mapped {
        if mounts()?.iter().any(|m| m.source == Path::new(mapped)) {
            return Err(AppError::new(
                "Mirror still has a mounted filesystem; leave it recoverable.",
            ));
        }
        driver::close(mapped)?;
        session.mapped = None;
        registry.save(session)?;
    }
    for index in (0..state(session)?.members.len()).rev() {
        if let Some(backing) = state(session)?.members[index].backing.clone() {
            let member = &state(session)?.members[index];
            native::verify_backing(&backing, &member.path)?;
            native::detach_file(&backing)?;
            state_mut(session)?.members[index].backing = None;
            registry.save(session)?;
        }
    }
    Ok(())
}

pub(super) fn cleanup(registry: &Registry, session: &mut Session) -> AppResult<()> {
    close_resources(registry, session)?;
    if state(session)?.creating && !state(session)?.committed && !state(session)?.rolled_back {
        let paths = state(session)?
            .members
            .iter()
            .filter(|m| m.identity.is_some())
            .map(|m| m.path.clone())
            .collect::<Vec<_>>();
        driver::ensure_unattached(&paths)?;
        rollback_publication(session)?;
        state_mut(session)?.rolled_back = true;
        registry.save(session)?;
    }
    remove_staging(session)?;
    registry.remove(session)
}

fn rollback_publication(session: &Session) -> AppResult<()> {
    let mirror = state(session)?;
    // A crash between link() and journal save is reconciled by the staged inode.
    let mut owned = Vec::new();
    for member in &mirror.members {
        if let Some(metadata) = fs::symlink_metadata(&member.target).ok() {
            if metadata.is_file()
                && member
                    .identity
                    .as_ref()
                    .is_some_and(|identity| regular_matches(identity, &member.target))
            {
                owned.push(member.target.clone());
            } else if member.published {
                return Err(AppError::new(
                    "A published mirror member was replaced; preserve transaction evidence.",
                ));
            }
        }
    }
    if let Some(identity) = &mirror.descriptor_identity {
        if fs::symlink_metadata(&mirror.descriptor).is_ok() {
            if identity.matches(&mirror.descriptor) {
                owned.push(mirror.descriptor.clone());
            } else if mirror.descriptor_published {
                return Err(AppError::new(
                    "Published descriptor identity changed; preserve transaction evidence.",
                ));
            }
        }
    }
    for path in owned {
        fs::remove_file(&path)
            .map_err(|e| AppError::io("Roll back owned incomplete mirror publication", e))?;
        sync_directory(path.parent().unwrap())?;
    }
    Ok(())
}

fn remove_staging(session: &Session) -> AppResult<()> {
    let mirror = state(session)?;
    for member in &mirror.members {
        if let Some(directory) = &member.stage_dir {
            if fs::symlink_metadata(directory)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                continue;
            }
            if !member
                .stage_identity
                .as_ref()
                .is_some_and(|id| id.matches(directory))
            {
                return Err(AppError::new(
                    "Mirror staging directory identity changed; refusing removal.",
                ));
            }
            private_owned(directory, true)?;
            if member
                .identity
                .as_ref()
                .is_some_and(|id| id.matches(&member.path))
            {
                fs::remove_file(&member.path)
                    .map_err(|e| AppError::io("Remove owned staged member", e))?;
            }
            if mirror
                .descriptor_stage
                .as_ref()
                .is_some_and(|p| p.parent() == Some(directory))
            {
                let path = mirror.descriptor_stage.as_ref().unwrap();
                if mirror
                    .descriptor_identity
                    .as_ref()
                    .is_some_and(|id| id.matches(path))
                {
                    fs::remove_file(path)
                        .map_err(|e| AppError::io("Remove owned staged descriptor", e))?;
                }
            }
            let root = directory.join("root");
            if root.exists() {
                if mounts()?.iter().any(|m| m.target == root) {
                    return Err(AppError::new(
                        "Initialization root remains mounted; preserve staging.",
                    ));
                }
                fs::remove_dir(&root)
                    .map_err(|e| AppError::io("Remove empty initialization mountpoint", e))?;
            }
            fs::remove_dir(directory)
                .map_err(|e| AppError::io("Remove empty mirror staging directory", e))?;
            sync_directory(directory.parent().unwrap())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_membership_paths_survive_directory_moves() {
        assert_eq!(
            relative_path(Path::new("/a/one"), Path::new("/a/two/member")),
            Path::new("../two/member")
        );
        assert_eq!(
            relative_path(Path::new("/a"), Path::new("/a/member")),
            Path::new("member")
        );
        assert_eq!(
            descriptor_path(Path::new("/a/vault.img")),
            Path::new("/a/vault.img.raid1c.json")
        );
    }
    /// Real link/journal boundaries, isolated from global registry and kernel storage.
    #[test]
    #[ignore = "requires root OpenBSD /tmp fixture"]
    fn publication_failure_rolls_back_only_owned_links_and_retains_foreign_peer() {
        require_root().unwrap();
        let directory = PathBuf::from(format!("/tmp/lurker-raid1c-publish-{}", unique_id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let registry_dir = directory.join("registry");
        fs::create_dir(&registry_dir).unwrap();
        let registry = Registry {
            directory: registry_dir.clone(),
            _lock: File::create(registry_dir.join("lock")).unwrap(),
        };
        let mut members = Vec::new();
        for index in 0..2 {
            let stage = directory.join(format!("stage-{index}"));
            fs::DirBuilder::new().mode(0o700).create(&stage).unwrap();
            let path = stage.join("member.img");
            fs::write(&path, format!("owned member {index}")).unwrap();
            members.push(Member {
                identity: Some(Identity::read(&path).unwrap()),
                path,
                target: directory.join(format!("member-{index}")),
                backing: None,
                stage_identity: Some(Identity::read(&stage).unwrap()),
                stage_dir: Some(stage),
                published: false,
            });
        }
        let mut session = empty_session(
            members[0].path.clone(),
            members[0].identity.clone().unwrap(),
            members,
            directory.join("member-0.raid1c.json"),
            true,
            false,
        );
        state_mut(&mut session).unwrap().uuid = Some("1234567890abcdef1234567890abcdef".into());
        registry.save(&session).unwrap();
        prepare_descriptor(
            &registry,
            &mut session,
            crate::new_filesystem::invoking_owner().unwrap(),
        )
        .unwrap();
        let foreign = state(&session).unwrap().members[1].target.clone();
        fs::write(&foreign, b"foreign file must survive").unwrap();
        assert!(publish(&registry, &mut session).is_err());
        assert!(state(&session).unwrap().members[0].published);
        assert!(!state(&session).unwrap().descriptor.exists());
        rollback_publication(&session).unwrap();
        assert!(!state(&session).unwrap().members[0].target.exists());
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign file must survive");
        // Reconcile a crash after link() but before the published flag was saved.
        let first = &state(&session).unwrap().members[0];
        fs::hard_link(&first.path, &first.target).unwrap();
        state_mut(&mut session).unwrap().members[0].published = false;
        rollback_publication(&session).unwrap();
        assert!(!state(&session).unwrap().members[0].target.exists());
        remove_staging(&session).unwrap();
        // Repeated cleanup after a partially completed staging removal is safe.
        state_mut(&mut session).unwrap().rolled_back = true;
        verify(&session).unwrap();
        remove_staging(&session).unwrap();
        registry.remove(&session).unwrap();
        drop(registry);
        fs::remove_file(&foreign).unwrap();
        fs::remove_file(registry_dir.join("lock")).unwrap();
        fs::remove_dir(&registry_dir).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
