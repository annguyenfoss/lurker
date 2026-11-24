use super::*;
use crate::api::{CommandAction, CreateCommand, CreateFilesystem, MountCommand, UnmountCommand};
use crate::error::{AppError, AppResult};
use crate::linux::{
    absolute_path, is_block_device_target, mapper_device_path, mapper_name_for_source,
    read_mountinfo, resolve_umount_target, size_gb_to_mib, source_kind_for_path,
    ResolvedUmountTarget,
};
use crate::linux_source::{lock_file_source, publish_locked_container as publish_container};
use crate::model::{CreateCipher, ResolvedTargetKind, SourceKind, VolumeType};
use crate::output::Output;
use crate::system::{trim_stdout, AppContext};
use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn run(command: CommandAction, output: Output) -> AppResult<()> {
    let volume_type = match &command {
        CommandAction::Create(c) => c.volume_type,
        CommandAction::Mount(c) => c.volume_type,
        CommandAction::Unmount(c) => c.volume_type,
        CommandAction::Keyslots(_) => VolumeType::Luks,
    };
    if volume_type == VolumeType::Apple {
        return Err(AppError::new("Apple encrypted disk images require macOS."));
    }
    let mut ctx = AppContext::new(&command, output)?;
    match command {
        CommandAction::Create(create) => create_device(&mut ctx, create),
        CommandAction::Mount(mount) => mount_device(&mut ctx, mount),
        CommandAction::Unmount(unmount) => unmount_device(&mut ctx, unmount),
        CommandAction::Keyslots(command) => crate::keyslots::run(&mut ctx, command),
    }
}

fn create_device(ctx: &mut AppContext, command: CreateCommand) -> AppResult<()> {
    // Resolve the authorized caller before any header or filesystem mutation.
    let owner = crate::new_filesystem::invoking_owner()?;
    if command.volume_type == VolumeType::Auto {
        return Err(AppError::new(
            "Option --type auto is not valid with create.",
        ));
    }
    if command.volume_type == VolumeType::Veracrypt {
        validate_veracrypt_passphrase(command.passphrase.as_deref())?;
    }
    assert_create_target(ctx, &command)?;
    let source_guard = if command.source_kind == SourceKind::File && command.target.exists() {
        let source = lock_file_source(&command.target)?;
        assert_file_not_attached(ctx, &source)?;
        Some(source)
    } else {
        None
    };
    assert_mapper_available(&mapper_name_for_source(&command.target, None)?)?;
    if let Some(module) = crate::system::linux_filesystem_module(command.filesystem) {
        if !Path::new("/sys/module").join(module).exists() {
            // Resolve module/signing prerequisites before creating a header,
            // especially for explicitly destructive physical-device requests.
            let modprobe = crate::trusted_system_executable("modprobe")?;
            let loaded = ctx.capture_command(
                &modprobe,
                &[module.into()],
                true,
                "Load filesystem driver before storage mutation",
            )?;
            if !loaded.status.success() {
                return Err(AppError::new(format!(
                    "Cannot load native {module} driver before storage creation: {}",
                    String::from_utf8_lossy(&loaded.stderr).trim()
                )));
            }
        }
    }
    match command.volume_type {
        VolumeType::Veracrypt => {
            veracrypt_create_device(ctx, &command, source_guard.as_ref(), owner)
        }
        VolumeType::Luks | VolumeType::Luks1 => {
            luks_create_device(ctx, &command, source_guard.as_ref(), owner)
        }
        VolumeType::Apple => Err(AppError::new("Apple encrypted disk images require macOS.")),
        VolumeType::Auto => Err(AppError::new(
            "Option --type auto is not valid with create.",
        )),
        other => Err(AppError::new(format!(
            "Backend {other} is not a Linux block-container backend."
        ))),
    }
}

fn mount_device(ctx: &mut AppContext, mut command: MountCommand) -> AppResult<()> {
    if source_kind_for_path(&command.source).is_none() {
        return Err(AppError::new(
            "Source must be a regular file or block device.",
        ));
    }
    if !command.mountpoint.is_dir() {
        return Err(AppError::new("Mountpoint must be an existing directory."));
    }
    command.source = absolute_path(&command.source)?;
    command.mountpoint = absolute_path(&command.mountpoint)?;
    let source_guard = if command.source.is_file() {
        let source = lock_file_source(&command.source)?;
        assert_file_not_attached(ctx, &source)?;
        Some(source)
    } else {
        None
    };
    let mountpoint = &command.mountpoint;
    if read_mountinfo()?
        .iter()
        .any(|mount| &mount.mount_point == mountpoint)
    {
        return Err(AppError::new("Mountpoint is already mounted."));
    }
    // cryptsetup flocks regular-file metadata itself. Keep our exclusive source
    // lock and give it a pinned loop block device, whose metadata lock is a
    // separate resource. This also prevents reopening a replaced pathname.
    let crypto_source = if let Some(source) = source_guard.as_ref() {
        attach_locked_source(ctx, source, &command.source, command.readonly)?
    } else {
        command.source.clone()
    };
    let detected_type = detect_source_volume_type(ctx, &crypto_source, command.volume_type)?;
    match detected_type {
        VolumeType::Veracrypt => {
            require_supported_veracrypt_tag(command.tag.as_deref())?;
            tcrypt_mount_device(
                ctx,
                &command.source,
                &crypto_source,
                &command.mountpoint,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                command.readonly,
                source_guard.as_ref(),
            )
        }
        VolumeType::Luks | VolumeType::Luks1 => luks_mount_device(
            ctx,
            &command.source,
            &crypto_source,
            &command.mountpoint,
            command.tag.as_deref(),
            command.passphrase.as_deref(),
            command.keyfile.as_deref(),
            command.readonly,
            source_guard.as_ref(),
        ),
        VolumeType::Apple => Err(AppError::new("Apple encrypted disk images require macOS.")),
        other => Err(AppError::new(format!(
            "Unsupported Linux container type: {other}"
        ))),
    }
}

fn unmount_device(ctx: &mut AppContext, command: UnmountCommand) -> AppResult<()> {
    warn_ignored_umount_tag(ctx, &command.target, command.tag.as_deref());
    // Hold the same inode lock used by create and mount for source-addressed
    // closes, including the opt-in shutdown hook.
    let source_guard = if source_kind_for_path(&command.target) == Some(SourceKind::File) {
        Some(lock_file_source(&absolute_path(&command.target)?)?)
    } else {
        None
    };
    let resolved = resolve_umount_target(&command.target, command.tag.as_deref())?;
    match (resolved.target_kind, source_guard.as_ref()) {
        (ResolvedTargetKind::File, Some(source)) => {
            verify_mapper_source(ctx, &resolved.mapper_name, source, &resolved.origin)?;
        }
        (ResolvedTargetKind::File, None) | (_, Some(_)) => {
            return Err(AppError::new(
                "Container path changed before close. Retry with the intended source.",
            ));
        }
        (_, None) => {}
    }
    let actual_type = active_mapper_volume_type(ctx, &resolved.mapper_name)?;
    let detected_type = resolve_volume_type(
        command.volume_type,
        actual_type,
        &command.target,
        "target",
        false,
    )?;
    if detected_type == VolumeType::Veracrypt {
        require_supported_veracrypt_tag(command.tag.as_deref())?;
    }
    dmcrypt_umount_device(ctx, &resolved)
}

fn assert_create_target(ctx: &mut AppContext, command: &CreateCommand) -> AppResult<()> {
    if command.source_kind == SourceKind::File && is_block_device_target(&command.target) {
        return Err(AppError::new("File create cannot target a block device."));
    }
    match command.source_kind {
        SourceKind::File => assert_create_file_target(&command.target, command.force),
        SourceKind::Block => assert_create_block_target(ctx, &command.target, command.force),
        SourceKind::Directory => Err(AppError::new(
            "Container creation requires a file or block device.",
        )),
    }
}

