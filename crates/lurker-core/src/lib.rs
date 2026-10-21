mod api;
mod error;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod model;
mod output;
#[cfg(target_os = "linux")]
mod sha256;
mod size;
#[cfg(target_os = "linux")]
mod system;
mod workflow;

pub use api::{
    ActiveVolume, CommandAction, CreateCommand, MountCommand, OperationResponse, SystemProbe,
    ToolStatus, UnmountCommand,
};
pub use error::{AppError, AppResult};
pub use model::{ColorMode, CreateCipher, ResolvedTargetKind, SourceKind, VolumeType};
pub use output::{Output, OutputBuffer, OutputEntry, OutputLevel};

#[cfg(target_os = "linux")]
use crate::linux::{canonical_device_path, read_mountinfo};
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        macos::run(command, output)
    }
    #[cfg(target_os = "linux")]
    {
        workflow::run(command, output)
    }
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
        macos::probe_system()
    }
    #[cfg(target_os = "linux")]
    {
        system::probe_system()
    }
}

pub fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    #[cfg(target_os = "macos")]
    {
        macos::list_active_volumes()
    }
    #[cfg(target_os = "linux")]
    {
        list_linux_volumes()
    }
}

#[cfg(target_os = "linux")]
fn list_linux_volumes() -> AppResult<Vec<ActiveVolume>> {
    let mounts = read_mountinfo()?;
    let mut volumes = Vec::new();
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
            source_path: None,
            readonly: false,
            mapper_name: mapper_name.to_string(),
            mapper_path: path,
            mountpoint: mount.map(|item| item.mount_point.clone()),
            filesystem_type: mount.map(|item| item.fs_type.clone()),
        });
    }

    volumes.sort_by(|left, right| left.mapper_name.cmp(&right.mapper_name));
    Ok(volumes)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("Lurker supports Linux and macOS only.");

pub fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments or preconditions.
    unsafe { libc::geteuid() }
}
