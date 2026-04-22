use crate::cli::{Cli, CommandAction, CreateCommand, MountCommand, UnmountCommand};
use crate::error::{AppError, AppResult};
use crate::linux::{
    absolute_path, canonical_device_path, find_mount_source_for_mountpoint, is_block_device_target,
    mapper_device_path, mapper_name_for_source, normalize_mapper_name, resolve_umount_target,
    size_gb_to_mib, source_kind_for_path,
};
use crate::model::{SourceKind, VolumeType};
use crate::output::Output;
use crate::system::{run_status, trim_stdout, AppContext};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn run(command: Cli, output: Output) -> AppResult<()> {
    let Cli { command, .. } = command;
    let mut ctx = AppContext::new(&command, output)?;
    match command {
        CommandAction::Create(create) => create_device(&mut ctx, create),
        CommandAction::Mount(mount) => mount_device(&mut ctx, mount),
        CommandAction::Unmount(unmount) => unmount_device(&mut ctx, unmount),
    }
}

fn create_device(ctx: &mut AppContext, command: CreateCommand) -> AppResult<()> {
    assert_create_target(ctx, &command)?;
    match command.volume_type {
        VolumeType::Veracrypt => veracrypt_create_device(ctx, &command),
        VolumeType::Luks => luks_create_device(ctx, &command),
        VolumeType::Auto => Err(AppError::new(
            "Option --type auto is not valid with create.",
        )),
    }
}

fn mount_device(ctx: &mut AppContext, command: MountCommand) -> AppResult<()> {
    let detected_type = detect_source_volume_type(ctx, &command.source, command.volume_type)?;
    match detected_type {
        VolumeType::Veracrypt => {
            require_supported_veracrypt_tag(command.tag.as_deref())?;
            if ctx.have_veracrypt() {
                veracrypt_mount_device(ctx, &command.source, &command.mountpoint)
            } else {
                ctx.notice_veracrypt_fallback();
                tcrypt_mount_device(ctx, &command.source, &command.mountpoint)
            }
        }
        VolumeType::Luks => luks_mount_device(
            ctx,
            &command.source,
            &command.mountpoint,
            command.tag.as_deref(),
        ),
        VolumeType::Auto => Err(AppError::new("Unsupported volume type: auto")),
    }
}

fn unmount_device(ctx: &mut AppContext, command: UnmountCommand) -> AppResult<()> {
    warn_ignored_umount_tag(ctx, &command.target, command.tag.as_deref());
    let detected_type = detect_umount_volume_type(ctx, &command.target, command.volume_type)?;
    match detected_type {
        VolumeType::Veracrypt => {
            require_supported_veracrypt_tag(command.tag.as_deref())?;
            if ctx.have_veracrypt() {
                veracrypt_umount_device(ctx, &command.target)
            } else {
                ctx.notice_veracrypt_fallback();
                dmcrypt_umount_device(ctx, &command.target, None)
            }
        }
        VolumeType::Luks => dmcrypt_umount_device(ctx, &command.target, command.tag.as_deref()),
        VolumeType::Auto => Err(AppError::new("Unsupported volume type: auto")),
    }
}

fn assert_create_target(ctx: &mut AppContext, command: &CreateCommand) -> AppResult<()> {
    match command.source_kind {
        SourceKind::File => assert_create_file_target(&command.target, command.force),
        SourceKind::Block => assert_create_block_target(ctx, &command.target, command.force),
    }
}

fn assert_create_file_target(path: &Path, force: bool) -> AppResult<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let metadata = fs::metadata(parent).map_err(|err| {
        AppError::io(
            format!("Directory does not exist: {}", parent.display()),
            err,
        )
    })?;
    if !metadata.is_dir() {
        return Err(AppError::new(format!(
            "Directory does not exist: {}",
            parent.display()
        )));
    }
    if metadata.permissions().readonly() {
        return Err(AppError::new(format!(
            "Directory is not writable: {}",
            parent.display()
        )));
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(AppError::new(format!(
                    "Refusing to create on symlink target: {}",
                    path.display()
                )));
            }
            if !metadata.file_type().is_file() {
                return Err(AppError::new(format!(
                    "Target exists and is not a regular file: {}",
                    path.display()
                )));
            }
            if !force {
                return Err(AppError::new(
                    "Target file already exists. Re-run with -F to overwrite it.",
                ));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(AppError::io(
                format!("Failed to inspect {}", path.display()),
                err,
            ));
        }
    }

    Ok(())
}