fn assert_create_file_target(path: &Path, force: bool) -> AppResult<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
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

    if block_device_tree_has_holders(ctx, path)? {
        return Err(AppError::new(
            "Block device or a child has active device holders.",
        ));
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

fn require_passphrase(passphrase: Option<&str>, keyfile: Option<&Path>) -> AppResult<()> {
    if passphrase.is_some() || keyfile.is_some() || io::stdin().is_terminal() {
        Ok(())
    } else {
        Err(AppError::new(
            "This action requires either a provided passphrase or an interactive TTY.",
        ))
    }
}

fn luks_create_device(
    ctx: &mut AppContext,
    command: &CreateCommand,
    existing_source: Option<&File>,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    require_passphrase(command.passphrase.as_deref(), command.keyfile.as_deref())?;
    let profile = cipher_profile(command.cipher);
    let mapper_name = mapper_name_for_source(&command.target, None)?;

    match command.source_kind {
        SourceKind::Directory => {
            return Err(AppError::new(
                "Container creation requires a file or block device.",
            ))
        }
        SourceKind::File => {
            let size_gb = command.size_gb.as_deref().unwrap_or_default();
            let size_mib = size_gb_to_mib(size_gb)?;
            let temp_path = temp_container_path(&command.target)?;
            let mut temp_guard =
                TempFileGuard::new(temp_path.clone(), &mapper_name, ctx.output.clone());

            ctx.output.msg(&format!(
                "Creating LUKS container at {}",
                command.target.display()
            ));
            ctx.output.msg2(&format!("LUKS profile: {}", profile.label));
            ctx.output
                .msg2(&format!("Target size: {} GB ({} MiB)", size_gb, size_mib));
            write_random_file(ctx.output.clone(), &temp_path, size_mib)?;

            ctx.output
                .msg2(if command.volume_type == VolumeType::Luks1 {
                    "Formatting LUKS1 header"
                } else {
                    "Formatting LUKS2 header"
                });
            luks_format(
                ctx,
                &temp_path,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                profile,
                command.volume_type == VolumeType::Luks1,
            )?;

            ctx.output.msg2(&format!(
                "Opening mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            luks_open(
                ctx,
                &temp_path,
                &mapper_name,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                (command.volume_type != VolumeType::Luks1).then_some(profile),
                false,
            )?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            format_filesystem(
                ctx,
                &mapper_device_path(&mapper_name),
                command.filesystem,
                owner,
            )?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();

            assign_new_image_owner(&temp_path, owner)?;
            publish_container(&temp_path, &command.target, command.force, existing_source)?;
            temp_guard.persist();
        }
        SourceKind::Block => {
            ctx.output.msg(&format!(
                "Creating LUKS volume on block device {}",
                command.target.display()
            ));
            ctx.output.msg2(&format!("LUKS profile: {}", profile.label));
            ctx.output.msg2("Destructive mode enabled by --force");

            ctx.output
                .msg2(if command.volume_type == VolumeType::Luks1 {
                    "Formatting LUKS1 header"
                } else {
                    "Formatting LUKS2 header"
                });
            luks_format(
                ctx,
                &command.target,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                profile,
                command.volume_type == VolumeType::Luks1,
            )?;

            ctx.output.msg2(&format!(
                "Opening mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            luks_open(
                ctx,
                &command.target,
                &mapper_name,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                (command.volume_type != VolumeType::Luks1).then_some(profile),
                false,
            )?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            format_filesystem(
                ctx,
                &mapper_device_path(&mapper_name),
                command.filesystem,
                owner,
            )?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();
        }
    }

    ctx.output.success(&format!(
        "Created LUKS+{} volume on {}",
        command
            .filesystem
            .unwrap_or(CreateFilesystem::Btrfs)
            .as_str(),
        command.target.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    ctx.output.msg2(&format!("LUKS cipher: {}", profile.label));
    Ok(())
}

fn veracrypt_create_device(
    ctx: &mut AppContext,
    command: &CreateCommand,
    existing_source: Option<&File>,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    require_passphrase(command.passphrase.as_deref(), command.keyfile.as_deref())?;
    let profile = cipher_profile(command.cipher);
    let mapper_name = mapper_name_for_source(&command.target, None)?;

    match command.source_kind {
        SourceKind::Directory => {
            return Err(AppError::new(
                "Container creation requires a file or block device.",
            ))
        }
        SourceKind::File => {
            let size_gb = command.size_gb.as_deref().unwrap_or_default();
            let size_mib = size_gb_to_mib(size_gb)?;
            let temp_path = temp_container_path(&command.target)?;
            let mut temp_guard =
                TempFileGuard::new(temp_path.clone(), &mapper_name, ctx.output.clone());

            ctx.output.msg(&format!(
                "Creating VeraCrypt container at {}",
                command.target.display()
            ));
            ctx.output
                .msg2(&format!("VeraCrypt profile: {}", profile.label));
            ctx.output
                .msg2(&format!("Target size: {} GB ({} MiB)", size_gb, size_mib));
            ctx.output.msg2(&format!(
                "Creating VeraCrypt header (normal, {}, SHA-512, filesystem=none)",
                profile.label
            ));
            veracrypt_create(
                ctx,
                &temp_path,
                SourceKind::File,
                Some(size_mib),
                profile,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
            )?;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600)).map_err(|err| {
                AppError::io(format!("Failed to secure {}", temp_path.display()), err)
            })?;

            ctx.output.msg2(&format!(
                "Opening mapper {} with cryptsetup",
                mapper_device_path(&mapper_name).display()
            ));
            tcrypt_open(
                ctx,
                &temp_path,
                &mapper_name,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                false,
            )?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            format_filesystem(
                ctx,
                &mapper_device_path(&mapper_name),
                command.filesystem,
                owner,
            )?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();

            assign_new_image_owner(&temp_path, owner)?;
            publish_container(&temp_path, &command.target, command.force, existing_source)?;
            temp_guard.persist();
        }
        SourceKind::Block => {
            ctx.output.msg(&format!(
                "Creating VeraCrypt volume on block device {}",
                command.target.display()
            ));
            ctx.output
                .msg2(&format!("VeraCrypt profile: {}", profile.label));
            ctx.output.msg2("Destructive mode enabled by --force");
            ctx.output
                .msg2("Using VeraCrypt quick format for device-hosted creation");
            ctx.output.msg2(&format!(
                "Creating VeraCrypt header (normal, {}, SHA-512, filesystem=none)",
                profile.label
            ));
            veracrypt_create(
                ctx,
                &command.target,
                SourceKind::Block,
                None,
                profile,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
            )?;

            ctx.output.msg2(&format!(
                "Opening mapper {} with cryptsetup",
                mapper_device_path(&mapper_name).display()
            ));
            tcrypt_open(
                ctx,
                &command.target,
                &mapper_name,
                command.passphrase.as_deref(),
                command.keyfile.as_deref(),
                false,
            )?;

            ctx.set_cleanup_mapper(mapper_name.clone());
            format_filesystem(
                ctx,
                &mapper_device_path(&mapper_name),
                command.filesystem,
                owner,
            )?;

            ctx.output.msg2(&format!(
                "Closing mapper {}",
                mapper_device_path(&mapper_name).display()
            ));
            crypt_close(ctx, &mapper_name)?;
            ctx.clear_cleanup_mapper();
        }
    }

    ctx.output.success(&format!(
        "Created VeraCrypt+{} volume on {}",
        command
            .filesystem
            .unwrap_or(CreateFilesystem::Btrfs)
            .as_str(),
        command.target.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    ctx.output
        .msg2(&format!("VeraCrypt cipher: {}", profile.label));
    Ok(())
}

fn luks_mount_device(
    ctx: &mut AppContext,
    source_path: &Path,
    crypto_source: &Path,
    mountpoint: &Path,
    tag_override: Option<&str>,
    passphrase: Option<&str>,
    keyfile: Option<&Path>,
    readonly: bool,
    locked_source: Option<&File>,
) -> AppResult<()> {
    require_passphrase(passphrase, keyfile)?;
    let mapper_name = mapper_name_for_source(source_path, tag_override)?;
    let profile = detect_luks_cipher_profile(ctx, crypto_source)?;

    ctx.output
        .msg(&format!("Mounting source {}", source_path.display()));
    if let Some(profile) = profile {
        ctx.output
            .msg2(&format!("Detected LUKS cipher: {}", profile.label));
    }
    ctx.output.msg2(&format!(
        "Opening mapper {}",
        mapper_device_path(&mapper_name).display()
    ));
    assert_mapper_available(&mapper_name)?;
    luks_open(
        ctx,
        crypto_source,
        &mapper_name,
        passphrase,
        keyfile,
        profile,
        readonly,
    )?;
    ctx.set_cleanup_mapper(mapper_name.clone());
    if let Some(source) = locked_source {
        verify_mapper_source(ctx, &mapper_name, source, source_path)?;
    }
    ctx.release_cleanup_loop()?;
    mount_open_mapper(ctx, &mapper_name, mountpoint, readonly)?;
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
    crypto_source: &Path,
    mountpoint: &Path,
    passphrase: Option<&str>,
    keyfile: Option<&Path>,
    readonly: bool,
    locked_source: Option<&File>,
) -> AppResult<()> {
    require_passphrase(passphrase, keyfile)?;
    let mapper_name = mapper_name_for_source(source_path, None)?;

    ctx.output.msg(&format!(
        "Mounting VeraCrypt source {}",
        source_path.display()
    ));
    ctx.output.msg2(&format!(
        "Opening mapper {} with cryptsetup",
        mapper_device_path(&mapper_name).display()
    ));
    assert_mapper_available(&mapper_name)?;
    tcrypt_open(
        ctx,
        crypto_source,
        &mapper_name,
        passphrase,
        keyfile,
        readonly,
    )?;
    ctx.set_cleanup_mapper(mapper_name.clone());
    if let Some(source) = locked_source {
        verify_mapper_source(ctx, &mapper_name, source, source_path)?;
    }
    ctx.release_cleanup_loop()?;
    mount_open_mapper(ctx, &mapper_name, mountpoint, readonly)?;
    ctx.clear_cleanup_mapper();

    ctx.output.success(&format!(
        "Mounted {} at {}",
        source_path.display(),
        mountpoint.display()
    ));
    ctx.output.msg2(&format!("Mapper name: {}", mapper_name));
    Ok(())
}

fn dmcrypt_umount_device(ctx: &mut AppContext, resolved: &ResolvedUmountTarget) -> AppResult<()> {
    ctx.output.msg(&format!(
        "Unmounting {} target {}",
        resolved.target_kind,
        resolved.origin.display()
    ));
    let filesystem = detect_filesystem_type(ctx, &resolved.mapper_path)?;
    if filesystem.as_deref() == Some("zfs_member") {
        export_zfs(ctx, &resolved.mapper_name, resolved.mountpoint.as_deref())?;
        crypt_close(ctx, &resolved.mapper_name)?;
        return Ok(());
    }
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

fn parse_device_pair(value: &str) -> AppResult<(u64, u64)> {
    let (major, minor) = value
        .trim()
        .split_once(':')
        .ok_or_else(|| AppError::new("Invalid native device identity."))?;
    Ok((
        major
            .parse()
            .map_err(|_| AppError::new("Invalid native device major number."))?,
        minor
            .parse()
            .map_err(|_| AppError::new("Invalid native device minor number."))?,
    ))
}

fn loop_source_identities(bytes: &[u8]) -> AppResult<Vec<(String, (u64, u64, u64))>> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| AppError::new(format!("Invalid native loop metadata: {error}")))?;
    let devices = value
        .get("loopdevices")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::new("Native loop metadata has no device inventory."))?;
    devices
        .iter()
        .map(|entry| {
            let name = entry
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::new("Cannot identify an active loop device."))?;
            let inode = entry
                .get("back-ino")
                .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
                .ok_or_else(|| {
                    AppError::new("Cannot verify an active loop device's backing inode.")
                })?;
            let device = entry
                .get("back-maj:min")
                .and_then(|value| value.as_str())
                .ok_or_else(|| {
                    AppError::new("Cannot verify an active loop device's backing filesystem.")
                })?;
            let (major, minor) = parse_device_pair(device)?;
            Ok((name.to_string(), (major, minor, inode)))
        })
        .collect()
}

fn native_loop_sources(ctx: &mut AppContext) -> AppResult<Vec<(String, (u64, u64, u64))>> {
    let program = crate::trusted_privileged_executable(ctx.losetup_path()?)?;
    let result = ctx.capture_command(
        &program,
        &[
            "--json".into(),
            "--list".into(),
            "--output".into(),
            "NAME,BACK-INO,BACK-MAJ:MIN".into(),
        ],
        true,
        "Failed to inspect active loop identities",
    )?;
    if !result.status.success() {
        return Err(AppError::new(
            "Cannot verify native loop backing identities. Refusing an ambiguous open.",
        ));
    }
    loop_source_identities(&result.stdout)
}

fn file_source_identity(source: &File) -> AppResult<(u64, u64, u64)> {
    let metadata = source
        .metadata()
        .map_err(|error| AppError::io("Cannot inspect source identity", error))?;
    Ok((
        libc::major(metadata.dev()) as u64,
        libc::minor(metadata.dev()) as u64,
        metadata.ino(),
    ))
}

fn assert_file_not_attached(ctx: &mut AppContext, source: &File) -> AppResult<()> {
    let identity = file_source_identity(source)?;
    if native_loop_sources(ctx)?
        .iter()
        .any(|(_, backing)| *backing == identity)
    {
        return Err(AppError::new("Container is already attached to a kernel loop device. Unmount it before opening it again."));
    }
    Ok(())
}

fn attach_locked_source(
    ctx: &mut AppContext,
    source: &File,
    path: &Path,
    readonly: bool,
) -> AppResult<PathBuf> {
    crate::linux_source::verify_path_identity(source, path)?;
    let lease = crate::linux_loop::LoopLease::start(ctx, source, readonly)?;
    let device = lease.device().to_path_buf();
    ctx.set_loop_lease(lease);
    let expected = file_source_identity(source)?;
    if !native_loop_sources(ctx)?
        .iter()
        .any(|(name, backing)| Path::new(name) == device && *backing == expected)
    {
        return Err(AppError::new(
            "New source loop does not match the locked container.",
        ));
    }
    crate::linux_source::verify_path_identity(source, path)?;
    Ok(device)
}

fn verify_mapper_source(
    ctx: &mut AppContext,
    mapper: &str,
    source: &File,
    path: &Path,
) -> AppResult<()> {
    crate::linux_source::verify_path_identity(source, path)?;
    let expected = file_source_identity(source)?;
    let mapper_device = fs::metadata(mapper_device_path(mapper))
        .map_err(|error| AppError::io("Cannot inspect active mapper", error))?
        .rdev();
    let slaves = PathBuf::from(format!(
        "/sys/dev/block/{}:{}/slaves",
        libc::major(mapper_device),
        libc::minor(mapper_device)
    ));
    let entries = fs::read_dir(slaves)
        .map_err(|error| AppError::io("Cannot inspect mapper backing device", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::io("Cannot inspect mapper backing device", error))?;
    if entries.len() != 1 {
        return Err(AppError::new(
            "Cannot verify an ambiguous mapper backing device.",
        ));
    }
    let device = fs::read_to_string(entries[0].path().join("dev"))
        .map_err(|error| AppError::io("Cannot identify mapper backing device", error))?;
    let expected_device = parse_device_pair(&device)?;
    for (name, backing) in native_loop_sources(ctx)? {
        let loop_device = fs::metadata(name)
            .map_err(|error| AppError::io("Cannot inspect loop device identity", error))?
            .rdev();
        if (
            libc::major(loop_device) as u64,
            libc::minor(loop_device) as u64,
        ) == expected_device
        {
            if backing != expected {
                return Err(AppError::new(
                    "Active mapper does not match the locked container.",
                ));
            }
            crate::linux_source::verify_path_identity(source, path)?;
            return Ok(());
        }
    }
    Err(AppError::new(
        "Cannot verify the active mapper's backing file.",
    ))
}

fn assert_mapper_available(mapper_name: &str) -> AppResult<()> {
    if fs::symlink_metadata(mapper_device_path(mapper_name)).is_ok() {
        return Err(AppError::new(format!(
            "Mapper is already active: {mapper_name}"
        )));
    }
    Ok(())
}

fn validate_veracrypt_passphrase(passphrase: Option<&str>) -> AppResult<()> {
    if let Some(passphrase) = passphrase {
        if passphrase.is_empty() || passphrase.contains(['\n', '\r', '\0']) {
            return Err(AppError::new(
                "VeraCrypt passphrase must be nonempty and cannot contain NUL or line breaks.",
            ));
        }
    }
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
        if requested_type != VolumeType::Auto
            && requested_type != actual_type
            && !(requested_type == VolumeType::Luks && actual_type == VolumeType::Luks1)
        {
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
    // VeraCrypt headers are encrypted and cannot be identified without a password.
    // Identify LUKS first; otherwise authenticate as TCRYPT during open, once.
    let source = absolute_path(source_path)?;
    if is_luks_source(ctx, &source)? {
        resolve_volume_type(
            requested_type,
            Some(if requested_type == VolumeType::Luks1 {
                // Verify the on-disk version, rather than treating every LUKS header as LUKS1.
                let cryptsetup = ctx.cryptsetup_path().to_path_buf();
                let dump = ctx.capture_command(
                    &cryptsetup,
                    &["luksDump".into(), source.as_os_str().into()],
                    true,
                    "Failed to inspect LUKS version",
                )?;
                if !dump.status.success()
                    || !String::from_utf8_lossy(&dump.stdout).lines().any(|line| {
                        line.trim()
                            .strip_prefix("Version:")
                            .is_some_and(|v| v.trim() == "1")
                    })
                {
                    return Err(AppError::new("Source is not a LUKS1 container."));
                }
                VolumeType::Luks1
            } else {
                VolumeType::Luks
            }),
            source_path,
            "source",
            false,
        )
    } else if matches!(requested_type, VolumeType::Luks | VolumeType::Luks1) {
        Err(AppError::new("Source is not a LUKS container."))
    } else {
        Ok(VolumeType::Veracrypt)
    }
}

fn is_luks_source(ctx: &mut AppContext, source: &Path) -> AppResult<bool> {
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    command_success(
        ctx,
        &cryptsetup,
        &[OsString::from("isLuks"), source.as_os_str().to_os_string()],
        true,
        "Failed to inspect source with cryptsetup isLuks",
    )
}

fn detect_luks_cipher_profile(
    ctx: &mut AppContext,
    source: &Path,
) -> AppResult<Option<&'static CreateCipherProfile>> {
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    let output = ctx.capture_command(
        &cryptsetup,
        &[
            OsString::from("luksDump"),
            source.as_os_str().to_os_string(),
        ],
        true,
        "Failed to inspect LUKS header",
    )?;
    if !output.status.success() {
        return Ok(None);
    }

    let header = String::from_utf8_lossy(&output.stdout);
    if header.lines().any(|line| {
        line.trim()
            .strip_prefix("Version:")
            .is_some_and(|version| version.trim() == "1")
    }) {
        return Ok(None); // Kernel performance tuning is only qualified for the LUKS2 profiles.
    }
    for line in header.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed
            .strip_prefix("Cipher:")
            .or_else(|| trimmed.strip_prefix("cipher:"))
        {
            if let Some(cipher) = parse_luks_cipher_name(value) {
                return Ok(Some(cipher_profile(cipher)));
            }
        }
    }

    Ok(None)
}

fn parse_luks_cipher_name(value: &str) -> Option<CreateCipher> {
    let value = value.trim().to_ascii_lowercase();
    if value.starts_with("aes") {
        Some(CreateCipher::Aes)
    } else if value.starts_with("serpent") {
        Some(CreateCipher::Serpent)
    } else if value.starts_with("twofish") {
        Some(CreateCipher::Twofish)
    } else {
        None
    }
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
                return Ok(Some(if value == "luks1" {
                    VolumeType::Luks1
                } else {
                    VolumeType::Luks
                }));
            }
            if value.starts_with("tcrypt") || value.starts_with("veracrypt") {
                return Ok(Some(VolumeType::Veracrypt));
            }
        }
    }
    Ok(None)
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

