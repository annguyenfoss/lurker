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
    Keyslots(KeyslotsCommand),
}

impl CommandAction {
    pub fn volume_type(&self) -> VolumeType {
        match self {
            Self::Create(c) => c.volume_type,
            Self::Mount(c) => c.volume_type,
            Self::Unmount(c) => c.volume_type,
            Self::Keyslots(_) => VolumeType::Luks,
        }
    }

    pub fn privilege_mode(&self) -> PrivilegeMode {
        match self {
            Self::Create(c) => c.privilege_mode,
            Self::Mount(c) => c.privilege_mode,
            Self::Unmount(c) => c.privilege_mode,
            Self::Keyslots(c) => c.privilege_mode,
        }
    }

    /// Validate mirrored softraid policy before a frontend authorizes or starts
    /// native operations. The explicit platform also supports form validation.
    pub fn validate_raid1c_options(&self, os: &str) -> crate::AppResult<()> {
        let (backend, peers, degraded) = match self {
            Self::Create(c) => (c.volume_type, c.peer_sources.as_slice(), false),
            Self::Mount(c) => (c.volume_type, c.peer_sources.as_slice(), c.allow_degraded),
            _ => return Ok(()),
        };
        if backend != VolumeType::Raid1c && peers.is_empty() && !degraded {
            return Ok(());
        }
        let reject = |message| Err(crate::AppError::new(message));
        if os != "openbsd" {
            return reject("RAID1C, member paths and degraded recovery require OpenBSD.");
        }
        if self.privilege_mode() != PrivilegeMode::Privileged {
            return reject(
                "RAID1C uses native OpenBSD authorization; the privilege mode selector is Linux-only.",
            );
        }
        match self {
            Self::Create(c) => {
                if backend != VolumeType::Raid1c || peers.len() != 1 {
                    return reject(
                        "RAID1C create requires --type raid1c and exactly one --member path (two members).",
                    );
                }
                if c.force || c.source_kind != SourceKind::File {
                    return reject(
                        "RAID1C create accepts new file containers only, without --force.",
                    );
                }
                if c.cipher != CreateCipher::Aes || c.keyfile.is_some() {
                    return reject("RAID1C uses native AES and password authentication only.");
                }
                if peers[0].as_os_str().is_empty() || peers[0] == c.target {
                    return reject("RAID1C members must have distinct, non-empty paths.");
                }
            }
            Self::Mount(c) => {
                if backend != VolumeType::Raid1c {
                    return reject(
                        "Member paths and degraded recovery require an explicit RAID1C command type.",
                    );
                }
                if peers.len() > 1 {
                    return reject("RAID1C mount accepts at most one --member path.");
                }
                if c.keyfile.is_some() || c.tag.is_some() {
                    return reject("RAID1C mounts accept passwords only and do not support tags.");
                }
                if peers
                    .iter()
                    .any(|p| p.as_os_str().is_empty() || *p == c.source)
                {
                    return reject("RAID1C members must have distinct, non-empty paths.");
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    pub fn with_privilege_mode(mut self, mode: PrivilegeMode) -> Self {
        match &mut self {
            Self::Create(c) => c.privilege_mode = mode,
            Self::Mount(c) => c.privilege_mode = mode,
            Self::Unmount(c) => c.privilege_mode = mode,
            Self::Keyslots(c) => c.privilege_mode = mode,
        }
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommand {
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
    pub target: PathBuf,
    #[serde(default)]
    pub peer_sources: Vec<PathBuf>,
    pub size_gb: Option<String>,
    pub force: bool,
    pub source_kind: SourceKind,
    pub volume_type: VolumeType,
    #[serde(default, alias = "luks_cipher")]
    pub cipher: CreateCipher,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<CreateFilesystem>,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub keyfile: Option<PathBuf>,
}

impl CreateCommand {
    pub fn validate_filesystem(&self, os: &str) -> crate::AppResult<()> {
        if self.filesystem.is_none() {
            return Ok(());
        }
        let native_device = self.source_kind == SourceKind::Block
            && self.privilege_mode == PrivilegeMode::Privileged
            && ((os == "linux"
                && matches!(
                    self.volume_type,
                    VolumeType::Luks | VolumeType::Luks1 | VolumeType::Veracrypt
                ))
                || (os == "macos" && self.volume_type == VolumeType::Apfs)
                || (os == "openbsd"
                    && matches!(
                        self.volume_type,
                        VolumeType::Softraid | VolumeType::Veracrypt
                    ))
                || (os == "freebsd"
                    && matches!(self.volume_type, VolumeType::Geli | VolumeType::Veracrypt))
                || (os == "netbsd" && self.volume_type == VolumeType::Cgd));
        if !matches!(
            os,
            "linux" | "macos" | "freebsd" | "openbsd" | "netbsd" | "dragonfly"
        ) || (self.source_kind != SourceKind::File && !native_device)
            || self.volume_type.is_directory()
        {
            return Err(crate::AppError::new(
                "Filesystem selection requires a supported encrypted file container or native device route.",
            ));
        }
        if self.volume_type == VolumeType::Apfs && self.filesystem != Some(CreateFilesystem::Apfs) {
            return Err(crate::AppError::new(
                "Native APFS encryption requires the APFS filesystem.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreateFilesystem {
    Ext4,
    Btrfs,
    Xfs,
    Apfs,
    Ufs1,
    Ufs2,
    Hammer2,
    Ntfs,
    Exfat,
    Zfs,
    Fat32,
}

impl CreateFilesystem {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ext4" => Some(Self::Ext4),
            "btrfs" => Some(Self::Btrfs),
            "xfs" => Some(Self::Xfs),
            "apfs" => Some(Self::Apfs),
            "ufs1" | "ffs1" => Some(Self::Ufs1),
            "ufs2" | "ffs2" => Some(Self::Ufs2),
            "hammer2" => Some(Self::Hammer2),
            "ntfs" => Some(Self::Ntfs),
            "exfat" => Some(Self::Exfat),
            "zfs" | "openzfs" => Some(Self::Zfs),
            "fat32" => Some(Self::Fat32),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ext4 => "ext4",
            Self::Btrfs => "btrfs",
            Self::Xfs => "xfs",
            Self::Apfs => "apfs",
            Self::Ufs1 => "ufs1",
            Self::Ufs2 => "ufs2",
            Self::Hammer2 => "hammer2",
            Self::Ntfs => "ntfs",
            Self::Exfat => "exfat",
            Self::Zfs => "zfs",
            Self::Fat32 => "fat32",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MountCommand {
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
    pub source: PathBuf,
    #[serde(default)]
    pub peer_sources: Vec<PathBuf>,
    #[serde(default)]
    pub allow_degraded: bool,
    pub mountpoint: PathBuf,
    pub tag: Option<String>,
    pub volume_type: VolumeType,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub keyfile: Option<PathBuf>,
    /// Internal resolved capability / legacy request compatibility only.
    /// Frontends cannot select a read-only mode.
    #[doc(hidden)]
    #[serde(default, skip_serializing, deserialize_with = "legacy_readonly")]
    pub readonly: bool,
}

fn legacy_readonly<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    if bool::deserialize(deserializer)? {
        return Err(serde::de::Error::custom(
            "Read-only mode has been removed. Mount access follows filesystem/driver capability.",
        ));
    }
    Ok(false)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnmountCommand {
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
    pub target: PathBuf,
    pub tag: Option<String>,
    pub volume_type: VolumeType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyslotOperation {
    List,
    Add,
    Remove,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyslotsCommand {
    #[serde(default)]
    pub privilege_mode: PrivilegeMode,
    pub source: PathBuf,
    pub operation: KeyslotOperation,
    #[serde(default)]
    pub slot: Option<u8>,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub keyfile: Option<PathBuf>,
    #[serde(default)]
    pub new_passphrase: Option<String>,
    #[serde(default)]
    pub new_keyfile: Option<PathBuf>,
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

/// Administrator-authorized native BSD sessions. The private registry remains
/// root-owned; login-user directory sessions are merged by the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NativeVolumeListResponse {
    pub ok: bool,
    pub volumes: Vec<ActiveVolume>,
    pub error: Option<String>,
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
    fn filesystem_selection_accepts_privileged_bsd_physical_veracrypt() {
        let mut command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/dev/test-disk", "source_kind": "block", "force": true,
            "volume_type": "veracrypt", "filesystem": "ufs2", "passphrase": null,
            "size_gb": null
        }))
        .unwrap();
        for os in ["freebsd", "openbsd"] {
            command.privilege_mode = PrivilegeMode::Privileged;
            assert!(command.validate_filesystem(os).is_ok());
            command.privilege_mode = PrivilegeMode::Unprivileged;
            assert!(command.validate_filesystem(os).is_err());
        }
    }

    #[test]
    fn filesystem_selection_accepts_existing_native_device_routes() {
        let mut command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/dev/test-disk", "source_kind": "block", "force": true,
            "volume_type": "luks", "filesystem": "ext4", "passphrase": null,
            "size_gb": null
        }))
        .unwrap();
        for backend in [VolumeType::Luks, VolumeType::Luks1, VolumeType::Veracrypt] {
            command.volume_type = backend;
            assert!(command.validate_filesystem("linux").is_ok());
            command.privilege_mode = PrivilegeMode::Unprivileged;
            assert!(command.validate_filesystem("linux").is_err());
            command.privilege_mode = PrivilegeMode::Privileged;
        }
        command.volume_type = VolumeType::Apfs;
        command.filesystem = Some(CreateFilesystem::Apfs);
        assert!(command.validate_filesystem("macos").is_ok());
        command.filesystem = Some(CreateFilesystem::Exfat);
        assert!(command.validate_filesystem("macos").is_err());
        for (os, backend) in [
            ("dragonfly", VolumeType::Luks1),
        ] {
            command.volume_type = backend;
            assert!(command.validate_filesystem(os).is_err());
        }
        command.volume_type = VolumeType::Cgd;
        command.filesystem = Some(CreateFilesystem::Ufs2);
        assert!(command.validate_filesystem("netbsd").is_ok());
        command.privilege_mode = PrivilegeMode::Unprivileged;
        assert!(command.validate_filesystem("netbsd").is_err());
        command.privilege_mode = PrivilegeMode::Privileged;
        command.volume_type = VolumeType::Gocryptfs;
        assert!(command.validate_filesystem("linux").is_err());
    }

    #[test]
    fn freebsd_geli_device_filesystem_requires_privileged_native_route() {
        let mut command: CreateCommand = serde_json::from_value(serde_json::json!({
            "target": "/dev/test-data", "source_kind": "block", "force": true,
            "volume_type": "geli", "filesystem": "ufs2", "passphrase": null,
            "size_gb": null
        })).unwrap();
        assert!(command.validate_filesystem("freebsd").is_ok());
        command.privilege_mode = PrivilegeMode::Unprivileged;
        assert!(command.validate_filesystem("freebsd").is_err());
        command.privilege_mode = PrivilegeMode::Privileged;
        command.volume_type = VolumeType::Pefs;
        assert!(command.validate_filesystem("freebsd").is_err());
    }

    fn raid_create() -> CommandAction {
        serde_json::from_value(serde_json::json!({
            "action": "create", "target": "first.img", "peer_sources": ["second.img"],
            "size_gb": "1", "force": false, "source_kind": "file",
            "volume_type": "raid1c", "passphrase": "secret"
        }))
        .unwrap()
    }

    fn raid_mount() -> CommandAction {
        serde_json::from_value(serde_json::json!({
            "action": "mount", "source": "first.img", "peer_sources": ["second.img"],
            "mountpoint": "mnt", "tag": null, "volume_type": "raid1c",
            "passphrase": "secret"
        }))
        .unwrap()
    }

    #[test]
    fn raid1c_json_preserves_members_and_existing_requests_default_safely() {
        let encoded = serde_json::to_value(raid_create()).unwrap();
        assert_eq!(encoded["volume_type"], "raid1c");
        assert_eq!(encoded["peer_sources"], serde_json::json!(["second.img"]));
        for mut value in [encoded, serde_json::to_value(raid_mount()).unwrap()] {
            value.as_object_mut().unwrap().remove("peer_sources");
            value.as_object_mut().unwrap().remove("allow_degraded");
            let command: CommandAction = serde_json::from_value(value).unwrap();
            match command {
                CommandAction::Create(c) => assert!(c.peer_sources.is_empty()),
                CommandAction::Mount(c) => {
                    assert!(c.peer_sources.is_empty());
                    assert!(!c.allow_degraded);
                }
                _ => panic!(),
            }
        }
        // Old helpers know softraid but must fail to deserialize the new type.
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum LegacyBackend {
            Auto,
            Softraid,
        }
        assert!(serde_json::from_value::<LegacyBackend>(serde_json::json!("raid1c")).is_err());
    }

    #[test]
    fn raid1c_validation_rejects_other_platforms_and_single_source_backends() {
        for os in ["linux", "macos", "freebsd", "netbsd", "dragonfly"] {
            for command in [raid_create(), raid_mount()] {
                assert!(command.validate_raid1c_options(os).is_err());
            }
        }
        for backend in [
            VolumeType::Auto,
            VolumeType::Softraid,
            VolumeType::Veracrypt,
        ] {
            let CommandAction::Create(mut c) = raid_create() else {
                panic!()
            };
            c.volume_type = backend;
            assert!(CommandAction::Create(c)
                .validate_raid1c_options("openbsd")
                .is_err());
            let CommandAction::Mount(mut c) = raid_mount() else {
                panic!()
            };
            c.volume_type = backend;
            assert!(CommandAction::Mount(c)
                .validate_raid1c_options("openbsd")
                .is_err());
        }
    }

    #[test]
    fn automatic_peer_requests_are_rejected_before_backend_dispatch() {
        let CommandAction::Mount(mut c) = raid_mount() else {
            panic!()
        };
        c.volume_type = VolumeType::Auto;
        let response = crate::run_buffered(CommandAction::Mount(c));
        assert!(!response.ok);
        let error = response.error.unwrap();
        if cfg!(target_os = "openbsd") {
            assert!(error.contains("explicit RAID1C command type"));
        } else {
            assert!(error.contains("require OpenBSD"));
        }
    }

    #[test]
    fn raid1c_create_policy_requires_two_new_password_only_file_members() {
        assert!(raid_create().validate_raid1c_options("openbsd").is_ok());
        let CommandAction::Create(c) = raid_create() else {
            panic!()
        };
        let mut invalid = Vec::new();
        let mut x = c.clone();
        x.peer_sources.clear();
        invalid.push(x);
        let mut x = c.clone();
        x.peer_sources.push("third.img".into());
        invalid.push(x);
        let mut x = c.clone();
        x.peer_sources = vec![c.target.clone()];
        invalid.push(x);
        let mut x = c.clone();
        x.peer_sources = vec!["".into()];
        invalid.push(x);
        let mut x = c.clone();
        x.force = true;
        invalid.push(x);
        let mut x = c.clone();
        x.source_kind = SourceKind::Block;
        invalid.push(x);
        let mut x = c.clone();
        x.cipher = CreateCipher::Serpent;
        invalid.push(x);
        let mut x = c.clone();
        x.keyfile = Some("key".into());
        invalid.push(x);
        let mut x = c.clone();
        x.privilege_mode = PrivilegeMode::Unprivileged;
        invalid.push(x);
        for c in invalid {
            assert!(CommandAction::Create(c)
                .validate_raid1c_options("openbsd")
                .is_err());
        }
    }

    #[test]
    fn raid1c_mount_allows_descriptor_inference_and_explicit_degraded_recovery() {
        assert!(raid_mount().validate_raid1c_options("openbsd").is_ok());
        let CommandAction::Mount(c) = raid_mount() else {
            panic!()
        };
        let mut inferred = c.clone();
        inferred.peer_sources.clear();
        assert!(CommandAction::Mount(inferred.clone())
            .validate_raid1c_options("openbsd")
            .is_ok());
        inferred.allow_degraded = true;
        assert!(CommandAction::Mount(inferred.clone())
            .validate_raid1c_options("openbsd")
            .is_ok());
        inferred.volume_type = VolumeType::Auto;
        assert!(CommandAction::Mount(inferred)
            .validate_raid1c_options("openbsd")
            .is_err());
        let mut invalid = Vec::new();
        let mut x = c.clone();
        x.peer_sources.push("third.img".into());
        invalid.push(x);
        let mut x = c.clone();
        x.peer_sources = vec![c.source.clone()];
        invalid.push(x);
        let mut x = c.clone();
        x.peer_sources = vec!["".into()];
        invalid.push(x);
        let mut x = c.clone();
        x.tag = Some("work".into());
        invalid.push(x);
        let mut x = c.clone();
        x.keyfile = Some("key".into());
        invalid.push(x);
        for c in invalid {
            assert!(CommandAction::Mount(c)
                .validate_raid1c_options("openbsd")
                .is_err());
        }
    }

    #[test]
    fn older_requests_keep_privileged_behavior_and_mode_round_trips() {
        let value = serde_json::json!({"action":"mount", "source":"vault.hc", "mountpoint":"mnt",
            "tag":null, "volume_type":"auto", "passphrase":"  secret  ", "readonly":false});
        let command: CommandAction = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(command.privilege_mode(), PrivilegeMode::Privileged);
        let command = command.with_privilege_mode(PrivilegeMode::Unprivileged);
        let encoded = serde_json::to_value(command).unwrap();
        assert_eq!(encoded["privilege_mode"], "unprivileged");
        assert_eq!(encoded["passphrase"], "  secret  ");
        assert!(encoded.get("readonly").is_none());
        let roundtrip: CommandAction = serde_json::from_value(encoded).unwrap();
        assert_eq!(roundtrip.privilege_mode(), PrivilegeMode::Unprivileged);
        let mut invalid = value;
        invalid["privilege_mode"] = "invalid".into();
        assert!(serde_json::from_value::<CommandAction>(invalid).is_err());
    }

    #[test]
    fn removed_readonly_request_is_rejected_instead_of_silently_mounting_writable() {
        let value = serde_json::json!({"action":"mount", "source":"vault.hc", "mountpoint":"mnt",
            "tag":null, "volume_type":"auto", "passphrase":null, "readonly":true});
        let error = serde_json::from_value::<CommandAction>(value).unwrap_err();
        assert!(error
            .to_string()
            .contains("Read-only mode has been removed"));
    }
}
