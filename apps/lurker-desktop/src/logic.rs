use lurker_core::{
    ActiveVolume, CreateCipher, CreateCommand, MountCommand, OperationResponse, SourceKind,
    UnmountCommand, VolumeType,
};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeRowView {
    pub name: String,
    pub source: String,
    pub mount: String,
    pub readonly: bool,
    pub cipher: String,
    pub source_kind: String,
}

pub fn build_create_command(
    target_kind: &str,
    format: &str,
    file_path: &str,
    file_name: &str,
    partition: &str,
    size: &str,
    size_unit: &str,
    cipher: &str,
    passphrase: &str,
    confirm: &str,
) -> Result<CreateCommand, String> {
    let source_kind = match target_kind {
        "file" => SourceKind::File,
        "partition" => SourceKind::Block,
        _ => return Err(format!("Unsupported target kind: {target_kind}")),
    };
    let volume_type = match format {
        "luks" => VolumeType::Luks,
        "apple" => VolumeType::Apple,
        "vera" | "veracrypt" => VolumeType::Veracrypt,
        _ => return Err(format!("Unsupported format: {format}")),
    };
    // Apple has a fixed profile and no cipher selector in the form. A previous
    // VeraCrypt selection may still be stored when the user switches formats.
    let cipher = if volume_type == VolumeType::Apple {
        CreateCipher::Aes
    } else {
        CreateCipher::parse(cipher).ok_or_else(|| format!("Unsupported create cipher: {cipher}"))?
    };

    let target = match source_kind {
        SourceKind::File => {
            let path = required_value("Location", file_path)?;
            let name = required_value("Filename", file_name)?;
            let joined = if path.ends_with('/') {
                format!("{path}{name}")
            } else {
                format!("{path}/{name}")
            };
            joined
        }
        SourceKind::Block => required_value("Partition", partition)?.to_string(),
    };

    let passphrase = required_secret("Passphrase", passphrase)?;
    let confirm = required_secret("Passphrase confirmation", confirm)?;
    if passphrase != confirm {
        return Err("Create passphrase confirmation does not match.".into());
    }

    let size_gb = match source_kind {
        SourceKind::File => {
            let raw = required_value("Size", size)?;
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("Size must be a number, got: {raw}"))?;
            let in_gb = match size_unit {
                "GB" | "gb" => value,
                "MB" | "mb" => value / 1024.0,
                other => return Err(format!("Unsupported size unit: {other}")),
            };
            Some(format!("{in_gb}"))
        }
        SourceKind::Block => None,
    };

    Ok(CreateCommand {
        target: PathBuf::from(target),
        size_gb,
        force: source_kind == SourceKind::Block,
        source_kind,
        volume_type,
        cipher,
        passphrase: Some(passphrase.to_string()),
    })
}

pub fn build_mount_command(
    source: &str,
    mount_point: &str,
    auth_method: &str,
    passphrase: &str,
    _key_file: &str,
    readonly: bool,
    source_kind: &str,
) -> Result<MountCommand, String> {
    if auth_method == "key" {
        return Err("Key-file unlock is not yet supported.".into());
    }

    let source = required_value("Source", source)?;
    let mountpoint = required_value("Mount point", mount_point)?;
    let passphrase = required_secret("Passphrase", passphrase)?;

    // The platform backend detects the container type.
    // We don't have a UI field for type in the new design, so pass Auto.
    let volume_type = VolumeType::Auto;
    let _ = source_kind;

    Ok(MountCommand {
        source: PathBuf::from(source),
        mountpoint: PathBuf::from(mountpoint),
        tag: None,
        volume_type,
        passphrase: Some(passphrase.to_string()),
        readonly,
    })
}

pub fn build_unmount_command_for_volume(volume: &ActiveVolume) -> UnmountCommand {
    UnmountCommand {
        target: volume
            .mountpoint
            .clone()
            .unwrap_or_else(|| volume.mapper_path.clone()),
        tag: None,
        volume_type: VolumeType::Auto,
    }
}

pub fn volume_rows(volumes: &[ActiveVolume]) -> Vec<VolumeRowView> {
    volumes
        .iter()
        .map(|volume| {
            let source_display = volume
                .source_path
                .as_ref()
                .unwrap_or(&volume.mapper_path)
                .display()
                .to_string();
            let mount_display = volume
                .mountpoint
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(unmounted)".to_string());
            // Linux currently exposes mapper devices; macOS also supplies the backing file.
            let source_kind = if volume.source_path.is_some() {
                "file"
            } else {
                "partition"
            }
            .to_string();
            VolumeRowView {
                name: volume.mapper_name.clone(),
                source: source_display,
                mount: mount_display,
                readonly: volume.readonly,
                cipher: volume.filesystem_type.as_deref().unwrap_or("").to_string(),
                source_kind,
            }
        })
        .collect()
}

pub fn response_error(response: &OperationResponse) -> String {
    response
        .error
        .clone()
        .unwrap_or_else(|| "The operation failed.".into())
}

fn required_value<'a>(label: &str, value: &'a str) -> Result<&'a str, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{label} is required."))
    } else {
        Ok(trimmed)
    }
}