fn mount_open_mapper(
    ctx: &mut AppContext,
    mapper_name: &str,
    mountpoint: &Path,
    readonly: bool,
) -> AppResult<()> {
    let mapper_path = mapper_device_path(mapper_name);
    let filesystem_type = detect_filesystem_type(ctx, &mapper_path)?;
    if filesystem_type.as_deref() == Some("zfs_member") {
        return mount_zfs(ctx, mapper_name, mountpoint, readonly);
    }
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
    if filesystem_type.as_deref() == Some("hammer2")
        && readonly
        && !Path::new("/sys/module/hammer2").exists()
    {
        if let Ok(driver) = crate::trusted_system_executable("hammer2-fuse") {
            return mount_fuse_filesystem(ctx, &driver, &mapper_path, mountpoint, true);
        }
    }
    if filesystem_type.as_deref() == Some("apfs")
        && readonly
        && !Path::new("/sys/module/apfs").exists()
    {
        for name in ["apfs-fuse", "fsapfsmount"] {
            if let Ok(driver) = crate::trusted_system_executable(name) {
                return mount_fuse_filesystem(ctx, &driver, &mapper_path, mountpoint, true);
            }
        }
    }
    let mut args = Vec::new();
    // A distro may also ship a read-only mount.apfs helper. Request the
    // selected native kernel adapter explicitly instead of invoking it.
    if filesystem_type.as_deref() == Some("apfs") {
        args.extend(["-i".into(), "-t".into(), "apfs".into()]);
    }
    let mut options: Vec<String> = Vec::new();
    if filesystem_type
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("btrfs"))
        .unwrap_or(false)
    {
        options.push("compress=zstd".into());
    }
    if matches!(filesystem_type.as_deref(), Some("ntfs" | "exfat" | "vfat")) {
        let owner = crate::new_filesystem::invoking_owner()?;
        options.extend([
            format!("uid={}", owner.uid),
            format!("gid={}", owner.gid),
            "umask=0077".into(),
        ]);
    }
    if filesystem_type.as_deref() == Some("ntfs") {
        // The installed NTFS-3G helper is the portable Linux read/write route.
        args.extend([OsString::from("-t"), OsString::from("ntfs-3g")]);
    }
    if filesystem_type.as_deref() == Some("apfs") && !readonly {
        // The separately installed upstream Linux APFS module requires this
        // explicit opt-in to its experimental writable implementation.
        options.push("readwrite".into());
    }
    if filesystem_type.as_deref() == Some("ufs") {
        args.extend(["-i".into(), "-t".into(), "ufs".into()]);
        let blkid = ctx
            .blkid_path()
            .ok_or_else(|| AppError::new("blkid is required for Linux UFS variant detection."))?
            .to_path_buf();
        let probe = ctx.capture_command(
            &blkid,
            &[
                "-p".into(),
                "-o".into(),
                "value".into(),
                "-s".into(),
                "VERSION".into(),
                "--".into(),
                mapper_path.clone().into_os_string(),
            ],
            true,
            "Inspect UFS variant",
        )?;
        match trim_stdout(&probe).as_str() {
            "1" => options.push("ufstype=44bsd".into()),
            "2" => {
                if let Ok(driver) = crate::trusted_system_executable("fuse-ufs") {
                    return mount_fuse_filesystem(ctx, &driver, &mapper_path, mountpoint, readonly);
                }
                options.push("ufstype=ufs2".into());
            }
            _ => {
                return Err(AppError::new(
                    "Cannot identify a supported UFS1/UFS2 variant; mount refused.",
                ))
            }
        }
    }
    if filesystem_type.as_deref() == Some("ufs") && !readonly {
        return Err(AppError::new(
            "Linux UFS imports require --readonly; UFS writes are not qualified.",
        ));
    }
    if readonly {
        options.push("ro".into());
    }
    if !options.is_empty() {
        args.push(OsString::from("-o"));
        args.push(OsString::from(options.join(",")));
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
    if !output.status.success() {
        return Err(AppError::new("lsblk failed to inspect block device."));
    }
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
    if !output.status.success() {
        return Err(AppError::new("lsblk failed to inspect block device."));
    }
    Ok(trim_stdout(&output))
}