fn assert_create_block_target(ctx: &mut AppContext, path: &Path, force: bool) -> AppResult<()> {
    if !is_block_device_target(path) {
        return Err(AppError::new(format!(
            "Block device does not exist: {}",
            path.display()
        )));
    }

    let metadata = fs::symlink_metadata(path)
        .map_err(|err| AppError::io(format!("Failed to inspect {}", path.display()), err))?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::new(format!(
            "Refusing to create on symlink target: {}",
            path.display()
        )));
    }

    if path.starts_with("/dev/mapper") {
        return Err(AppError::new(format!(
            "Refusing to create on an active mapper device: {}",
            path.display()
        )));
    }

    if !force {
        return Err(AppError::new(format!(
            "Block-device create is destructive. Re-run with -F to continue: {}",
            path.display()
        )));
    }

    let device_type = block_device_type(ctx, path)?;
    match device_type.as_str() {
        "disk" | "part" | "loop" => {}
        "" => {
            return Err(AppError::new(format!(
                "Failed to inspect block device: {}",
                path.display()
            )));
        }
        _ => {
            return Err(AppError::new(format!(
                "Unsupported block device type for create: {} ({})",
                path.display(),
                device_type
            )));
        }
    }

    if block_device_tree_has_mounts(ctx, path)? {
        return Err(AppError::new(format!(
            "Block device or one of its children is mounted: {}",
            path.display()
        )));
    }

    let existing_signature = block_device_filesystem(ctx, path)?;
    if !existing_signature.is_empty() {
        ctx.output.warn(&format!(
            "Existing on-disk signature detected on {}: {}",
            path.display(),
            existing_signature
        ));
    }

    Ok(())
}

fn require_passphrase_tty() -> AppResult<()> {
    if io::stdin().is_terminal() {
        Ok(())
    } else {
        Err(AppError::new(
            "This action requires an interactive TTY for the passphrase prompt.",
        ))
    }
}

