use lurker_core::{
    ActiveVolume, CreateCipher, CreateCommand, KeyslotOperation, KeyslotsCommand, MountCommand,
    OperationResponse, SourceKind, UnmountCommand, VolumeType,
};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeRowView {
    pub name: String,
    pub source: String,
    pub mount: String,
    pub readonly: bool,
    pub filesystem: String,
    pub source_kind: String,
    pub unprivileged: bool,
}

/// Keep presentation choices within the core's approved OS matrix.
pub fn create_formats(os: &str, unprivileged: bool) -> Vec<&'static str> {
    [
        "luks",
        "apple",
        "geli",
        "cgd",
        "softraid",
        "raid1c",
        "luks1",
        "vera",
        "gocryptfs",
        "cryfs",
        "cryptomator",
        "pefs",
    ]
    .into_iter()
    .filter(|name| {
        let backend = parse_format(name).unwrap();
        backend.supports_platform(os)
            && (!unprivileged
                || backend == VolumeType::Veracrypt
                || matches!(
                    backend,
                    VolumeType::Gocryptfs | VolumeType::Cryfs | VolumeType::Cryptomator
                ))
    })
    .collect()
}

fn parse_format(format: &str) -> Option<VolumeType> {
    VolumeType::parse(if format == "vera" {
        "veracrypt"
    } else {
        format
    })
    .filter(|backend| *backend != VolumeType::Auto)
}

pub fn validate_create_platform(
    command: &CreateCommand,
    os: &str,
    mode: lurker_core::PrivilegeMode,
) -> Result<(), String> {
    if !command.volume_type.supports_platform(os) {
        return Err(format!(
            "{} creation is unavailable on {os}.",
            command.volume_type
        ));
    }
    if command.source_kind == SourceKind::Block
        && (os != "linux" || mode == lurker_core::PrivilegeMode::Unprivileged)
    {
        return Err("Physical-device creation requires Linux Privileged mode.".into());
    }
    if mode == lurker_core::PrivilegeMode::Unprivileged
        && !create_formats(os, true)
            .iter()
            .any(|name| parse_format(name) == Some(command.volume_type))
    {
        return Err("Choose VeraCrypt or a directory backend in Unprivileged mode.".into());
    }
    Ok(())
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
    auth_method: &str,
    key_file: &str,
) -> Result<CreateCommand, String> {
    let source_kind = match target_kind {
        "file" => SourceKind::File,
        "partition" => SourceKind::Block,
        "directory" => SourceKind::Directory,
        _ => return Err(format!("Unsupported target kind: {target_kind}")),
    };
    let volume_type =
        parse_format(format).ok_or_else(|| format!("Unsupported format: {format}"))?;
    if volume_type.is_directory() != (source_kind == SourceKind::Directory) {
        return Err("Choose a directory target for directory encryption, or a file/device for container encryption.".into());
    }
    // Hidden fixed-profile fields must not carry a previous backend's cipher.
    let cipher = if matches!(
        volume_type,
        VolumeType::Luks | VolumeType::Luks1 | VolumeType::Veracrypt
    ) {
        CreateCipher::parse(cipher).ok_or_else(|| format!("Unsupported create cipher: {cipher}"))?
    } else {
        CreateCipher::Aes
    };

    let target = match source_kind {
        SourceKind::File | SourceKind::Directory => {
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

    let (passphrase, keyfile) = selected_credential(auth_method, passphrase, key_file)?;
    if let Some(passphrase) = &passphrase {
        if passphrase != required_secret("Passphrase confirmation", confirm)? {
            return Err("Create passphrase confirmation does not match.".into());
        }
    }
    if !matches!(
        volume_type,
        VolumeType::Luks
            | VolumeType::Luks1
            | VolumeType::Veracrypt
            | VolumeType::Geli
            | VolumeType::Pefs
    ) && keyfile.is_some()
    {
        return Err(format!("{volume_type} accepts a passphrase only."));
    }

    let size_gb = match source_kind {
        SourceKind::File => {
            let raw = required_value("Size", size)?;
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("Size must be a number, got: {raw}"))?;
            if !value.is_finite() || value <= 0.0 {
                return Err("Size must be a positive finite number.".into());
            }
            let in_gb = match size_unit {
                "GB" | "gb" => value,
                "MB" | "mb" => value / 1024.0,
                other => return Err(format!("Unsupported size unit: {other}")),
            };
            Some(format!("{in_gb}"))
        }
        SourceKind::Directory | SourceKind::Block => None,
    };

    Ok(CreateCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        target: PathBuf::from(target),
        peer_sources: Vec::new(),
        size_gb,
        force: source_kind == SourceKind::Block,
        source_kind,
        volume_type,
        cipher,
        passphrase,
        keyfile,
    })
}

