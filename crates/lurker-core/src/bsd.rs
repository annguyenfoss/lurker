//! Native BSD file-container lifecycle, shared by small OS adapters.
//!
//! Only recorded, identity-checked resources are closed. No forced unmount,
//! physical-device formatting or implicit privilege escalation occurs here.
use crate::{
    ActiveVolume, AppError, AppResult, CommandAction, CreateCommand, MountCommand, Output,
    PrivilegeMode, SourceKind, SystemProbe, ToolStatus, UnmountCommand, VolumeType,
};
use serde::{Deserialize, Serialize};
use staging::FileStage;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
#[cfg(target_os = "freebsd")]
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod staging;

#[cfg(target_os = "freebsd")]
mod freebsd;
#[cfg(target_os = "freebsd")]
mod pefs;
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
mod veracrypt;
#[cfg(target_os = "freebsd")]
use freebsd as native;
#[cfg(target_os = "netbsd")]
mod netbsd;
#[cfg(target_os = "netbsd")]
use netbsd as native;
#[cfg(target_os = "openbsd")]
mod openbsd;
#[cfg(target_os = "openbsd")]
mod raid1c;
#[cfg(target_os = "openbsd")]
mod raid1c_native;
#[cfg(target_os = "openbsd")]
use openbsd as native;
#[cfg(target_os = "dragonfly")]
mod dragonfly;
#[cfg(target_os = "dragonfly")]
use dragonfly as native;
#[cfg(target_os = "dragonfly")]
use dragonfly::veracrypt as dragonfly_vc;

const STATE: &str = "/var/run/lurker-bsd";

pub(super) enum Credential {
    Password(Vec<u8>),
    Keyfile(PathBuf),
}

