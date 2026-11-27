mod api;
#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod bsd;
mod credentials;
mod error;
pub mod file_crypto;
mod folder;
#[cfg(target_os = "linux")]
mod keyslots;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_loop;
#[cfg(target_os = "linux")]
mod linux_source;
#[cfg(target_os = "macos")]
mod macos;
mod model;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod new_filesystem;
mod output;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
mod sha256;
mod size;
#[cfg(target_os = "linux")]
mod system;
mod trust;
#[cfg(target_os = "linux")]
mod userspace;
mod workflow;

pub use api::{
    ActiveVolume, CommandAction, CreateCommand, CreateFilesystem, KeyslotOperation, KeyslotsCommand,
    MountCommand, NativeVolumeListResponse, OperationResponse, SystemProbe, ToolStatus, UnmountCommand,
};
pub use error::{AppError, AppResult};
#[doc(hidden)]
#[cfg(target_os = "linux")]
pub use linux_loop::serve_loop_lease;
pub use model::{
    ColorMode, CreateCipher, PrivilegeMode, ResolvedTargetKind, SourceKind, VolumeType,
};
pub use output::{Output, OutputBuffer, OutputEntry, OutputLevel};
#[cfg(target_os = "linux")]
pub use userspace::unprivileged_veracrypt_readiness;

pub use trust::{trusted_privileged_executable, trusted_system_executable};
#[cfg(not(target_os = "macos"))]
pub use trust::trusted_privileged_storage_source;

#[cfg(target_os = "linux")]
use crate::linux::{canonical_device_path, read_mountinfo};
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    credentials::validate_request(&command)?;
    validate_platform(&command)?;
    if !native_directory_request(&command) && folder::handles(&command)? {
        return folder::run(command, output);
    }
    #[cfg(target_os = "macos")]
    {
        if command.privilege_mode() == PrivilegeMode::Unprivileged {
            return Err(AppError::new("The privilege mode selector is Linux-only. macOS already runs in your user session."));
        }
        macos::run(command, output)
    }
    #[cfg(target_os = "linux")]
    {
        if command.privilege_mode() == PrivilegeMode::Unprivileged {
            return userspace::run(command, output);
        }
        validate_request(&command)?;
        workflow::run(command, output)
    }
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        bsd::run(command, output)
    }
}

/// Close a configured encrypted source through the normal teardown path.
#[cfg(not(target_os = "macos"))]
pub fn run_configured_close(command: UnmountCommand, output: Output) -> AppResult<()> {
    if !command.volume_type.is_directory() {
        #[cfg(target_os = "linux")]
        if command.privilege_mode == PrivilegeMode::Unprivileged {
            return userspace::unmount_source(command, output);
        }
        return run(CommandAction::Unmount(command), output);
    }
    #[cfg(target_os = "freebsd")]
    if command.volume_type == VolumeType::Pefs {
        return bsd::unmount_source(command, output);
    }
    folder::unmount_source(command, output)
}

/// Directory engines normally remain in the login user's session even when
/// an older frontend requests privileged mode. PEFS and OpenBSD CryFS require
/// native authorization; OpenBSD CryFS then limits root identity to its mount
/// acquisition while ordinary vault I/O runs as the invoking account.
pub fn request_requires_user_session(command: &CommandAction) -> AppResult<bool> {
    #[cfg(target_os = "openbsd")]
    {
        if command.volume_type() == VolumeType::Cryfs { return Ok(false); }
        if command.volume_type() == VolumeType::Auto {
            match command {
                CommandAction::Unmount(_) => return Ok(false),
                CommandAction::Mount(c) if c.source.is_dir()
                    && folder::detect_type(&c.source)? == VolumeType::Cryfs => return Ok(false),
                _ => (),
            }
        }
    }
    if native_directory_request(command) {
        return Ok(false);
    }
    folder::handles(command)
}

/// Negotiate the macOS Cryptomator mountpoint fence before sending secrets.
/// Auto is deliberately conservative: paths/formats may change before helper
/// execution, and an older helper cannot be trusted to detect a fenced vault.
pub fn request_requires_cryptomator_fence(command: &CommandAction) -> AppResult<bool> {
    Ok(folder::requires_fence_protocol(command.volume_type(), cfg!(target_os = "macos")))
}

fn native_directory_request(command: &CommandAction) -> bool {
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        bsd::handles_directory(command)
    }
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        let _ = command;
        false
    }
}

fn validate_platform(command: &CommandAction) -> AppResult<()> {
    command.validate_raid1c_options(std::env::consts::OS)?;
    if let CommandAction::Create(create) = command {
        create.validate_filesystem(std::env::consts::OS)?;
    }
    let backend = command.volume_type();
    if !backend.supports_platform(std::env::consts::OS) {
        return Err(AppError::new(format!(
            "Backend {backend} is not supported on {}.",
            std::env::consts::OS
        )));
    }
    Ok(())
}

/// Called before a desktop helper elevates, so a privileged request cannot
/// overwrite a container already served by the login user's userspace backend.
pub fn validate_request(command: &CommandAction) -> AppResult<()> {
    credentials::validate_request(command)?;
    validate_platform(command)?;
    #[cfg(target_os = "linux")]
    if command.privilege_mode() == PrivilegeMode::Privileged {
        userspace::check_privileged_conflicts(command)?;
    }
    Ok(())
}

