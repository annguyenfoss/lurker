use lurker_core::{
    AppError, ColorMode, CommandAction, CreateCipher, CreateCommand, CreateFilesystem,
    KeyslotOperation, KeyslotsCommand, MountCommand, SourceKind, UnmountCommand, VolumeType,
};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::IsTerminal;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub struct Cli {
    pub color_mode: ColorMode,
    pub command: CommandAction,
}

#[derive(Debug)]
pub enum ParseOutcome {
    Run(Cli),
    ListNative(ColorMode),
    SessionClose {
        command: UnmountCommand,
        timeout: Option<Duration>,
        color_mode: ColorMode,
    },
    Help(HelpRequest),
}

#[derive(Debug)]
pub struct HelpRequest {
    pub script_name: String,
    pub color_mode: ColorMode,
    pub to_stdout: bool,
}

#[derive(Debug)]
pub struct ParseFailure {
    pub message: Option<String>,
    pub exit_code: i32,
    pub show_help: bool,
    pub help_to_stdout: bool,
    pub script_name: String,
    pub color_mode: ColorMode,
}

pub fn parse_env_args<I>(args: I) -> Result<ParseOutcome, ParseFailure>
where
    I: IntoIterator<Item = OsString>,
{
    parse_args_for_platform(args, std::env::consts::OS)
}