impl Drop for Credential {
    fn drop(&mut self) {
        if let Self::Password(bytes) = self {
            bytes.fill(0);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Identity {
    dev: u64,
    ino: u64,
}

impl Identity {
    fn read(path: &Path) -> AppResult<Self> {
        let m = fs::metadata(path).map_err(|e| AppError::io("Inspect container identity", e))?;
        Self::from_metadata(&m)
    }

    fn from_file(file: &File) -> AppResult<Self> {
        let m = file.metadata().map_err(|e| AppError::io("Inspect open BSD file identity", e))?;
        Self::from_metadata(&m)
    }

    fn from_metadata(m: &fs::Metadata) -> AppResult<Self> {
        if !m.is_file() && !m.is_dir() {
            return Err(AppError::new(
                "BSD container sources must be regular files or directories.",
            ));
        }
        Ok(Self {
            dev: m.dev(),
            ino: m.ino(),
        })
    }
    fn matches(&self, path: &Path) -> bool {
        Self::read(path).is_ok_and(|now| now.dev == self.dev && now.ino == self.ino)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Mounted {
    source: PathBuf,
    target: PathBuf,
    fsid: Vec<u8>,
    filesystem: String,
    readonly: bool,
    #[cfg(target_os = "netbsd")]
    #[serde(default)]
    journaled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct InitializationDirectory {
    path: PathBuf,
    identity: Option<Identity>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    version: u32,
    id: String,
    backend: VolumeType,
    source: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    create_target: Option<PathBuf>,
    #[serde(default)]
    peer_sources: Vec<PathBuf>,
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    #[serde(default)]
    veracrypt_proof: Option<veracrypt::RecoveryProof>,
    identity: Identity,
    backing: Option<String>,
    #[cfg(target_os = "freebsd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    freebsd_detach: Option<freebsd::DetachIntent>,
    #[cfg(target_os = "netbsd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    netbsd_detach: Option<netbsd::DetachIntent>,
    #[cfg(target_os = "openbsd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    openbsd_detach: Option<openbsd::DetachIntent>,
    #[cfg(target_os = "dragonfly")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dragonfly_backing_reservation: Option<dragonfly::BackingReservation>,
    #[cfg(target_os = "dragonfly")]
    #[serde(default)]
    dragonfly_backing_detach_requested: bool,
    #[cfg(target_os = "dragonfly")]
    #[serde(default)]
    dragonfly_layer: Option<dragonfly::linear::Layer>,
    #[cfg(target_os = "dragonfly")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dragonfly_veracrypt: Option<dragonfly_vc::Mapping>,
    mapped: Option<String>,
    mounted: Option<Mounted>,
    #[serde(default)]
    intended_mountpoint: Option<PathBuf>,
    #[serde(default)]
    initialization_directory: Option<InitializationDirectory>,
    readonly: bool,
    #[cfg(target_os = "openbsd")]
    #[serde(default)]
    mirror: Option<raid1c::Mirror>,
}

struct Registry {
    directory: PathBuf,
    _lock: File,
}

impl Registry {
    fn open() -> AppResult<Self> {
        require_root()?;
        let directory = PathBuf::from(STATE);
        match fs::create_dir(&directory) {
            Ok(()) => fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|e| AppError::io("Protect BSD session directory", e))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(AppError::io("Create BSD session directory", e)),
        }
        private_owned(&directory, true)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("lock"))
            .map_err(|e| AppError::io("Open BSD session lock", e))?;
        private_owned(&directory.join("lock"), false)?;
        // SAFETY: lock is an owned, valid file descriptor for this registry.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(AppError::io(
                "Lock BSD sessions",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(Self {
            directory,
            _lock: lock,
        })
    }
    fn records(&self) -> AppResult<Vec<Session>> {
        let mut result = Vec::new();
        for entry in
            fs::read_dir(&self.directory).map_err(|e| AppError::io("Read BSD sessions", e))?
        {
            let path = entry
                .map_err(|e| AppError::io("Read BSD session", e))?
                .path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            private_owned(&path, false)?;
            let bytes = fs::read(&path).map_err(|e| AppError::io("Read BSD session", e))?;
            let record: Session = serde_json::from_slice(&bytes).map_err(|_| {
                AppError::new("Invalid BSD session record; preserve it for recovery.")
            })?;
            if record.version != 1 || self.path(&record) != path {
                return Err(AppError::new(
                    "Unknown or misplaced BSD session; preserve it for recovery.",
                ));
            }
            result.push(record);
        }
        Ok(result)
    }
    fn path(&self, s: &Session) -> PathBuf {
        self.directory.join(format!("{}.json", s.id))
    }
    fn save(&self, s: &Session) -> AppResult<()> {
        let bytes =
            serde_json::to_vec(s).map_err(|e| AppError::new(format!("Encode BSD session: {e}")))?;
        let tmp = self.directory.join(format!("{}.tmp", unique_id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| AppError::io("Create BSD session", e))?;
        let outcome = (|| {
            file.write_all(&bytes)
                .map_err(|e| AppError::io("Write BSD session", e))?;
            file.sync_all()
                .map_err(|e| AppError::io("Sync BSD session", e))?;
            fs::rename(&tmp, self.path(s)).map_err(|e| AppError::io("Publish BSD session", e))?;
            sync_directory(&self.directory)
        })();
        if outcome.is_err() {
            let _ = fs::remove_file(tmp);
        }
        outcome
    }
    fn remove(&self, s: &Session) -> AppResult<()> {
        fs::remove_file(self.path(s))
            .map_err(|e| AppError::io("Remove completed BSD session", e))?;
        sync_directory(&self.directory)
    }
    fn ensure_unused(&self, path: &Path) -> AppResult<()> {
        let identity = fs::metadata(path).ok();
        for s in self.records()? {
            if s.source == path
                || s.create_target.as_deref() == Some(path)
                || s.peer_sources.iter().any(|peer| peer == path)
                || {
                    #[cfg(target_os = "openbsd")]
                    {
                        s.mirror
                            .as_ref()
                            .is_some_and(|m| m.owns(path, identity.as_ref()))
                    }
                    #[cfg(not(target_os = "openbsd"))]
                    {
                        false
                    }
                }
                || identity
                    .as_ref()
                    .is_some_and(|m| m.dev() == s.identity.dev && m.ino() == s.identity.ino)
            {
                return Err(AppError::new(
                    "Container has an active or recoverable BSD session; close it first.",
                ));
            }
        }
        Ok(())
    }
}

fn private_owned(path: &Path, directory: bool) -> AppResult<()> {
    let m = fs::symlink_metadata(path).map_err(|e| AppError::io("Inspect private BSD state", e))?;
    if m.uid() != 0
        || m.mode() & 0o077 != 0
        || if directory {
            !m.is_dir()
        } else {
            !m.is_file() || m.nlink() != 1
        }
    {
        return Err(AppError::new(
            "BSD session state must be private, root-owned and not a symlink/hardlink.",
        ));
    }
    Ok(())
}

fn require_root() -> AppResult<()> {
    if crate::effective_uid() != 0 {
        return Err(AppError::new(
            "Native BSD file-container operations require root; run the CLI through sudo or doas.",
        ));
    }
    Ok(())
}

fn default_backend() -> VolumeType {
    #[cfg(target_os = "freebsd")]
    {
        VolumeType::Geli
    }
    #[cfg(target_os = "netbsd")]
    {
        VolumeType::Cgd
    }
    #[cfg(target_os = "openbsd")]
    {
        VolumeType::Softraid
    }
    #[cfg(target_os = "dragonfly")]
    {
        VolumeType::Luks1
    }
}

fn check_backend(kind: VolumeType) -> AppResult<VolumeType> {
    let kind = if kind == VolumeType::Auto {
        default_backend()
    } else {
        kind
    };
    if kind != default_backend()
        && !(cfg!(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "dragonfly"
        )) && kind == VolumeType::Veracrypt)
    {
        return Err(AppError::new(format!(
            "Backend {kind} is not implemented by this native BSD adapter."
        )));
    }
    Ok(kind)
}

fn mount_backend(kind: VolumeType, source: &Path) -> AppResult<VolumeType> {
    let backend = check_backend(kind)?;
    #[cfg(target_os = "freebsd")]
    if kind == VolumeType::Auto {
        // GELI metadata is identifiable without a credential. An absent native
        // header selects authentication with the private VeraCrypt engine;
        // malformed recognized native metadata is an error, never a fallback.
        return Ok(if native::probe_image(source)?.is_some() {
            VolumeType::Geli
        } else {
            VolumeType::Veracrypt
        });
    }
    #[cfg(target_os = "dragonfly")]
    if kind == VolumeType::Auto {
        return dragonfly_vc::classify(source);
    }
    let _ = source;
    Ok(backend)
}

#[cfg(not(target_os = "dragonfly"))]
fn attach_native_file(source: &Path, readonly: bool) -> AppResult<String> {
    #[cfg(target_os = "freebsd")]
    if let Some(sector_size) = native::probe_image(source)? {
        return native::attach_file_with_sector(source, readonly, sector_size);
    }
    native::attach_file(source, readonly)
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
fn managed_veracrypt(registry: &Registry) -> AppResult<Vec<veracrypt::ManagedAttachment>> {
    let mut managed = Vec::new();
    for session in registry.records()? {
        if session.backend != VolumeType::Veracrypt {
            continue;
        }
        let Some(mapped) = &session.mapped else {
            continue;
        };
        // A printed source path alone is not the encrypted file identity held
        // by an engine. Only a root journal, its unchanged source inode, and
        // the verified native mapping/auxiliary backing establish management.
        verify_session(&session)?;
        managed.push(veracrypt::ManagedAttachment {
            source: session.source.clone(),
            dev: session.identity.dev,
            ino: session.identity.ino,
            mapped: mapped.clone(),
        });
    }
    Ok(managed)
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
fn capture_veracrypt_recovery(registry: &Registry, session: &mut Session) -> AppResult<()> {
    // Journal the acquired mapping even if control/proof capture fails next.
    registry.save(session)?;
    let mapped = session
        .mapped
        .as_deref()
        .ok_or_else(|| AppError::new("VeraCrypt mapping missing during recovery capture."))?;
    session.veracrypt_proof = Some(veracrypt::capture_recovery_proof(&session.source, mapped)?);
    registry.save(session)
}

fn credential(
    password: Option<&str>,
    keyfile: Option<&Path>,
    confirm: bool,
) -> AppResult<Credential> {
    crate::credentials::validate(password, keyfile)?;
    if let Some(path) = keyfile {
        return Ok(Credential::Keyfile(
            fs::canonicalize(path).map_err(|e| AppError::io("Resolve keyfile", e))?,
        ));
    }
    let password = match password {
        Some(s) => s.to_owned(),
        None => crate::credentials::prompt_passphrase("Passphrase: ")
            .map_err(|e| AppError::io("Read passphrase", e))?,
    };
    crate::credentials::validate(Some(&password), None)?;
    if confirm {
        let repeated = crate::credentials::prompt_passphrase("Confirm passphrase: ")
            .map_err(|e| AppError::io("Confirm passphrase", e))?;
        if repeated != password {
            return Err(AppError::new("Passphrases do not match."));
        }
    }
    Ok(Credential::Password(password.into_bytes()))
}

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    command.validate_raid1c_options(std::env::consts::OS)?;
    if command.privilege_mode() != PrivilegeMode::Privileged {
        return Err(AppError::new(
            "Native BSD disk backends require Privileged mode.",
        ));
    }
    #[cfg(target_os = "openbsd")]
    if raid1c::handles(&command) {
        return raid1c::run(command, output);
    }
    match command {
        #[cfg(target_os = "freebsd")]
        CommandAction::Create(c) if c.volume_type == VolumeType::Pefs => create_pefs(c, output),
        #[cfg(target_os = "freebsd")]
        CommandAction::Mount(c)
            if c.volume_type == VolumeType::Pefs
                || (c.volume_type == VolumeType::Auto && c.source.is_dir()) =>
        {
            mount_pefs(c, output)
        }
        CommandAction::Create(c) => create(c, output),
        CommandAction::Mount(c) => mount(c, output),
        CommandAction::Unmount(c) => unmount(c, output),
        CommandAction::Keyslots(_) => Err(AppError::new("LUKS keyslot management requires Linux.")),
    }
}

/// PEFS uses the native privileged route even though its source is a directory.
/// A dangling .pefs.conf symlink is the native profile declaration, so inspect
/// the directory entry rather than following it with Path::exists().
pub(crate) fn handles_directory(command: &CommandAction) -> bool {
    #[cfg(target_os = "freebsd")]
    match command {
        CommandAction::Create(c) => c.volume_type == VolumeType::Pefs,
        CommandAction::Mount(c) => {
            c.volume_type == VolumeType::Pefs
                || (c.volume_type == VolumeType::Auto
                    && fs::symlink_metadata(c.source.join(".pefs.conf")).is_ok()
                    && c.source.join(".pefs.db").is_file())
        }
        CommandAction::Unmount(c) => c.volume_type == VolumeType::Pefs,
        _ => false,
    }
    #[cfg(not(target_os = "freebsd"))]
    {
        let _ = command;
        false
    }
}

#[cfg(target_os = "netbsd")]
fn params_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".cgdparams");
    PathBuf::from(name)
}

fn create(c: CreateCommand, output: Output) -> AppResult<()> {
    let owner = crate::new_filesystem::invoking_owner()?;
    let backend = check_backend(c.volume_type)?;
    if c.source_kind != SourceKind::File {
        return Err(AppError::new(
            "BSD native creation supports file containers only.",
        ));
    }
    let size = crate::size::size_gb_to_mib(
        c.size_gb
            .as_deref()
            .ok_or_else(|| AppError::new("File-container creation requires --size."))?,
    )?
    .checked_mul(1024 * 1024)
    .ok_or_else(|| AppError::new("Container size is too large."))?;
    if size < 64 * 1024 * 1024 {
        return Err(AppError::new(
            "BSD native file containers require at least 64 MiB.",
        ));
    }
    let target = absolute_destination(&c.target)?;
    if fs::symlink_metadata(&target).is_ok_and(|m| !m.is_file()) {
        return Err(AppError::new(
            "Refusing to overwrite a symlink or non-file container target.",
        ));
    }
    if target.exists() && !c.force {
        return Err(AppError::new(
            "Container target exists; use --force to replace it.",
        ));
    }
    #[cfg(target_os = "netbsd")]
    if target.exists() || params_path(&target).exists() {
        return Err(AppError::new("CGD replacement is refused: its image and required parameters must be published together to a new path."));
    }
    let registry = Registry::open()?;
    registry.ensure_unused(&target)?;
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    if backend == VolumeType::Veracrypt {
        veracrypt::backend()?;
        if target.exists() {
            veracrypt::ensure_source_available_with_managed(
                &target,
                &managed_veracrypt(&registry)?,
            )?;
        }
    }
    let credential = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        c.passphrase.is_none() && c.keyfile.is_none(),
    )?;
    #[cfg(target_os = "dragonfly")]
    let vc_program = if backend == VolumeType::Veracrypt {
        dragonfly_vc::validate_create_size(size)?;
        Some(dragonfly_vc::validate(&credential, Some(&target))?)
    } else {
        None
    };
    let parent = target
        .parent()
        .ok_or_else(|| AppError::new("Container needs a parent directory."))?;
    let mut stage = FileStage::create(parent)?;
    let source = stage.source();
    let prepared = (|| -> AppResult<(File, Identity)> {
        let file = stage.create_image()?;
        #[cfg(not(target_os = "dragonfly"))]
        crate::new_filesystem::assign_file(&file, owner)?;
        file.set_len(size).map_err(|e| AppError::io("Size staged container", e))?;
        let identity = Identity::from_file(&file)?;
        Ok((file, identity))
    })();
    let (file, identity) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            stage.discard();
            return Err(error);
        }
    };
    let mut session = Session {
        version: 1,
        id: unique_id(),
        backend,
        source: source.clone(),
        create_target: Some(target.clone()),
        identity,
        backing: None,
        #[cfg(target_os = "freebsd")]
        freebsd_detach: None,
        #[cfg(target_os = "netbsd")]
        netbsd_detach: None,
        #[cfg(target_os = "openbsd")]
        openbsd_detach: None,
        #[cfg(target_os = "dragonfly")]
        dragonfly_backing_reservation: None,
        #[cfg(target_os = "dragonfly")]
        dragonfly_backing_detach_requested: false,
        #[cfg(target_os = "dragonfly")]
        dragonfly_layer: None,
        #[cfg(target_os = "dragonfly")]
        dragonfly_veracrypt: None,
        mapped: None,
        mounted: None,
        peer_sources: Vec::new(),
        #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
        veracrypt_proof: None,
        #[cfg(target_os = "openbsd")]
        mirror: None,
        intended_mountpoint: None,
        initialization_directory: None,
        readonly: false,
    };
    registry.save(&session)?;
    let initialized = (|| {
        stage.verify_path()?;
        verify_session(&session)?;
        #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
        if backend == VolumeType::Veracrypt {
            veracrypt::format_with_managed(
                &source,
                size / (1024 * 1024),
                c.cipher,
                &credential,
                &managed_veracrypt(&registry)?,
            )?;
            session.mapped = Some(veracrypt::unlock_with_managed(
                &source,
                false,
                &credential,
                &managed_veracrypt(&registry)?,
            )?);
            capture_veracrypt_recovery(&registry, &mut session)?;
            initialize_acquired_mapping(&registry, &session, veracrypt::make_filesystem)?;
            initialize_new_filesystem_owner(&registry, &mut session, owner)?;
            return Ok(());
        }
        #[cfg(target_os = "freebsd")]
        native::allocate_image(&file, size)?;
        #[cfg(target_os = "dragonfly")]
        private_owned(&source, false)?;
        #[cfg(target_os = "dragonfly")]
        native::attach_backing(&registry, &mut session)?;
        #[cfg(not(target_os = "dragonfly"))]
        {
            session.backing = Some(native::attach_file(&source, false)?);
            registry.save(&session)?;
        }
        verify_session(&session)?;
        let backing = session.backing.as_deref().unwrap();
        #[cfg(target_os = "netbsd")]
        let params = {
            let formatted = native::format(backing, c.cipher, &credential, &params_path(&source));
            let params = stage.capture_params();
            formatted?;
            let params = params?;
            session.mapped = Some(native::unlock_initial(
                backing,
                &credential,
                &params_path(&source),
            )?);
            params
        };
        #[cfg(target_os = "openbsd")]
        {
            session.mapped = Some(native::format(backing, c.cipher, &credential)?);
        }
        #[cfg(target_os = "freebsd")]
        {
            native::format(backing, c.cipher, &credential)?;
            session.mapped = Some(native::unlock(backing, false, &credential)?);
        }
        #[cfg(target_os = "dragonfly")]
        if let Some(program) = &vc_program {
            dragonfly_vc::format(&registry, &session, program, size, c.cipher, &credential)?;
            session.mapped = Some(dragonfly_vc::activate(
                &registry,
                &mut session,
                program,
                &credential,
            )?);
            return initialize_acquired_mapping(&registry, &session, |mapped| {
                dragonfly_vc::make_filesystem(&registry, mapped, size)
            });
        }
        #[cfg(target_os = "dragonfly")]
        {
            native::format(backing, c.cipher, &credential)?;
            session.mapped = Some(dragonfly::linear::activate(
                &registry, &mut session, false, &credential,
            )?);
        }
        // Persist every acquired crypto device before filesystem work. Failed
        // or interrupted newfs must leave an explicitly recoverable mapping.
        initialize_acquired_mapping(&registry, &session, native::make_filesystem)?;
        #[cfg(target_os = "netbsd")]
        crate::new_filesystem::assign_file(&params, owner)?;
        initialize_new_filesystem_owner(&registry, &mut session, owner)?;
        Ok(())
    })();
    let cleaned = cleanup(&registry, &mut session);
    if let Err(error) = cleaned {
        return Err(AppError::new(format!(
            "{}; resources retained for recovery in {}: {error}",
            initialized
                .err()
                .map(|e: AppError| e.message)
                .unwrap_or_else(|| "Container initialized".into()),
            stage.path().display()
        )));
    }
    if let Err(error) = initialized {
        stage.discard();
        return Err(error);
    }
    #[cfg(target_os = "dragonfly")]
    {
        verify_session(&session)?;
        crate::new_filesystem::assign_file(&file, owner)?;
    }
    file.sync_all()
        .map_err(|e| AppError::io("Sync initialized container", e))?;
    #[cfg(target_os = "netbsd")]
    let published = stage.publish_cgd(&target, &params_path(&target));
    #[cfg(not(target_os = "netbsd"))]
    let published = stage.publish(&target, c.force);
    stage.finish(published)?;
    output.success(&format!(
        "Created {backend} file container: {}",
        target.display()
    ));
    Ok(())
}