pub fn run_buffered(command: CommandAction) -> OperationResponse {
    let (output, buffer) = Output::buffered();
    match run(command, output) {
        Ok(()) => OperationResponse {
            ok: true,
            logs: buffer.entries(),
            error: None,
        },
        Err(err) => {
            let mut logs = buffer.entries();
            logs.push(OutputEntry::error(err.message.clone()));
            OperationResponse {
                ok: false,
                logs,
                error: Some(err.message),
            }
        }
    }
}

pub fn probe_system() -> AppResult<SystemProbe> {
    #[cfg(target_os = "macos")]
    {
        let mut probe = macos::probe_system()?;
        probe.tools.extend(folder::tool_statuses());
        Ok(probe)
    }
    #[cfg(target_os = "linux")]
    {
        let mut probe = system::probe_system()?;
        probe.tools.extend(folder::tool_statuses());
        Ok(probe)
    }
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        let mut probe = bsd::probe_system()?;
        probe.tools.extend(folder::tool_statuses());
        Ok(probe)
    }
}

/// Read the private BSD native namespace after helper authorization. Do not
/// inspect root's unrelated directory sessions while serving a login user.
pub fn list_native_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        if effective_uid() != 0 {
            return Err(AppError::new(
                "Native volume listing requires administrator authorization.",
            ));
        }
        let mut volumes = bsd::list_active_volumes()?;
        #[cfg(target_os = "openbsd")]
        volumes.extend(folder::list_active_volumes()?);
        Ok(volumes)
    }
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        Err(AppError::new(
            "Authorized native listing is supported on BSD only.",
        ))
    }
}

pub fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    #[cfg(target_os = "macos")]
    {
        let mut volumes = macos::list_active_volumes()?;
        volumes.extend(folder::list_active_volumes()?);
        Ok(volumes)
    }
    #[cfg(target_os = "linux")]
    {
        list_linux_volumes()
    }
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        let mut volumes = bsd::list_active_volumes()?;
        volumes.extend(folder::list_active_volumes()?);
        Ok(volumes)
    }
}

#[cfg(target_os = "linux")]
fn list_linux_volumes() -> AppResult<Vec<ActiveVolume>> {
    let mounts = read_mountinfo()?;
    let mut volumes = userspace::list_active_volumes()?;
    volumes.extend(folder::list_active_volumes()?);
    let entries = match fs::read_dir("/dev/mapper") {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(volumes),
        Err(err) => return Err(AppError::io("Failed to inspect /dev/mapper", err)),
    };
    for entry in entries {
        let entry = entry.map_err(|err| AppError::io("Failed to inspect /dev/mapper", err))?;
        let path = entry.path();
        if path == Path::new("/dev/mapper/control") {
            continue;
        }

        let Some(mapper_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !mapper_name.starts_with("lurker_") {
            continue;
        }

        let canonical_mapper = canonical_device_path(&path);
        let mount = mounts.iter().find(|mount| {
            mount
                .source
                .as_ref()
                .map(|source| canonical_device_path(source) == canonical_mapper)
                .unwrap_or(false)
        });

        volumes.push(ActiveVolume {
            privilege_mode: crate::PrivilegeMode::Privileged,
            source_path: None,
            readonly: mount.is_some_and(|m| m.readonly),
            mapper_name: mapper_name.to_string(),
            mapper_path: path,
            mountpoint: mount.map(|item| item.mount_point.clone()),
            filesystem_type: mount.map(|item| item.fs_type.clone()),
        });
    }

    volumes.sort_by(|left, right| left.mapper_name.cmp(&right.mapper_name));
    Ok(volumes)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
compile_error!("Lurker supports Linux, macOS, FreeBSD, DragonFly BSD, NetBSD and OpenBSD.");

pub fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments or preconditions.
    unsafe { libc::geteuid() }
}

/// Internal frontend entry point for the platform directory backend monitor.
pub fn run_folder_supervisor_if_requested() -> Option<i32> {
    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        folder::run_supervisor_if_requested()
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        None
    }
}

/// Internal frontend entry point for an optional macOS filesystem VM monitor.
pub fn run_macos_filesystem_supervisor_if_requested() -> Option<i32> {
    #[cfg(target_os = "macos")]
    {
        macos::run_filesystem_supervisor_if_requested()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(all(test, target_os = "openbsd"))]
mod openbsd_directory_policy_tests {
    use super::*;
    #[test]
    fn automatic_directory_authorization_is_exclusive_to_actual_cryfs() {
        let root = tempfile::tempdir().unwrap();
        let command = CommandAction::Mount(MountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            source: root.path().to_owned(), peer_sources: Vec::new(), allow_degraded: false,
            mountpoint: root.path().join("mount"), tag: None, volume_type: VolumeType::Auto,
            passphrase: None, keyfile: None, readonly: false,
        });
        for (marker, authorized) in [("cryfs.config", true), ("vault.cryptomator", false), ("gocryptfs.conf", false)] {
            let path = root.path().join(marker); std::fs::write(&path, b"{}").unwrap();
            assert_eq!(request_requires_user_session(&command).unwrap(), !authorized, "{marker}");
            std::fs::remove_file(path).unwrap();
        }
    }
}