/// Add mirrored members only after the ordinary form has selected credentials.
pub fn with_create_members(
    mut command: CreateCommand,
    member: &str,
    os: &str,
) -> Result<CreateCommand, String> {
    if command.volume_type == VolumeType::Raid1c {
        command
            .peer_sources
            .push(input_path(required_value("Second member file", member)?)?);
    }
    lurker_core::CommandAction::Create(command.clone())
        .validate_raid1c_options(os)
        .map_err(|error| error.message)?;
    Ok(command)
}

pub fn with_mount_members(
    mut command: MountCommand,
    mirrored: bool,
    member: &str,
    degraded: bool,
    os: &str,
) -> Result<MountCommand, String> {
    if mirrored {
        command.volume_type = VolumeType::Raid1c;
        command.allow_degraded = degraded;
        if !member.trim().is_empty() {
            command.peer_sources.push(input_path(member.trim())?);
        }
    }
    lurker_core::CommandAction::Mount(command.clone())
        .validate_raid1c_options(os)
        .map_err(|error| error.message)?;
    Ok(command)
}

pub fn build_mount_command(
    source: &str,
    mount_point: &str,
    auth_method: &str,
    passphrase: &str,
    key_file: &str,
    readonly: bool,
    source_kind: &str,
) -> Result<MountCommand, String> {
    let source = required_value("Source", source)?;
    let mountpoint = required_value("Mount point", mount_point)?;
    let (passphrase, keyfile) = selected_credential(auth_method, passphrase, key_file)?;

    // The platform backend detects the container type.
    // We don't have a UI field for type in the new design, so pass Auto.
    let volume_type = VolumeType::Auto;
    let _ = source_kind;

    Ok(MountCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        source: PathBuf::from(source),
        peer_sources: Vec::new(),
        allow_degraded: false,
        mountpoint: PathBuf::from(mountpoint),
        tag: None,
        volume_type,
        passphrase,
        keyfile,
        readonly,
    })
}

// Resolve typed paths in the desktop session before the helper can elevate
// and run with a different working directory or HOME.
fn input_path(value: &str) -> Result<PathBuf, String> {
    let path = if let Some(suffix) = value.strip_prefix("~/") {
        PathBuf::from(std::env::var_os("HOME").ok_or("Could not resolve your home directory.")?)
            .join(suffix)
    } else {
        PathBuf::from(value)
    };
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|error| format!("Could not resolve the current directory: {error}"))
}

fn selected_credential(
    auth_method: &str,
    passphrase: &str,
    key_file: &str,
) -> Result<(Option<String>, Option<PathBuf>), String> {
    match auth_method {
        "pass" | "password" => Ok((
            Some(required_secret("Passphrase", passphrase)?.to_owned()),
            None,
        )),
        "key" => Ok((
            None,
            Some(input_path(required_value("Key file", key_file)?)?),
        )),
        _ => Err("Choose Passphrase or Key file.".into()),
    }
}

pub fn build_keyslots_command(
    source: &str,
    operation: &str,
    slot: &str,
    auth_method: &str,
    passphrase: &str,
    key_file: &str,
    new_auth_method: &str,
    new_passphrase: &str,
    confirm: &str,
    new_key_file: &str,
) -> Result<KeyslotsCommand, String> {
    let operation = match operation {
        "list" => KeyslotOperation::List,
        "add" => KeyslotOperation::Add,
        "remove" => KeyslotOperation::Remove,
        _ => return Err("Choose List, Add or Remove.".into()),
    };
    let source = input_path(required_value("LUKS container", source)?)?;
    let mut command = KeyslotsCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        source,
        operation,
        slot: None,
        passphrase: None,
        keyfile: None,
        new_passphrase: None,
        new_keyfile: None,
    };
    if operation == KeyslotOperation::List {
        return Ok(command);
    }
    if !slot.trim().is_empty() {
        command.slot = Some(
            slot.trim()
                .parse::<u8>()
                .ok()
                .filter(|id| *id < 32)
                .ok_or("Keyslot must be between 0 and 31.")?,
        );
    } else if operation == KeyslotOperation::Remove {
        return Err("Choose the keyslot to remove.".into());
    }
    (command.passphrase, command.keyfile) = selected_credential(auth_method, passphrase, key_file)?;
    if operation == KeyslotOperation::Add {
        (command.new_passphrase, command.new_keyfile) =
            selected_credential(new_auth_method, new_passphrase, new_key_file)?;
        if command
            .new_passphrase
            .as_deref()
            .is_some_and(|value| value != confirm)
        {
            return Err("New passphrase confirmation does not match.".into());
        }
    }
    Ok(command)
}