/// A new filesystem's root belongs to the invoking login user. Its temporary
/// mount is recorded before changing ownership and closed through normal cleanup.
fn initialize_new_filesystem_owner(
    registry: &Registry,
    session: &mut Session,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    // The OS mount-name limit must not constrain encrypted-image parent paths.
    let target = registry.directory.join(format!("init-{}", session.id));
    session.initialization_directory = Some(InitializationDirectory {
        path: target.clone(),
        identity: None,
    });
    registry.save(session)?;
    fs::create_dir(&target)
        .map_err(|e| AppError::io("Create new filesystem initialization mountpoint", e))?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
        .map_err(|e| AppError::io("Protect new filesystem mountpoint", e))?;
    session.initialization_directory.as_mut().unwrap().identity = Some(Identity::read(&target)?);
    session.intended_mountpoint = Some(target.to_owned());
    registry.save(session)?;
    sync_directory(&registry.directory)?;
    verify_session(session)?;
    let mapped = session
        .mapped
        .as_deref()
        .ok_or_else(|| AppError::new("New filesystem has no recorded crypto mapping."))?;
    mount_filesystem(mapped, &target, false)?;
    let mounted = mounts()?
        .into_iter()
        .find(|m| m.target == target)
        .ok_or_else(|| {
            AppError::new("New filesystem mount identity missing; preserve initialization journal.")
        })?;
    if mounted.source != Path::new(mapped) || mounted.filesystem != filesystem() || mounted.readonly
    {
        return Err(AppError::new(
            "New filesystem mount identity differs; preserve initialization journal.",
        ));
    }
    session.mounted = Some(mounted.clone());
    registry.save(session)?;
    require_write_journal(&mounted)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(target)
        .map_err(|e| AppError::io("Open new filesystem root", e))?;
    if !mounts()?.iter().any(|m| m == &mounted) {
        return Err(AppError::new(
            "New filesystem mount changed before ownership initialization.",
        ));
    }
    crate::new_filesystem::assign_root(&directory, owner)
}

fn remove_initialization_directory(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(record) = &session.initialization_directory else {
        return Ok(());
    };
    if record.path != registry.directory.join(format!("init-{}", session.id)) {
        return Err(AppError::new(
            "Initialization directory is outside its reserved journal path.",
        ));
    }
    if mounts()?.iter().any(|m| m.target == record.path) {
        return Err(AppError::new(
            "Initialization directory remains mounted; preserve its journal.",
        ));
    }
    match fs::symlink_metadata(&record.path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(AppError::io("Inspect initialization directory", e)),
        Ok(metadata) => {
            if !metadata.is_dir()
                || !record
                    .identity
                    .as_ref()
                    .is_some_and(|id| id.matches(&record.path))
            {
                return Err(AppError::new(
                    "Initialization directory identity changed; preserve its journal.",
                ));
            }
            private_owned(&record.path, true)?;
            // remove_dir refuses any unexpected entry; never traverse/recurse.
            fs::remove_dir(&record.path)
                .map_err(|e| AppError::io("Remove empty initialization directory", e))?;
            sync_directory(&registry.directory)?;
        }
    }
    session.initialization_directory = None;
    registry.save(session)
}

fn absolute_destination(path: &Path) -> AppResult<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| AppError::new("Container destination needs a filename."))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent =
        fs::canonicalize(parent).map_err(|e| AppError::io("Resolve destination parent", e))?;
    Ok(parent.join(name))
}

/// The injection boundary is after a real crypto attachment has been acquired.
/// Persist and revalidate its identity before any filesystem initializer runs.
fn initialize_acquired_mapping(
    registry: &Registry,
    session: &Session,
    initialize: impl FnOnce(&str) -> AppResult<()>,
) -> AppResult<()> {
    let mapped = session
        .mapped
        .as_deref()
        .ok_or_else(|| AppError::new("Missing newly acquired crypto device."))?;
    registry.save(session)?;
    verify_session(session)?;
    initialize(mapped)
}

