use lurker_core::{
    AppError, ColorMode, CommandAction, CreateCipher, CreateCommand, MountCommand, SourceKind,
    UnmountCommand, VolumeType,
};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::IsTerminal;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Cli {
    pub color_mode: ColorMode,
    pub command: CommandAction,
}

#[derive(Debug)]
pub enum ParseOutcome {
    Run(Cli),
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
    let mut create_cipher = CreateCipher::Aes;
    let mut create_cipher_explicit = false;
    let mut parsed_args = Vec::new();

    let mut index = 0usize;
    while index < raw_args.len() {
        let current = raw_args[index].to_string_lossy();
        match current.as_ref() {
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
                        "Invalid --type value: {}. Use auto, luks, or veracrypt.",
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
            message: Some("Action is required. Use: create, createvc, mount, or umount.".into()),
            exit_code: 1,
            show_help: true,
            help_to_stdout: false,
            script_name,
            color_mode,
        });
    }

    let mut action = first;
    if action == "unmount" {
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

    match action.as_str() {
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
                            "Option --type auto is not valid with create. Use luks or veracrypt."
                                .into(),
                        ),
                        exit_code: 1,
                        show_help: false,
                        help_to_stdout: false,
                        script_name,
                        color_mode,
                    });
                }
                (VolumeType::Auto, false) => VolumeType::Luks,
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
            let source_kind = classify_create_target(&target, &script_name, color_mode)?;
            let size_gb = match source_kind {
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

            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command: CommandAction::Create(CreateCommand {
                    target,
                    size_gb,
                    source_kind,
                    force,
                    volume_type,
                    cipher: create_cipher,
                    passphrase: None,
                }),
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
            let source_kind = path_source_kind(&source).ok_or_else(|| ParseFailure {
                message: Some(format!(
                    "Source does not exist or is not a regular file or block device: {}",
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
            if !mountpoint.is_dir() {
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

            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command: CommandAction::Mount(MountCommand {
                    source,
                    mountpoint,
                    tag,
                    volume_type,
                    passphrase: None,
                    readonly: false,
                }),
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

            Ok(ParseOutcome::Run(Cli {
                color_mode,
                command: CommandAction::Unmount(UnmountCommand {
                    target: PathBuf::from(&action_args[0]),
                    tag,
                    volume_type,
                }),
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

    format!(
        "\n{header}Usage:{reset}\n  {accent}{script_name} create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force]{reset}\n  {accent}{script_name} create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force]{reset}\n  {accent}{script_name} createvc [--cipher CIPHER] <file> <size-gb> [-F|--force]{reset}\n  {accent}{script_name} createvc [--cipher CIPHER] <block-device> [-F|--force]{reset}\n  {accent}{script_name} mount [--type TYPE] [-t TAG] <source> <mountpoint>{reset}\n  {accent}{script_name} unmount [--type TYPE] [-t TAG] <target>{reset}\n  {accent}{script_name} help{reset}\n\n{header}Examples:{reset}\n  {accent}{script_name} create ./vault.img 4{reset}\n  {accent}{script_name} create --cipher serpent ./vault-serpent.img 4{reset}\n  {accent}{script_name} create --cipher twofish /dev/sda2 --force{reset}\n  {accent}{script_name} create --type veracrypt --cipher serpent ./vault.hc 4{reset}\n  {accent}{script_name} createvc --cipher twofish /dev/sdb2 --force{reset}\n  {accent}{script_name} mount ./vault.img /mnt/vault{reset}\n  {accent}{script_name} mount -t work /dev/sda2 /mnt/work{reset}\n  {accent}{script_name} unmount /mnt/work{reset}\n  {accent}{script_name} unmount -t work /dev/sda2{reset}\n\n{header}Options:{reset}\n  {accent}-F, --force{reset}       Overwrite an existing file or allow destructive block-device create\n  {accent}-t TAG{reset}            Use /dev/mapper/lurker_TAG for mount or unmount\n  {accent}--type TYPE{reset}       Container type: create uses luks|veracrypt; mount/unmount use auto|luks|veracrypt\n  {accent}--cipher CIPHER{reset}   Create cipher: aes, serpent, or twofish\n  {accent}--color=WHEN{reset}      Color output: auto, always, never\n  {accent}-h, --help{reset}        Show this help text\n"
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
            "Invalid --type value: {}. Use auto, luks, or veracrypt.",
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
    if file_type.is_block_device() {
        Some(SourceKind::Block)
    } else if file_type.is_file() {
        Some(SourceKind::File)
    } else {
        None
    }
}

fn classify_create_target(
    path: &Path,
    script_name: &str,
    color_mode: ColorMode,
) -> Result<SourceKind, ParseFailure> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_block_device() {
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
        "umount" => format!("Operand flags are no longer supported. Use: {script_name} umount <target>"),
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
    fn parses_createvc_alias() {
        let args = vec![
            OsString::from("lurker"),
            OsString::from("createvc"),
            OsString::from("vault.hc"),
            OsString::from("4"),
        ];

        let command = match parse_env_args(args).unwrap() {
            ParseOutcome::Run(command) => command,
            ParseOutcome::Help(_) => panic!("unexpected help"),
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
            ParseOutcome::Help(_) => panic!("unexpected help"),
        };

        match command.command {
            CommandAction::Create(create) => {
                assert_eq!(create.volume_type, VolumeType::Luks);
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
            ParseOutcome::Help(_) => panic!("unexpected help"),
        };

        match command.command {
            CommandAction::Create(create) => {
                assert_eq!(create.volume_type, VolumeType::Veracrypt);
                assert_eq!(create.cipher, CreateCipher::Twofish);
            }
            _ => panic!("unexpected command"),
        }
    }
}