// The existing Generate button must never return a fixed example password.
pub fn generate_passphrase() -> Result<String, String> {
    use std::io::Read;
    let mut random = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|e| format!("Could not read system randomness: {e}"))?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::{build_create_command, build_mount_command, volume_rows};
    use lurker_core::{ActiveVolume, CreateCipher, SourceKind, VolumeType};
    use std::path::PathBuf;

    #[test]
    fn generated_passphrases_use_system_randomness() {
        let first = super::generate_passphrase().unwrap();
        let second = super::generate_passphrase().unwrap();
        assert_eq!(first.len(), 32);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn build_create_command_joins_file_path_and_name() {
        let cmd = build_create_command(
            "file",
            "luks",
            "~/vaults/",
            "archive.luks",
            "",
            "4",
            "GB",
            "aes",
            "secret",
            "secret",
        )
        .unwrap();
        assert_eq!(cmd.target, PathBuf::from("~/vaults/archive.luks"));
        assert_eq!(cmd.source_kind, SourceKind::File);
        assert_eq!(cmd.volume_type, VolumeType::Luks);
        assert_eq!(cmd.cipher, CreateCipher::Aes);
        assert_eq!(cmd.size_gb.as_deref(), Some("4"));
    }

    #[test]
    fn build_create_command_converts_mb_to_gb() {
        let cmd = build_create_command(
            "file", "luks", "~/", "a.luks", "", "1024", "MB", "aes", "secret", "secret",
        )
        .unwrap();
        assert_eq!(cmd.size_gb.as_deref(), Some("1"));
    }

    #[test]
    fn build_create_command_for_partition_skips_size() {
        let cmd = build_create_command(
            "partition",
            "vera",
            "",
            "",
            "/dev/sdb1",
            "",
            "GB",
            "twofish",
            "secret",
            "secret",
        )
        .unwrap();
        assert_eq!(cmd.target, PathBuf::from("/dev/sdb1"));
        assert_eq!(cmd.source_kind, SourceKind::Block);
        assert_eq!(cmd.volume_type, VolumeType::Veracrypt);
        assert_eq!(cmd.cipher, CreateCipher::Twofish);
        assert_eq!(cmd.size_gb, None);
    }

    #[test]
    fn build_create_command_rejects_mismatched_confirm() {
        let err = build_create_command(
            "file", "luks", "~/", "a", "", "1", "GB", "aes", "alpha", "bravo",
        )
        .unwrap_err();
        assert!(err.contains("confirmation"));
    }

    #[test]
    fn build_mount_command_accepts_readonly_flag() {
        let cmd = build_mount_command("/tmp/v.luks", "/mnt/v", "pass", "secret", "", true, "file")
            .unwrap();
        assert_eq!(cmd.source, PathBuf::from("/tmp/v.luks"));
        assert_eq!(cmd.mountpoint, PathBuf::from("/mnt/v"));
        assert!(cmd.readonly);
        assert_eq!(cmd.tag, None);
    }

    #[test]
    fn build_mount_command_rejects_key_file_auth() {
        let err = build_mount_command(
            "/tmp/v.luks",
            "/mnt/v",
            "key",
            "",
            "/tmp/k.key",
            false,
            "file",
        )
        .unwrap_err();
        assert!(err.contains("Key"));
    }

    #[test]
    fn volume_rows_projects_active_volumes() {
        let rows = volume_rows(&[ActiveVolume {
            source_path: None,
            readonly: false,
            mapper_name: "lurker_test".into(),
            mapper_path: PathBuf::from("/dev/mapper/lurker_test"),
            mountpoint: Some(PathBuf::from("/mnt/test")),
            filesystem_type: Some("btrfs".into()),
        }]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "lurker_test");
        assert_eq!(rows[0].mount, "/mnt/test");
        assert_eq!(rows[0].cipher, "btrfs");
    }
    #[test]
    fn apple_create_preserves_passphrase_spaces_and_mac_volume_metadata() {
        let c = build_create_command(
            "file", "apple", "/tmp", "a.dmg", "", "32", "MB", "aes", " secret ", " secret ",
        )
        .unwrap();
        assert_eq!(c.volume_type, VolumeType::Apple);
        assert_eq!(c.passphrase.as_deref(), Some(" secret "));
        let v = ActiveVolume {
            mapper_name: "a.dmg".into(),
            mapper_path: "/dev/disk10".into(),
            source_path: Some("/tmp/a.dmg".into()),
            readonly: true,
            mountpoint: Some("/tmp/mount".into()),
            filesystem_type: Some("apfs".into()),
        };
        let rows = volume_rows(&[v]);
        assert_eq!(rows[0].source, "/tmp/a.dmg");
        assert_eq!(rows[0].source_kind, "file");
        assert!(rows[0].readonly);
        let c = build_mount_command(
            "/tmp/a.dmg",
            "/tmp/mount",
            "pass",
            " secret ",
            "",
            true,
            "file",
        )
        .unwrap();
        assert_eq!(c.passphrase.as_deref(), Some(" secret "));
    }
}

fn required_secret<'a>(label: &str, value: &'a str) -> Result<&'a str, String> {
    if value.is_empty() {
        Err(format!("{label} is required."))
    } else {
        Ok(value)
    }
}