fn mount(c: MountCommand, output: Output) -> AppResult<()> {
    if c.tag.is_some() {
        return Err(AppError::new("Native BSD mounts do not support tags."));
    }
    let source = fs::canonicalize(&c.source).map_err(|e| AppError::io("Resolve container", e))?;
    if !fs::metadata(&source)
        .map_err(|e| AppError::io("Inspect container", e))?
        .is_file()
    {
        return Err(AppError::new(
            "Native BSD disk containers must be regular files.",
        ));
    }
    let identity = Identity::read(&source)?;
    let backend = mount_backend(c.volume_type, &source)?;
    let registry = Registry::open()?;
    registry.ensure_unused(&source)?;
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    if backend == VolumeType::Veracrypt {
        veracrypt::ensure_source_available_with_managed(&source, &managed_veracrypt(&registry)?)?;
    }
    #[cfg(target_os = "dragonfly")]
    if backend == VolumeType::Veracrypt && c.readonly {
        return Err(AppError::new(
            "DragonFly VeraCrypt read-only mode is unavailable because the native mapper cannot enforce it.",
        ));
    }
    let credential = credential(c.passphrase.as_deref(), c.keyfile.as_deref(), false)?;
    #[cfg(target_os = "dragonfly")]
    let vc = (backend == VolumeType::Veracrypt)
        .then(|| -> AppResult<_> {
            Ok((
                dragonfly_vc::validate(&credential, Some(&source))?,
                crate::new_filesystem::invoking_owner()?,
            ))
        })
        .transpose()?;
    if !c.mountpoint.exists() {
        fs::create_dir_all(&c.mountpoint).map_err(|e| AppError::io("Create mountpoint", e))?;
    }
    if fs::symlink_metadata(&c.mountpoint)
        .map_err(|e| AppError::io("Inspect mountpoint", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new("Mountpoint must not be a symlink."));
    }
    let target =
        fs::canonicalize(&c.mountpoint).map_err(|e| AppError::io("Resolve mountpoint", e))?;
    // Native mount tables truncate longer paths on these kernels. Refuse them
    // before mounting instead of losing an exact cleanup target.
    #[cfg(any(target_os = "openbsd", target_os = "dragonfly"))]
    {
        use std::os::unix::ffi::OsStrExt;
        let limit = if cfg!(target_os = "openbsd") { 90 } else { 80 };
        if target.as_os_str().as_bytes().len() >= limit {
            return Err(AppError::new(
                "Mountpoint exceeds this BSD's native mount identity limit.",
            ));
        }
    }
    if mounts()?.iter().any(|m| m.target == target) {
        return Err(AppError::new("Mountpoint is already mounted."));
    }
    if fs::read_dir(&target)
        .map_err(|e| AppError::io("Inspect mountpoint", e))?
        .next()
        .is_some()
    {
        return Err(AppError::new("Mountpoint must be empty."));
    }
    let mut session = Session {
        version: 1,
        id: unique_id(),
        backend,
        source: source.clone(),
        create_target: None,
        identity,
        backing: None,
        #[cfg(target_os = "freebsd")]
        freebsd_detach: None,
        #[cfg(target_os = "netbsd")]
        netbsd_detach: None,
        #[cfg(target_os = "openbsd")]
        openbsd_detach: None,
        #[cfg(target_os = "dragonfly")]
        dragonfly_backing_reservation: None,
        #[cfg(target_os = "dragonfly")]
        dragonfly_backing_detach_requested: false,
        #[cfg(target_os = "dragonfly")]
        dragonfly_layer: None,
        #[cfg(target_os = "dragonfly")]
        dragonfly_veracrypt: None,
        mapped: None,
        mounted: None,
        peer_sources: Vec::new(),
        #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
        veracrypt_proof: None,
        #[cfg(target_os = "openbsd")]
        mirror: None,
        intended_mountpoint: Some(target.clone()),
        initialization_directory: None,
        readonly: c.readonly,
    };
    registry.save(&session)?;
    let result = (|| {
        #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
        if backend == VolumeType::Veracrypt {
            session.mapped = Some(veracrypt::unlock_with_managed(
                &source,
                c.readonly,
                &credential,
                &managed_veracrypt(&registry)?,
            )?);
            capture_veracrypt_recovery(&registry, &mut session)?;
        }
        if session.mapped.is_none() {
            #[cfg(target_os = "dragonfly")]
            native::attach_backing(&registry, &mut session)?;
            #[cfg(not(target_os = "dragonfly"))]
            {
                session.backing = Some(attach_native_file(&source, c.readonly)?);
                registry.save(&session)?;
            }
            verify_session(&session)?;
            #[cfg(target_os = "openbsd")]
            if c.volume_type == VolumeType::Auto
                && !native::probe_crypto_header(session.backing.as_deref().unwrap())?
            {
                // Inspect the existing native partition through its kernel
                // layout, without invoking bioctl on an unknown format. Release
                // that owned vnode before the authenticated VeraCrypt route.
                openbsd::detach_backing(&registry, &mut session)?;
                veracrypt::ensure_source_available_with_managed(
                    &source,
                    &managed_veracrypt(&registry)?,
                )?;
                session.backend = VolumeType::Veracrypt;
                registry.save(&session)?;
                session.mapped = Some(veracrypt::unlock_with_managed(
                    &source,
                    c.readonly,
                    &credential,
                    &managed_veracrypt(&registry)?,
                )?);
                capture_veracrypt_recovery(&registry, &mut session)?;
            }
            if session.mapped.is_none() {
                #[cfg(target_os = "netbsd")]
                {
                    session.mapped = Some(native::unlock(
                        session.backing.as_deref().unwrap(),
                        c.readonly,
                        &credential,
                        &params_path(&source),
                    )?);
                }
                #[cfg(target_os = "dragonfly")]
                {
                    session.mapped = Some(if let Some((program, _)) = &vc {
                        dragonfly_vc::activate(&registry, &mut session, program, &credential)?
                    } else {
                        dragonfly::linear::activate(
                            &registry,
                            &mut session,
                            c.readonly,
                            &credential,
                        )?
                    });
                }
                #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
                {
                    session.mapped = Some(native::unlock(
                        session.backing.as_deref().unwrap(),
                        c.readonly,
                        &credential,
                    )?);
                }
                registry.save(&session)?;
            }
        }
        #[cfg(target_os = "dragonfly")]
        if let Some((_, owner)) = &vc {
            dragonfly_vc::mount(&registry, &session, &target, *owner)?;
        } else {
            mount_filesystem(session.mapped.as_deref().unwrap(), &target, c.readonly)?;
        }
        #[cfg(not(target_os = "dragonfly"))]
        mount_filesystem(session.mapped.as_deref().unwrap(), &target, c.readonly)?;
        session.mounted = mounts()?.into_iter().find(|m| m.target == target);
        let mounted = session.mounted.as_ref().ok_or_else(|| AppError::new("Mount succeeded but native mount identity is missing; preserve session for recovery."))?;
        if mounted.source != Path::new(session.mapped.as_deref().unwrap())
            || mounted.readonly != c.readonly
        {
            return Err(AppError::new(
                "Unexpected mounted device or read-only mode; preserve session for recovery.",
            ));
        }
        registry.save(&session)?;
        #[cfg(target_os = "dragonfly")]
        if let Some((_, owner)) = &vc {
            if mounted.filesystem != "msdos" {
                return Err(AppError::new(
                    "Unexpected mounted DragonFly VeraCrypt filesystem.",
                ));
            }
            dragonfly_vc::verify_access(&target, *owner)?;
        }
        require_write_journal(mounted)?;
        Ok(())
    })();
    if let Err(error) = result {
        return match cleanup(&registry, &mut session) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(AppError::new(format!(
                "{error}; recoverable BSD session {} retained: {cleanup}",
                session.id
            ))),
        };
    }
    output.success(&format!(
        "Mounted {} at {}",
        session.backend,
        target.display()
    ));
    Ok(())
}

#[cfg(target_os = "freebsd")]
fn create_pefs(c: CreateCommand, output: Output) -> AppResult<()> {
    if c.source_kind != SourceKind::Directory || c.size_gb.is_some() {
        return Err(AppError::new(
            "PEFS creation requires a directory and does not take --size.",
        ));
    }
    let owner = crate::new_filesystem::invoking_owner()?;
    let registry = Registry::open()?;
    pefs::check_runtime()?;
    let target = absolute_destination(&c.target)?;
    registry.ensure_unused(&target)?;
    let credential = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        c.passphrase.is_none() && c.keyfile.is_none(),
    )?;
    // mkdir reserves the destination without replacing an existing directory,
    // including an empty one. A failed initialization remains private for
    // inspection; --force never deletes a directory's existing contents.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&target)
        .map_err(|e| AppError::io("Reserve new private PEFS directory", e))?;
    if let Err(error) = pefs::initialize(&target, c.cipher, &credential) {
        return Err(AppError::new(format!("{error}; incomplete private PEFS directory retained at {}. Inspect it before retrying.", target.display())));
    }
    let database = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(target.join(".pefs.db"))
        .map_err(|e| AppError::io("Open new PEFS database", e))?;
    crate::new_filesystem::assign_file(&database, owner)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(&target)
        .map_err(|e| AppError::io("Open new PEFS root", e))?;
    crate::new_filesystem::assign_root(&directory, owner)?;
    sync_directory(&target)?;
    sync_directory(target.parent().unwrap())?;
    output.success(&format!("Created PEFS directory: {}", target.display()));
    Ok(())
}

#[cfg(target_os = "freebsd")]
fn mount_pefs(c: MountCommand, output: Output) -> AppResult<()> {
    if c.tag.is_some() {
        return Err(AppError::new("PEFS mounts do not support tags."));
    }
    let registry = Registry::open()?;
    pefs::check_runtime()?;
    let source =
        fs::canonicalize(&c.source).map_err(|e| AppError::io("Resolve PEFS directory", e))?;
    if !source.is_dir() {
        return Err(AppError::new("PEFS source must be a directory."));
    }
    registry.ensure_unused(&source)?;
    let identity = Identity::read(&source)?;
    let credential = credential(c.passphrase.as_deref(), c.keyfile.as_deref(), false)?;
    if !c.mountpoint.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&c.mountpoint)
            .map_err(|e| AppError::io("Create PEFS mountpoint", e))?;
    }
    if fs::symlink_metadata(&c.mountpoint)
        .map_err(|e| AppError::io("Inspect PEFS mountpoint", e))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::new("PEFS mountpoint must not be a symlink."));
    }
    let target =
        fs::canonicalize(&c.mountpoint).map_err(|e| AppError::io("Resolve PEFS mountpoint", e))?;
    if source.starts_with(&target) || target.starts_with(&source) {
        return Err(AppError::new(
            "PEFS source and mountpoint must be disjoint directories.",
        ));
    }
    if mounts()?.iter().any(|m| m.target == target)
        || fs::read_dir(&target)
            .map_err(|e| AppError::io("Inspect PEFS mountpoint", e))?
            .next()
            .is_some()
    {
        return Err(AppError::new(
            "PEFS mountpoint must be empty and unmounted.",
        ));
    }
    let mut session = Session {
        version: 1,
        id: unique_id(),
        backend: VolumeType::Pefs,
        source: source.clone(),
        create_target: None,
        identity,
        backing: None,
        freebsd_detach: None,
        mapped: None,
        mounted: None,
        peer_sources: Vec::new(),
        #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
        veracrypt_proof: None,
        #[cfg(target_os = "openbsd")]
        mirror: None,
        intended_mountpoint: Some(target.clone()),
        initialization_directory: None,
        readonly: c.readonly,
    };
    registry.save(&session)?;
    let attached = pefs::attach(&source, &target, c.readonly, &credential);
    // Capture any owned mount even after an authentication/cleanup error, so a
    // busy mount stays recoverable instead of losing its session record.
    if let Some(mounted) = mounts()?.into_iter().find(|m| m.target == target) {
        if mounted.source != source || mounted.filesystem != "pefs" {
            return Err(AppError::new(
                "Unexpected mount occupies the PEFS target; session retained for recovery.",
            ));
        }
        session.mounted = Some(mounted);
        registry.save(&session)?;
    }
    let attached = attached.and_then(|()| {
        if session.mounted.is_none() || pefs::is_readonly(&target)? != c.readonly {
            Err(AppError::new(
                "PEFS mount identity or read-only mode is missing.",
            ))
        } else {
            Ok(())
        }
    });
    if let Err(error) = attached {
        return match cleanup(&registry, &mut session) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(AppError::new(format!(
                "{error}; recoverable PEFS session {} retained: {cleanup}",
                session.id
            ))),
        };
    }
    output.success(&format!("Mounted PEFS at {}", target.display()));
    Ok(())
}

fn filesystem() -> &'static str {
    if cfg!(any(target_os = "netbsd", target_os = "openbsd")) {
        "ffs"
    } else {
        "ufs"
    }
}

fn mount_filesystem(mapped: &str, target: &Path, readonly: bool) -> AppResult<()> {
    let mut argv = args(&["-t", filesystem(), "-o", mount_options(readonly)]);
    argv.push(mapped.into());
    argv.push(target.as_os_str().to_owned());
    exec("/sbin/mount", &argv, None)?;
    Ok(())
}