pub fn build_unmount_command_for_volume(volume: &ActiveVolume) -> UnmountCommand {
    UnmountCommand {
        privilege_mode: volume.privilege_mode,
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
                unprivileged: volume.privilege_mode == lurker_core::PrivilegeMode::Unprivileged,
                name: volume.mapper_name.clone(),
                source: source_display,
                mount: mount_display,
                readonly: volume.readonly,
                filesystem: volume.filesystem_type.as_deref().unwrap_or("").to_string(),
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
    fn mirrored_forms_validate_members_and_ignore_inactive_recovery_fields() {
        let create = build_create_command(
            "file", "raid1c", "/tmp", "mirror-a", "", "128", "MB", "twofish", "password",
            "password", "pass", "",
        )
        .unwrap();
        assert_eq!(create.cipher, CreateCipher::Aes);
        assert!(super::with_create_members(create.clone(), "", "openbsd").is_err());
        assert!(super::with_create_members(create.clone(), "/tmp/mirror-a", "openbsd").is_err());
        assert!(super::with_create_members(create.clone(), "/tmp/mirror-b", "linux").is_err());
        let created = super::with_create_members(create, "/tmp/mirror-b", "openbsd").unwrap();
        assert_eq!(created.peer_sources, vec![PathBuf::from("/tmp/mirror-b")]);
        let mount = build_mount_command(
            "/tmp/mirror-a",
            "/tmp/mount",
            "pass",
            "password",
            "",
            false,
            "file",
        )
        .unwrap();
        let full =
            super::with_mount_members(mount.clone(), true, "/tmp/mirror-b", false, "openbsd")
                .unwrap();
        assert_eq!(full.volume_type, VolumeType::Raid1c);
        assert_eq!(full.peer_sources, created.peer_sources);
        let degraded = super::with_mount_members(mount.clone(), true, "", true, "openbsd").unwrap();
        assert!(degraded.allow_degraded && degraded.peer_sources.is_empty());
        let inferred =
            super::with_mount_members(mount.clone(), true, "", false, "openbsd").unwrap();
        assert!(inferred.peer_sources.is_empty());
        assert!(
            super::with_mount_members(mount.clone(), true, "/tmp/mirror-a", false, "openbsd")
                .is_err()
        );
        assert!(super::with_mount_members(mount.clone(), true, "", true, "freebsd").is_err());
        let ordinary =
            super::with_mount_members(mount, false, "stale peer", true, "linux").unwrap();
        assert_eq!(ordinary.volume_type, VolumeType::Auto);
        assert!(ordinary.peer_sources.is_empty() && !ordinary.allow_degraded);
    }

    #[test]
    fn create_matrix_defaults_and_directory_profiles_follow_the_core() {
        use lurker_core::PrivilegeMode;
        for (os, default, expected_count) in [
            ("linux", "luks", 6),
            ("macos", "apple", 5),
            ("freebsd", "geli", 4),
            ("netbsd", "cgd", 1),
            ("openbsd", "softraid", 3),
            ("dragonfly", "luks1", 2),
        ] {
            let formats = super::create_formats(os, false);
            assert_eq!(formats[0], default);
            assert_eq!(formats.len(), expected_count);
            for format in formats {
                let backend = super::parse_format(format).unwrap();
                let directory = backend.is_directory();
                let command = build_create_command(
                    if directory { "directory" } else { "file" },
                    format,
                    "/tmp",
                    "desktop-validation",
                    "",
                    if directory {
                        "stale invalid size"
                    } else {
                        "128"
                    },
                    "MB",
                    "serpent",
                    "test password",
                    "test password",
                    "pass",
                    "",
                )
                .unwrap();
                assert_eq!(command.volume_type, backend);
                assert_eq!(command.size_gb.is_none(), directory);
                assert!(!command.force);
                assert_eq!(
                    command.cipher == CreateCipher::Serpent,
                    matches!(
                        backend,
                        VolumeType::Luks | VolumeType::Luks1 | VolumeType::Veracrypt
                    )
                );
                let command =
                    super::with_create_members(command, "/tmp/desktop-validation-peer", os)
                        .unwrap();
                super::validate_create_platform(&command, os, PrivilegeMode::Privileged).unwrap();
                let unprivileged =
                    super::validate_create_platform(&command, os, PrivilegeMode::Unprivileged);
                assert_eq!(
                    unprivileged.is_ok(),
                    backend == VolumeType::Veracrypt
                        || matches!(
                            backend,
                            VolumeType::Gocryptfs | VolumeType::Cryfs | VolumeType::Cryptomator
                        )
                );
            }
        }
        assert!(super::create_formats("windows", false).is_empty());
    }

    #[test]
    fn unsupported_targets_credentials_and_invalid_sizes_fail_before_helper() {
        use lurker_core::PrivilegeMode;
        for format in ["gocryptfs", "cryfs", "cryptomator", "pefs"] {
            assert!(build_create_command(
                "file", format, "/tmp", "vault", "", "1", "GB", "aes", "password", "password",
                "pass", ""
            )
            .is_err());
        }
        for format in [
            "apple",
            "cgd",
            "softraid",
            "gocryptfs",
            "cryfs",
            "cryptomator",
        ] {
            let directory = super::parse_format(format).unwrap().is_directory();
            assert!(build_create_command(
                if directory { "directory" } else { "file" },
                format,
                "/tmp",
                "vault",
                "",
                "1",
                "GB",
                "aes",
                "",
                "",
                "key",
                "/tmp/key"
            )
            .is_err());
        }
        for size in ["NaN", "inf", "0", "-1"] {
            assert!(build_create_command(
                "file", "luks", "/tmp", "vault", "", size, "GB", "aes", "password", "password",
                "pass", ""
            )
            .is_err());
        }
        let raw = build_create_command(
            "partition",
            "vera",
            "",
            "",
            "/dev/fake",
            "",
            "GB",
            "aes",
            "password",
            "password",
            "pass",
            "",
        )
        .unwrap();
        for os in ["macos", "freebsd", "netbsd", "openbsd", "dragonfly"] {
            assert!(super::validate_create_platform(&raw, os, PrivilegeMode::Privileged).is_err());
        }
        assert!(
            super::validate_create_platform(&raw, "linux", PrivilegeMode::Unprivileged).is_err()
        );
        super::validate_create_platform(&raw, "linux", PrivilegeMode::Privileged).unwrap();
    }

    #[test]
    fn create_keyfile_authentication_never_carries_a_password() {
        for format in ["luks", "vera"] {
            let command = build_create_command(
                "file",
                format,
                "/tmp",
                "vault",
                "",
                "1",
                "GB",
                "aes",
                "stale password",
                "mismatch",
                "key",
                "/tmp/binary.key",
            )
            .unwrap();
            assert!(command.passphrase.is_none());
            assert_eq!(command.keyfile, Some("/tmp/binary.key".into()));
        }
        assert!(build_create_command(
            "file", "apple", "/tmp", "vault", "", "1", "GB", "aes", "", "", "key", "/tmp/key"
        )
        .is_err());
        assert!(build_create_command(
            "file", "vera", "/tmp", "vault", "", "1", "GB", "aes", "", "", "key", " "
        )
        .is_err());
        assert!(
            build_mount_command("vault", "/mnt/v", "invalid", "password", "", false, "file")
                .is_err()
        );
    }

    #[test]
    fn keyslot_forms_build_independent_credentials_and_validate_selection() {
        for old_method in ["pass", "key"] {
            for new_method in ["pass", "key"] {
                let command = super::build_keyslots_command(
                    "vault", "add", "31", old_method, " old ", "old.key", new_method, " new ",
                    " new ", "new.key",
                )
                .unwrap();
                assert_eq!(command.slot, Some(31));
                assert_eq!(command.passphrase.is_some(), old_method == "pass");
                assert_eq!(command.keyfile.is_some(), old_method == "key");
                assert_eq!(command.new_passphrase.is_some(), new_method == "pass");
                assert_eq!(command.new_keyfile.is_some(), new_method == "key");
                if let Some(password) = command.new_passphrase {
                    assert_eq!(password, " new ");
                }
            }
        }
        let listed = super::build_keyslots_command(
            "vault",
            "list",
            "stale",
            "pass",
            "stale password",
            "",
            "key",
            "",
            "",
            "stale.key",
        )
        .unwrap();
        assert!(
            listed.slot.is_none() && listed.passphrase.is_none() && listed.new_keyfile.is_none()
        );
        let automatic = super::build_keyslots_command(
            "vault", "add", "", "pass", "old", "", "key", "", "", "new.key",
        )
        .unwrap();
        assert!(automatic.slot.is_none());
        for slot in ["", "32", "-1", "abc"] {
            assert!(super::build_keyslots_command(
                "vault", "remove", slot, "pass", "old", "", "key", "", "", "new.key"
            )
            .is_err());
        }
        assert!(super::build_keyslots_command(
            "vault",
            "add",
            "1",
            "pass",
            "old",
            "",
            "pass",
            "new",
            "different",
            ""
        )
        .is_err());
        let removed = super::build_keyslots_command(
            "vault",
            "remove",
            "0",
            "key",
            "stale",
            "remaining.key",
            "pass",
            "stale",
            "stale",
            "stale.key",
        )
        .unwrap();
        assert!(
            removed.passphrase.is_none()
                && removed.new_passphrase.is_none()
                && removed.new_keyfile.is_none()
        );
        assert_eq!(
            removed.keyfile,
            Some(std::env::current_dir().unwrap().join("remaining.key"))
        );
    }

    #[test]
    fn generated_passphrases_use_system_randomness() {
        let first = super::generate_passphrase().unwrap();
        let second = super::generate_passphrase().unwrap();
        assert_eq!(first.len(), 32);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn passwords_preserve_leading_and_trailing_whitespace() {
        let password = "  secret 密碼  ";
        let cmd = build_create_command(
            "file", "vera", "/tmp", "vault.hc", "", "1", "GB", "aes", password, password, "pass",
            "",
        )
        .unwrap();
        assert_eq!(cmd.passphrase.as_deref(), Some(password));
        assert!(build_create_command(
            "file",
            "vera",
            "/tmp",
            "vault.hc",
            "",
            "1",
            "GB",
            "aes",
            password,
            password.trim(),
            "pass",
            ""
        )
        .is_err());
        let cmd = build_mount_command(
            "/tmp/vault.hc",
            "/tmp/mnt",
            "password",
            password,
            "",
            false,
            "file",
        )
        .unwrap();
        assert_eq!(cmd.passphrase.as_deref(), Some(password));
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
            "pass",
            "",
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
            "file", "luks", "~/", "a.luks", "", "1024", "MB", "aes", "secret", "secret", "pass", "",
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
            "pass",
            "",
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
            "file", "luks", "~/", "a", "", "1", "GB", "aes", "alpha", "bravo", "pass", "",
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
    fn build_mount_command_accepts_key_file_auth() {
        let command = build_mount_command(
            "/tmp/v.luks",
            "/mnt/v",
            "key",
            "",
            "/tmp/k.key",
            false,
            "file",
        )
        .unwrap();
        assert!(command.passphrase.is_none());
        assert_eq!(command.keyfile, Some("/tmp/k.key".into()));
    }

    #[test]
    fn unmount_keeps_the_volumes_original_privilege_mode() {
        let volume = ActiveVolume {
            privilege_mode: lurker_core::PrivilegeMode::Unprivileged,
            source_path: Some("/tmp/vault.hc".into()),
            mapper_name: "vault.hc".into(),
            mapper_path: "/tmp/private/raw/volume".into(),
            mountpoint: Some("/tmp/mnt".into()),
            readonly: true,
            filesystem_type: Some("btrfs".into()),
        };
        let command = super::build_unmount_command_for_volume(&volume);
        assert_eq!(
            command.privilege_mode,
            lurker_core::PrivilegeMode::Unprivileged
        );
        assert!(super::volume_rows(&[volume])[0].unprivileged);
    }

    #[test]
    fn volume_rows_projects_active_volumes() {
        let rows = volume_rows(&[ActiveVolume {
            privilege_mode: lurker_core::PrivilegeMode::Privileged,
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
        assert_eq!(rows[0].filesystem, "btrfs");
    }
    #[test]
    fn apple_create_preserves_passphrase_spaces_and_mac_volume_metadata() {
        let c = build_create_command(
            "file", "apple", "/tmp", "a.dmg", "", "32", "MB", "aes", " secret ", " secret ",
            "pass", "",
        )
        .unwrap();
        assert_eq!(c.volume_type, VolumeType::Apple);
        assert_eq!(c.passphrase.as_deref(), Some(" secret "));
        let v = ActiveVolume {
            privilege_mode: lurker_core::PrivilegeMode::Privileged,
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