fn luks_create_device(ctx: &mut AppContext, command: &CreateCommand) -> AppResult<()> {
    require_passphrase_tty()?;
    let mapper_name = mapper_name_for_source(&command.target, None)?;

    match command.source_kind {
        SourceKind::File => {
            let size_gb = command.size_gb.as_deref().unwrap_or_default();
            let size_mib = size_gb_to_mib(size_gb)?;
            let temp_path = temp_container_path(&command.target)?;
            let mut temp_guard = TempFileGuard::new(temp_path.clone());

            ctx.output.msg(&format!(
                "Creating LUKS container at {}",
                command.target.display()
            ));
            ctx.output
                .msg2(&format!("Target size: {} GB ({} MiB)", size_gb, size_mib));
            write_random_file(ctx.output.clone(), &temp_path, size_mib)?;

            ctx.output.msg2("Formatting LUKS2 header");
            luks_format(ctx, &temp_path)?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            ctx.output.msg2(&format!(
                "Opening mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            luks_open(ctx, &temp_path, &mapper_name)?;

            ctx.output.msg2("Creating Btrfs filesystem");
            mkfs_btrfs(ctx, &mapper_device_path(&mapper_name))?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();

            fs::rename(&temp_path, &command.target).map_err(|err| {
                AppError::io(format!("Failed to move {}", command.target.display()), err)
            })?;
            temp_guard.persist();
        }
        SourceKind::Block => {
            ctx.output.msg(&format!(
                "Creating LUKS volume on block device {}",
                command.target.display()
            ));
            ctx.output.msg2("Destructive mode enabled by --force");

            ctx.output.msg2("Formatting LUKS2 header");
            luks_format(ctx, &command.target)?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            ctx.output.msg2(&format!(
                "Opening mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            luks_open(ctx, &command.target, &mapper_name)?;

            ctx.output.msg2("Creating Btrfs filesystem");
            mkfs_btrfs(ctx, &mapper_device_path(&mapper_name))?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();
        }
    }

    ctx.output.success(&format!(
        "Created LUKS+Btrfs volume on {}",
        command.target.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    Ok(())
}

fn veracrypt_create_device(ctx: &mut AppContext, command: &CreateCommand) -> AppResult<()> {
    require_passphrase_tty()?;
    let mapper_name = mapper_name_for_source(&command.target, None)?;

    match command.source_kind {
        SourceKind::File => {
            let size_gb = command.size_gb.as_deref().unwrap_or_default();
            let size_mib = size_gb_to_mib(size_gb)?;
            let temp_path = temp_container_path(&command.target)?;
            let mut temp_guard = TempFileGuard::new(temp_path.clone());

            ctx.output.msg(&format!(
                "Creating VeraCrypt container at {}",
                command.target.display()
            ));
            ctx.output
                .msg2(&format!("Target size: {} GB ({} MiB)", size_gb, size_mib));
            ctx.output.msg2(
                "Creating VeraCrypt header (normal, password-only, AES, SHA-512, filesystem=none)",
            );
            veracrypt_create(ctx, &temp_path, SourceKind::File, Some(size_mib))?;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600)).map_err(|err| {
                AppError::io(format!("Failed to secure {}", temp_path.display()), err)
            })?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            ctx.output.msg2(&format!(
                "Opening mapper {} with cryptsetup",
                mapper_device_path(&mapper_name).display()
            ));
            tcrypt_open(ctx, &temp_path, &mapper_name)?;

            ctx.output.msg2("Creating Btrfs filesystem");
            mkfs_btrfs(ctx, &mapper_device_path(&mapper_name))?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();

            fs::rename(&temp_path, &command.target).map_err(|err| {
                AppError::io(format!("Failed to move {}", command.target.display()), err)
            })?;
            temp_guard.persist();
        }
        SourceKind::Block => {
            ctx.output.msg(&format!(
                "Creating VeraCrypt volume on block device {}",
                command.target.display()
            ));
            ctx.output.msg2("Destructive mode enabled by --force");
            ctx.output
                .msg2("Using VeraCrypt quick format for device-hosted creation");
            ctx.output.msg2(
                "Creating VeraCrypt header (normal, password-only, AES, SHA-512, filesystem=none)",
            );
            veracrypt_create(ctx, &command.target, SourceKind::Block, None)?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            ctx.output.msg2(&format!(
                "Opening mapper {} with cryptsetup",
                mapper_device_path(&mapper_name).display()
            ));
            tcrypt_open(ctx, &command.target, &mapper_name)?;

            ctx.output.msg2("Creating Btrfs filesystem");
            mkfs_btrfs(ctx, &mapper_device_path(&mapper_name))?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();
        }
    }

    ctx.output.success(&format!(
        "Created VeraCrypt+Btrfs volume on {}",
        command.target.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    Ok(())
}

fn luks_mount_device(
    ctx: &mut AppContext,
    source_path: &Path,
    mountpoint: &Path,
    tag_override: Option<&str>,
) -> AppResult<()> {
    require_passphrase_tty()?;
    let mapper_name = mapper_name_for_source(source_path, tag_override)?;
    ctx.set_cleanup_mapper(mapper_name.clone());

    ctx.output
        .msg(&format!("Mounting source {}", source_path.display()));
    ctx.output.msg2(&format!(
        "Opening mapper {}",
        mapper_device_path(&mapper_name).display()
    ));
    luks_open(ctx, source_path, &mapper_name)?;
    mount_open_mapper(ctx, &mapper_name, mountpoint)?;
    ctx.clear_cleanup_mapper();

    ctx.output.success(&format!(
        "Mounted {} at {}",
        source_path.display(),
        mountpoint.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    Ok(())
}

fn tcrypt_mount_device(
    ctx: &mut AppContext,
    source_path: &Path,
    mountpoint: &Path,
) -> AppResult<()> {
    require_passphrase_tty()?;
    let mapper_name = mapper_name_for_source(source_path, None)?;
    ctx.set_cleanup_mapper(mapper_name.clone());

    ctx.output.msg(&format!(
        "Mounting VeraCrypt source {}",
        source_path.display()
    ));
    ctx.output.msg2(&format!(
        "Opening mapper {} with cryptsetup fallback",
        mapper_device_path(&mapper_name).display()
    ));
    tcrypt_open(ctx, source_path, &mapper_name)?;
    mount_open_mapper(ctx, &mapper_name, mountpoint)?;
    ctx.clear_cleanup_mapper();

    ctx.output.success(&format!(
        "Mounted {} at {}",
        source_path.display(),
        mountpoint.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    Ok(())
}

fn veracrypt_mount_device(
    ctx: &mut AppContext,
    source_path: &Path,
    mountpoint: &Path,
) -> AppResult<()> {
    require_passphrase_tty()?;
    ctx.output.msg(&format!(
        "Mounting VeraCrypt source {}",
        source_path.display()
    ));
    let veracrypt = ctx.veracrypt_path()?.to_path_buf();
    let args = vec![
        OsString::from("-t"),
        OsString::from("--mount"),
        OsString::from("-k"),
        OsString::from(""),
        OsString::from("--pim=0"),
        OsString::from("--protect-hidden=no"),
        source_path.as_os_str().to_os_string(),
        mountpoint.as_os_str().to_os_string(),
    ];
    ctx.run_command(
        &veracrypt,
        &args,
        true,
        true,
        "Failed to mount VeraCrypt container",
    )?;
    ctx.output.success(&format!(
        "Mounted {} at {}",
        source_path.display(),
        mountpoint.display()
    ));
    Ok(())
}

fn dmcrypt_umount_device(
    ctx: &mut AppContext,
    target: &Path,
    tag_override: Option<&str>,
) -> AppResult<()> {
    let resolved = resolve_umount_target(target, tag_override)?;

    ctx.output.msg(&format!(
        "Unmounting {} target {}",
        resolved.target_kind,
        resolved.origin.display()
    ));
    if let Some(mountpoint) = &resolved.mountpoint {
        ctx.output.msg2(&format!(
            "Unmounting filesystem at {}",
            mountpoint.display()
        ));
        let args = vec![mountpoint.as_os_str().to_os_string()];
        let umount = ctx.umount_path()?.to_path_buf();
        ctx.run_command(&umount, &args, true, false, "Failed to unmount filesystem")?;
    } else {
        ctx.output.msg2(&format!(
            "No mounted filesystem detected for {}",
            resolved.mapper_path.display()
        ));
    }

    ctx.output.msg2(&format!(
        "Closing mapper {}",
        resolved.mapper_path.display()
    ));
    crypt_close(ctx, &resolved.mapper_name)?;
    ctx.output
        .success(&format!("Closed mapper {}", resolved.mapper_name));
    ctx.output
        .msg2(&format!("Resolved input kind: {}", resolved.target_kind));
    Ok(())
}

fn veracrypt_umount_device(ctx: &mut AppContext, target: &Path) -> AppResult<()> {
    let identifier = resolve_veracrypt_umount_identifier(ctx, target)?;
    ctx.output.msg(&format!(
        "Unmounting VeraCrypt target {}",
        identifier.display()
    ));
    let veracrypt = ctx.veracrypt_path()?.to_path_buf();
    let args = vec![
        OsString::from("-t"),
        OsString::from("-u"),
        identifier.as_os_str().to_os_string(),
    ];
    ctx.run_command(
        &veracrypt,
        &args,
        true,
        false,
        "Failed to unmount VeraCrypt target",
    )?;
    ctx.output.success(&format!(
        "Unmounted VeraCrypt target {}",
        identifier.display()
    ));
    Ok(())
}

fn require_supported_veracrypt_tag(tag: Option<&str>) -> AppResult<()> {
    if tag.is_some() {
        Err(AppError::new(
            "Tags are not yet supported for VeraCrypt containers.",
        ))
    } else {
        Ok(())
    }
}

fn warn_ignored_umount_tag(ctx: &AppContext, target: &Path, tag_override: Option<&str>) {
    if tag_override.is_none() {
        return;
    }

    if target.starts_with("/dev/mapper") {
        ctx.output
            .warn("Ignoring -t for /dev/mapper input; the explicit mapper path takes precedence.");
    } else if target.is_dir() {
        ctx.output.warn(
            "Ignoring -t for mountpoint input; the active mapper is resolved from the mounted filesystem.",
        );
    }
}

fn resolve_volume_type(
    requested_type: VolumeType,
    actual_type: Option<VolumeType>,
    subject: &Path,
    subject_kind: &str,
    allow_unknown_explicit: bool,
) -> AppResult<VolumeType> {
    if let Some(actual_type) = actual_type {
        if requested_type != VolumeType::Auto && requested_type != actual_type {
            return Err(AppError::new(format!(
                "Requested --type {} but detected {} container for {}: {}",
                requested_type,
                actual_type,
                subject_kind,
                subject.display()
            )));
        }
        return Ok(actual_type);
    }

    if requested_type != VolumeType::Auto {
        if allow_unknown_explicit {
            return Ok(requested_type);
        }
        return Err(AppError::new(format!(
            "Requested --type {} but could not verify that {} as a {} container: {}",
            requested_type,
            subject_kind,
            requested_type,
            subject.display()
        )));
    }

    Err(AppError::new(format!(
        "Unsupported or unrecognized encrypted {}: {}",
        subject_kind,
        subject.display()
    )))
}

fn detect_source_volume_type(
    ctx: &mut AppContext,
    source_path: &Path,
    requested_type: VolumeType,
) -> AppResult<VolumeType> {
    let actual_type = probe_source_volume_type(ctx, source_path)?;
    resolve_volume_type(requested_type, actual_type, source_path, "source", false)
}

fn detect_umount_volume_type(
    ctx: &mut AppContext,
    target: &Path,
    requested_type: VolumeType,
) -> AppResult<VolumeType> {
    if target.starts_with("/dev/mapper") {
        let mapper_name = normalize_mapper_name(target);
        let mut actual_type = active_mapper_volume_type(ctx, &mapper_name)?;
        if actual_type.is_none() && ctx.have_veracrypt() {
            if let Some(mountpoint) = crate::linux::find_mountpoint_for_source(target)? {
                if veracrypt_target_is_mounted(ctx, &mountpoint)? {
                    actual_type = Some(VolumeType::Veracrypt);
                }
            }
        }
        if requested_type == VolumeType::Auto && actual_type.is_none() {
            return Ok(VolumeType::Luks);
        }
        return resolve_volume_type(requested_type, actual_type, target, "target", true);
    }

    if target.is_dir() {
        let canonical_target = absolute_path(target)?;
        let source = find_mount_source_for_mountpoint(&canonical_target)?;
        let mut actual_type = None;
        if ctx.have_veracrypt() && veracrypt_target_is_mounted(ctx, &canonical_target)? {
            actual_type = Some(VolumeType::Veracrypt);
        } else if let Some(source) = source {
            if source.starts_with("/dev/mapper") {
                let mapper_name = normalize_mapper_name(&source);
                actual_type = active_mapper_volume_type(ctx, &mapper_name)?;
            }
        }

        if requested_type == VolumeType::Auto && actual_type.is_none() {
            return Ok(VolumeType::Luks);
        }
        return resolve_volume_type(requested_type, actual_type, target, "target", true);
    }

    let actual_type = probe_source_volume_type(ctx, target)?;
    resolve_volume_type(requested_type, actual_type, target, "target", false)
}

fn probe_source_volume_type(
    ctx: &mut AppContext,
    source_path: &Path,
) -> AppResult<Option<VolumeType>> {
    let canonical_source = if is_block_device_target(source_path) {
        canonical_device_path(source_path)
    } else {
        absolute_path(source_path)?
    };

    if is_luks_source(ctx, &canonical_source)? {
        return Ok(Some(VolumeType::Luks));
    }
    if is_veracrypt_source(ctx, &canonical_source)? {
        return Ok(Some(VolumeType::Veracrypt));
    }
    Ok(None)
}

fn is_luks_source(ctx: &mut AppContext, source: &Path) -> AppResult<bool> {
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    command_success(
        ctx,
        &cryptsetup,
        &[OsString::from("isLuks"), source.as_os_str().to_os_string()],
        false,
        "Failed to inspect source with cryptsetup isLuks",
    )
}

fn is_veracrypt_source(ctx: &mut AppContext, source: &Path) -> AppResult<bool> {
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    command_success(
        ctx,
        &cryptsetup,
        &[
            OsString::from("tcryptDump"),
            OsString::from("--type"),
            OsString::from("tcrypt"),
            OsString::from("--veracrypt"),
            source.as_os_str().to_os_string(),
        ],
        false,
        "Failed to inspect source with cryptsetup tcryptDump",
    )
}

fn active_mapper_volume_type(
    ctx: &mut AppContext,
    mapper_name: &str,
) -> AppResult<Option<VolumeType>> {
    let args = vec![OsString::from("status"), OsString::from(mapper_name)];
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    let output =
        ctx.capture_command(&cryptsetup, &args, true, "Failed to inspect active mapper")?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(value) = line.trim().strip_prefix("type:") {
            let value = value.trim().to_ascii_lowercase();
            if value.starts_with("luks") {
                return Ok(Some(VolumeType::Luks));
            }
            if value.starts_with("tcrypt") || value.starts_with("veracrypt") {
                return Ok(Some(VolumeType::Veracrypt));
            }
        }
    }
    Ok(None)
}

fn veracrypt_target_is_mounted(ctx: &mut AppContext, identifier: &Path) -> AppResult<bool> {
    if !ctx.have_veracrypt() {
        return Ok(false);
    }
    let veracrypt = ctx.veracrypt_path()?.to_path_buf();
    command_success(
        ctx,
        &veracrypt,
        &[
            OsString::from("-t"),
            OsString::from("-l"),
            identifier.as_os_str().to_os_string(),
        ],
        true,
        "Failed to inspect VeraCrypt mounts",
    )
}

fn resolve_veracrypt_umount_identifier(ctx: &mut AppContext, target: &Path) -> AppResult<PathBuf> {
    if target.starts_with("/dev/mapper") {
        let mountpoint = crate::linux::find_mountpoint_for_source(target)?.ok_or_else(|| {
            AppError::new(format!(
                "VeraCrypt mapper is not mounted: {}",
                target.display()
            ))
        })?;
        if !veracrypt_target_is_mounted(ctx, &mountpoint)? {
            return Err(AppError::new(format!(
                "VeraCrypt mapper is not mounted: {}",
                target.display()
            )));
        }
        return Ok(mountpoint);
    }

    if target.is_dir() {
        let canonical_target = absolute_path(target)?;
        if !veracrypt_target_is_mounted(ctx, &canonical_target)? {
            return Err(AppError::new(format!(
                "Not a mounted VeraCrypt path: {}",
                target.display()
            )));
        }
        return Ok(canonical_target);
    }

    if is_block_device_target(target) {
        if veracrypt_target_is_mounted(ctx, target)? {
            return Ok(target.to_path_buf());
        }
        let canonical_target = canonical_device_path(target);
        if canonical_target != target && veracrypt_target_is_mounted(ctx, &canonical_target)? {
            return Ok(canonical_target);
        }
        return Err(AppError::new(format!(
            "VeraCrypt source is not mounted: {}",
            target.display()
        )));
    }

    match source_kind_for_path(target) {
        Some(SourceKind::File) => {}
        _ => {
            return Err(AppError::new(format!(
                "File does not exist or is not a regular file: {}",
                target.display()
            )));
        }
    }

    if veracrypt_target_is_mounted(ctx, target)? {
        return Ok(target.to_path_buf());
    }
    let canonical_target = absolute_path(target)?;
    if canonical_target != target && veracrypt_target_is_mounted(ctx, &canonical_target)? {
        return Ok(canonical_target);
    }
    Err(AppError::new(format!(
        "VeraCrypt source is not mounted: {}",
        target.display()
    )))
}

fn detect_filesystem_type(ctx: &mut AppContext, device_path: &Path) -> AppResult<Option<String>> {
    if let Some(blkid) = ctx.blkid_path().map(PathBuf::from) {
        let output = ctx.capture_command(
            &blkid,
            &[
                OsString::from("-o"),
                OsString::from("value"),
                OsString::from("-s"),
                OsString::from("TYPE"),
                OsString::from("--"),
                device_path.as_os_str().to_os_string(),
            ],
            true,
            "Failed to inspect filesystem type with blkid",
        )?;
        if output.status.success() {
            let value = trim_stdout(&output);
            if !value.is_empty() {
                return Ok(Some(value));
            }
        }
    }

    if let Ok(lsblk) = ctx.lsblk_path().map(PathBuf::from) {
        let output = ctx.capture_command(
            &lsblk,
            &[
                OsString::from("-ndo"),
                OsString::from("FSTYPE"),
                OsString::from("--"),
                device_path.as_os_str().to_os_string(),
            ],
            false,
            "Failed to inspect filesystem type with lsblk",
        )?;
        if output.status.success() {
            let value = trim_stdout(&output);
            if !value.is_empty() {
                return Ok(Some(value));
            }
        }
    }

    Ok(None)
}

fn mount_open_mapper(ctx: &mut AppContext, mapper_name: &str, mountpoint: &Path) -> AppResult<()> {
    let mapper_path = mapper_device_path(mapper_name);
    let filesystem_type = detect_filesystem_type(ctx, &mapper_path)?;
    if let Some(filesystem_type) = &filesystem_type {
        ctx.output
            .msg2(&format!("Detected filesystem: {}", filesystem_type));
    } else {
        ctx.output.msg2(
            "Filesystem type could not be detected; mounting without filesystem-specific options",
        );
    }

    ctx.output
        .msg2(&format!("Mounting filesystem at {}", mountpoint.display()));
    let mut args = Vec::new();
    if filesystem_type
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("btrfs"))
        .unwrap_or(false)
    {
        args.push(OsString::from("-o"));
        args.push(OsString::from("compress=zstd"));
    }
    args.push(mapper_path.as_os_str().to_os_string());
    args.push(mountpoint.as_os_str().to_os_string());
    let mount = ctx.mount_path()?.to_path_buf();
    ctx.run_command(&mount, &args, true, false, "Failed to mount filesystem")
}

fn block_device_type(ctx: &mut AppContext, device_path: &Path) -> AppResult<String> {
    let lsblk = ctx.lsblk_path()?.to_path_buf();
    let output = ctx.capture_command(
        &lsblk,
        &[
            OsString::from("-ndo"),
            OsString::from("TYPE"),
            OsString::from("--"),
            device_path.as_os_str().to_os_string(),
        ],
        false,
        "Failed to inspect block device",
    )?;
    Ok(trim_stdout(&output))
}

fn block_device_filesystem(ctx: &mut AppContext, device_path: &Path) -> AppResult<String> {
    let lsblk = ctx.lsblk_path()?.to_path_buf();
    let output = ctx.capture_command(
        &lsblk,
        &[
            OsString::from("-ndo"),
            OsString::from("FSTYPE"),
            OsString::from("--"),
            device_path.as_os_str().to_os_string(),
        ],
        false,
        "Failed to inspect block device",
    )?;
    Ok(trim_stdout(&output))
}

fn block_device_tree_has_mounts(ctx: &mut AppContext, device_path: &Path) -> AppResult<bool> {
    let lsblk = ctx.lsblk_path()?.to_path_buf();
    let output = ctx.capture_command(
        &lsblk,
        &[
            OsString::from("-nrpo"),
            OsString::from("MOUNTPOINT"),
            OsString::from("--"),
            device_path.as_os_str().to_os_string(),
        ],
        false,
        "Failed to inspect block device tree",
    )?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| !line.trim().is_empty()))
}

fn luks_format(ctx: &mut AppContext, target: &Path) -> AppResult<()> {
    let privileged = is_block_device_target(target);
    let args = vec![
        OsString::from("-v"),
        OsString::from("--type"),
        OsString::from("luks2"),
        OsString::from("-c"),
        OsString::from("aes-xts-plain64"),
        OsString::from("-h"),
        OsString::from("sha512"),
        OsString::from("-s"),
        OsString::from("512"),
        OsString::from("-i"),
        OsString::from("2000"),
        OsString::from("--use-urandom"),
        OsString::from("-y"),
        OsString::from("-q"),
        OsString::from("luksFormat"),
        target.as_os_str().to_os_string(),
    ];
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    let mut command = ctx.prepare_command(&cryptsetup, &args, privileged, true)?;
    command.stdout(Stdio::null());
    run_status(&mut command, "Failed to format LUKS container")
}

fn luks_open(ctx: &mut AppContext, source: &Path, mapper_name: &str) -> AppResult<()> {
    let args = vec![
        OsString::from("luksOpen"),
        source.as_os_str().to_os_string(),
        OsString::from(mapper_name),
    ];
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    ctx.run_command(&cryptsetup, &args, true, true, "Failed to open LUKS mapper")
}

fn crypt_close(ctx: &mut AppContext, mapper_name: &str) -> AppResult<()> {
    let args = vec![OsString::from("close"), OsString::from(mapper_name)];
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    ctx.run_command(&cryptsetup, &args, true, false, "Failed to close mapper")
}

fn tcrypt_open(ctx: &mut AppContext, source: &Path, mapper_name: &str) -> AppResult<()> {
    let args = vec![
        OsString::from("open"),
        OsString::from("--type"),
        OsString::from("tcrypt"),
        OsString::from("--veracrypt"),
        source.as_os_str().to_os_string(),
        OsString::from(mapper_name),
    ];
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    ctx.run_command(
        &cryptsetup,
        &args,
        true,
        true,
        "Failed to open VeraCrypt mapper with cryptsetup",
    )
}

fn veracrypt_create(
    ctx: &mut AppContext,
    source_path: &Path,
    target_kind: SourceKind,
    size_mib: Option<u64>,
) -> AppResult<()> {
    let mut args = vec![
        OsString::from("-t"),
        OsString::from("--create"),
        OsString::from("--force"),
        OsString::from("--volume-type=normal"),
        OsString::from("--encryption=AES"),
        OsString::from("--hash=SHA-512"),
        OsString::from("--filesystem=none"),
        OsString::from("--random-source=/dev/urandom"),
        OsString::from("--pim=0"),
        OsString::from("-k"),
        OsString::from(""),
    ];
    match target_kind {
        SourceKind::File => {
            let size_mib = size_mib.ok_or_else(|| AppError::new("Missing file container size."))?;
            args.push(OsString::from(format!("--size={}M", size_mib)));
        }
        SourceKind::Block => {
            args.push(OsString::from("--size=max"));
            args.push(OsString::from("--quick"));
        }
    }
    args.push(source_path.as_os_str().to_os_string());
    let veracrypt = ctx.veracrypt_path()?.to_path_buf();
    ctx.run_command(
        &veracrypt,
        &args,
        target_kind == SourceKind::Block,
        true,
        "Failed to create VeraCrypt container",
    )
}

fn mkfs_btrfs(ctx: &mut AppContext, device_path: &Path) -> AppResult<()> {
    let args = vec![OsString::from("-q"), device_path.as_os_str().to_os_string()];
    let mkfs_btrfs = ctx.mkfs_btrfs_path()?.to_path_buf();
    ctx.run_command(
        &mkfs_btrfs,
        &args,
        true,
        false,
        "Failed to create Btrfs filesystem",
    )
}

fn command_success(
    ctx: &mut AppContext,
    program: &Path,
    args: &[OsString],
    privileged: bool,
    context: &str,
) -> AppResult<bool> {
    let mut command = ctx.prepare_command(program, args, privileged, false)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = command.status().map_err(|err| AppError::io(context, err))?;
    Ok(status.success())
}

fn temp_container_path(target: &Path) -> AppResult<PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let base = target
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "volume".into());
    let pid = std::process::id();
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..1024u32 {
        let candidate = parent.join(format!(
            ".{}.lurker-tmp-{}-{}-{}",
            base, pid, epoch, attempt
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(AppError::new(format!(
        "Failed to create a temporary path for {}",
        target.display()
    )))
}

fn write_random_file(output: Output, path: &Path, size_mib: u64) -> AppResult<()> {
    let mut random = File::open("/dev/urandom")
        .map_err(|err| AppError::io("Failed to open /dev/urandom", err))?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| AppError::io(format!("Failed to create {}", path.display()), err))?;

    let mut remaining_bytes = size_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| AppError::new("Size is too large."))?;
    let total_bytes = remaining_bytes;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut last_update = Instant::now();
    let show_progress = output.stderr_is_tty();

    if show_progress {
        output.progress("  -> Writing random data:   0%");
    } else {
        output.msg2("Writing random data");
    }

    while remaining_bytes > 0 {
        let chunk_len = usize::try_from(remaining_bytes.min(buffer.len() as u64))
            .map_err(|_| AppError::new("Size is too large."))?;
        random
            .read_exact(&mut buffer[..chunk_len])
            .map_err(|err| AppError::io("Failed to read from /dev/urandom", err))?;
        file.write_all(&buffer[..chunk_len])
            .map_err(|err| AppError::io(format!("Failed to write {}", path.display()), err))?;
        remaining_bytes -= chunk_len as u64;

        if show_progress && last_update.elapsed() >= Duration::from_millis(200) {
            let written = total_bytes - remaining_bytes;
            let percent = written
                .checked_mul(100)
                .and_then(|value| value.checked_div(total_bytes))
                .unwrap_or(100) as u32;
            output.progress(&format!("  -> Writing random data: {:>3}%", percent));
            last_update = Instant::now();
        }
    }

    file.sync_all()
        .map_err(|err| AppError::io(format!("Failed to sync {}", path.display()), err))?;
    if show_progress {
        output.progress("  -> Writing random data: 100%");
        output.finish_progress();
    }
    Ok(())
}

struct TempFileGuard {
    path: PathBuf,
    persist: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            persist: false,
        }
    }

    fn persist(&mut self) {
        self.persist = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.persist {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::require_supported_veracrypt_tag;

    #[test]
    fn veracrypt_tags_are_rejected() {
        assert!(require_supported_veracrypt_tag(Some("work")).is_err());
        assert!(require_supported_veracrypt_tag(None).is_ok());
    }
}
