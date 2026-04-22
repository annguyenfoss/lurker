mod api;
mod error;
mod linux;
mod model;
mod output;
mod sha256;
mod system;
mod workflow;

pub use api::{
    ActiveVolume, CommandAction, CreateCommand, MountCommand, OperationResponse, SystemProbe,
    ToolStatus, UnmountCommand,
};
pub use error::{AppError, AppResult};
pub use model::{ColorMode, CreateCipher, ResolvedTargetKind, SourceKind, VolumeType};
pub use output::{Output, OutputBuffer, OutputEntry, OutputLevel};

use crate::linux::{canonical_device_path, read_mountinfo};
use std::fs;
use std::path::Path;

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    workflow::run(command, output)
}

pub fn run_buffered(command: CommandAction) -> OperationResponse {
    let (output, buffer) = Output::buffered();
    match workflow::run(command, output) {
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
    system::probe_system()
}

pub fn list_active_volumes() -> AppResult<Vec<ActiveVolume>> {
    let mounts = read_mountinfo()?;
    let mut volumes = Vec::new();
    let entries = fs::read_dir("/dev/mapper")
        .map_err(|err| AppError::io("Failed to inspect /dev/mapper", err))?;
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
            mapper_name: mapper_name.to_string(),
            mapper_path: path,
            mountpoint: mount.map(|item| item.mount_point.clone()),
            filesystem_type: mount.map(|item| item.fs_type.clone()),
        });
    }

    volumes.sort_by(|left, right| left.mapper_name.cmp(&right.mapper_name));
    Ok(volumes)
}