fn mount_options(readonly: bool) -> &'static str {
    if readonly {
        "ro"
    } else if cfg!(target_os = "netbsd") {
        "rw,log"
    } else if cfg!(target_os = "openbsd") {
        // Synchronous FFS writes kept cut fixtures recoverable by manual preen.
        "rw,sync"
    } else {
        "rw"
    }
}

fn require_write_journal(_mounted: &Mounted) -> AppResult<()> {
    // A NetBSD kernel without WAPBL can silently clear the requested flag.
    // Plain FFS fsync does not flush the cache of the underlying CGD device.
    #[cfg(target_os = "netbsd")]
    if !_mounted.readonly && !_mounted.journaled {
        return Err(AppError::new(
            "NetBSD CGD writable mounts require active FFS journaling (WAPBL).",
        ));
    }
    Ok(())
}

fn unmount(c: UnmountCommand, output: Output) -> AppResult<()> {
    unmount_matching(c, output, false)
}

#[cfg(target_os = "freebsd")]
pub(crate) fn unmount_source(c: UnmountCommand, output: Output) -> AppResult<()> {
    unmount_matching(c, output, true)
}

fn unmount_matching(c: UnmountCommand, output: Output, source_only: bool) -> AppResult<()> {
    if c.tag.is_some() {
        return Err(AppError::new("Native BSD unmount does not support tags."));
    }
    let registry = Registry::open()?;
    let target = fs::canonicalize(&c.target)
        .or_else(|_| absolute_destination(&c.target))
        .unwrap_or(c.target);
    let identity = fs::metadata(&target).ok();
    let mut found = registry.records()?.into_iter().filter(|s| {
        if source_only {
            return s.source == target;
        }
        s.source == target
            || s.create_target.as_ref() == Some(&target)
            || s.peer_sources.iter().any(|peer| peer == &target)
            || {
                #[cfg(target_os = "openbsd")]
                {
                    s.mirror
                        .as_ref()
                        .is_some_and(|m| m.owns(&target, identity.as_ref()))
                }
                #[cfg(not(target_os = "openbsd"))]
                {
                    false
                }
            }
            || s.mounted.as_ref().is_some_and(|m| m.target == target)
            || (cfg!(target_os = "dragonfly")
                && s.backend == VolumeType::Veracrypt
                && s.intended_mountpoint.as_ref() == Some(&target))
            || s.mapped.as_ref().is_some_and(|m| Path::new(m) == target)
            || identity
                .as_ref()
                .is_some_and(|m| m.dev() == s.identity.dev && m.ino() == s.identity.ino)
    });
    let mut session = found
        .next()
        .ok_or_else(|| AppError::new("No owned BSD session matches this target."))?;
    if found.next().is_some() {
        return Err(AppError::new(
            "Ambiguous BSD session target; refusing cleanup.",
        ));
    }
    if c.volume_type != VolumeType::Auto && c.volume_type != session.backend {
        return Err(AppError::new(
            "Requested backend differs from the recorded BSD session.",
        ));
    }
    cleanup(&registry, &mut session)?;
    if session.create_target.is_some() {
        output.success(&format!(
            "Closed the interrupted BSD creation. Its staged image is at {}.",
            session.source.display()
        ));
    } else {
        output.success("Closed native BSD container.");
    }
    Ok(())
}

fn verify_session(s: &Session) -> AppResult<()> {
    #[cfg(target_os = "openbsd")]
    if s.backend == VolumeType::Raid1c {
        return raid1c::verify(s);
    }
    if !s.identity.matches(&s.source) {
        return Err(AppError::new(
            "Container identity changed; refusing to detach any recorded resource.",
        ));
    }
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    if s.backend == VolumeType::Veracrypt {
        if let Some(mapped) = &s.mapped {
            if let Some(proof) = &s.veracrypt_proof {
                veracrypt::verify_recovery_proof(&s.source, mapped, proof)?;
            } else {
                // Old journals require the live engine's complete identity.
                veracrypt::verify_mapping(&s.source, mapped)?;
            }
        }
        return Ok(());
    }
    #[cfg(target_os = "freebsd")]
    if s.backend == VolumeType::Pefs {
        if let Some(mounted) = &s.mounted {
            pefs::verify_mapping(&s.source, &mounted.target)?;
        }
        return Ok(());
    }
    if let Some(backing) = &s.backing {
        native::verify_backing(backing, &s.source)?;
    }
    #[cfg(target_os = "dragonfly")]
    {
        if s.backend == VolumeType::Veracrypt {
            return dragonfly_vc::verify(s);
        }
        dragonfly::linear::verify(Path::new(STATE), s)?;
    }
    if let (Some(mapped), Some(backing)) = (&s.mapped, &s.backing) {
        #[cfg(target_os = "dragonfly")]
        let backing = &*dragonfly::linear::crypto_backing(s, backing);
        native::verify_mapping(mapped, backing)?;
    }
    Ok(())
}

fn cleanup(registry: &Registry, s: &mut Session) -> AppResult<()> {
    #[cfg(target_os = "openbsd")]
    if s.backend == VolumeType::Raid1c {
        return raid1c::cleanup(registry, s);
    }
    #[cfg(target_os = "freebsd")]
    freebsd::reconcile_detach(registry, s)?;
    #[cfg(target_os = "openbsd")]
    openbsd::reconcile_detach(registry, s)?;
    #[cfg(target_os = "netbsd")]
    netbsd::reconcile_detach(registry, s)?;
    #[cfg(target_os = "dragonfly")]
    {
        dragonfly::reconcile_backing_reservation(registry, s)?;
        if s.backend == VolumeType::Veracrypt {
            dragonfly_vc::reconcile(registry, s)?;
        } else {
            dragonfly::linear::reconcile_activation(registry, s)?;
        }
        dragonfly::reconcile_backing_detach(registry, s)?;
    }
    verify_session(s)?;
    if s.mounted.is_none() {
        if let Some(target) = &s.intended_mountpoint {
            if mounts()?.iter().any(|m| &m.target == target) {
                return Err(AppError::new("An unrecorded mount occupies the intended target; preserve this session for explicit recovery."));
            }
        }
    }
    if let Some(recorded) = &s.mounted {
        let current = mounts()?;
        if let Some(now) = current.iter().find(|m| m.target == recorded.target) {
            if now != recorded {
                return Err(AppError::new(
                    "Mount identity changed; refusing to unmount its replacement.",
                ));
            }
            let argv = [recorded.target.as_os_str().to_owned()];
            #[cfg(target_os = "dragonfly")]
            if s.backend == VolumeType::Veracrypt {
                exec_dragonfly_locked(registry, Path::new("/sbin/umount"), &argv, None, Duration::from_secs(300))?;
            } else {
                exec("/sbin/umount", &argv, None)?;
            }
            #[cfg(not(target_os = "dragonfly"))]
            exec("/sbin/umount", &argv, None)?;
        }
        s.mounted = None;
        s.intended_mountpoint = None;
        registry.save(s)?;
    }
    if let Some(mapped) = &s.mapped {
        if mounts()?.iter().any(|m| m.source == Path::new(mapped)) {
            return Err(AppError::new(
                "The crypto device still has a mounted filesystem; leave session recoverable.",
            ));
        }
        #[cfg(target_os = "netbsd")]
        {
            s.netbsd_detach = Some(netbsd::DetachIntent::Cgd);
            registry.save(s)?;
        }
        #[cfg(target_os = "openbsd")]
        if s.backend == VolumeType::Softraid {
            s.openbsd_detach = Some(openbsd::DetachIntent::Volume);
            registry.save(s)?;
        }
        #[cfg(target_os = "freebsd")]
        if s.backend == VolumeType::Geli {
            s.freebsd_detach = Some(freebsd::DetachIntent::Geli);
            registry.save(s)?;
        }
        #[cfg(target_os = "dragonfly")]
        if s.backend == VolumeType::Veracrypt {
            dragonfly_vc::remove(registry, s)?;
        } else {
            close_mapping(s, mapped)?;
        }
        #[cfg(not(target_os = "dragonfly"))]
        close_mapping(s, mapped)?;
        s.mapped = None;
        #[cfg(target_os = "freebsd")]
        {
            s.freebsd_detach = None;
        }
        #[cfg(target_os = "netbsd")]
        {
            s.netbsd_detach = None;
        }
        #[cfg(target_os = "openbsd")]
        {
            s.openbsd_detach = None;
        }
        registry.save(s)?;
    }
    #[cfg(target_os = "dragonfly")]
    dragonfly::linear::release(registry, s)?;
    #[cfg(target_os = "openbsd")]
    if s.backend == VolumeType::Softraid && s.backing.is_some() {
        openbsd::detach_backing(registry, s)?;
    }
    if let Some(backing) = &s.backing {
        native::verify_backing(backing, &s.source)?;
        #[cfg(target_os = "dragonfly")]
        {
            s.dragonfly_backing_detach_requested = true;
            registry.save(s)?;
        }
        #[cfg(target_os = "netbsd")]
        {
            s.netbsd_detach = Some(netbsd::DetachIntent::Vnd);
            registry.save(s)?;
        }
        #[cfg(target_os = "freebsd")]
        if s.backend == VolumeType::Geli {
            s.freebsd_detach = Some(freebsd::DetachIntent::Md);
            registry.save(s)?;
        }
        native::detach_file(backing)?;
        s.backing = None;
        #[cfg(target_os = "freebsd")]
        {
            s.freebsd_detach = None;
        }
        #[cfg(target_os = "netbsd")]
        {
            s.netbsd_detach = None;
        }
        registry.save(s)?;
    }
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    if s.backend == VolumeType::Veracrypt && s.mapped.is_none() && veracrypt::attached(&s.source)? {
        return Err(AppError::new("A VeraCrypt attachment exists without a recorded device identity; preserve this session for explicit recovery."));
    }
    remove_initialization_directory(registry, s)?;
    registry.remove(s)
}

