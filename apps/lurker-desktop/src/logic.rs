use lurker_core::{
    ActiveVolume, CreateCipher, CreateCommand, MountCommand, OperationResponse, SourceKind,
    UnmountCommand, VolumeType,
};
use slint::SharedString;
use std::path::PathBuf;

pub fn build_create_command(
    target: &str,
    size_gb: &str,
    force: bool,
    source_kind: &str,
    volume_type: &str,
    cipher: &str,
    passphrase: &str,
    passphrase_confirm: &str,
) -> Result<CreateCommand, String> {
    let target = required_value("Target path", target)?;
    let source_kind = parse_source_kind(source_kind)?;
    let volume_type = parse_create_volume_type(volume_type)?;
    let cipher = CreateCipher::parse(cipher)
        .ok_or_else(|| format!("Unsupported create cipher: {cipher}"))?;
    let passphrase = required_value("Passphrase", passphrase)?;
    let passphrase_confirm = required_value("Passphrase confirmation", passphrase_confirm)?;
    if passphrase != passphrase_confirm {
        return Err("Create passphrase confirmation does not match.".into());
    }

    let size_gb = match source_kind {
        SourceKind::File => Some(required_value("Size in GB", size_gb)?.to_string()),
        SourceKind::Block => None,
    };

    Ok(CreateCommand {
        target: PathBuf::from(target),
        size_gb,
        force,
        source_kind,
        volume_type,
        cipher,
        passphrase: Some(passphrase.to_string()),
    })
}

pub fn build_mount_command(
    source: &str,
    mountpoint: &str,
    tag: &str,
    volume_type: &str,
    passphrase: &str,
) -> Result<MountCommand, String> {
    let source = required_value("Source path", source)?;
    let mountpoint = required_value("Mountpoint", mountpoint)?;
    let volume_type = VolumeType::parse(volume_type)
        .ok_or_else(|| format!("Unsupported volume type: {volume_type}"))?;
    let passphrase = required_value("Passphrase", passphrase)?;
    let tag = match volume_type {
        VolumeType::Veracrypt => None,
        _ => optional_value(tag),
    };

    Ok(MountCommand {
        source: PathBuf::from(source),
        mountpoint: PathBuf::from(mountpoint),
        tag,
        volume_type,
        passphrase: Some(passphrase.to_string()),
    })
}

pub fn build_unmount_command(
    target: &str,
    tag: &str,
    volume_type: &str,
) -> Result<UnmountCommand, String> {
    let target = required_value("Target path", target)?;
    let volume_type = VolumeType::parse(volume_type)
        .ok_or_else(|| format!("Unsupported volume type: {volume_type}"))?;
    let tag = match volume_type {
        VolumeType::Veracrypt => None,
        _ => optional_value(tag),
    };

    Ok(UnmountCommand {
        target: PathBuf::from(target),
        tag,
        volume_type,
    })
}

pub fn active_volume_items(volumes: &[ActiveVolume]) -> Vec<SharedString> {
    volumes
        .iter()
        .map(|volume| {
            let detail = volume
                .mountpoint
                .as_ref()
                .map(|mountpoint| mountpoint.display().to_string())
                .unwrap_or_else(|| "not mounted".into());
            SharedString::from(format!("{}    {}", volume.mapper_name, detail))
        })
        .collect()
}

pub fn suggested_unmount_target(volume: &ActiveVolume) -> String {
    volume
        .mountpoint
        .as_ref()
        .unwrap_or(&volume.mapper_path)
        .display()
        .to_string()
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

fn optional_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn parse_source_kind(value: &str) -> Result<SourceKind, String> {
    match value {
        "file" => Ok(SourceKind::File),
        "block" => Ok(SourceKind::Block),
        _ => Err(format!("Unsupported source kind: {value}")),
    }
}

fn parse_create_volume_type(value: &str) -> Result<VolumeType, String> {
    match value {
        "luks" => Ok(VolumeType::Luks),
        "veracrypt" => Ok(VolumeType::Veracrypt),
        _ => Err(format!("Unsupported create volume type: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_create_command, build_mount_command, build_unmount_command, suggested_unmount_target,
    };
    use lurker_core::{ActiveVolume, CreateCipher, SourceKind, VolumeType};
    use std::path::PathBuf;

    #[test]
    fn build_create_command_requires_confirmation_match() {
        let error = build_create_command(
            "/tmp/vault.img",
            "4",
            false,
            "file",
            "luks",
            "aes",
            "alpha",
            "bravo",
        )
        .unwrap_err();

        assert!(error.contains("confirmation"));
    }

    #[test]
    fn build_create_command_supports_block_without_size() {
        let command = build_create_command(
            "/dev/sdb1",
            "",
            true,
            "block",
            "veracrypt",
            "twofish",
            "secret",
            "secret",
        )
        .unwrap();

        assert_eq!(command.source_kind, SourceKind::Block);
        assert_eq!(command.volume_type, VolumeType::Veracrypt);
        assert_eq!(command.cipher, CreateCipher::Twofish);
        assert_eq!(command.size_gb, None);
    }

    #[test]
    fn build_mount_command_drops_veracrypt_tag() {
        let command = build_mount_command(
            "/tmp/vault.hc",
            "/mnt/vault",
            "ignored",
            "veracrypt",
            "secret",
        )
        .unwrap();

        assert_eq!(command.volume_type, VolumeType::Veracrypt);
        assert_eq!(command.tag, None);
    }

    #[test]
    fn build_unmount_command_keeps_luks_tag() {
        let command = build_unmount_command("/mnt/vault", "work", "luks").unwrap();
        assert_eq!(command.volume_type, VolumeType::Luks);
        assert_eq!(command.tag.as_deref(), Some("work"));
    }

    #[test]
    fn suggested_unmount_target_prefers_mountpoint() {
        let volume = ActiveVolume {
            mapper_name: "lurker_test".into(),
            mapper_path: PathBuf::from("/dev/mapper/lurker_test"),
            mountpoint: Some(PathBuf::from("/mnt/test")),
            filesystem_type: Some("btrfs".into()),
        };

        assert_eq!(suggested_unmount_target(&volume), "/mnt/test");
    }
}