fn block_device_tree_has_holders(ctx: &mut AppContext, device_path: &Path) -> AppResult<bool> {
    let lsblk = ctx.lsblk_path()?.to_path_buf();
    let output = ctx.capture_command(
        &lsblk,
        &[
            OsString::from("-nrpo"),
            OsString::from("NAME"),
            OsString::from("--"),
            device_path.as_os_str().to_os_string(),
        ],
        false,
        "Failed to inspect block device holders",
    )?;
    if !output.status.success() {
        return Err(AppError::new(
            "lsblk failed to inspect block device holders.",
        ));
    }
    for device in String::from_utf8_lossy(&output.stdout).lines() {
        let canonical = fs::canonicalize(device.trim())
            .map_err(|err| AppError::io("Failed to resolve block device", err))?;
        let name = canonical
            .file_name()
            .ok_or_else(|| AppError::new("Invalid block device path."))?;
        let holders = Path::new("/sys/class/block").join(name).join("holders");
        let mut entries = fs::read_dir(holders)
            .map_err(|err| AppError::io("Failed to inspect block device holders", err))?;
        if entries
            .next()
            .transpose()
            .map_err(|err| AppError::io("Failed to inspect holders", err))?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
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
    if !output.status.success() {
        return Err(AppError::new("lsblk failed to inspect block device tree."));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| !line.trim().is_empty()))
}

