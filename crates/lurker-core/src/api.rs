use crate::model::{CreateCipher, SourceKind, VolumeType};
use crate::output::OutputEntry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CommandAction {
    Create(CreateCommand),
    Mount(MountCommand),
    Unmount(UnmountCommand),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommand {
    pub target: PathBuf,
    pub size_gb: Option<String>,
    pub force: bool,
    pub source_kind: SourceKind,
    pub volume_type: VolumeType,
    #[serde(default, alias = "luks_cipher")]
    pub cipher: CreateCipher,
    pub passphrase: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MountCommand {
    pub source: PathBuf,
    pub mountpoint: PathBuf,
    pub tag: Option<String>,
    pub volume_type: VolumeType,
    pub passphrase: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnmountCommand {
    pub target: PathBuf,
    pub tag: Option<String>,
    pub volume_type: VolumeType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub path: Option<PathBuf>,
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemProbe {
    pub is_root: bool,
    pub tools: Vec<ToolStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveVolume {
    pub mapper_name: String,
    pub mapper_path: PathBuf,
    pub mountpoint: Option<PathBuf>,
    pub filesystem_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationResponse {
    pub ok: bool,
    pub logs: Vec<OutputEntry>,
    pub error: Option<String>,
}
