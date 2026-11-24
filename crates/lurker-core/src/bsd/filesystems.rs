//! Small native filesystem dispatch; encryption acquisition and identity journals
//! remain owned by the shared BSD lifecycle.
use super::{Mounted, native};
use crate::{AppResult, CreateFilesystem, VolumeType};
use std::path::Path;

pub(super) fn validate(selected: Option<CreateFilesystem>, backend: VolumeType) -> AppResult<()> {
    native::validate_filesystem(selected, backend)
}
pub(super) fn format(mapped: &str, selected: Option<CreateFilesystem>) -> AppResult<()> {
    native::make_selected_filesystem(mapped, selected)
}
pub(super) fn mount(mapped: &str, target: &Path, readonly: bool) -> AppResult<()> {
    native::mount_filesystem(mapped, target, readonly)
}
pub(super) fn matches_mount(mapped: &str, mounted: &Mounted) -> bool {
    native::matches_filesystem_mount(mapped, mounted)
}