fn luks_format_arguments(profile: &CreateCipherProfile, luks1: bool) -> Vec<OsString> {
    if !luks1 {
        return profile.format_args.iter().map(OsString::from).collect();
    }
    let cipher = profile
        .format_args
        .windows(2)
        .find(|p| p[0] == "-c")
        .unwrap()[1];
    let time = profile
        .format_args
        .windows(2)
        .find(|p| p[0] == "-i")
        .unwrap()[1];
    [
        "--type",
        "luks1",
        "-c",
        cipher,
        "-s",
        "512",
        "--pbkdf",
        "pbkdf2",
        "--hash",
        "sha512",
        "-i",
        time,
        "--use-urandom",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn luks_format(
    ctx: &mut AppContext,
    target: &Path,
    passphrase: Option<&str>,
    keyfile: Option<&Path>,
    profile: &CreateCipherProfile,
    luks1: bool,
) -> AppResult<()> {
    let privileged = is_block_device_target(target);
    let mut args = vec![OsString::from("-v")];
    args.extend(luks_format_arguments(profile, luks1));
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    if let Some(keyfile) = keyfile {
        args.extend([
            OsString::from("--batch-mode"),
            OsString::from("luksFormat"),
            target.as_os_str().into(),
            fs::canonicalize(keyfile)
                .map_err(|e| AppError::io("Failed to resolve keyfile", e))?
                .into_os_string(),
        ]);
        ctx.run_command(
            &cryptsetup,
            &args,
            privileged,
            false,
            "Failed to format LUKS container",
        )
    } else if let Some(passphrase) = passphrase {
        args.push(OsString::from("--batch-mode"));
        args.push(OsString::from("luksFormat"));
        args.push(target.as_os_str().to_os_string());
        args.push(OsString::from("-"));
        ctx.run_command_with_input(
            &cryptsetup,
            &args,
            privileged,
            passphrase.as_bytes(),
            "Failed to format LUKS container",
        )
    } else {
        // Lurker has already checked staged-file publication or explicit
        // destructive --force. Suppress the duplicate native YES question,
        // while explicitly retaining both passphrase entries in batch mode.
        args.push(OsString::from("--batch-mode"));
        args.push(OsString::from("--verify-passphrase"));
        args.push(OsString::from("luksFormat"));
        args.push(target.as_os_str().to_os_string());
        ctx.run_command(
            &cryptsetup,
            &args,
            privileged,
            true,
            "Failed to format LUKS container",
        )
    }
}

fn luks_open(
    ctx: &mut AppContext,
    source: &Path,
    mapper_name: &str,
    passphrase: Option<&str>,
    keyfile: Option<&Path>,
    profile: Option<&CreateCipherProfile>,
    readonly: bool,
) -> AppResult<()> {
    let mut args = vec![
        OsString::from("open"),
        OsString::from("--type"),
        OsString::from("luks"),
    ];
    if readonly {
        args.push(OsString::from("--readonly"));
    }
    if let Some(profile) = profile {
        args.extend(profile.open_args.iter().map(OsString::from));
        if !profile.open_args.is_empty() {
            ctx.output.msg2(&format!(
                "Applying {} open performance flags",
                profile.label
            ));
        }
    }
    if let Some(keyfile) = keyfile {
        args.push("--key-file".into());
        args.push(
            fs::canonicalize(keyfile)
                .map_err(|e| AppError::io("Failed to resolve keyfile", e))?
                .into_os_string(),
        );
    } else if passphrase.is_some() {
        args.push("--key-file".into());
        args.push("-".into());
    }
    args.push(source.as_os_str().to_os_string());
    args.push(OsString::from(mapper_name));
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    if let Some(passphrase) = passphrase {
        ctx.run_command_with_input(
            &cryptsetup,
            &args,
            true,
            passphrase.as_bytes(),
            "Failed to open LUKS mapper",
        )
    } else {
        ctx.run_command(&cryptsetup, &args, true, true, "Failed to open LUKS mapper")
    }
}

fn crypt_close(ctx: &mut AppContext, mapper_name: &str) -> AppResult<()> {
    let args = vec![OsString::from("close"), OsString::from(mapper_name)];
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    ctx.run_command(&cryptsetup, &args, true, false, "Failed to close mapper")
}

fn tcrypt_open(
    ctx: &mut AppContext,
    source: &Path,
    mapper_name: &str,
    passphrase: Option<&str>,
    keyfile: Option<&Path>,
    readonly: bool,
) -> AppResult<()> {
    validate_veracrypt_passphrase(passphrase)?;
    let passphrase = if keyfile.is_some() {
        Some("")
    } else {
        passphrase
    };
    let mut args = vec![
        OsString::from("open"),
        OsString::from("--type"),
        OsString::from("tcrypt"),
        OsString::from("--veracrypt"),
    ];
    if readonly {
        args.push(OsString::from("--readonly"));
    }
    if let Some(keyfile) = keyfile {
        args.push("--key-file".into());
        args.push(
            fs::canonicalize(keyfile)
                .map_err(|e| AppError::io("Failed to resolve keyfile", e))?
                .into_os_string(),
        );
    }
    // TCRYPT --key-file mixes a keyfile with the password. Read the password
    // directly from stdin instead; it is line based in this mode.
    if passphrase.is_some() {
        args.push(OsString::from("--batch-mode"));
    }
    args.extend([
        source.as_os_str().to_os_string(),
        OsString::from(mapper_name),
    ]);
    let cryptsetup = ctx.cryptsetup_path().to_path_buf();
    if let Some(passphrase) = passphrase {
        ctx.run_command_with_input(
            &cryptsetup,
            &args,
            true,
            if keyfile.is_some() {
                b"\n"
            } else {
                passphrase.as_bytes()
            },
            "Failed to open VeraCrypt mapper with cryptsetup",
        )
    } else {
        ctx.run_command(
            &cryptsetup,
            &args,
            true,
            true,
            "Failed to open VeraCrypt mapper with cryptsetup",
        )
    }
}

fn veracrypt_create(
    ctx: &mut AppContext,
    source_path: &Path,
    target_kind: SourceKind,
    size_mib: Option<u64>,
    profile: &CreateCipherProfile,
    passphrase: Option<&str>,
    keyfile: Option<&Path>,
) -> AppResult<()> {
    let passphrase = if keyfile.is_some() {
        Some("")
    } else {
        passphrase
    };
    let mut args: Vec<OsString> = VERACRYPT_CREATE_ARGS_COMMON
        .iter()
        .map(OsString::from)
        .collect();
    args.push(OsString::from(format!(
        "--encryption={}",
        profile.veracrypt_encryption
    )));
    if passphrase.is_some() {
        args.push(OsString::from("--stdin"));
        args.push(OsString::from("--non-interactive"));
    }
    args.push("-k".into());
    args.push(crate::credentials::veracrypt_keyfiles(keyfile)?);
    match target_kind {
        SourceKind::Directory => {
            return Err(AppError::new(
                "VeraCrypt cannot create a directory backend.",
            ))
        }
        SourceKind::File => {
            let size_mib = size_mib.ok_or_else(|| AppError::new("Missing file container size."))?;
            args.push(OsString::from(format!("--size={}M", size_mib)));
        }
        SourceKind::Block => {
            args.extend(VERACRYPT_CREATE_ARGS_BLOCK.iter().map(OsString::from));
        }
    }
    args.push(source_path.as_os_str().to_os_string());
    let veracrypt = ctx.veracrypt_path()?.to_path_buf();
    if let Some(passphrase) = passphrase {
        ctx.run_command_with_input(
            &veracrypt,
            &args,
            target_kind == SourceKind::Block,
            if keyfile.is_some() {
                b"\n"
            } else {
                passphrase.as_bytes()
            },
            "Failed to create VeraCrypt container",
        )
    } else {
        ctx.run_command(
            &veracrypt,
            &args,
            target_kind == SourceKind::Block,
            true,
            "Failed to create VeraCrypt container",
        )
    }
}

fn run_formatter(
    ctx: &mut AppContext,
    program: &Path,
    args: &[OsString],
    context: &str,
) -> AppResult<()> {
    // Native tools must not mix progress/version text into helper JSON stdout.
    let output = ctx.capture_command(program, args, true, context)?;
    if !output.status.success() {
        let detail = if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        };
        return Err(AppError::new(format!(
            "{context}: {}",
            String::from_utf8_lossy(detail)
                .trim()
                .chars()
                .take(8192)
                .collect::<String>()
        )));
    }
    Ok(())
}

fn format_filesystem(
    ctx: &mut AppContext,
    device_path: &Path,
    filesystem: Option<CreateFilesystem>,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    let filesystem = filesystem.unwrap_or(CreateFilesystem::Btrfs);
    ctx.output
        .msg2(&format!("Creating {} filesystem", filesystem.as_str()));
    if filesystem == CreateFilesystem::Zfs {
        return create_zfs(ctx, device_path, owner);
    }
    if filesystem == CreateFilesystem::Ufs2 {
        let seed = EmptyFilesystemRoot::new(owner)?;
        let blockdev = crate::trusted_system_executable("blockdev")?;
        let size = ctx.capture_command(
            &blockdev,
            &["--getsize64".into(), device_path.as_os_str().to_os_string()],
            true,
            "Inspect new UFS2 device size",
        )?;
        let bytes = trim_stdout(&size)
            .parse::<u64>()
            .map_err(|_| AppError::new("Cannot determine new UFS2 device size."))?;
        let formatter = ctx.formatter_path()?.to_path_buf();
        run_formatter(
            ctx,
            &formatter,
            &[
                "-t".into(),
                "ffs".into(),
                "-B".into(),
                "little".into(),
                "-o".into(),
                "version=2,bsize=32768,fsize=4096".into(),
                "-s".into(),
                bytes.to_string().into(),
                device_path.as_os_str().to_os_string(),
                seed.descriptor_path().into_os_string(),
            ],
            "Create new UFS2 filesystem",
        )?;
        // Validate the actual FreeBSD layout through its installed driver before
        // publishing a newly encrypted container, and initialize its private root.
        return initialize_native_root(ctx, device_path.as_os_str(), "ufs2", None, owner);
    }
    let mut args = crate::system::linux_format_args(filesystem, owner);
    // Seed/prototype initialization sets ownership only for newly created roots;
    // ordinary unlock never alters an existing filesystem's metadata.
    let seed = if matches!(filesystem, CreateFilesystem::Btrfs | CreateFilesystem::Xfs) {
        Some(EmptyFilesystemRoot::new(owner)?)
    } else {
        None
    };
    if filesystem == CreateFilesystem::Btrfs {
        args.extend([
            OsString::from("--rootdir"),
            seed.as_ref().unwrap().descriptor_path().into_os_string(),
        ]);
    }
    if filesystem == CreateFilesystem::Xfs {
        let prototype = seed.as_ref().unwrap().parent.join("prototype");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&prototype)
            .map_err(|e| AppError::io("Create private XFS prototype", e))?;
        writeln!(
            file,
            "/dev/null\n0 0\nd--700 {} {}\n$",
            owner.uid, owner.gid
        )
        .map_err(|e| AppError::io("Write private XFS prototype", e))?;
        args.extend([OsString::from("-p"), prototype.into_os_string()]);
    }
    args.push(device_path.as_os_str().to_os_string());
    let formatter = ctx.formatter_path()?.to_path_buf();
    let result = run_formatter(ctx, &formatter, &args, "Failed to create filesystem");
    if let Some(seed) = seed.as_ref() {
        let _ = fs::remove_file(seed.parent.join("prototype"));
    }
    result?;
    match filesystem {
        CreateFilesystem::Apfs => initialize_native_root(
            ctx,
            device_path.as_os_str(),
            "apfs",
            Some("readwrite"),
            owner,
        )?,
        CreateFilesystem::Hammer2 => {
            initialize_native_root(ctx, device_path.as_os_str(), "hammer2", None, owner)?
        }
        CreateFilesystem::Ext4 => {
            // e2fsprogs 1.47.0 lacks root_perms. Initialize through the private
            // mount so supported distribution tools still produce mode 0700.
            initialize_native_root(ctx, device_path.as_os_str(), "ext4", None, owner)?
        }
        _ => {}
    }
    Ok(())
}