fn close_mapping(s: &Session, mapped: &str) -> AppResult<()> {
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    if s.backend == VolumeType::Veracrypt {
        return if let Some(proof) = &s.veracrypt_proof {
            veracrypt::close_with_proof(&s.source, mapped, proof)
        } else {
            veracrypt::close(&s.source)
        };
    }
    let _ = s;
    native::close(mapped)
}

pub fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    // Native sessions are privileged state. An ordinary user's global probe
    // must still be able to merge its independent userspace folder sessions.
    if crate::effective_uid() != 0 {
        return Ok(Vec::new());
    }
    if !Path::new(STATE).exists() {
        return Ok(Vec::new());
    }
    let registry = Registry::open()?;
    let current = mounts()?;
    registry
        .records()?
        .into_iter()
        .map(|s| {
            verify_session(&s)?;
            let mounted = s
                .mounted
                .as_ref()
                .filter(|m| current.iter().any(|now| now == *m));
            Ok(ActiveVolume {
                privilege_mode: PrivilegeMode::Privileged,
                source_path: Some(s.source),
                readonly: s.readonly,
                mapper_name: format!("lurker_{}", s.id),
                mapper_path: s
                    .mapped
                    .or(s.backing)
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                mountpoint: mounted.map(|m| m.target.clone()),
                filesystem_type: mounted.map(|m| m.filesystem.clone()),
            })
        })
        .collect()
}

pub fn probe_system() -> AppResult<SystemProbe> {
    let native = if cfg!(target_os = "freebsd") {
        vec!["/sbin/mdconfig", "/sbin/geli", "/sbin/geom"]
    } else if cfg!(target_os = "netbsd") {
        vec!["/usr/sbin/vndconfig", "/sbin/cgdconfig"]
    } else if cfg!(target_os = "openbsd") {
        vec!["/sbin/vnconfig", "/sbin/bioctl", "/sbin/disklabel"]
    } else {
        vec!["/usr/sbin/vnconfig", "/sbin/cryptsetup", "/sbin/dmsetup"]
    };
    let mut tools: Vec<_> = native
        .into_iter()
        .chain(["/sbin/newfs", "/sbin/mount", "/sbin/umount"])
        .map(|s| ToolStatus {
            name: Path::new(s)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: Path::new(s).is_file().then(|| PathBuf::from(s)),
            required: true,
        })
        .collect();
    if cfg!(target_os = "dragonfly") {
        tools.push(ToolStatus {
            name: "mount_msdos".into(),
            path: Path::new("/sbin/mount_msdos")
                .is_file()
                .then(|| "/sbin/mount_msdos".into()),
            required: false,
        });
        tools.push(ToolStatus {
            name: "newfs_msdos".into(),
            path: Path::new("/sbin/newfs_msdos")
                .is_file()
                .then(|| "/sbin/newfs_msdos".into()),
            required: false,
        });
    }
    #[cfg(target_os = "dragonfly")]
    tools.push(ToolStatus {
        name: "lurker-tcplay".into(),
        path: dragonfly_vc::backend().ok(),
        required: false,
    });
    Ok(SystemProbe {
        is_root: crate::effective_uid() == 0,
        tools,
    })
}

pub(super) fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

/// Tool output is bounded and secrets are never interpolated into diagnostics.
pub(super) fn exec(program: &str, argv: &[OsString], input: Option<&[u8]>) -> AppResult<String> {
    exec_in_locale(
        program,
        argv,
        input,
        "C",
        None,
        Duration::from_secs(300),
        None,
    )
}

#[cfg(target_os = "dragonfly")]
pub(super) fn exec_interruptible(
    program: &str,
    argv: &[OsString],
    input: Option<&[u8]>,
) -> AppResult<String> {
    exec_in_locale(
        program,
        argv,
        input,
        "C",
        Some(Duration::from_secs(5)),
        Duration::from_secs(300),
        None,
    )
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
pub(super) fn exec_utf8(
    program: &str,
    argv: &[OsString],
    input: Option<&[u8]>,
) -> AppResult<String> {
    exec_in_locale(
        program,
        argv,
        input,
        "C.UTF-8",
        None,
        Duration::from_secs(300),
        None,
    )
}

#[cfg(target_os = "dragonfly")]
fn exec_dragonfly_locked(
    registry: &Registry,
    program: &Path,
    argv: &[OsString],
    input: Option<&[u8]>,
    timeout: Duration,
) -> AppResult<String> {
    let program = program
        .to_str()
        .ok_or_else(|| AppError::new("Invalid DragonFly backend path."))?;
    exec_in_locale(
        program,
        argv,
        input,
        "C",
        None,
        timeout,
        Some(registry._lock.as_raw_fd()),
    )
}

fn stop_timed_out_child(
    child: &mut Child,
    interrupt_grace: Option<Duration>,
) -> std::io::Result<ExitStatus> {
    if let Some(grace) = interrupt_grace {
        // SAFETY: the caller owns this child and has not reaped it, so this PID
        // cannot be reused before we send the signal. No process group is signaled.
        if unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) } == 0 {
            let start = Instant::now();
            while start.elapsed() < grace {
                match child.try_wait() {
                    Ok(Some(status)) => return Ok(status),
                    Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                    Err(_) => break,
                }
            }
        }
    }
    let _ = child.kill();
    child.wait()
}

fn exec_in_locale(
    program: &str,
    argv: &[OsString],
    input: Option<&[u8]>,
    locale: &str,
    interrupt_grace: Option<Duration>,
    timeout: Duration,
    lock: Option<i32>,
) -> AppResult<String> {
    use std::os::unix::process::CommandExt;

    let executable = crate::trusted_privileged_executable(Path::new(program))?;
    let mut command = Command::new(&executable);
    if let Some(lock) = lock {
        // Keep the open file description and its flock alive until the native
        // operation exits. Recovery can then reason about terminal state.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(lock, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command
        .args(argv)
        .env_clear()
        .env("LC_ALL", locale)
        .env("PATH", "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/sbin:/usr/local/bin:/usr/pkg/sbin:/usr/pkg/bin")
        .env("HOME", "/root")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::io(format!("Launch {program}"), e))?;
    let mut stdin = child.stdin.take().unwrap();
    let mut bytes = input.unwrap_or_default().to_vec();
    let writer = std::thread::spawn(move || {
        let result = stdin.write_all(&bytes);
        bytes.fill(0);
        drop(stdin);
        result
    });
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    fn drain(mut reader: impl Read) -> Vec<u8> {
        let mut kept = Vec::new();
        let mut chunk = [0; 4096];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let take = n.min(1024 * 1024 - kept.len());
            kept.extend_from_slice(&chunk[..take]);
        }
        kept
    }
    let out = std::thread::spawn(move || drain(stdout));
    let err = std::thread::spawn(move || drain(stderr));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| AppError::io("Wait for BSD tool", e))?
        {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = stop_timed_out_child(&mut child, interrupt_grace);
            return Err(AppError::new(format!(
                "{program} timed out; inspect retained BSD session before retrying."
            )));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let _ = writer.join();
    let stdout = out
        .join()
        .map_err(|_| AppError::new("BSD output reader failed."))?;
    let stderr = err
        .join()
        .map_err(|_| AppError::new("BSD error reader failed."))?;
    if !status.success() {
        let message = String::from_utf8_lossy(&stderr);
        let bounded: String = message.chars().take(2048).collect();
        return Err(AppError::new(format!(
            "{program} failed ({status}): {}",
            bounded.trim()
        )));
    }
    String::from_utf8(stdout).map_err(|_| AppError::new("BSD tool returned non-UTF-8 metadata."))
}

fn unique_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn sync_directory(path: &Path) -> AppResult<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| AppError::io("Sync directory", e))
}