fn parse_args_for_platform<I>(args: I, os: &str) -> Result<ParseOutcome, ParseFailure>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let script = args.next().unwrap_or_else(|| OsString::from("lurker"));
    let script_name = Path::new(&script)
        .file_name()
        .unwrap_or(script.as_os_str())
        .to_string_lossy()
        .into_owned();
    let raw_args: Vec<OsString> = args.collect();

    let mut color_mode = ColorMode::Auto;
    let mut volume_type = VolumeType::Auto;
    let mut volume_type_explicit = false;
    let mut peer_sources = Vec::new();
    let mut allow_degraded = false;
    let mut privilege_mode = lurker_core::PrivilegeMode::Privileged;
    let mut privilege_mode_explicit = false;
    let mut create_cipher = CreateCipher::Aes;
    let mut create_cipher_explicit = false;
    let mut create_filesystem = None;
    let mut keyfile = None;
    let mut new_keyfile = None;
    let mut slot = None;
    let mut close_timeout = None;
    let mut parsed_args = Vec::new();

    let mut index = 0usize;
    while index < raw_args.len() {
        let current = raw_args[index].to_string_lossy();
        let option = current.split('=').next().unwrap();
        if option == "--readonly" {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "Option --readonly was removed. Mount access follows filesystem and driver capabilities.",
            ));
        }
        if matches!(
            option,
            "--key-file" | "--new-key-file" | "--slot" | "--member" | "--timeout" | "--filesystem"
        ) {
            let value = if current.contains('=') {
                OsString::from_vec(raw_args[index].as_bytes()[option.len() + 1..].to_vec())
            } else {
                index += 1;
                raw_args
                    .get(index)
                    .ok_or_else(|| {
                        usage_failure(
                            &script_name,
                            color_mode,
                            &format!("Option {option} requires an argument."),
                        )
                    })?
                    .clone()
            };
            if value.is_empty() {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    &format!("Option {option} requires a non-empty argument."),
                ));
            }
            match option {
                "--member" => peer_sources.push(PathBuf::from(value)),
                "--filesystem" => {
                    let filesystem = value.to_str().and_then(CreateFilesystem::parse).ok_or_else(|| {
                        usage_failure(
                            &script_name,
                            color_mode,
                            "Choose ext4, btrfs, xfs, apfs, ufs1, ufs2, hammer2, ntfs, exfat, zfs or fat32 for --filesystem. Availability depends on the OS and installed tools.",
                        )
                    })?;
                    if create_filesystem.replace(filesystem).is_some() {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Specify --filesystem only once.",
                        ));
                    }
                }
                "--timeout" => {
                    let seconds = value
                        .to_str()
                        .and_then(|value| value.parse::<u64>().ok())
                        .filter(|seconds| (1..=300).contains(seconds))
                        .ok_or_else(|| {
                            usage_failure(
                                &script_name,
                                color_mode,
                                "Timeout must be 1–300 seconds.",
                            )
                        })?;
                    if close_timeout
                        .replace(Duration::from_secs(seconds))
                        .is_some()
                    {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Specify --timeout only once.",
                        ));
                    }
                }
                "--key-file" => {
                    if keyfile.replace(PathBuf::from(value)).is_some() {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Specify --key-file only once.",
                        ));
                    }
                }
                "--new-key-file" => {
                    if new_keyfile.replace(PathBuf::from(value)).is_some() {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Specify --new-key-file only once.",
                        ));
                    }
                }
                _ => {
                    let parsed = value
                        .to_str()
                        .and_then(|value| value.parse::<u8>().ok())
                        .filter(|slot| *slot < 32)
                        .ok_or_else(|| {
                            usage_failure(
                                &script_name,
                                color_mode,
                                "Keyslot must be between 0 and 31.",
                            )
                        })?;
                    if slot.replace(parsed).is_some() {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Specify --slot only once.",
                        ));
                    }
                }
            }
            index += 1;
            continue;
        }
        match current.as_ref() {
            "--unprivileged" | "--privileged" => {
                let chosen = if current == "--unprivileged" {
                    lurker_core::PrivilegeMode::Unprivileged
                } else {
                    lurker_core::PrivilegeMode::Privileged
                };
                if privilege_mode_explicit && chosen != privilege_mode {
                    return Err(usage_failure(
                        &script_name,
                        color_mode,
                        "Choose either --privileged or --unprivileged.",
                    ));
                }
                privilege_mode = chosen;
                privilege_mode_explicit = true;
                index += 1;
            }
            "--color" => {
                let value = raw_args.get(index + 1).ok_or_else(|| ParseFailure {
                    message: Some("Option --color requires an argument.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                color_mode = parse_color(value, color_mode, &script_name)?;
                index += 2;
            }
            "--type" => {
                let value = raw_args.get(index + 1).ok_or_else(|| ParseFailure {
                    message: Some("Option --type requires an argument.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                volume_type = parse_volume_type(value, color_mode, &script_name)?;
                volume_type_explicit = true;
                index += 2;
            }
            "--cipher" => {
                let value = raw_args.get(index + 1).ok_or_else(|| ParseFailure {
                    message: Some("Option --cipher requires an argument.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                create_cipher = parse_create_cipher(value, color_mode, &script_name)?;
                create_cipher_explicit = true;
                index += 2;
            }
            "--degraded" => {
                allow_degraded = true;
                index += 1;
            }
            "--help" => {
                parsed_args.push(OsString::from("-h"));
                index += 1;
            }
            "--force" => {
                parsed_args.push(OsString::from("-F"));
                index += 1;
            }
            "--" => {
                parsed_args.extend(raw_args[index + 1..].iter().cloned());
                break;
            }
            _ if current.starts_with("--color=") => {
                let value = &current["--color=".len()..];
                color_mode = ColorMode::parse(value).ok_or_else(|| ParseFailure {
                    message: Some(format!(
                        "Invalid --color value: {}. Use auto, always, or never.",
                        value
                    )),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                index += 1;
            }
            _ if current.starts_with("--type=") => {
                let value = &current["--type=".len()..];
                volume_type = VolumeType::parse(value).ok_or_else(|| ParseFailure {
                    message: Some(format!(
                        "Invalid --type value: {}. Use auto, luks, luks1, veracrypt, apple, apfs, geli, cgd, softraid, raid1c, pefs, gocryptfs, cryfs, or cryptomator.",
                        value
                    )),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                volume_type_explicit = true;
                index += 1;
            }
            _ if current.starts_with("--cipher=") => {
                let value = &current["--cipher=".len()..];
                create_cipher = CreateCipher::parse(value).ok_or_else(|| ParseFailure {
                    message: Some(format!(
                        "Invalid --cipher value: {}. Use aes, serpent, or twofish.",
                        value
                    )),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                create_cipher_explicit = true;
                index += 1;
            }
            _ if current.starts_with("--") => {
                return Err(ParseFailure {
                    message: Some(format!("Unsupported argument chosen: {}", current)),
                    exit_code: 1,
                    show_help: true,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }
            _ => {
                parsed_args.push(raw_args[index].clone());
                index += 1;
            }
        }
    }

    if parsed_args.is_empty() {
        return Err(ParseFailure {
            message: None,
            exit_code: 2,
            show_help: true,
            help_to_stdout: false,
            script_name,
            color_mode,
        });
    }

    let first = parsed_args[0].to_string_lossy().into_owned();
    if first == "-h" || first == "help" {
        return Ok(ParseOutcome::Help(HelpRequest {
            script_name,
            color_mode,
            to_stdout: true,
        }));
    }

    if first == "list" && matches!(os, "freebsd" | "dragonfly" | "netbsd" | "openbsd") {
        if parsed_args.len() != 1
            || volume_type_explicit
            || create_cipher_explicit
            || create_filesystem.is_some()
            || privilege_mode_explicit
            || keyfile.is_some()
            || new_keyfile.is_some()
            || slot.is_some()
            || close_timeout.is_some()
            || !peer_sources.is_empty()
            || allow_degraded
        {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "Usage: lurker list [--color=WHEN]",
            ));
        }
        return Ok(ParseOutcome::ListNative(color_mode));
    }

    if first == "-a" {
        return Err(ParseFailure {
            message: Some("The -a flag has been removed. Use: lurker <action> ...".into()),
            exit_code: 1,
            show_help: false,
            help_to_stdout: false,
            script_name,
            color_mode,
        });
    }

    if first.starts_with('-') {
        return Err(ParseFailure {
            message: Some("Action is required. Use: create, createvc, mount, umount, keyslots, add-key or remove-key.".into()),
            exit_code: 1,
            show_help: true,
            help_to_stdout: false,
            script_name,
            color_mode,
        });
    }

    let mut action = first;
    let close_worker = action == "__session-close-worker";
    let session_close = action == "session-close" || close_worker;
    if session_close {
        if os == "macos" {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "Session hooks are available on Linux and BSD.",
            ));
        }
        if close_worker && close_timeout.is_some() {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "--timeout requires session-close.",
            ));
        }
        action = "umount".into();
    } else if close_timeout.is_some() {
        return Err(usage_failure(
            &script_name,
            color_mode,
            "--timeout requires session-close.",
        ));
    } else if action == "unmount" {
        action = "umount".into();
    }

    if action == "createvc" {
        if volume_type_explicit && volume_type != VolumeType::Veracrypt {
            return Err(ParseFailure {
                message: Some("Action createvc only supports --type veracrypt.".into()),
                exit_code: 1,
                show_help: false,
                help_to_stdout: false,
                script_name,
                color_mode,
            });
        }
        volume_type = VolumeType::Veracrypt;
        action = "create".into();
    }

    if create_filesystem.is_some() && action != "create" {
        return Err(usage_failure(
            &script_name,
            color_mode,
            "--filesystem requires create.",
        ));
    }

    let mut force = false;
    let mut tag: Option<String> = None;
    let mut action_args = Vec::new();
    let mut remainder = 1usize;
    while remainder < parsed_args.len() {
        let current = parsed_args[remainder].to_string_lossy();
        match current.as_ref() {
            "-F" => {
                force = true;
                remainder += 1;
            }
            "-t" => {
                let value = parsed_args.get(remainder + 1).ok_or_else(|| ParseFailure {
                    message: Some("Option -t requires an argument.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.clone(),
                    color_mode,
                })?;
                let value = value.to_string_lossy().into_owned();
                if value.is_empty() {
                    return Err(ParseFailure {
                        message: Some("Option -t requires a non-empty argument.".into()),
                        exit_code: 1,
                        show_help: false,
                        help_to_stdout: false,
                        script_name,
                        color_mode,
                    });
                }
                tag = Some(value);
                remainder += 2;
            }
            "-h" => {
                return Ok(ParseOutcome::Help(HelpRequest {
                    script_name,
                    color_mode,
                    to_stdout: true,
                }));
            }
            "-f" | "-m" | "-s" => {
                return Err(ParseFailure {
                    message: Some(reject_removed_operand_flag(&action, &script_name)),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }
            "--" => {
                action_args.extend(parsed_args[remainder + 1..].iter().cloned());
                break;
            }
            _ if current.starts_with('-') => {
                return Err(ParseFailure {
                    message: Some(format!("Unsupported argument chosen: {}", current)),
                    exit_code: 1,
                    show_help: true,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }
            _ => {
                action_args.push(parsed_args[remainder].clone());
                remainder += 1;
            }
        }
    }

    if !peer_sources.is_empty() || allow_degraded || volume_type == VolumeType::Raid1c {
        if os != "openbsd" {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "RAID1C, --member and --degraded require OpenBSD.",
            ));
        }
        if !matches!(action.as_str(), "create" | "mount" | "umount") {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "RAID1C options are valid only with create, mount or unmount.",
            ));
        }
        if allow_degraded && action != "mount" {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "Option --degraded is only valid with mount for existing-array recovery.",
            ));
        }
        if !peer_sources.is_empty() {
            if action == "mount" && volume_type == VolumeType::Auto {
                // Preserve the explicit type across helper JSON: older helpers
                // must reject RAID1C instead of ignoring unfamiliar peer fields.
                volume_type = VolumeType::Raid1c;
            }
            if !matches!(action.as_str(), "create" | "mount") || volume_type != VolumeType::Raid1c {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "--member requires create --type raid1c or a RAID1C mount.",
                ));
            }
        }
        if allow_degraded && volume_type != VolumeType::Raid1c {
            return Err(usage_failure(
                &script_name,
                color_mode,
                "Degraded recovery requires mount --type raid1c --degraded (or --member).",
            ));
        }
    }

    if new_keyfile.is_some() && action != "add-key" {
        return Err(usage_failure(
            &script_name,
            color_mode,
            "Option --new-key-file is only valid with add-key.",
        ));
    }
    if slot.is_some() && !matches!(action.as_str(), "add-key" | "remove-key") {
        return Err(usage_failure(
            &script_name,
            color_mode,
            "Option --slot is only valid with add-key or remove-key.",
        ));
    }
    if keyfile.is_some()
        && !matches!(
            action.as_str(),
            "create" | "mount" | "add-key" | "remove-key"
        )
    {
        return Err(usage_failure(
            &script_name,
            color_mode,
            "Option --key-file is only valid with create, mount, add-key or remove-key.",
        ));
    }
    if keyfile.is_some()
        && !matches!(
            volume_type,
            VolumeType::Auto
                | VolumeType::Luks
                | VolumeType::Luks1
                | VolumeType::Veracrypt
                | VolumeType::Geli
                | VolumeType::Pefs
        )
    {
        return Err(usage_failure(
            &script_name,
            color_mode,
            "Keyfile authentication is supported for LUKS, VeraCrypt, GELI and PEFS only.",
        ));
    }
    match action.as_str() {
        "keyslots" | "add-key" | "remove-key" => {
            if !cfg!(target_os = "linux")
                || privilege_mode == lurker_core::PrivilegeMode::Unprivileged
            {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "LUKS keyslot management requires Linux Privileged mode.",
                ));
            }
            if !matches!(
                volume_type,
                VolumeType::Auto | VolumeType::Luks | VolumeType::Luks1
            ) || create_cipher_explicit
                || force
                || tag.is_some()
            {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "Keyslot operations support LUKS only and do not take cipher, force or tag options.",
                ));
            }
            if action_args.len() != 1 {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "Usage: lurker keyslots|add-key|remove-key <source> [--slot N] [--key-file FILE] [--new-key-file FILE]",
                ));
            }
            let operation = match action.as_str() {
                "keyslots" => KeyslotOperation::List,
                "add-key" => KeyslotOperation::Add,
                _ => KeyslotOperation::Remove,
            };
            if operation == KeyslotOperation::Remove && slot.is_none() {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "remove-key requires --slot N.",
                ));
            }
            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command: CommandAction::Keyslots(KeyslotsCommand {
                    privilege_mode,
                    source: PathBuf::from(&action_args[0]),
                    operation,
                    slot,
                    passphrase: None,
                    keyfile,
                    new_passphrase: None,
                    new_keyfile,
                }),
            }))
        }
        "create" => {
            if tag.is_some() {
                return Err(ParseFailure {
                    message: Some("Option -t is only valid with mount or umount.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }

            let volume_type = match (volume_type, volume_type_explicit) {
                (VolumeType::Auto, true) => {
                    return Err(ParseFailure {
                        message: Some(
                            "Option --type auto is not valid with create. Use luks, veracrypt, or apple."
                                .into(),
                        ),
                        exit_code: 1,
                        show_help: false,
                        help_to_stdout: false,
                        script_name,
                        color_mode,
                    });
                }
                (VolumeType::Auto, false)
                    if privilege_mode == lurker_core::PrivilegeMode::Unprivileged =>
                {
                    VolumeType::Veracrypt
                }
                (VolumeType::Auto, false) => VolumeType::default_create(),
                (value, _) => value,
            };

            if action_args.is_empty() || action_args.len() > 2 {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "Usage: lurker create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force] or lurker create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force]",
                ));
            }

            let target = PathBuf::from(&action_args[0]);
            let source_kind = if volume_type.is_directory() {
                SourceKind::Directory
            } else {
                classify_create_target(&target, &script_name, color_mode)?
            };
            if volume_type.is_directory() && (create_cipher_explicit || force) {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "Directory backends use their fixed native profile and do not accept --cipher or --force.",
                ));
            }
            let size_gb = match source_kind {
                SourceKind::Directory => {
                    if action_args.len() != 1 {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Usage: lurker create --type BACKEND <directory>",
                        ));
                    }
                    None
                }
                SourceKind::Block => {
                    if action_args.len() != 1 {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Usage: lurker create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force]",
                        ));
                    }
                    None
                }
                SourceKind::File => {
                    if action_args.len() != 2 {
                        return Err(usage_failure(
                            &script_name,
                            color_mode,
                            "Usage: lurker create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force]",
                        ));
                    }
                    Some(action_args[1].to_string_lossy().into_owned())
                }
            };

            let create = CreateCommand {
                privilege_mode,
                target,
                peer_sources,
                size_gb,
                source_kind,
                force,
                volume_type,
                cipher: create_cipher,
                filesystem: create_filesystem,
                passphrase: None,
                keyfile,
            };
            create
                .validate_filesystem(os)
                .map_err(|err| usage_failure(&script_name, color_mode, &err.message))?;
            let command = CommandAction::Create(create);
            command
                .validate_raid1c_options(os)
                .map_err(|err| usage_failure(&script_name, color_mode, &err.message))?;
            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command,
            }))
        }
        "mount" => {
            if create_cipher_explicit {
                return Err(ParseFailure {
                    message: Some("Option --cipher is only valid with create.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }

            if force {
                return Err(ParseFailure {
                    message: Some("Option -F is only valid with create.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }

            if action_args.len() != 2 {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "Usage: lurker mount [--type TYPE] [-t tag] <source> <mountpoint>",
                ));
            }

            let source = PathBuf::from(&action_args[0]);
            let source_kind = if volume_type == VolumeType::Raid1c {
                // A missing logical primary can identify an existing mirrored
                // array through its descriptor and explicit surviving peer.
                // lstat distinguishes an absent entry from a dangling symlink.
                match fs::symlink_metadata(&source) {
                    Ok(metadata) if metadata.file_type().is_file() => Some(SourceKind::File),
                    Ok(metadata)
                        if os == "openbsd"
                            && cfg!(target_os = "openbsd")
                            && native_device_type(metadata.file_type()) =>
                    {
                        Some(SourceKind::Block)
                    }
                    Err(err)
                        if err.kind() == std::io::ErrorKind::NotFound
                            && allow_degraded
                            && peer_sources.len() == 1 =>
                    {
                        Some(SourceKind::File)
                    }
                    _ => None,
                }
            } else {
                path_source_kind(&source)
            };
            let source_kind = source_kind.ok_or_else(|| ParseFailure {
                message: Some(format!(
                    "Source does not exist or is not a regular file, directory or supported device: {}",
                    source.display()
                )),
                exit_code: 1,
                show_help: false,
                help_to_stdout: false,
                script_name: script_name.clone(),
                color_mode,
            })?;
            let _ = source_kind;

            let mountpoint = PathBuf::from(&action_args[1]);
            if !mountpoint.is_dir()
                && !((cfg!(target_os = "macos")
                    || volume_type.is_directory()
                    || source_kind == SourceKind::Directory
                    || privilege_mode == lurker_core::PrivilegeMode::Unprivileged)
                    && !mountpoint.exists())
            {
                return Err(ParseFailure {
                    message: Some(format!(
                        "Directory does not exist: {}",
                        mountpoint.display()
                    )),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }

            let command = CommandAction::Mount(MountCommand {
                privilege_mode,
                source,
                peer_sources,
                allow_degraded,
                mountpoint,
                tag,
                volume_type,
                passphrase: None,
                keyfile,
                readonly: false,
            });
            command
                .validate_raid1c_options(os)
                .map_err(|err| usage_failure(&script_name, color_mode, &err.message))?;
            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command,
            }))
        }
        "umount" => {
            if create_cipher_explicit {
                return Err(ParseFailure {
                    message: Some("Option --cipher is only valid with create.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }

            if force {
                return Err(ParseFailure {
                    message: Some("Option -F is only valid with create.".into()),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name,
                    color_mode,
                });
            }

            if action_args.len() != 1 {
                return Err(usage_failure(
                    &script_name,
                    color_mode,
                    "Usage: lurker umount [--type TYPE] [-t tag] <target>",
                ));
            }

            let command = UnmountCommand {
                privilege_mode,
                target: PathBuf::from(&action_args[0]),
                tag,
                volume_type,
            };
            if session_close {
                if !command.target.is_absolute()
                    || volume_type == VolumeType::Auto
                    || !volume_type.supports_platform(os)
                    || command.tag.is_some()
                {
                    return Err(usage_failure(&script_name, color_mode,
                        "Session close requires an absolute vault source and an explicit supported backend without a mapper tag."));
                }
                return Ok(ParseOutcome::SessionClose {
                    command,
                    timeout: (!close_worker)
                        .then(|| close_timeout.unwrap_or(Duration::from_secs(30))),
                    color_mode,
                });
            }
            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command: CommandAction::Unmount(command),
            }))
        }
        _ => Err(ParseFailure {
            message: Some("Action create, mount, or umount expected.".into()),
            exit_code: 1,
            show_help: true,
            help_to_stdout: false,
            script_name,
            color_mode,
        }),
    }
}

pub fn render_help(script_name: &str, color_mode: ColorMode, stdout_destination: bool) -> String {
    let use_color = match color_mode {
        ColorMode::Always => true,
        ColorMode::Auto => {
            if stdout_destination {
                std::io::stdout().is_terminal()
            } else {
                std::io::stderr().is_terminal()
            }
        }
        ColorMode::Never => false,
    };

    let (header, accent, reset) = if use_color {
        ("\u{1b}[1;34m", "\u{1b}[1;32m", "\u{1b}[0m")
    } else {
        ("", "", "")
    };

    let mount_help = "\nMount access follows filesystem and driver capabilities.\n";
    let filesystem_help = "\nFilesystems (file-container create, Linux privileged devices, macOS APFS partitions, NetBSD CGD GPT partitions):\n  --filesystem ext4|btrfs|xfs|apfs|ufs1|ufs2|hammer2|ntfs|exfat|zfs\n  Native default when omitted. Available choices depend on the OS, encryption route and installed tools.\n";
    let directory_help = "\nDirectory backends (where supported):\n  create --type gocryptfs|cryfs|cryptomator|pefs <directory>\n  mount --type BACKEND <encrypted-directory> <mountpoint>\n  unmount --type BACKEND <mountpoint>\n  No size or cipher option; fixed native encryption profile.\n";
    let session_help = "\nSession shutdown hook:\n  session-close --type BACKEND [--timeout SECONDS] <absolute-vault-source>\n";
    let raid_help = if cfg!(target_os = "openbsd") {
        "\nRAID1C (two mirrored files or physical disks, native AES/password only):\n  create --type raid1c --member <peer-file> <file> <size-gb>\n  create --type raid1c --member <peer-disk> <disk> --force\n  mount [--type raid1c] [--member <peer>] <source> <mountpoint>\n  mount --type raid1c --degraded <surviving-member> <mountpoint>\n  --degraded explicitly recovers an existing array; never used during create.\n  File members require a size and no --force; physical disks require --force and no size.\n  Keep file descriptors with their members; back up physical descriptors in /var/db/lurker-raid1c.\n  Physical RAID1C uses two idle whole /dev/sdNc disks. Keyfiles are unavailable.\n"
    } else {
        ""
    };
    let dragonfly_help = if cfg!(target_os = "dragonfly") {
        "\nDragonFly VeraCrypt candidate:\n  create --type veracrypt [--filesystem hammer2|fat32] <file> <size-gb>\n  HAMMER2 is the default. Select FAT32 for cross-platform exchange.\n"
    } else {
        ""
    };
    let native_targets = if cfg!(target_os = "netbsd") {
        "File containers and CGD GPT data partitions"
    } else if cfg!(target_os = "freebsd") {
        "File containers and GELI data providers"
    } else if cfg!(target_os = "openbsd") {
        "File containers, softraid disks and VeraCrypt data partitions"
    } else {
        "File containers only"
    };
    let native_device_help = if cfg!(target_os = "netbsd") {
        "\nPhysical CGD data partitions:\n  create --type cgd [--filesystem NAME] <GPT-wedge> --force\n  mount --type cgd <GPT-wedge> <mountpoint>\n  Back up /var/db/lurker-cgd/<partition-UUID>.cgdparams with the encrypted partition.\n"
    } else if cfg!(target_os = "freebsd") {
        "\nPhysical GELI data providers:\n  create --type geli [--filesystem NAME] <data-provider> --force\n  mount --type geli <data-provider> <mountpoint>\n  System disks and providers in use are refused.\n"
    } else if cfg!(target_os = "openbsd") {
        "\nPhysical OpenBSD encryption:\n  create --type softraid [--filesystem NAME] <whole-/dev/sdNc-disk> --force\n  create --type veracrypt [--filesystem NAME] <data-partition> --force\n  VeraCrypt partitions must exclude the outer disklabel sector.\n  System disks and disks in use are refused.\n"
    } else {
        ""
    };
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return format!(
            "Usage: {script_name} create [--type TYPE] <file> <size-gb>\n  {script_name} mount [--type TYPE] <file> <mountpoint>\n  {script_name} unmount [--type TYPE] <file|mountpoint>\n  {script_name} list\nNative default: {}. {native_targets}; native device operations require administrator privileges.\n{native_device_help}{raid_help}{dragonfly_help}{mount_help}{filesystem_help}{directory_help}{session_help}",
            VolumeType::default_create()
        );
    }
    if cfg!(target_os = "macos") {
        return format!(
            "\n{header}Usage:{reset}\n  {accent}{script_name} create [--type apple|apfs|veracrypt] [--cipher CIPHER] <file> <size-gb> [-F|--force]{reset}\n  {accent}{script_name} createvc [--cipher CIPHER] <file> <size-gb> [-F|--force]{reset}\n  {accent}{script_name} mount [--type auto|apple|apfs|veracrypt] <file> <mountpoint>{reset}\n  {accent}{script_name} unmount [--type auto|apple|apfs|veracrypt] <file|mountpoint|virtual-device>{reset}\n\n{header}Examples:{reset}\n  {accent}{script_name} create ./vault.dmg 4{reset}\n  {accent}{script_name} createvc --cipher serpent ./vault.hc 4{reset}\n  {accent}{script_name} mount ./vault.dmg ~/vault-mount{reset}\n  {accent}{script_name} unmount ~/vault-mount{reset}\n\n{header}Options:{reset}\n  {accent}--type TYPE{reset}       Create defaults to apple; mount/unmount default to auto\n  {accent}--key-file FILE{reset}   Use a keyfile instead of a password (VeraCrypt create or mount)\n  {accent}--cipher CIPHER{reset}   aes (default), serpent, twofish; Apple uses AES-256 only\n  {accent}-F, --force{reset}       Replace an existing unmounted regular file after successful create\n  {accent}--color=WHEN{reset}      auto, always, never\n  {accent}-h, --help{reset}        Show this help text\n\n  macOS supports file containers (minimum 32 MiB). APFS also supports unmounted external data partitions with --force. LUKS and tags require Linux.\n{mount_help}{filesystem_help}{directory_help}"
        );
    }

    format!(
        "\n{header}Usage:{reset}\n  {accent}{script_name} create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force]{reset}\n  {accent}{script_name} create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force]{reset}\n  {accent}{script_name} createvc [--cipher CIPHER] <file> <size-gb> [-F|--force]{reset}\n  {accent}{script_name} createvc [--cipher CIPHER] <block-device> [-F|--force]{reset}\n  {accent}{script_name} mount [--type TYPE] [-t TAG] <source> <mountpoint>{reset}\n  {accent}{script_name} unmount [--type TYPE] [-t TAG] <target>{reset}\n  {accent}{script_name} keyslots <source>{reset}\n  {accent}{script_name} add-key <source> [--slot N] [--key-file FILE] [--new-key-file FILE]{reset}\n  {accent}{script_name} remove-key <source> --slot N [--key-file FILE]{reset}\n  {accent}{script_name} help{reset}\n\n{header}Examples:{reset}\n  {accent}{script_name} create ./vault.img 4{reset}\n  {accent}{script_name} create --type veracrypt --cipher serpent ./vault-serpent.hc 4{reset}\n  {accent}{script_name} create --cipher twofish /dev/sda2 --force{reset}\n  {accent}{script_name} create --type veracrypt --cipher serpent ./vault.hc 4{reset}\n  {accent}{script_name} createvc --cipher twofish /dev/sdb2 --force{reset}\n  {accent}{script_name} mount ./vault.img /mnt/vault{reset}\n  {accent}{script_name} mount -t work /dev/sda2 /mnt/work{reset}\n  {accent}{script_name} unmount /mnt/work{reset}\n  {accent}{script_name} unmount -t work /dev/sda2{reset}\n\n{header}Options:{reset}\n  {accent}-F, --force{reset}       Overwrite an existing file or allow destructive block-device create\n  {accent}-t TAG{reset}            Linux LUKS only: use /dev/mapper/lurker_TAG\n  {accent}--slot N{reset}          LUKS slot to add (optional) or remove (required)\n  {accent}--new-key-file FILE{reset}   New LUKS credential for add-key; otherwise prompt for a new password\n  {accent}--unprivileged{reset}    Linux VeraCrypt files without elevation (userspace helpers required)\n  {accent}--privileged{reset}      Use the existing kernel-backed route (default)\n  {accent}--type TYPE{reset}       Container type: luks (Linux default), luks1, apple (macOS default), apfs, veracrypt, geli, cgd, softraid, raid1c, pefs, gocryptfs, cryfs, cryptomator; auto for mount/unmount\n  {accent}--key-file FILE{reset}   Use a keyfile instead of a password (LUKS/VeraCrypt create or mount)\n  {accent}--cipher CIPHER{reset}   Create cipher: aes (default), serpent, twofish; Apple uses AES-256 only\n  {accent}--color=WHEN{reset}      Color output: auto, always, never\n  {accent}-h, --help{reset}        Show this help text\n{mount_help}{filesystem_help}{directory_help}{session_help}"
    )
}

fn parse_color(
    value: &OsStr,
    color_mode: ColorMode,
    script_name: &str,
) -> Result<ColorMode, ParseFailure> {
    ColorMode::parse(&value.to_string_lossy()).ok_or_else(|| ParseFailure {
        message: Some(format!(
            "Invalid --color value: {}. Use auto, always, or never.",
            value.to_string_lossy()
        )),
        exit_code: 1,
        show_help: false,
        help_to_stdout: false,
        script_name: script_name.into(),
        color_mode,
    })
}

fn parse_volume_type(
    value: &OsStr,
    color_mode: ColorMode,
    script_name: &str,
) -> Result<VolumeType, ParseFailure> {
    VolumeType::parse(&value.to_string_lossy()).ok_or_else(|| ParseFailure {
        message: Some(format!(
            "Invalid --type value: {}. Use auto, luks, luks1, veracrypt, apple, apfs, geli, cgd, softraid, raid1c, pefs, gocryptfs, cryfs, or cryptomator.",
            value.to_string_lossy()
        )),
        exit_code: 1,
        show_help: false,
        help_to_stdout: false,
        script_name: script_name.into(),
        color_mode,
    })
}

fn parse_create_cipher(
    value: &OsStr,
    color_mode: ColorMode,
    script_name: &str,
) -> Result<CreateCipher, ParseFailure> {
    CreateCipher::parse(&value.to_string_lossy()).ok_or_else(|| ParseFailure {
        message: Some(format!(
            "Invalid --cipher value: {}. Use aes, serpent, or twofish.",
            value.to_string_lossy()
        )),
        exit_code: 1,
        show_help: false,
        help_to_stdout: false,
        script_name: script_name.into(),
        color_mode,
    })
}

fn path_source_kind(path: &Path) -> Option<SourceKind> {
    let metadata = fs::metadata(path).ok()?;
    let file_type = metadata.file_type();
    if native_device_type(file_type) {
        Some(SourceKind::Block)
    } else if file_type.is_dir() {
        Some(SourceKind::Directory)
    } else if file_type.is_file() {
        Some(SourceKind::File)
    } else {
        None
    }
}

fn native_device_type(file_type: fs::FileType) -> bool {
    file_type.is_block_device()
        || (cfg!(any(
            target_os = "freebsd", target_os = "openbsd",
            target_os = "netbsd", target_os = "dragonfly"
        )) && file_type.is_char_device())
}

fn classify_create_target(
    path: &Path,
    script_name: &str,
    color_mode: ColorMode,
) -> Result<SourceKind, ParseFailure> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if native_device_type(file_type) {
                Ok(SourceKind::Block)
            } else if file_type.is_file() {
                Ok(SourceKind::File)
            } else {
                Err(ParseFailure {
                    message: Some(format!(
                        "Target exists and is neither a regular file nor block device: {}",
                        path.display()
                    )),
                    exit_code: 1,
                    show_help: false,
                    help_to_stdout: false,
                    script_name: script_name.into(),
                    color_mode,
                })
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(SourceKind::File),
        Err(err) => Err(ParseFailure {
            message: Some(
                AppError::io(format!("Failed to inspect target {}", path.display()), err).message,
            ),
            exit_code: 1,
            show_help: false,
            help_to_stdout: false,
            script_name: script_name.into(),
            color_mode,
        }),
    }
}

fn reject_removed_operand_flag(action: &str, script_name: &str) -> String {
    match action {
        "create" => format!(
            "Operand flags are no longer supported. Use: {script_name} create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force], {script_name} create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force], or {script_name} createvc ..."
        ),
        "mount" => format!(
            "Operand flags are no longer supported. Use: {script_name} mount <source> <mountpoint>"
        ),
        "umount" => {
            format!("Operand flags are no longer supported. Use: {script_name} umount <target>")
        }
        _ => "Unsupported argument chosen.".into(),
    }
}

fn usage_failure(script_name: &str, color_mode: ColorMode, message: &str) -> ParseFailure {
    ParseFailure {
        message: Some(message.replace("lurker", script_name)),
        exit_code: 1,
        show_help: false,
        help_to_stdout: false,
        script_name: script_name.into(),
        color_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_env_args, CommandAction, ParseOutcome};
    use lurker_core::{CreateCipher, SourceKind, VolumeType};
    use std::ffi::OsString;

    #[test]
    fn mount_has_no_user_selectable_access_mode_on_any_platform() {
        let executable = std::env::current_exe().unwrap();
        let source = executable.to_str().unwrap();
        let target = std::env::temp_dir();
        let mountpoint = target.to_str().unwrap();
        for os in [
            "linux",
            "macos",
            "freebsd",
            "dragonfly",
            "netbsd",
            "openbsd",
        ] {
            let parse = |arguments: &[&str]| {
                super::parse_args_for_platform(arguments.iter().copied().map(OsString::from), os)
            };
            let ParseOutcome::Run(cli) = parse(&["lurker", "mount", source, mountpoint]).unwrap()
            else {
                panic!("expected a mount command on {os}");
            };
            let CommandAction::Mount(command) = cli.command else {
                panic!("expected a mount command on {os}");
            };
            assert!(!command.readonly);
            for option in ["--readonly", "--readonly=true", "--readonly=false"] {
                let failure = parse(&["lurker", "mount", option, source, mountpoint]).unwrap_err();
                assert!(failure
                    .message
                    .unwrap()
                    .contains("Option --readonly was removed."));
            }
        }
    }

    #[test]
    fn filesystem_selection_requires_container_create() {
        use lurker_core::CreateFilesystem;
        let parse = |args: &[&str], os| {
            super::parse_args_for_platform(args.iter().copied().map(OsString::from), os)
        };
        let args = ["lurker", "createvc", "--filesystem=fat32", "vault.hc", "1"];
        let Ok(ParseOutcome::Run(cli)) = parse(&args, "dragonfly") else {
            panic!("DragonFly createvc must accept the FAT32 profile");
        };
        assert!(
            matches!(cli.command, CommandAction::Create(c) if c.filesystem == Some(CreateFilesystem::Fat32))
        );
        assert!(parse(&args, "linux").is_ok());
        for (os, backend) in [
            ("linux", "luks"),
            ("macos", "apple"),
            ("freebsd", "geli"),
            ("openbsd", "softraid"),
            ("netbsd", "cgd"),
            ("dragonfly", "luks1"),
        ] {
            assert!(
                parse(
                    &[
                        "lurker",
                        "create",
                        "--type",
                        backend,
                        "--filesystem=exfat",
                        "vault.img",
                        "1"
                    ],
                    os
                )
                .is_ok(),
                "{os}"
            );
        }
        for args in [
            &["lurker", "createvc", "--filesystem=ufs", "vault.hc", "1"][..],
            &[
                "lurker",
                "createvc",
                "--filesystem=hammer2",
                "--filesystem=hammer2",
                "vault.hc",
                "1",
            ],
            &[
                "lurker",
                "mount",
                "--filesystem=hammer2",
                "vault.hc",
                "mountpoint",
            ],
            &["lurker", "list", "--filesystem=hammer2"],
        ] {
            assert!(parse(args, "dragonfly").is_err(), "{args:?}");
        }
        assert!(parse(
            &[
                "lurker",
                "create",
                "--type=gocryptfs",
                "--filesystem=ext4",
                "vault"
            ],
            "linux"
        )
        .is_err());
        assert!(parse(
            &[
                "lurker",
                "create",
                "--type=apfs",
                "--filesystem=exfat",
                "vault.img",
                "1"
            ],
            "macos"
        )
        .is_err());
    }

    #[test]
    fn native_list_accepts_color_without_operation_options() {
        let parse = |args| super::parse_args_for_platform(args, "dragonfly");
        assert!(matches!(
            parse(["lurker", "list", "--color=never"].map(OsString::from)),
            Ok(ParseOutcome::ListNative(lurker_core::ColorMode::Never))
        ));
        for argument in ["--type=luks1", "--readonly", "extra"] {
            assert!(parse(["lurker", "list", argument].map(OsString::from)).is_err());
        }
    }

    #[test]
    fn session_close_requires_an_explicit_backend_and_absolute_source() {
        let parse = |args: &[&str], os: &str| {
            super::parse_args_for_platform(args.iter().copied().map(OsString::from), os)
        };
        let ParseOutcome::SessionClose {
            command, timeout, ..
        } = parse(
            &[
                "lurker",
                "session-close",
                "--type=raid1c",
                "--timeout",
                "7",
                "/vault/a.img",
            ],
            "openbsd",
        )
        .unwrap()
        else {
            panic!("expected the session close hook");
        };
        assert_eq!(command.target, std::path::Path::new("/vault/a.img"));
        assert_eq!(command.volume_type, VolumeType::Raid1c);
        assert_eq!(timeout.unwrap().as_secs(), 7);
        for args in [
            &["lurker", "session-close", "/vault/a.img"][..],
            &["lurker", "session-close", "--type=luks", "a.img"],
            &[
                "lurker",
                "session-close",
                "--type=luks",
                "--timeout=0",
                "/vault/a.img",
            ],
            &[
                "lurker",
                "unmount",
                "--type=luks",
                "--timeout=1",
                "/vault/a.img",
            ],
        ] {
            assert!(parse(args, "linux").is_err());
        }
        assert!(parse(
            &[
                "lurker",
                "session-close",
                "--type=veracrypt",
                "/vault/a.img"
            ],
            "macos",
        )
        .is_err());
    }

    #[test]
    fn raid1c_create_parses_one_peer_without_changing_native_default() {
        let args = [
            "lurker",
            "create",
            "--type",
            "raid1c",
            "--member=second.img",
            "first.img",
            "1",
        ];
        let ParseOutcome::Run(cli) =
            super::parse_args_for_platform(args.map(OsString::from), "openbsd").unwrap()
        else {
            panic!()
        };
        let CommandAction::Create(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.volume_type, VolumeType::Raid1c);
        assert_eq!(c.peer_sources, vec![std::path::PathBuf::from("second.img")]);
        assert!(!c.force);
        for args in [
            vec![
                "lurker",
                "create",
                "--member",
                "second.img",
                "first.img",
                "1",
            ],
            vec!["lurker", "create", "--type=raid1c", "first.img", "1"],
            vec![
                "lurker",
                "create",
                "--type=raid1c",
                "--member=second.img",
                "--member=third.img",
                "first.img",
                "1",
            ],
            vec![
                "lurker",
                "create",
                "--type=raid1c",
                "--member=second.img",
                "--force",
                "first.img",
                "1",
            ],
            vec![
                "lurker",
                "create",
                "--type=raid1c",
                "--member=second.img",
                "--cipher=twofish",
                "first.img",
                "1",
            ],
            vec![
                "lurker",
                "create",
                "--type=raid1c",
                "--member=second.img",
                "--degraded",
                "first.img",
                "1",
            ],
        ] {
            assert!(
                super::parse_args_for_platform(args.iter().map(OsString::from), "openbsd").is_err(),
                "{args:?}"
            );
        }
    }

    #[cfg(target_os = "openbsd")]
    #[test]
    #[ignore = "stat-only parser checks; requires LURKER_TEST_OPENBSD_PHYSICAL_SOURCE naming the verified scratch disk"]
    fn raid1c_native_members_are_admitted_as_devices_before_core_validation() {
        use std::os::unix::fs::FileTypeExt;
        let source = std::env::var("LURKER_TEST_OPENBSD_PHYSICAL_SOURCE").unwrap();
        assert!(std::fs::symlink_metadata(&source)
            .unwrap()
            .file_type()
            .is_block_device());
        let raw = source.replacen("/dev/", "/dev/r", 1);
        assert!(std::fs::symlink_metadata(&raw)
            .unwrap()
            .file_type()
            .is_char_device());
        for source in [&source, &raw] {
            let args = [
                "lurker",
                "mount",
                "--type=raid1c",
                source,
                "/tmp",
            ];
            let ParseOutcome::Run(cli) =
                super::parse_args_for_platform(args.map(OsString::from), "openbsd").unwrap()
            else {
                panic!()
            };
            let CommandAction::Mount(command) = cli.command else {
                panic!()
            };
            assert_eq!(command.source, std::path::Path::new(source));
            assert_eq!(command.volume_type, VolumeType::Raid1c);
        }
    }

    #[test]
    fn raid1c_mount_normalizes_auto_members_before_helper_serialization() {
        for flag in [
            vec!["--member", "peer.img"],
            vec!["--type=auto", "--member=peer.img", "--degraded"],
        ] {
            let mut args = vec!["lurker", "mount", "Cargo.toml", "/tmp"];
            args.extend(flag);
            let ParseOutcome::Run(cli) =
                super::parse_args_for_platform(args.iter().map(OsString::from), "openbsd").unwrap()
            else {
                panic!()
            };
            let CommandAction::Mount(c) = cli.command else {
                panic!()
            };
            assert_eq!(c.volume_type, VolumeType::Raid1c);
            assert_eq!(c.peer_sources, vec![std::path::PathBuf::from("peer.img")]);
            assert_eq!(c.allow_degraded, args.contains(&"--degraded"));
        }
        let args = [
            "lurker",
            "mount",
            "--type=raid1c",
            "--degraded",
            "Cargo.toml",
            "/tmp",
        ];
        assert!(super::parse_args_for_platform(args.map(OsString::from), "openbsd").is_ok());
        for args in [
            vec!["lurker", "mount", "--degraded", "Cargo.toml", "/tmp"],
            vec![
                "lurker",
                "mount",
                "--type=softraid",
                "--member=peer",
                "Cargo.toml",
                "/tmp",
            ],
            vec![
                "lurker",
                "mount",
                "--member=peer",
                "--member=third",
                "Cargo.toml",
                "/tmp",
            ],
            vec![
                "lurker",
                "mount",
                "--member=peer",
                "-t",
                "work",
                "Cargo.toml",
                "/tmp",
            ],
            vec!["lurker", "unmount", "--member=peer", "Cargo.toml"],
            vec!["lurker", "unmount", "--degraded", "Cargo.toml"],
        ] {
            assert!(
                super::parse_args_for_platform(args.iter().map(OsString::from), "openbsd").is_err(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn raid1c_degraded_mount_accepts_absent_primary_but_rejects_invalid_entries() {
        use std::fs;
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(
            std::env::temp_dir().join(format!("lurker-cli-raid1c-{}-{nonce}", std::process::id())),
        );
        fs::create_dir(&fixture.0).unwrap();
        let missing = fixture.0.join("missing-primary.img");
        let peer = fixture.0.join("surviving-peer.img");
        let mountpoint = fixture.0.join("mount");
        let link = fixture.0.join("dangling-link.img");
        fs::write(&peer, b"fixture only; no native storage operations").unwrap();
        fs::create_dir(&mountpoint).unwrap();
        symlink(&missing, &link).unwrap();
        let parse = |source: &std::path::Path, flags: &[&str], with_peer: bool| {
            let mut args = vec![OsString::from("lurker"), "mount".into()];
            args.extend(flags.iter().map(OsString::from));
            if with_peer {
                args.extend([OsString::from("--member"), peer.clone().into_os_string()]);
            }
            args.extend([
                source.as_os_str().to_owned(),
                mountpoint.as_os_str().to_owned(),
            ]);
            super::parse_args_for_platform(args, "openbsd")
        };
        let ParseOutcome::Run(cli) =
            parse(&missing, &["--type=raid1c", "--degraded"], true).unwrap()
        else {
            panic!()
        };
        let CommandAction::Mount(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.source, missing);
        assert_eq!(c.peer_sources, vec![peer.clone()]);
        assert!(c.allow_degraded);
        // Auto + member is normalized before the missing-source exception.
        assert!(parse(&missing, &["--degraded"], true).is_ok());
        assert!(parse(&missing, &["--type=raid1c"], true).is_err());
        assert!(parse(&missing, &["--type=raid1c", "--degraded"], false).is_err());
        assert!(parse(&missing, &["--type=softraid"], false).is_err());
        assert!(parse(&missing, &["--type=veracrypt"], false).is_err());
        assert!(parse(&link, &["--type=raid1c", "--degraded"], true).is_err());
        fs::remove_file(&link).unwrap();
        symlink(&peer, &link).unwrap();
        assert!(parse(&link, &["--type=raid1c", "--degraded"], true).is_err());
        assert!(parse(&mountpoint, &["--type=raid1c", "--degraded"], true).is_err());
        // Native character nodes reach the core inspector on OpenBSD. This
        // parsing decision grants no device access; /dev/null is rejected by
        // the core's physical-disk namespace and geometry validation.
        assert_eq!(
            parse(
                std::path::Path::new("/dev/null"),
                &["--type=raid1c", "--degraded"],
                true
            )
            .is_ok(),
            cfg!(target_os = "openbsd")
        );
        let args = [
            OsString::from("lurker"),
            "unmount".into(),
            "--type=raid1c".into(),
            missing.as_os_str().to_owned(),
        ];
        let ParseOutcome::Run(cli) = super::parse_args_for_platform(args, "openbsd").unwrap()
        else {
            panic!()
        };
        let CommandAction::Unmount(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.target, missing);
    }

    #[test]
    fn raid1c_flags_fail_before_source_inspection_on_other_platforms() {
        for os in ["linux", "macos", "freebsd", "netbsd", "dragonfly"] {
            for option in ["--member=peer", "--degraded", "--type=raid1c"] {
                let args = [
                    "lurker",
                    "mount",
                    option,
                    "missing-source",
                    "missing-mountpoint",
                ];
                let err = super::parse_args_for_platform(args.map(OsString::from), os).unwrap_err();
                assert!(err.message.unwrap().contains("require OpenBSD"));
            }
        }
    }

    #[test]
    fn raid1c_member_paths_preserve_non_utf8_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let member = OsString::from_vec(b"peer-\xff.img".to_vec());
        let args = [
            OsString::from("lurker"),
            "create".into(),
            "--type=raid1c".into(),
            "--member".into(),
            member.clone(),
            "first.img".into(),
            "1".into(),
        ];
        let ParseOutcome::Run(cli) = super::parse_args_for_platform(args, "openbsd").unwrap()
        else {
            panic!()
        };
        let CommandAction::Create(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.peer_sources[0].as_os_str(), member);
    }

    #[test]
    fn folder_create_has_no_size_and_rejects_container_options() {
        for kind in ["gocryptfs", "cryfs", "cryptomator", "pefs"] {
            let parsed = parse_env_args(
                ["lurker", "create", "--type", kind, "new-vault"].map(OsString::from),
            )
            .unwrap();
            let ParseOutcome::Run(cli) = parsed else {
                panic!()
            };
            let CommandAction::Create(c) = cli.command else {
                panic!()
            };
            assert_eq!(c.source_kind, SourceKind::Directory);
            assert!(c.size_gb.is_none());
            for flag in ["--cipher=aes", "--force", "1"] {
                assert!(parse_env_args(
                    ["lurker", "create", "--type", kind, "new-vault", flag].map(OsString::from)
                )
                .is_err());
            }
        }
    }

    #[test]
    fn only_native_pefs_directory_backend_accepts_a_keyfile() {
        for kind in ["gocryptfs", "cryfs", "cryptomator", "pefs"] {
            let result = parse_env_args(
                [
                    "lurker",
                    "create",
                    "--type",
                    kind,
                    "new-vault",
                    "--key-file=Cargo.toml",
                ]
                .map(OsString::from),
            );
            assert_eq!(result.is_ok(), kind == "pefs");
        }
    }

    #[test]
    fn keyfiles_and_keyslot_actions_parse_without_password_arguments() {
        for args in [
            vec![
                "lurker",
                "create",
                "--type",
                "luks",
                "--key-file",
                "binary key",
                "vault",
                "1",
            ],
            vec!["lurker", "createvc", "--key-file=binary key", "vault", "1"],
        ] {
            let ParseOutcome::Run(cli) =
                parse_env_args(args.into_iter().map(OsString::from)).unwrap()
            else {
                panic!()
            };
            let CommandAction::Create(c) = cli.command else {
                panic!()
            };
            assert_eq!(c.keyfile, Some("binary key".into()));
            assert!(c.passphrase.is_none());
        }
        let ParseOutcome::Run(cli) = parse_env_args(
            [
                "lurker",
                "mount",
                "Cargo.toml",
                "/tmp",
                "--key-file",
                "binary key",
            ]
            .map(OsString::from),
        )
        .unwrap() else {
            panic!()
        };
        let CommandAction::Mount(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.keyfile, Some("binary key".into()));
        assert!(c.passphrase.is_none());
        #[cfg(target_os = "linux")]
        for (verb, operation) in [
            ("keyslots", super::KeyslotOperation::List),
            ("add-key", super::KeyslotOperation::Add),
            ("remove-key", super::KeyslotOperation::Remove),
        ] {
            let mut args = vec!["lurker", verb, "vault"];
            if verb != "keyslots" {
                args.extend(["--slot=31", "--key-file=old"]);
            }
            if verb == "add-key" {
                args.extend(["--new-key-file", "new"]);
            }
            let ParseOutcome::Run(cli) =
                parse_env_args(args.into_iter().map(OsString::from)).unwrap()
            else {
                panic!()
            };
            let CommandAction::Keyslots(c) = cli.command else {
                panic!()
            };
            assert_eq!(c.operation, operation);
            assert_eq!(c.slot, (verb != "keyslots").then_some(31));
            assert_eq!(c.new_keyfile, (verb == "add-key").then(|| "new".into()));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn explicit_luks1_is_accepted_for_header_detected_keyslot_actions() {
        let ParseOutcome::Run(cli) =
            parse_env_args(["lurker", "keyslots", "--type=luks1", "vault"].map(OsString::from))
                .unwrap()
        else {
            panic!()
        };
        assert!(matches!(cli.command, CommandAction::Keyslots(_)));
    }

    #[test]
    fn rejects_misplaced_or_ambiguous_credential_options() {
        for args in [
            vec!["lurker", "create", "vault", "1", "--key-file"],
            vec!["lurker", "create", "vault", "1", "--key-file="],
            vec![
                "lurker",
                "create",
                "vault",
                "1",
                "--key-file=a",
                "--key-file=b",
            ],
            vec!["lurker", "create", "vault", "1", "--slot=0"],
            vec!["lurker", "create", "vault", "1", "--new-key-file=a"],
            vec![
                "lurker",
                "create",
                "--type=apple",
                "vault",
                "1",
                "--key-file=a",
            ],
            vec!["lurker", "unmount", "vault", "--key-file=a"],
            vec!["lurker", "keyslots", "vault", "--key-file=a"],
            vec!["lurker", "add-key", "vault", "--slot=32"],
            vec!["lurker", "add-key", "vault", "--slot=-1"],
            vec!["lurker", "add-key", "vault", "--slot=0", "--slot=1"],
            vec!["lurker", "add-key", "vault", "--type=veracrypt"],
            vec!["lurker", "add-key", "vault", "--cipher=aes"],
            vec!["lurker", "add-key", "vault", "--unprivileged"],
            vec!["lurker", "remove-key", "vault"],
            vec![
                "lurker",
                "remove-key",
                "vault",
                "--slot=0",
                "--new-key-file=a",
            ],
        ] {
            assert!(
                parse_env_args(args.iter().map(OsString::from)).is_err(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn unprivileged_create_selects_veracrypt_without_changing_cipher() {
        let input = [
            "lurker",
            "create",
            "--unprivileged",
            "--cipher",
            "twofish",
            "vault.hc",
            "1",
        ]
        .map(OsString::from);
        let ParseOutcome::Run(cli) = parse_env_args(input).unwrap() else {
            panic!()
        };
        let CommandAction::Create(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.privilege_mode, lurker_core::PrivilegeMode::Unprivileged);
        assert_eq!(c.volume_type, VolumeType::Veracrypt);
        assert_eq!(c.cipher, CreateCipher::Twofish);
        assert!(parse_env_args(
            [
                "lurker",
                "create",
                "--privileged",
                "--unprivileged",
                "vault",
                "1"
            ]
            .map(OsString::from)
        )
        .is_err());
    }

    #[test]
    fn parses_createvc_alias() {
        let args = vec![
            OsString::from("lurker"),
            OsString::from("createvc"),
            OsString::from("vault.hc"),
            OsString::from("4"),
        ];

        let command = match parse_env_args(args).unwrap() {
            ParseOutcome::Run(command) => command,
            _ => panic!("unexpected parse outcome"),
        };

        match command.command {
            CommandAction::Create(create) => {
                assert_eq!(create.volume_type, VolumeType::Veracrypt);
                assert_eq!(create.source_kind, SourceKind::File);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn parses_luks_cipher_profile() {
        let args = vec![
            OsString::from("lurker"),
            OsString::from("create"),
            OsString::from("--cipher"),
            OsString::from("serpent"),
            OsString::from("vault.img"),
            OsString::from("4"),
        ];

        let command = match parse_env_args(args).unwrap() {
            ParseOutcome::Run(command) => command,
            _ => panic!("unexpected parse outcome"),
        };

        match command.command {
            CommandAction::Create(create) => {
                assert_eq!(create.volume_type, VolumeType::default_create());
                assert_eq!(create.cipher, CreateCipher::Serpent);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn createvc_accepts_cipher_profile() {
        let args = vec![
            OsString::from("lurker"),
            OsString::from("createvc"),
            OsString::from("--cipher"),
            OsString::from("twofish"),
            OsString::from("vault.hc"),
            OsString::from("4"),
        ];

        let command = match parse_env_args(args).unwrap() {
            ParseOutcome::Run(command) => command,
            _ => panic!("unexpected parse outcome"),
        };

        match command.command {
            CommandAction::Create(create) => {
                assert_eq!(create.volume_type, VolumeType::Veracrypt);
                assert_eq!(create.cipher, CreateCipher::Twofish);
            }
            _ => panic!("unexpected command"),
        }
    }
    #[test]
    fn apple_type_and_platform_default_round_trip() {
        for flag in [vec!["--type", "apple"], vec!["--type=apple"]] {
            let mut input = vec![OsString::from("lurker"), OsString::from("create")];
            input.extend(flag.iter().map(OsString::from));
            input.extend([OsString::from("vault.dmg"), OsString::from("1")]);
            let ParseOutcome::Run(cli) = parse_env_args(input).unwrap() else {
                panic!()
            };
            let CommandAction::Create(c) = cli.command else {
                panic!()
            };
            assert_eq!(c.volume_type, VolumeType::Apple);
        }
        let input = ["lurker", "create", "vault", "1"].map(OsString::from);
        let ParseOutcome::Run(cli) = parse_env_args(input).unwrap() else {
            panic!()
        };
        let CommandAction::Create(c) = cli.command else {
            panic!()
        };
        assert_eq!(c.volume_type, VolumeType::default_create());
        assert_eq!(c.cipher, CreateCipher::Aes);
        for input in [
            vec!["lurker", "create", "--type", "auto", "vault", "1"],
            vec!["lurker", "createvc", "--type", "apple", "vault", "1"],
            vec!["lurker", "unmount", "--cipher", "aes", "vault"],
        ] {
            assert!(parse_env_args(input.into_iter().map(OsString::from)).is_err());
        }
    }
}