// Native FUSE imports retain the same mapper/source and mount identity guards.
// fsname binds the mount table back to the exact mapper rather than a generic
// driver label, so ordinary unmount resolution cannot select another device.
fn mount_fuse_filesystem(
    ctx: &mut AppContext,
    driver: &Path,
    mapper: &Path,
    mountpoint: &Path,
    readonly: bool,
) -> AppResult<()> {
    let mut options = format!(
        "{},allow_other,fsname={}",
        if readonly { "ro" } else { "rw" },
        mapper.display()
    );
    let mut args: Vec<OsString>;
    if driver
        .file_name()
        .is_some_and(|name| name == "hammer2-fuse")
    {
        if !readonly {
            return Err(AppError::new(
                "hammer2-fuse supports read-only imports only.",
            ));
        }
        // This driver always sets RO and uses the exact source as FSName.
        args = vec!["--allow_other".into()];
    } else if driver.file_name().is_some_and(|name| name == "fsapfsmount") {
        if !readonly {
            return Err(AppError::new(
                "fsapfsmount supports read-only APFS imports only.",
            ));
        }
        // This forensic reader synthesizes ownership; keep its view private to
        // the authorized caller without changing any existing APFS metadata.
        let owner = crate::new_filesystem::invoking_owner()?;
        options.push_str(&format!(
            ",default_permissions,uid={},gid={},umask=0077",
            owner.uid, owner.gid
        ));
        args = vec!["-f".into(), "1".into(), "-X".into(), options.into()];
    } else {
        args = vec!["-o".into(), options.into()];
    }
    args.extend([
        mapper.as_os_str().to_os_string(),
        mountpoint.as_os_str().to_os_string(),
    ]);
    let output = ctx.capture_command(driver, &args, true, "Start native filesystem driver")?;
    if !output.status.success() {
        let detail = if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        };
        return Err(AppError::new(format!(
            "Native filesystem driver failed: {}",
            String::from_utf8_lossy(detail)
                .trim()
                .chars()
                .take(8192)
                .collect::<String>()
        )));
    }
    if !output.stderr.is_empty() {
        ctx.output
            .warn(String::from_utf8_lossy(&output.stderr).trim());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(mount) = read_mountinfo()?
            .into_iter()
            .find(|m| m.mount_point == mountpoint)
        {
            if mount
                .source
                .as_deref()
                .map(crate::linux::canonical_device_path)
                != Some(crate::linux::canonical_device_path(mapper))
                || (readonly && !mount.readonly)
            {
                return Err(AppError::new("Native filesystem driver published an unexpected mount identity; preserving it."));
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(AppError::new(
                "Native filesystem driver did not publish its mount.",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

// ZFS is an inner filesystem here: encryption remains the surrounding mapper.
// Every pool operation is restricted to one verified mapper; there is no broad
// import, forced export, persistent cache, or recursive dataset mounting.
fn zfs_command(
    ctx: &mut AppContext,
    args: &[OsString],
    capture: bool,
) -> AppResult<std::process::Output> {
    let program = crate::trusted_system_executable("zpool")?;
    let output = ctx.capture_command(&program, args, true, "OpenZFS operation failed")?;
    if !output.status.success() && !capture {
        return Err(AppError::new(format!(
            "OpenZFS operation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output)
}

fn verify_zfs_mapper(ctx: &mut AppContext, pool: &str) -> AppResult<()> {
    let output = zfs_command(ctx, &["status".into(), "-LP".into(), pool.into()], false)?;
    let text = String::from_utf8_lossy(&output.stdout);
    let leaves: Vec<_> = text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|token| token.starts_with('/'))
        .collect();
    let mapper = fs::canonicalize(mapper_device_path(pool))
        .map_err(|e| AppError::io("Inspect ZFS mapper identity", e))?;
    if leaves.len() != 1 || fs::canonicalize(leaves[0]).ok().as_ref() != Some(&mapper) {
        return Err(AppError::new(
            "ZFS pool does not contain exactly its expected Lurker mapper; preserving it.",
        ));
    }
    Ok(())
}

fn zfs_export_mounts_safe(
    pool: &str,
    mounts: &[crate::linux::MountInfo],
    expected: Option<&Path>,
) -> bool {
    let mut root_count = 0;
    for mount in mounts.iter().filter(|m| m.fs_type == "zfs") {
        let Some(source) = mount.source.as_deref() else {
            continue;
        };
        let source = source.as_os_str().to_string_lossy();
        if source == pool {
            root_count += 1;
            if expected != Some(mount.mount_point.as_path()) || root_count > 1 {
                return false;
            }
        } else if source.starts_with(&format!("{pool}/")) || source.starts_with(&format!("{pool}@"))
        {
            return false;
        }
    }
    true
}

fn export_zfs(ctx: &mut AppContext, pool: &str, expected: Option<&Path>) -> AppResult<()> {
    verify_zfs_mapper(ctx, pool)?;
    if !zfs_export_mounts_safe(pool, &read_mountinfo()?, expected) {
        return Err(AppError::new("ZFS has an unrecorded dataset, snapshot or additional root mount; close it explicitly before unmounting this vault."));
    }
    zfs_command(ctx, &["export".into(), pool.into()], false)?;
    Ok(())
}

fn single_zfs_leaf_labels(text: &str) -> bool {
    let mut names = 0;
    let mut children = 0;
    let mut disks = 0;
    for line in text.lines().map(str::trim) {
        if line.starts_with("name:") {
            names += 1;
        }
        if let Some(value) = line.strip_prefix("vdev_children:") {
            if value.trim() != "1" {
                return false;
            }
            children += 1;
        }
        if let Some(value) = line.strip_prefix("type:") {
            if value.trim() != "'disk'" {
                return false;
            }
            disks += 1;
        }
        for key in ["children", "spares", "l2cache"] {
            if line
                .strip_prefix(key)
                .is_some_and(|rest| rest.starts_with('[') || rest.starts_with(':'))
            {
                return false;
            }
        }
    }
    names > 0 && names == children && names == disks
}

fn mount_zfs(ctx: &mut AppContext, pool: &str, mountpoint: &Path, readonly: bool) -> AppResult<()> {
    let existing = zfs_command(
        ctx,
        &[
            "list".into(),
            "-H".into(),
            "-o".into(),
            "name".into(),
            pool.into(),
        ],
        true,
    )?;
    if existing.status.success() {
        return Err(AppError::new(
            "The requested ZFS pool name is already imported; refusing to reuse it.",
        ));
    }
    let mapper = mapper_device_path(pool);
    let zdb = crate::trusted_system_executable("zdb")?;
    let labels = ctx.capture_command(
        &zdb,
        &["-l".into(), mapper.clone().into_os_string()],
        true,
        "Inspect inner ZFS labels",
    )?;
    if !labels.status.success() || !single_zfs_leaf_labels(&String::from_utf8_lossy(&labels.stdout))
    {
        return Err(AppError::new("ZFS import requires healthy labels for exactly one disk vdev, without child, spare or cache devices; import refused before attaching a pool."));
    }
    let discovered = zfs_command(
        ctx,
        &[
            "import".into(),
            "-d".into(),
            mapper.clone().into_os_string(),
        ],
        true,
    )?;
    let text = String::from_utf8_lossy(&discovered.stdout);
    let ids: Vec<_> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("id:"))
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .collect();
    if ids.len() != 1 {
        return Err(AppError::new(
            "Expected exactly one exported ZFS pool on this mapper; import refused.",
        ));
    }
    let mut args: Vec<OsString> = ["import", "-N", "-d"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.extend([
        mapper.into_os_string(),
        "-o".into(),
        "cachefile=none".into(),
    ]);
    if readonly {
        args.extend(["-o".into(), "readonly=on".into()]);
    }
    args.extend([ids[0].into(), pool.into()]);
    zfs_command(ctx, &args, false)?;
    verify_zfs_mapper(ctx, pool)?;
    let mut args = vec!["-t".into(), "zfs".into()];
    if readonly {
        args.extend(["-o".into(), "ro".into()]);
    }
    args.extend([pool.into(), mountpoint.as_os_str().to_os_string()]);
    let mount = crate::trusted_system_executable("mount")?;
    let result = ctx.run_command(
        &mount,
        &args,
        true,
        false,
        "Failed to mount inner ZFS filesystem",
    );
    if result.is_err() {
        export_zfs(ctx, pool, None)?;
    }
    result
}

fn create_zfs(
    ctx: &mut AppContext,
    device: &Path,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    let pool = device
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| s.starts_with("lurker_"))
        .ok_or_else(|| AppError::new("ZFS creation requires a Lurker mapper."))?;
    zfs_command(
        ctx,
        &[
            "create".into(),
            "-m".into(),
            "legacy".into(),
            "-o".into(),
            "cachefile=none".into(),
            "-O".into(),
            "canmount=noauto".into(),
            pool.into(),
            device.as_os_str().to_os_string(),
        ],
        false,
    )?;
    verify_zfs_mapper(ctx, pool)?;
    let initialized = initialize_native_root(ctx, std::ffi::OsStr::new(pool), "zfs", None, owner);
    export_zfs(ctx, pool, None)?;
    initialized
}

fn descriptor_mount_id(file: &File) -> AppResult<u64> {
    let info = fs::read_to_string(format!("/proc/self/fdinfo/{}", file.as_raw_fd()))
        .map_err(|e| AppError::io("Inspect initialization directory mount identity", e))?;
    info.lines()
        .find_map(|line| {
            line.strip_prefix("mnt_id:")
                .and_then(|id| id.trim().parse().ok())
        })
        .ok_or_else(|| AppError::new("Initialization directory has no verified mount identity."))
}

fn initialize_native_root(
    ctx: &mut AppContext,
    source: &std::ffi::OsStr,
    filesystem: &str,
    options: Option<&str>,
    owner: crate::new_filesystem::Owner,
) -> AppResult<()> {
    let seed = EmptyFilesystemRoot::new(owner)?;
    if filesystem == "ufs2" {
        let driver = crate::trusted_system_executable("fuse-ufs")?;
        mount_fuse_filesystem(ctx, &driver, Path::new(source), &seed.path, false)?;
    } else {
        let mount = crate::trusted_system_executable("mount")?;
        let mut args: Vec<OsString> = vec!["-t".into(), filesystem.into()];
        if filesystem == "apfs" {
            args.insert(0, "-i".into());
        }
        if let Some(options) = options {
            args.extend(["-o".into(), options.into()]);
        }
        args.extend([source.to_os_string(), seed.path.clone().into_os_string()]);
        ctx.run_command(&mount, &args, true, false, "Initialize new filesystem root")?;
    }
    let mounted = read_mountinfo()?
        .into_iter()
        .find(|m| m.mount_point == seed.path)
        .ok_or_else(|| {
            AppError::new("Initialization mount is missing; refusing ownership changes.")
        })?;
    let initialized = (|| {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&seed.path)
            .map_err(|e| AppError::io("Open new filesystem root", e))?;
        if descriptor_mount_id(&directory)? != mounted.mount_id {
            return Err(AppError::new(
                "Initialization directory is covered by another mount; refusing ownership changes.",
            ));
        }
        let descriptor = OsString::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            directory.as_raw_fd()
        ));
        let metadata = directory
            .metadata()
            .map_err(|e| AppError::io("Inspect new filesystem root", e))?;
        if metadata.uid() != owner.uid || metadata.gid() != owner.gid {
            let chown = crate::trusted_system_executable("chown")?;
            ctx.run_command(
                &chown,
                &[
                    format!("{}:{}", owner.uid, owner.gid).into(),
                    descriptor.clone(),
                ],
                true,
                false,
                "Assign new filesystem root owner",
            )?;
        }
        if metadata.mode() & 0o777 != 0o700 {
            let chmod = crate::trusted_system_executable("chmod")?;
            ctx.run_command(
                &chmod,
                &["0700".into(), descriptor],
                true,
                false,
                "Protect new filesystem root",
            )?;
        }
        Ok(())
    })();
    if !read_mountinfo()?
        .iter()
        .any(|m| m.mount_id == mounted.mount_id && m.mount_point == seed.path)
    {
        return Err(AppError::new(
            "Initialization mount identity changed; preserving it.",
        ));
    }
    let visible = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&seed.path)
        .map_err(|e| AppError::io("Recheck initialization mount", e))?;
    if descriptor_mount_id(&visible)? != mounted.mount_id {
        return Err(AppError::new(
            "Initialization mount is covered or replaced; preserving it.",
        ));
    }
    drop(visible);
    let umount = crate::trusted_system_executable("umount")?;
    ctx.run_command(
        &umount,
        &[seed.path.clone().into_os_string()],
        true,
        false,
        "Close new filesystem initialization mount",
    )?;
    initialized
}

fn assign_new_image_owner(path: &Path, owner: crate::new_filesystem::Owner) -> AppResult<()> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| AppError::io("Open newly created container for ownership", e))?;
    crate::linux_source::verify_path_identity(&file, path)?;
    if file
        .metadata()
        .map_err(|e| AppError::io("Inspect new container", e))?
        .nlink()
        != 1
    {
        return Err(AppError::new(
            "New container acquired an unexpected hard link before publication.",
        ));
    }
    crate::new_filesystem::assign_file(&file, owner)?;
    crate::linux_source::verify_path_identity(&file, path)
}

struct EmptyFilesystemRoot {
    parent: PathBuf,
    path: PathBuf,
    directory: File,
}

impl EmptyFilesystemRoot {
    fn new(owner: crate::new_filesystem::Owner) -> AppResult<Self> {
        let mut random = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut random))
            .map_err(|e| AppError::io("Generate private filesystem seed name", e))?;
        let nonce: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let parent = std::env::temp_dir().join(format!("lurker-new-filesystem-{nonce}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&parent)
            .map_err(|e| AppError::io("Create private filesystem seed parent", e))?;
        let path = parent.join("root");
        let prepared = (|| {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .map_err(|e| AppError::io("Create empty filesystem root seed", e))?;
            let directory = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&path)
                .map_err(|e| AppError::io("Open new filesystem root seed", e))?;
            crate::new_filesystem::assign_root(&directory, owner)?;
            Ok(directory)
        })();
        match prepared {
            Ok(directory) => Ok(Self {
                parent,
                path,
                directory,
            }),
            Err(error) => {
                let _ = fs::remove_dir(&path);
                let _ = fs::remove_dir(&parent);
                Err(error)
            }
        }
    }

    fn descriptor_path(&self) -> PathBuf {
        // sudo may close inherited descriptors; the parent keeps this exact
        // directory open while the privileged formatter reads its proc path.
        format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.directory.as_raw_fd()
        )
        .into()
    }
}