fn mounts() -> AppResult<Vec<Mounted>> {
    // getmntinfo uses a process-global buffer. Serialize access through a mutex;
    // copy every field before releasing it. Unlike `mount` text, spaces in
    // native paths remain unambiguous.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK
        .lock()
        .map_err(|_| AppError::new("Native mount snapshot lock failed."))?;
    let mut entries = std::ptr::null_mut();
    // SAFETY: getmntinfo initializes pointer to its valid static mount buffer.
    // DragonFly's libc bindings omit this public sys/mount.h constant.
    #[cfg(target_os = "dragonfly")]
    let nowait = 0x0002;
    #[cfg(not(target_os = "dragonfly"))]
    let nowait = libc::MNT_NOWAIT;
    let count = unsafe { libc::getmntinfo(&mut entries, nowait) };
    if count <= 0 || entries.is_null() {
        return Err(AppError::io(
            "Read native mount table",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: count entries belong to libc until the next getmntinfo call.
    let entries = unsafe { std::slice::from_raw_parts(entries, count as usize) };
    entries
        .iter()
        .map(|entry| {
            fn text(value: &[libc::c_char]) -> AppResult<String> {
                let bytes: Vec<u8> = value
                    .iter()
                    .take_while(|c| **c != 0)
                    .map(|c| *c as u8)
                    .collect();
                String::from_utf8(bytes)
                    .map_err(|_| AppError::new("Native mounted path is not UTF-8."))
            }
            // fsid contains two integer words and no pointers/padding on these BSDs.
            #[cfg(target_os = "netbsd")]
            let native_fsid = &entry.f_fsidx;
            #[cfg(not(target_os = "netbsd"))]
            let native_fsid = &entry.f_fsid;
            let fsid = unsafe {
                std::slice::from_raw_parts(
                    (native_fsid as *const libc::fsid_t).cast::<u8>(),
                    std::mem::size_of::<libc::fsid_t>(),
                )
            }
            .to_vec();
            #[cfg(target_os = "netbsd")]
            let flags = entry.f_flag as u64;
            #[cfg(not(target_os = "netbsd"))]
            let flags = entry.f_flags as u64;
            Ok(Mounted {
                source: PathBuf::from(text(&entry.f_mntfromname)?),
                target: PathBuf::from(text(&entry.f_mntonname)?),
                fsid,
                filesystem: text(&entry.f_fstypename)?,
                readonly: flags & libc::MNT_RDONLY as u64 != 0,
                #[cfg(target_os = "netbsd")]
                journaled: flags & libc::MNT_LOG as u64 != 0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "dragonfly")]
    #[test]
    fn tool_timeout_allows_interrupt_cleanup_and_bounds_the_wait() {
        use std::io::{BufRead, BufReader};
        use std::os::unix::process::ExitStatusExt;

        for (trap, grace, code, signal) in [
            ("'exit 17'", Duration::from_secs(2), Some(17), None),
            ("''", Duration::from_millis(75), None, Some(libc::SIGKILL)),
        ] {
            let mut child = Command::new("/bin/sh")
                .args([
                    "-c",
                    &format!("trap {trap} INT; echo ready; while :; do :; done"),
                ])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut ready = String::new();
            let handshake = BufReader::new(child.stdout.take().unwrap()).read_line(&mut ready);
            let status = stop_timed_out_child(&mut child, Some(grace)).unwrap();
            handshake.unwrap();
            assert_eq!(ready, "ready\n");
            assert_eq!(status.code(), code);
            assert_eq!(status.signal(), signal);
        }
    }

    #[cfg(target_os = "netbsd")]
    #[test]
    fn netbsd_requests_and_requires_journaling_only_for_writable_mounts() {
        assert_eq!(mount_options(false), "rw,log");
        assert_eq!(mount_options(true), "ro");
        let mut mount = Mounted {
            source: "/dev/cgd0d".into(),
            target: "/tmp/test".into(),
            fsid: Vec::new(),
            filesystem: "ffs".into(),
            readonly: false,
            journaled: false,
        };
        assert!(require_write_journal(&mount).is_err());
        mount.journaled = true;
        assert!(require_write_journal(&mount).is_ok());
        mount.readonly = true;
        mount.journaled = false;
        assert!(require_write_journal(&mount).is_ok());
    }
    #[test]
    #[ignore = "requires root BSD /tmp fixture"]
    fn initialization_cleanup_refuses_replacements_and_nonempty_directories() {
        require_root().unwrap();
        let directory = PathBuf::from(format!("/tmp/lurker-init-cleanup-{}", unique_id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let source = directory.join("image");
        fs::write(&source, b"fixture").unwrap();
        let registry = Registry {
            directory: directory.clone(),
            _lock: File::create(directory.join("lock")).unwrap(),
        };
        let mut session = Session {
            version: 1,
            id: unique_id(),
            backend: default_backend(),
            source: source.clone(),
            create_target: None,
            peer_sources: Vec::new(),
            identity: Identity::read(&source).unwrap(),
            backing: None,
            #[cfg(target_os = "freebsd")]
            freebsd_detach: None,
            #[cfg(target_os = "netbsd")]
            netbsd_detach: None,
            #[cfg(target_os = "openbsd")]
            openbsd_detach: None,
            #[cfg(target_os = "dragonfly")]
            dragonfly_backing_reservation: None,
            #[cfg(target_os = "dragonfly")]
            dragonfly_backing_detach_requested: false,
            #[cfg(target_os = "dragonfly")]
            dragonfly_layer: None,
            #[cfg(target_os = "dragonfly")]
            dragonfly_veracrypt: None,
            mapped: None,
            mounted: None,
            intended_mountpoint: None,
            initialization_directory: None,
            readonly: false,
            #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
            veracrypt_proof: None,
            #[cfg(target_os = "openbsd")]
            mirror: None,
        };
        let target = directory.join(format!("init-{}", session.id));
        fs::create_dir(&target).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        session.initialization_directory = Some(InitializationDirectory {
            path: target.clone(),
            identity: Some(Identity::read(&target).unwrap()),
        });
        registry.save(&session).unwrap();
        let held = directory.join("held");
        fs::rename(&target, &held).unwrap();
        fs::create_dir(&target).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(remove_initialization_directory(&registry, &mut session).is_err());
        assert!(target.is_dir());
        fs::remove_dir(&target).unwrap();
        fs::rename(&held, &target).unwrap();
        let unexpected = target.join("unexpected");
        fs::write(&unexpected, b"preserve").unwrap();
        assert!(remove_initialization_directory(&registry, &mut session).is_err());
        assert_eq!(fs::read(&unexpected).unwrap(), b"preserve");
        fs::remove_file(&unexpected).unwrap();
        remove_initialization_directory(&registry, &mut session).unwrap();
        assert!(!target.exists());
        assert!(session.initialization_directory.is_none());
        registry.remove(&session).unwrap();
        drop(registry);
        fs::remove_file(directory.join("lock")).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn unprivileged_discovery_does_not_read_the_privileged_registry() {
        if crate::effective_uid() != 0 {
            assert!(list_active_volumes().unwrap().is_empty());
        }
    }
    #[test]
    fn privileged_subprocess_rejects_a_writable_staging_path() {
        let directory = std::env::temp_dir().join(format!("lurker-native-exec-{}", unique_id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o770)).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let error = exec(helper.to_str().unwrap(), &[], None).unwrap_err();
        assert!(error.message.contains("untrusted"));
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn no_clobber_publication_preserves_existing_file() {
        let root = std::env::temp_dir().join(format!("lurker-publish-{}", unique_id()));
        fs::create_dir(&root).unwrap();
        let mut stage = FileStage::create(&root).unwrap();
        let target = root.join("target");
        stage.create_image().unwrap().write_all(b"new").unwrap();
        fs::write(&target, "old").unwrap();
        assert!(stage.finish(stage.publish(&target, false)).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert!(!stage.path().exists());

        let mut stage = FileStage::create(&root).unwrap();
        stage.create_image().unwrap().write_all(b"new").unwrap();
        stage.finish(stage.publish(&target, true)).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(!stage.path().exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn source_identity_detects_replaced_file() {
        let path = std::env::temp_dir().join(format!("lurker-identity-{}", unique_id()));
        fs::write(&path, "original").unwrap();
        let id = Identity::read(&path).unwrap();
        let alias = path.with_extension("alias");
        fs::hard_link(&path, &alias).unwrap();
        assert!(id.matches(&alias));
        fs::remove_file(&path).unwrap();
        fs::write(&path, "replacement").unwrap();
        assert!(!id.matches(&path));
        fs::remove_file(path).unwrap();
        fs::remove_file(alias).unwrap();
    }

    #[cfg(any(target_os = "netbsd", target_os = "openbsd"))]
    #[test]
    #[ignore = "requires root; real disposable native crypto attachments under /tmp"]
    fn initialization_failure_retains_a_journaled_mapping_for_owned_cleanup() {
        require_root().unwrap();
        let registry = Registry::open().unwrap();
        let directory = std::env::temp_dir().join(format!("lurker-init-failure-{}", unique_id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let requested = directory.join("unpublished-vault");
        let target = absolute_destination(&requested).unwrap();
        let source = directory.join("image");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&source)
            .unwrap();
        file.set_len(128 * 1024 * 1024).unwrap();
        let credential = Credential::Password(b"temporary initialization failure test".to_vec());
        let mut session = Session {
            version: 1,
            id: unique_id(),
            backend: default_backend(),
            source: source.clone(),
            create_target: Some(target.clone()),
            identity: Identity::read(&source).unwrap(),
            backing: None,
            #[cfg(target_os = "freebsd")]
            freebsd_detach: None,
            #[cfg(target_os = "netbsd")]
            netbsd_detach: None,
            #[cfg(target_os = "openbsd")]
            openbsd_detach: None,
            #[cfg(target_os = "dragonfly")]
            dragonfly_backing_reservation: None,
            #[cfg(target_os = "dragonfly")]
            dragonfly_backing_detach_requested: false,
            #[cfg(target_os = "dragonfly")]
            dragonfly_layer: None,
            #[cfg(target_os = "dragonfly")]
            dragonfly_veracrypt: None,
            mapped: None,
            mounted: None,
            peer_sources: Vec::new(),
            #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
            veracrypt_proof: None,
            #[cfg(target_os = "openbsd")]
            mirror: None,
            intended_mountpoint: None,
            initialization_directory: None,
            readonly: false,
        };
        registry.save(&session).unwrap();
        session.backing = Some(native::attach_file(&source, false).unwrap());
        registry.save(&session).unwrap();
        let backing = session.backing.as_deref().unwrap();
        #[cfg(target_os = "netbsd")]
        {
            native::format(
                backing,
                crate::CreateCipher::Aes,
                &credential,
                &params_path(&source),
            )
            .unwrap();
            session.mapped =
                Some(native::unlock_initial(backing, &credential, &params_path(&source)).unwrap());
        }
        #[cfg(target_os = "openbsd")]
        {
            session.mapped =
                Some(native::format(backing, crate::CreateCipher::Aes, &credential).unwrap());
        }
        let mapped = session.mapped.clone().unwrap();
        let error = initialize_acquired_mapping(&registry, &session, |device| {
            assert_eq!(device, mapped);
            let saved = registry
                .records()
                .unwrap()
                .into_iter()
                .find(|s| s.id == session.id)
                .unwrap();
            assert_eq!(saved.mapped.as_deref(), Some(device));
            verify_session(&saved).unwrap();
            Err(AppError::new("Injected filesystem initialization failure"))
        })
        .unwrap_err();
        assert_eq!(error.message, "Injected filesystem initialization failure");
        let saved = registry
            .records()
            .unwrap()
            .into_iter()
            .find(|s| s.id == session.id)
            .unwrap();
        assert_eq!(saved.mapped.as_deref(), Some(mapped.as_str()));
        verify_session(&saved).unwrap();
        assert!(registry.ensure_unused(&target).is_err());
        let record_path = registry.path(&saved);
        drop(registry);
        let (output, messages) = Output::buffered();
        run(
            CommandAction::Unmount(UnmountCommand {
                privilege_mode: PrivilegeMode::Privileged,
                target: requested,
                tag: None,
                volume_type: VolumeType::Auto,
            }),
            output,
        )
        .unwrap();
        assert!(!record_path.exists());
        let staged_path = source.to_string_lossy();
        assert!(messages
            .entries()
            .iter()
            .any(|entry| entry.message.contains(staged_path.as_ref())));
        assert!(native::verify_mapping(&mapped, session.backing.as_deref().unwrap()).is_err());
        assert!(native::verify_backing(session.backing.as_deref().unwrap(), &source).is_err());
        assert!(session.identity.matches(&source));
        assert!(!target.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    #[test]
    #[ignore = "requires root and the complete trusted private VeraCrypt runtime; disposable /tmp images"]
    fn auto_dispatch_authenticates_existing_formats_without_changing_unknown_data() {
        require_root().unwrap();
        veracrypt::backend().unwrap();
        let directory = std::env::temp_dir().join(format!("lurker-auto-{}", unique_id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let mountpoint = directory.join("mnt");
        let passphrase = "temporary Auto dispatch password";
        fn digest(path: &Path) -> String {
            #[cfg(target_os = "freebsd")]
            let tool = "/sbin/sha256";
            #[cfg(target_os = "openbsd")]
            let tool = "/bin/sha256";
            exec(tool, &[path.as_os_str().to_owned()], None).unwrap()
        }
        let mount_request = |source: &Path, password: &str| MountCommand {
            peer_sources: Vec::new(),
            allow_degraded: false,
            privilege_mode: PrivilegeMode::Privileged,
            source: source.to_owned(),
            mountpoint: mountpoint.clone(),
            tag: None,
            volume_type: VolumeType::Auto,
            passphrase: Some(password.to_owned()),
            keyfile: None,
            readonly: false,
        };
        let close = || {
            unmount(
                UnmountCommand {
                    privilege_mode: PrivilegeMode::Privileged,
                    target: mountpoint.clone(),
                    tag: None,
                    volume_type: VolumeType::Auto,
                },
                Output::silent(),
            )
            .unwrap()
        };
        for backend in [default_backend(), VolumeType::Veracrypt] {
            let source = directory.join(format!("{backend} 密碼 with spaces.img"));
            create(
                CreateCommand {
                    peer_sources: Vec::new(),
                    privilege_mode: PrivilegeMode::Privileged,
                    target: source.clone(),
                    size_gb: Some("0.125".into()),
                    force: false,
                    source_kind: SourceKind::File,
                    volume_type: backend,
                    cipher: crate::CreateCipher::Aes,
                    passphrase: Some(passphrase.into()),
                    keyfile: None,
                },
                Output::silent(),
            )
            .unwrap();
            mount(mount_request(&source, passphrase), Output::silent()).unwrap();
            {
                let registry = Registry::open().unwrap();
                let session = registry
                    .records()
                    .unwrap()
                    .into_iter()
                    .find(|s| s.source == source)
                    .unwrap();
                assert_eq!(session.backend, backend);
                verify_session(&session).unwrap();
            }
            let marker = mountpoint.join("sentinel");
            fs::write(&marker, b"durably committed Auto data").unwrap();
            File::open(&marker).unwrap().sync_all().unwrap();
            close();
            mount(mount_request(&source, passphrase), Output::silent()).unwrap();
            assert_eq!(fs::read(&marker).unwrap(), b"durably committed Auto data");
            close();
            let before = digest(&source);
            assert!(mount(
                mount_request(&source, "incorrect Auto credential"),
                Output::silent()
            )
            .is_err());
            assert_eq!(digest(&source), before);
            assert!(!mounts().unwrap().iter().any(|m| m.target == mountpoint));
            assert!(!Registry::open()
                .unwrap()
                .records()
                .unwrap()
                .iter()
                .any(|s| s.source == source));
        }
        let unknown = directory.join("random unknown image");
        let mut random = File::open("/dev/urandom").unwrap();
        let mut image = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&unknown)
            .unwrap();
        let mut block = [0; 65536];
        for _ in 0..1024 {
            random.read_exact(&mut block).unwrap();
            image.write_all(&block).unwrap();
        }
        image.sync_all().unwrap();
        drop(image);
        let before = digest(&unknown);
        assert!(mount(mount_request(&unknown, passphrase), Output::silent()).is_err());
        assert_eq!(digest(&unknown), before);
        assert!(!mounts().unwrap().iter().any(|m| m.target == mountpoint));
        assert!(!Registry::open()
            .unwrap()
            .records()
            .unwrap()
            .iter()
            .any(|s| s.source == unknown));
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    #[test]
    #[ignore = "requires root and trusted private VeraCrypt runtime; disposable external-attachment fixture"]
    fn external_veracrypt_alias_cannot_be_mounted_or_force_replaced() {
        require_root().unwrap();
        veracrypt::backend().unwrap();
        let directory = std::env::temp_dir().join(format!("lurker-external-{}", unique_id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let source = directory.join("external Slot: 1 密碼.img");
        let alias = directory.join("hardlink alias.img");
        let mountpoint = directory.join("mnt");
        let passphrase = "temporary external attachment test";
        let command = CreateCommand {
            peer_sources: Vec::new(),
            privilege_mode: PrivilegeMode::Privileged,
            target: source.clone(),
            size_gb: Some("0.125".into()),
            force: false,
            source_kind: SourceKind::File,
            volume_type: VolumeType::Veracrypt,
            cipher: crate::CreateCipher::Aes,
            passphrase: Some(passphrase.into()),
            keyfile: None,
        };
        create(command.clone(), Output::silent()).unwrap();
        fs::hard_link(&source, &alias).unwrap();
        let identity = Identity::read(&source).unwrap();
        let credential = Credential::Password(passphrase.as_bytes().to_vec());
        // Attach directly through the engine, outside the shared Registry.
        let external = veracrypt::unlock(&source, false, &credential).unwrap();
        #[cfg(target_os = "freebsd")]
        let sha256 = "/sbin/sha256";
        #[cfg(target_os = "openbsd")]
        let sha256 = "/bin/sha256";
        let before = exec(sha256, &[source.as_os_str().to_owned()], None).unwrap();
        for kind in [VolumeType::Auto, VolumeType::Veracrypt] {
            let attempted = mount(
                MountCommand {
                    peer_sources: Vec::new(),
                    allow_degraded: false,
                    privilege_mode: PrivilegeMode::Privileged,
                    source: alias.clone(),
                    mountpoint: mountpoint.clone(),
                    tag: None,
                    volume_type: kind,
                    passphrase: Some(passphrase.into()),
                    keyfile: None,
                    readonly: false,
                },
                Output::silent(),
            );
            assert!(attempted.is_err());
            veracrypt::verify_mapping(&source, &external).unwrap();
        }
        let mut replace = command;
        replace.target = alias.clone();
        replace.force = true;
        assert!(create(replace, Output::silent()).is_err());
        assert!(identity.matches(&source) && identity.matches(&alias));
        assert_eq!(
            exec(sha256, &[source.as_os_str().to_owned()], None).unwrap(),
            before
        );
        assert!(!mounts().unwrap().iter().any(|m| m.target == mountpoint));
        assert!(!Registry::open()
            .unwrap()
            .records()
            .unwrap()
            .iter()
            .any(|s| s.identity.dev == identity.dev && s.identity.ino == identity.ino));
        assert!(!fs::read_dir(&directory).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".lurker-bsd-")));
        // The rejected operations must leave the original external attachment
        // intact. Its fixture owner closes it explicitly after verification.
        veracrypt::verify_mapping(&source, &external).unwrap();
        veracrypt::close(&source).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    #[test]
    #[ignore = "requires root and trusted private VeraCrypt runtime; two disposable managed sessions"]
    fn verified_managed_veracrypt_sessions_can_coexist() {
        require_root().unwrap();
        veracrypt::backend().unwrap();
        let directory = std::env::temp_dir().join(format!("lurker-managed-{}", unique_id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let mut sessions = Vec::new();
        for label in ["first", "second"] {
            let source = directory.join(format!("{label}.img"));
            let mountpoint = directory.join(format!("{label}-mnt"));
            let passphrase = "temporary managed session test";
            create(
                CreateCommand {
                    peer_sources: Vec::new(),
                    privilege_mode: PrivilegeMode::Privileged,
                    target: source.clone(),
                    size_gb: Some("0.125".into()),
                    force: false,
                    source_kind: SourceKind::File,
                    volume_type: VolumeType::Veracrypt,
                    cipher: crate::CreateCipher::Aes,
                    passphrase: Some(passphrase.into()),
                    keyfile: None,
                },
                Output::silent(),
            )
            .unwrap();
            mount(
                MountCommand {
                    peer_sources: Vec::new(),
                    allow_degraded: false,
                    privilege_mode: PrivilegeMode::Privileged,
                    source: source.clone(),
                    mountpoint: mountpoint.clone(),
                    tag: None,
                    volume_type: VolumeType::Auto,
                    passphrase: Some(passphrase.into()),
                    keyfile: None,
                    readonly: false,
                },
                Output::silent(),
            )
            .unwrap();
            let marker = mountpoint.join("sentinel");
            fs::write(&marker, label).unwrap();
            File::open(&marker).unwrap().sync_all().unwrap();
            sessions.push((source, mountpoint, label));
        }
        let active = list_active_volumes().unwrap();
        assert_eq!(
            active
                .iter()
                .filter(|v| v
                    .source_path
                    .as_ref()
                    .is_some_and(|p| p.starts_with(&directory)))
                .count(),
            2
        );
        for (_, mountpoint, label) in sessions.iter().rev() {
            assert_eq!(
                fs::read_to_string(mountpoint.join("sentinel")).unwrap(),
                *label
            );
            unmount(
                UnmountCommand {
                    privilege_mode: PrivilegeMode::Privileged,
                    target: mountpoint.clone(),
                    tag: None,
                    volume_type: VolumeType::Auto,
                },
                Output::silent(),
            )
            .unwrap();
        }
        assert!(!Registry::open()
            .unwrap()
            .records()
            .unwrap()
            .iter()
            .any(|s| s.source.starts_with(&directory)));
        fs::remove_dir_all(directory).unwrap();
    }
}
