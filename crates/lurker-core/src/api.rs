use crate::model::{CreateCipher, PrivilegeMode, SourceKind, VolumeType};
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

impl CommandAction {
    pub fn privilege_mode(&self) -> PrivilegeMode {
        match self {
            Self::Create(c) => c.privilege_mode,
            Self::Mount(c) => c.privilege_mode,
            Self::Unmount(c) => c.privilege_mode,
        }
    }

    pub fn with_privilege_mode(mut self, mode: PrivilegeMode) -> Self {
        match &mut self {
            Self::Create(c) => c.privilege_mode = mode,
            Self::Mount(c) => c.privilege_mode = mode,
            Self::Unmount(c) => c.privilege_mode = mode,
        }
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommand {
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
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
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
    pub source: PathBuf,
    pub mountpoint: PathBuf,
    pub tag: Option<String>,
    pub volume_type: VolumeType,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnmountCommand {
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
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
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
    #[serde(default)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub readonly: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_requests_keep_privileged_behavior_and_mode_round_trips() {
        let value = serde_json::json!({"action":"mount", "source":"vault.hc", "mountpoint":"mnt",
            "tag":null, "volume_type":"auto", "passphrase":"  secret  ", "readonly":true});
        let command: CommandAction = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(command.privilege_mode(), PrivilegeMode::Privileged);
        let command = command.with_privilege_mode(PrivilegeMode::Unprivileged);
        let encoded = serde_json::to_value(command).unwrap();
        assert_eq!(encoded["privilege_mode"], "unprivileged");
        assert_eq!(encoded["passphrase"], "  secret  ");
        let roundtrip: CommandAction = serde_json::from_value(encoded).unwrap();
        assert_eq!(roundtrip.privilege_mode(), PrivilegeMode::Unprivileged);
        let mut invalid = value;
        invalid["privilege_mode"] = "invalid".into();
        assert!(serde_json::from_value::<CommandAction>(invalid).is_err());
    }
}