impl Drop for EmptyFilesystemRoot {
    fn drop(&mut self) {
        // Never recursively remove data if an unexpected entry appeared.
        let _ = fs::remove_dir(&self.path);
        let _ = fs::remove_dir(&self.parent);
    }
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
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
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
    mapper_name: String,
    output: Output,
    persist: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf, mapper_name: &str, output: Output) -> Self {
        Self {
            path,
            mapper_name: mapper_name.into(),
            output,
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
            // A failed ordinary initialization unmount may leave its mapper
            // active. Keep its backing inode recoverable instead of unlinking
            // it before AppContext attempts its normal mapper cleanup.
            if mapper_device_path(&self.mapper_name).exists() {
                self.output.warn(&format!(
                    "Staged container retained at {} while mapper {} remains open.",
                    self.path.display(),
                    self.mapper_name
                ));
                return;
            }
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zfs_labels_require_one_disk_without_auxiliary_or_missing_topology() {
        let valid = "name: 'any_pool'\nvdev_children: 1\nvdev_tree:\n type: 'disk'\n path: '/dev/mapper/lurker_test'\n";
        assert!(single_zfs_leaf_labels(valid));
        assert!(single_zfs_leaf_labels(&valid.repeat(4)));
        assert!(!single_zfs_leaf_labels(
            &valid.replace("vdev_children: 1", "vdev_children: 2")
        ));
        assert!(!single_zfs_leaf_labels(
            &valid.replace("'disk'", "'mirror'")
        ));
        assert!(!single_zfs_leaf_labels(
            &valid.replace("vdev_children: 1\n", "")
        ));
        for extra in ["children[0]:", "spares:", "l2cache[0]:"] {
            assert!(!single_zfs_leaf_labels(&format!("{valid}{extra}\n")));
        }
    }

    #[test]
    fn zfs_export_refuses_unrecorded_children_snapshots_and_root_aliases() {
        let mount = |source: &str, target: &str| crate::linux::MountInfo {
            mount_id: 1,
            readonly: false,
            mount_point: target.into(),
            source: Some(source.into()),
            fs_type: "zfs".into(),
        };
        let pool = "lurker_test";
        let root = Path::new("/private/vault");
        let active = mount(pool, "/private/vault");
        assert!(zfs_export_mounts_safe(pool, &[], None));
        assert!(zfs_export_mounts_safe(pool, &[active.clone()], Some(root)));
        assert!(!zfs_export_mounts_safe(pool, &[active.clone()], None));
        for extra in [
            mount("lurker_test/child", "/elsewhere"),
            mount(
                "lurker_test@snapshot",
                "/private/vault/.zfs/snapshot/snapshot",
            ),
            mount(pool, "/private/alias"),
        ] {
            assert!(!zfs_export_mounts_safe(
                pool,
                &[active.clone(), extra],
                Some(root)
            ));
        }
        assert!(zfs_export_mounts_safe(
            pool,
            &[active, mount("lurker_test_other/child", "/other")],
            Some(root)
        ));
    }

    #[test]
    fn new_filesystem_seed_is_private_and_pinned_after_rename() {
        let owner = crate::new_filesystem::Owner {
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
        };
        let seed = EmptyFilesystemRoot::new(owner).unwrap();
        let parent = seed.parent.clone();
        let renamed = parent.with_extension("renamed");
        let original = seed.directory.metadata().unwrap();
        assert_eq!(original.uid(), owner.uid);
        assert_eq!(original.gid(), owner.gid);
        assert_eq!(original.mode() & 0o777, 0o700);
        fs::rename(&parent, &renamed).unwrap();
        let pinned = fs::metadata(seed.descriptor_path()).unwrap();
        assert_eq!(
            (pinned.dev(), pinned.ino()),
            (original.dev(), original.ino())
        );
        assert_eq!(fs::read_dir(seed.descriptor_path()).unwrap().count(), 0);
        fs::rename(&renamed, &parent).unwrap();
        drop(seed);
        assert!(!parent.exists());
    }

    #[test]
    fn source_lock_is_shared_by_hardlink_aliases() {
        let root = std::env::temp_dir().join(format!("lurker-inode-lock-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        let alias = root.join("alias");
        fs::write(&source, b"not a real container").unwrap();
        fs::hard_link(&source, &alias).unwrap();
        let guard = lock_file_source(&source).unwrap();
        assert!(lock_file_source(&alias).is_err());
        drop(guard);
        assert!(lock_file_source(&alias).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn loop_identity_uses_kernel_inode_and_device_without_filenames() {
        assert_eq!(
            loop_source_identities(
                br#"{"loopdevices":[{"name":"/dev/loop0","back-ino":7241,"back-maj:min":"0:43"}]}"#
            )
            .unwrap(),
            vec![("/dev/loop0".into(), (0, 43, 7241))]
        );
        assert!(loop_source_identities(
            br#"{"loopdevices":[{"back-ino":null,"back-maj:min":"0:43"}]}"#
        )
        .is_err());
    }

    #[test]
    fn luks1_profile_never_inherits_luks2_kdf_or_sector_arguments() {
        for cipher in [
            CreateCipher::Aes,
            CreateCipher::Serpent,
            CreateCipher::Twofish,
        ] {
            let args = luks_format_arguments(cipher_profile(cipher), true);
            let values: Vec<_> = args.iter().map(|arg| arg.to_str().unwrap()).collect();
            assert!(values.windows(2).any(|pair| pair == ["--type", "luks1"]));
            assert!(values.windows(2).any(|pair| pair == ["--pbkdf", "pbkdf2"]));
            assert!(values.windows(2).any(|pair| pair == ["--hash", "sha512"]));
            assert!(!values.iter().any(|value| value.starts_with("--pbkdf-")
                || *value == "--sector-size"
                || *value == "argon2id"));
        }
    }

    #[test]
    fn veracrypt_tags_are_rejected() {
        assert!(require_supported_veracrypt_tag(Some("work")).is_err());
        assert!(require_supported_veracrypt_tag(None).is_ok());
    }

    #[test]
    fn veracrypt_password_transport_rejects_truncation() {
        for value in ["", "first\nsecond", "first\rsecond", "first\0second"] {
            assert!(validate_veracrypt_passphrase(Some(value)).is_err());
        }
        assert!(validate_veracrypt_passphrase(Some("  пароль 密碼  ")).is_ok());
        assert!(validate_veracrypt_passphrase(None).is_ok());
    }

    #[test]
    fn luks_cipher_detection_handles_header_versions() {
        for (name, expected) in [
            (" aes-xts-plain64", CreateCipher::Aes),
            ("SERPENT", CreateCipher::Serpent),
            ("twofish-xts-plain64", CreateCipher::Twofish),
        ] {
            assert_eq!(parse_luks_cipher_name(name), Some(expected));
        }
        assert_eq!(parse_luks_cipher_name("camellia-xts-plain64"), None);
    }

    #[test]
    fn pinned_types_reject_mismatches_and_unknown_mappers() {
        let path = Path::new("vault");
        for actual in [VolumeType::Luks, VolumeType::Veracrypt] {
            assert_eq!(
                resolve_volume_type(VolumeType::Auto, Some(actual), path, "source", false).unwrap(),
                actual
            );
            assert!(resolve_volume_type(actual, Some(actual), path, "source", false).is_ok());
            let wrong = if actual == VolumeType::Luks {
                VolumeType::Veracrypt
            } else {
                VolumeType::Luks
            };
            assert!(resolve_volume_type(wrong, Some(actual), path, "source", false).is_err());
            assert!(resolve_volume_type(actual, None, path, "target", false).is_err());
        }
        assert!(resolve_volume_type(VolumeType::Auto, None, path, "target", false).is_err());
    }

    #[test]
    fn atomic_publication_never_clobbers_without_force() {
        let dir = std::env::temp_dir().join(format!("lurker-publish-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let temp = dir.join("temp");
        let target = dir.join("target");
        fs::write(&temp, "new").unwrap();
        fs::write(&target, "original").unwrap();
        assert!(publish_container(&temp, &target, false, None).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
        let original = lock_file_source(&target).unwrap();
        publish_container(&temp, &target, true, Some(&original)).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(!temp.exists());
        fs::write(&temp, "another").unwrap();
        fs::remove_file(&target).unwrap();
        publish_container(&temp, &target, false, None).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "another");
        assert!(!temp.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_target_guards_preserve_existing_content() {
        let dir = std::env::temp_dir().join(format!("lurker-target-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let file = dir.join("file");
        let link = dir.join("link");
        fs::write(&file, "original").unwrap();
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(assert_create_file_target(&file, false).is_err());
        assert!(assert_create_file_target(&file, true).is_ok());
        assert!(assert_create_file_target(&link, true).is_err());
        assert!(assert_create_file_target(&dir, true).is_err());
        assert!(assert_create_file_target(&dir.join("missing/file"), false).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "original");
        fs::remove_dir_all(dir).unwrap();
    }
}
