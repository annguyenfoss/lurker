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
mod folder;
#[cfg(target_os = "linux")]
mod keyslots;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_source;
#[cfg(target_os = "macos")]
mod macos;
mod model;
mod output;
#[cfg(target_os = "linux")]
mod sha256;
mod size;
#[cfg(target_os = "linux")]
mod system;
mod trust;
#[cfg(target_os = "linux")]
mod userspace;
mod workflow;

pub use api::{
    ActiveVolume, CommandAction, CreateCommand, KeyslotOperation, KeyslotsCommand, MountCommand,
    OperationResponse, SystemProbe, ToolStatus, UnmountCommand,
};
pub use error::{AppError, AppResult};
pub use model::{
    ColorMode, CreateCipher, PrivilegeMode, ResolvedTargetKind, SourceKind, VolumeType,
};
pub use output::{Output, OutputBuffer, OutputEntry, OutputLevel};
pub use trust::{trusted_privileged_executable, trusted_system_executable};

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

/// Directory engines must remain in the login user's session even when an
/// older frontend sends the default privileged mode. PEFS uses its native BSD
/// authorization route and is intentionally excluded by the folder adapter.
pub fn request_requires_user_session(command: &CommandAction) -> AppResult<bool> {
    if native_directory_request(command) {
        return Ok(false);
    }
    folder::handles(command)
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
