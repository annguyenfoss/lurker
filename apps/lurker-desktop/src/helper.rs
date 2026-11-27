use lurker_core::{
    ActiveVolume, CommandAction, NativeVolumeListResponse, OperationResponse, PrivilegeMode,
};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn resolve_helper_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("LURKER_HELPER_PATH") {
        let path = PathBuf::from(path);
        if is_executable(&path) {
            return Ok(path);
        }
        return Err(format!(
            "LURKER_HELPER_PATH does not point to an executable file: {}",
            path.display()
        ));
    }

    let current_exe = env::current_exe()
        .map_err(|err| format!("Failed to resolve desktop executable path: {err}"))?;
    if let Some(parent) = current_exe.parent() {
        let sibling = parent.join("lurker-helper");
        if is_executable(&sibling) {
            return Ok(sibling);
        }
    }

    find_executable("lurker-helper").ok_or_else(|| {
        "Failed to locate lurker-helper. Expected a sibling executable named `lurker-helper`, a PATH entry, or LURKER_HELPER_PATH.".into()
    })
}

/// BSD native sessions remain in a private root registry and are listed only
/// through the same authorization boundary as native mount/unmount operations.
pub fn list_desktop_volumes(helper_path: Option<&Path>) -> Result<Vec<ActiveVolume>, String> {
    let local = lurker_core::list_active_volumes().map_err(|error| error.message)?;
    if matches!(
        std::env::consts::OS,
        "freebsd" | "dragonfly" | "netbsd" | "openbsd"
    ) {
        let helper_path = helper_path.ok_or("Native volume listing requires lurker-helper.")?;
        let native = list_native_from_helper(helper_path)?;
        Ok(merge_native_and_user_volumes(local, native))
    } else {
        Ok(local)
    }
}

fn merge_native_and_user_volumes(
    local: Vec<ActiveVolume>,
    native: Vec<ActiveVolume>,
) -> Vec<ActiveVolume> {
    // The administrator-authorized native namespace is global, as in the CLI.
    // Folder records always come from the desktop's own login-user session.
    local
        .into_iter()
        .filter(|volume| volume.privilege_mode == PrivilegeMode::Unprivileged)
        .chain(
            native
                .into_iter()
                .filter(|volume| volume.privilege_mode == PrivilegeMode::Privileged),
        )
        .collect()
}

fn list_native_from_helper(helper_path: &Path) -> Result<Vec<ActiveVolume>, String> {
    let capabilities = Command::new(helper_path)
        .arg("capabilities")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Failed to check helper listing capabilities: {error}"))?;
    let values: serde_json::Value =
        serde_json::from_slice(&capabilities.stdout).unwrap_or_default();
    if !capabilities.status.success()
        || values["protocol_version"] != 3
        || values["native_volume_listing"] != 1
        || values["trusted_privileged_tools"] != true
        || values["noninteractive_sudo"] != true
    {
        return Err("Installed lurker-helper does not support authorized native volume listing. Update it together with the desktop app.".into());
    }
    let output = Command::new(helper_path)
        .arg("list-native")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Failed to list native volumes through lurker-helper: {error}"))?;
    let response: NativeVolumeListResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Invalid native volume list from lurker-helper: {error}"))?;
    if !output.status.success() || !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "Native volume listing failed.".into()));
    }
    Ok(response.volumes)
}

pub fn run_helper(helper_path: &Path, command: CommandAction) -> Result<OperationResponse, String> {
    let cryptomator_fence =
        lurker_core::request_requires_cryptomator_fence(&command).map_err(|error| error.message)?;
    run_helper_checked(helper_path, command, cryptomator_fence)
}

fn run_helper_checked(
    helper_path: &Path,
    command: CommandAction,
    cryptomator_fence: bool,
) -> Result<OperationResponse, String> {
    let directory_backend =
        lurker_core::request_requires_user_session(&command).map_err(|e| e.message)?;
    // Directory requests have their own login-session capability. A row's
    // Unprivileged mode must not require Linux's separate VeraCrypt transport.
    let unprivileged =
        !directory_backend && command.privilege_mode() == lurker_core::PrivilegeMode::Unprivileged;
    // Closing an owned userspace session needs its existing processes and
    // fusermount, even if package helpers have since been removed.
    let check_userspace_ready = unprivileged && !matches!(&command, CommandAction::Unmount(_));
    let privileged_tools = !cfg!(target_os = "macos")
        && !directory_backend
        && command.privilege_mode() == lurker_core::PrivilegeMode::Privileged;
    let keyfile = match &command {
        CommandAction::Create(c) => c.keyfile.is_some(),
        CommandAction::Mount(c) => c.keyfile.is_some(),
        CommandAction::Keyslots(c) => c.keyfile.is_some() || c.new_keyfile.is_some(),
        CommandAction::Unmount(_) => false,
    };
    let keyslots = matches!(&command, CommandAction::Keyslots(_));
    let filesystem = matches!(&command, CommandAction::Create(c) if c.filesystem.is_some());
    // OpenBSD Auto unmounts can refer to mirrored sessions by mountpoint alone.
    // Require the member-aware helper there before sending any native request.
    let raid1c = (cfg!(target_os = "openbsd") && !directory_backend)
        || command.volume_type() == lurker_core::VolumeType::Raid1c
        || match &command {
            CommandAction::Create(c) => !c.peer_sources.is_empty(),
            CommandAction::Mount(c) => {
                !c.peer_sources.is_empty()
                    || c.allow_degraded
                    || std::path::PathBuf::from(format!("{}.raid1c.json", c.source.display()))
                        .exists()
            }
            _ => false,
        };
    if privileged_tools
        || directory_backend
        || unprivileged
        || keyfile
        || keyslots
        || filesystem
        || raid1c
        || cryptomator_fence
    {
        let mut query = Command::new(helper_path);
        query.arg("capabilities");
        if check_userspace_ready {
            query.arg("--userspace-readiness");
        }
        let result = query.stdin(Stdio::null())
            .output()
            .map_err(|error| format!("Failed to check helper capabilities: {error}"))?;
        let capabilities: serde_json::Value =
            serde_json::from_slice(&result.stdout).unwrap_or_default();
        if !result.status.success()
            || !matches!(capabilities["protocol_version"].as_u64(), Some(2 | 3))
        {
            return Err("Installed lurker-helper does not support unprivileged or credential operations. Update it together with the desktop app.".into());
        }
        if directory_backend
            && (capabilities["protocol_version"] != 3 || capabilities["directory_backends"] != true)
        {
            return Err("Installed lurker-helper does not support directory backends. Update it together with the desktop app.".into());
        }
        if directory_backend && capabilities["folder_supervision_protocol"] != 1 {
            return Err("Installed lurker-helper does not support supervised directory sessions. Update it together with the desktop app.".into());
        }
        if cryptomator_fence
            && (capabilities["protocol_version"] != 3
                || capabilities["cryptomator_fence_protocol"].as_u64() != Some(1))
        {
            return Err("Installed lurker-helper does not support fail-closed Cryptomator mountpoints on macOS. Update it together with the desktop app.".into());
        }
        if unprivileged && capabilities["unprivileged_veracrypt"] != true {
            return Err("Installed lurker-helper does not support unprivileged operation. Update it together with the desktop app.".into());
        }
        let userspace_protocol = capabilities["userspace_backend_protocol"].as_u64();
        let userspace_protocol_supported = if check_userspace_ready {
            userspace_protocol == Some(5)
        } else {
            matches!(userspace_protocol, Some(4 | 5))
        };
        if unprivileged
            && (capabilities["protocol_version"] != 3 || !userspace_protocol_supported)
        {
            return Err("Installed lurker-helper does not support the durable userspace backend protocol. Update the helper and native components together.".into());
        }
        if check_userspace_ready && capabilities["userspace_backend_ready"] == false {
            return Err(capabilities["userspace_backend_error"]
                .as_str()
                .filter(|error| !error.trim().is_empty())
                .unwrap_or("Unprivileged VeraCrypt prerequisites are unavailable. Install its crypto support and host filesystem driver.")
                .to_owned());
        }
        if (keyfile && capabilities["keyfile_authentication"] != true)
            || (keyslots && capabilities["luks_keyslots"] != true)
        {
            return Err("Installed lurker-helper does not support keyfiles or LUKS keyslots. Update it together with the desktop app.".into());
        }
        if filesystem && capabilities["dragonfly_veracrypt_filesystem"] != true {
            return Err("Installed lurker-helper does not support selecting the DragonFly VeraCrypt filesystem. Update it together with the desktop app.".into());
        }
        if privileged_tools
            && (capabilities["protocol_version"] != 3
                || capabilities["trusted_privileged_tools"] != true)
        {
            return Err("Installed lurker-helper does not enforce trusted privileged tool paths. Update it together with the desktop app.".into());
        }
        if cfg!(target_os = "linux")
            && privileged_tools
            && capabilities["linux_loop_lease_protocol"] != 1
        {
            return Err("Installed lurker-helper does not support the owned loop-device lease protocol. Update it together with the desktop app.".into());
        }
        if raid1c
            && (capabilities["protocol_version"] != 3 || capabilities["raid1c_members"] != true)
        {
            return Err("Installed lurker-helper does not support RAID1C members and degraded recovery. Update it together with the desktop app.".into());
        }
        if privileged_tools
            && matches!(
                std::env::consts::OS,
                "freebsd" | "dragonfly" | "netbsd" | "openbsd"
            )
            && capabilities["noninteractive_sudo"] != true
        {
            return Err("Installed lurker-helper does not support desktop authorization without a terminal. Update it together with the desktop app.".into());
        }
    }
    let payload = serde_json::to_vec(&command)
        .map_err(|err| format!("Failed to encode helper request: {err}"))?;

    let mut child = Command::new(helper_path)
        .arg("run")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to spawn lurker-helper: {err}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&payload)
            .map_err(|err| format!("Failed to write helper request: {err}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("Failed waiting for lurker-helper: {err}"))?;

    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(format!(
                "lurker-helper returned no output (exit code {:?}).",
                output.status.code()
            ));
        }
        return Err(stderr);
    }

    serde_json::from_slice::<OperationResponse>(&output.stdout)
        .map_err(|err| format!("Failed to decode helper response: {err}"))
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 {
        return is_executable(path).then(|| path.to_path_buf());
    }

    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lurker_core::{PrivilegeMode, UnmountCommand, VolumeType};

    // Parallel script creation and subprocess spawning can briefly inherit a
    // sibling fixture's writable descriptor before exec (ETXTBSY on Linux).
    static HELPER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn cryptomator_fence_capability_is_required_before_secret_stdin() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-cryptomator-fence-helper-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let vault = directory.join("vault");
        fs::create_dir(&vault).unwrap();
        fs::write(vault.join("vault.cryptomator"), b"fixture marker").unwrap();
        let helper = directory.join("helper");
        let commands = [
            CommandAction::Create(lurker_core::CreateCommand {
                privilege_mode: PrivilegeMode::Privileged,
                target: vault.clone(),
                peer_sources: Vec::new(),
                size_gb: None,
                force: false,
                source_kind: lurker_core::SourceKind::Directory,
                volume_type: VolumeType::Cryptomator,
                cipher: lurker_core::CreateCipher::Aes,
                filesystem: None,
                passphrase: Some("fixture password".into()),
                keyfile: None,
            }),
            CommandAction::Mount(lurker_core::MountCommand {
                privilege_mode: PrivilegeMode::Privileged,
                source: vault.clone(),
                peer_sources: Vec::new(),
                allow_degraded: false,
                mountpoint: directory.join("mount"),
                tag: None,
                volume_type: VolumeType::Auto,
                passphrase: Some("fixture password".into()),
                keyfile: None,
                readonly: false,
            }),
        ];
        for capability in [
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!("1"),
            serde_json::json!(1.0),
            serde_json::json!(0),
            serde_json::json!(2),
        ] {
            let capabilities = serde_json::json!({"protocol_version":3,"directory_backends":true,"folder_supervision_protocol":1,"cryptomator_fence_protocol":capability});
            fs::write(&helper, format!("#!/bin/sh\nif [ \"$1\" = capabilities ]; then printf '%s\\n' '{}'; else : > \"${{0}}.ran\"; fi\n", capabilities)).unwrap();
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
            for command in &commands {
                let error = run_helper_checked(&helper, command.clone(), true).unwrap_err();
                assert!(error.contains("fail-closed Cryptomator"), "{error}");
                assert!(!directory.join("helper.ran").exists());
            }
        }
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"directory_backends":true,"folder_supervision_protocol":1,"cryptomator_fence_protocol":1}'
else
  cat >/dev/null
  printf '%s\n' '{"ok":true,"logs":[],"error":null}'
fi
"#).unwrap();
        for command in commands {
            assert!(run_helper_checked(&helper, command, true).unwrap().ok);
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_auto_cryptomator_detection_rejects_unfenced_helpers() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-auto-cryptomator-fence-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let vault = directory.join("vault");
        fs::create_dir(&vault).unwrap();
        fs::write(vault.join("vault.cryptomator"), b"fixture marker").unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"directory_backends":true,"folder_supervision_protocol":1}'
else
  touch "${0}.ran"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        // An unresolved Auto path can change before the helper inspects it.
        // It needs the same protection even when no current directory marker
        // causes the ordinary folder capability checks to run.
        for source in [vault, directory.join("unresolved.dmg")] {
            let command = CommandAction::Mount(lurker_core::MountCommand {
                privilege_mode: PrivilegeMode::Privileged,
                source,
                peer_sources: Vec::new(),
                allow_degraded: false,
                mountpoint: directory.join("mount"),
                tag: None,
                volume_type: VolumeType::Auto,
                passphrase: Some("fixture password".into()),
                keyfile: None,
                readonly: false,
            });
            assert!(lurker_core::request_requires_cryptomator_fence(&command).unwrap());
            let error = run_helper(&helper, command).unwrap_err();
            assert!(error.contains("fail-closed Cryptomator"), "{error}");
            assert!(!directory.join("helper.ran").exists());
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn native_listing_requires_a_new_capability_and_preserves_authorization_errors() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-native-list-helper-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"trusted_privileged_tools":true,"noninteractive_sudo":true}'
else
  touch "${0}.ran"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(list_native_from_helper(&helper)
            .unwrap_err()
            .contains("authorized native volume listing"));
        assert!(!directory.join("helper.ran").exists());
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"trusted_privileged_tools":true,"noninteractive_sudo":true,"native_volume_listing":1}'
else
  printf '%s\n' '{"ok":false,"volumes":[],"error":"sudo: a password is required"}'
fi
"#).unwrap();
        assert_eq!(
            list_native_from_helper(&helper).unwrap_err(),
            "sudo: a password is required"
        );
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"trusted_privileged_tools":true,"noninteractive_sudo":true,"native_volume_listing":1}'
else
  printf '%s\n' '{"ok":true,"volumes":[],"error":null}'
fi
"#).unwrap();
        assert!(list_native_from_helper(&helper).unwrap().is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn native_list_merge_uses_only_callers_folders_and_authorized_native_rows() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        fn row(name: &str, mode: PrivilegeMode) -> ActiveVolume {
            ActiveVolume {
                privilege_mode: mode,
                source_path: Some(format!("/tmp/{name}").into()),
                readonly: false,
                mapper_name: name.into(),
                mapper_path: format!("/tmp/{name}-device").into(),
                mountpoint: Some(format!("/tmp/{name}-mount").into()),
                filesystem_type: None,
            }
        }
        let local = vec![
            row("my-folder", PrivilegeMode::Unprivileged),
            row("duplicate-native", PrivilegeMode::Privileged),
        ];
        let native = vec![
            row("authorized-native", PrivilegeMode::Privileged),
            row("root-folder-excluded", PrivilegeMode::Unprivileged),
        ];
        let merged = merge_native_and_user_volumes(local, native);
        assert_eq!(
            merged
                .iter()
                .map(|volume| volume.mapper_name.as_str())
                .collect::<Vec<_>>(),
            vec!["my-folder", "authorized-native"]
        );
    }

    #[test]
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn bsd_native_requests_require_terminal_free_authorization_support() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-bsd-auth-helper-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"trusted_privileged_tools":true,"raid1c_members":true}'
else
  touch "${0}.ran"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            target: directory.join("unused"),
            tag: None,
            volume_type: VolumeType::default_create(),
        });
        let error = run_helper(&helper, command).unwrap_err();
        assert!(
            error.contains("authorization without a terminal"),
            "{error}"
        );
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_requests_require_supervision_before_receiving_credentials() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-folder-supervision-helper-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"directory_backends":true}'
else
  touch "${0}.ran"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Create(lurker_core::CreateCommand {
            privilege_mode: PrivilegeMode::Privileged,
            target: directory.join("vault"),
            peer_sources: Vec::new(),
            size_gb: None,
            force: false,
            source_kind: lurker_core::SourceKind::Directory,
            volume_type: VolumeType::Gocryptfs,
            cipher: lurker_core::CreateCipher::Aes,
            filesystem: None,
            passphrase: Some("fixture password".into()),
            keyfile: None,
        });
        let error = run_helper(&helper, command).unwrap_err();
        assert!(error.contains("supervised directory sessions"), "{error}");
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn privileged_linux_requests_require_owned_loop_lease_capability() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-loop-helper-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"trusted_privileged_tools":true}'
else
  touch "${0}.ran"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            target: directory.join("unused"),
            tag: None,
            volume_type: VolumeType::Luks,
        });
        let error = run_helper(&helper, command).unwrap_err();
        assert!(
            error.contains("owned loop-device lease protocol"),
            "{error}"
        );
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn mirrored_requests_require_explicit_member_capability_before_stdin() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-raid1c-helper-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        let command = CommandAction::Mount(lurker_core::MountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            source: directory.join("primary"),
            peer_sources: vec![directory.join("peer")],
            allow_degraded: false,
            mountpoint: directory.join("mount"),
            tag: None,
            volume_type: VolumeType::Raid1c,
            passphrase: Some("fixture password".into()),
            keyfile: None,
            readonly: false,
        });
        fs::write(&helper, "#!/bin/sh\nif [ \"$1\" = capabilities ]; then printf '%s\\n' '{\"protocol_version\":3,\"trusted_privileged_tools\":true,\"linux_loop_lease_protocol\":1}'; else : > \"${0}.ran\"; fi\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let error = run_helper(&helper, command.clone()).unwrap_err();
        assert!(error.contains("does not support RAID1C"), "{error}");
        assert!(!directory.join("helper.ran").exists());
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"trusted_privileged_tools":true,"linux_loop_lease_protocol":1,"raid1c_members":true,"noninteractive_sudo":true}'
else
  cat >/dev/null
  printf '%s\n' '{"ok":true,"logs":[],"error":null}'
fi
"#,
        )
        .unwrap();
        assert!(run_helper(&helper, command).unwrap().ok);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_requests_require_the_new_helper_contract() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-folder-helper-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, "#!/bin/sh\nif [ \"$1\" = capabilities ]; then printf '%s\\n' '{\"protocol_version\":2}'; else : > \"${0}.ran\"; fi\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            target: directory.join("unused"),
            tag: None,
            volume_type: VolumeType::Gocryptfs,
        });
        assert!(run_helper(&helper, command)
            .unwrap_err()
            .contains("does not support directory backends"));
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_session_does_not_require_linux_veracrypt_capability() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-folder-capability-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"directory_backends":true,"folder_supervision_protocol":1,"unprivileged_veracrypt":false,"userspace_backend_ready":false}'
else
  cat >/dev/null
  printf '%s\n' '{"ok":true,"logs":[],"error":null}'
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            target: directory.join("unused"),
            tag: None,
            volume_type: VolumeType::Gocryptfs,
        });
        assert!(run_helper(&helper, command).unwrap().ok);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn older_helper_is_never_sent_an_unprivileged_operation() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-helper-capabilities-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("old-helper");
        fs::write(
            &helper,
            "#!/bin/sh\nif [ \"$1\" = run ]; then : > \"${0}.ran\"; fi\nexit 64\n",
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            target: directory.join("unused"),
            tag: None,
            // Pin the transport under test; Auto may inspect unrelated folder
            // recovery state before capability negotiation.
            volume_type: VolumeType::Veracrypt,
        });
        assert!(run_helper(&helper, command)
            .unwrap_err()
            .contains("does not support unprivileged"));
        assert!(!directory.join("old-helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn userspace_requests_reject_helpers_without_the_durable_protocol() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-transport-capability-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"unprivileged_veracrypt":true,"userspace_backend_protocol":3}'
else
  touch "${0}.ran"
fi
"#).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            target: directory.join("unused"),
            tag: None,
            volume_type: VolumeType::Veracrypt,
        });
        assert!(run_helper(&helper, command)
            .unwrap_err()
            .contains("durable userspace backend protocol"));
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn userspace_readiness_precedes_secrets_but_does_not_block_owned_close() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-userspace-readiness-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  if [ "${2-}" = --userspace-readiness ]; then : > "${0}.readiness"; fi
  printf '%s\n' '{"protocol_version":3,"unprivileged_veracrypt":true,"userspace_backend_protocol":5,"userspace_backend_ready":false,"userspace_backend_error":"Install the host filesystem driver."}'
else
  cat > "${0}.request"
  printf '%s\n' '{"ok":true,"logs":[],"error":null}'
fi
"#).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let mount = CommandAction::Mount(lurker_core::MountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            source: directory.join("vault"),
            peer_sources: Vec::new(),
            allow_degraded: false,
            mountpoint: directory.join("mount"),
            tag: None,
            volume_type: VolumeType::Veracrypt,
            passphrase: Some("fixture password".into()),
            keyfile: None,
            readonly: false,
        });
        assert_eq!(run_helper(&helper, mount).unwrap_err(), "Install the host filesystem driver.");
        assert!(directory.join("helper.readiness").exists());
        assert!(!directory.join("helper.request").exists());
        fs::remove_file(directory.join("helper.readiness")).unwrap();
        let close = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            target: directory.join("mount"),
            tag: None,
            volume_type: VolumeType::Veracrypt,
        });
        assert!(run_helper(&helper, close).unwrap().ok);
        assert!(!directory.join("helper.readiness").exists());
        assert!(directory.join("helper.request").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn userspace_protocol_five_helpers_accept_optional_readiness_fields() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-userspace-ready-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        for capabilities in [
            r#"{"protocol_version":3,"unprivileged_veracrypt":true,"userspace_backend_protocol":5,"userspace_backend_ready":true}"#,
            r#"{"protocol_version":3,"unprivileged_veracrypt":true,"userspace_backend_protocol":5}"#,
        ] {
            let script = r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  [ "${2-}" = --userspace-readiness ] || exit 64
  printf '%s\n' '__CAPABILITIES__'
else
  cat > "${0}.request"
  printf '%s\n' '{"ok":true,"logs":[],"error":null}'
fi
"#.replace("__CAPABILITIES__", capabilities);
            fs::write(&helper, script).unwrap();
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
            let mount = CommandAction::Mount(lurker_core::MountCommand {
                privilege_mode: PrivilegeMode::Unprivileged,
                source: directory.join("vault"),
                peer_sources: Vec::new(),
                allow_degraded: false,
                mountpoint: directory.join("mount"),
                tag: None,
                volume_type: VolumeType::Veracrypt,
                passphrase: Some("fixture password".into()),
                keyfile: None,
                readonly: false,
            });
            assert!(run_helper(&helper, mount).unwrap().ok);
            assert!(fs::read_to_string(directory.join("helper.request"))
                .unwrap().contains("fixture password"));
            fs::remove_file(directory.join("helper.request")).unwrap();
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn protocol_four_helper_can_close_but_cannot_receive_new_mount_secrets() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-userspace-legacy-close-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"unprivileged_veracrypt":true,"userspace_backend_protocol":4}'
else
  cat > "${0}.request"
  printf '%s\n' '{"ok":true,"logs":[],"error":null}'
fi
"#).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let mount = CommandAction::Mount(lurker_core::MountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            source: directory.join("vault"),
            peer_sources: Vec::new(),
            allow_degraded: false,
            mountpoint: directory.join("mount"),
            tag: None,
            volume_type: VolumeType::Veracrypt,
            passphrase: Some("must not be transmitted".into()),
            keyfile: None,
            readonly: false,
        });
        assert!(run_helper(&helper, mount).unwrap_err()
            .contains("durable userspace backend protocol"));
        assert!(!directory.join("helper.request").exists());
        let close = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Unprivileged,
            target: directory.join("mount"),
            tag: None,
            volume_type: VolumeType::Veracrypt,
        });
        assert!(run_helper(&helper, close).unwrap().ok);
        assert!(!fs::read_to_string(directory.join("helper.request")).unwrap()
            .contains("must not be transmitted"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn plain_privileged_requests_require_the_trusted_tool_contract() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "lurker-privileged-capability-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":2,"keyfile_authentication":true,"luks_keyslots":true}'
else
  touch "${0}.ran"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let command = CommandAction::Unmount(UnmountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            target: directory.join("unused"),
            tag: None,
            volume_type: VolumeType::Luks,
        });
        assert!(run_helper(&helper, command)
            .unwrap_err()
            .contains("does not enforce trusted privileged tool paths"));
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn helpers_must_advertise_credential_support_before_receiving_secrets() {
        let _guard = HELPER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("lurker-helper-credentials-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, "#!/bin/sh\nif [ \"$1\" = capabilities ]; then printf '%s\\n' '{\"protocol_version\":2,\"unprivileged_veracrypt\":true}'; else : > \"${0}.ran\"; fi\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let keyfile = CommandAction::Mount(lurker_core::MountCommand {
            privilege_mode: PrivilegeMode::Privileged,
            source: "vault".into(),
            peer_sources: Vec::new(),
            allow_degraded: false,
            mountpoint: "mnt".into(),
            tag: None,
            volume_type: VolumeType::Veracrypt,
            passphrase: None,
            keyfile: Some("key".into()),
            readonly: false,
        });
        assert!(run_helper(&helper, keyfile)
            .unwrap_err()
            .contains("does not support keyfiles"));
        let keyslots = CommandAction::Keyslots(lurker_core::KeyslotsCommand {
            privilege_mode: PrivilegeMode::Privileged,
            source: "vault".into(),
            operation: lurker_core::KeyslotOperation::Add,
            slot: None,
            passphrase: Some("old".into()),
            keyfile: None,
            new_passphrase: Some("new".into()),
            new_keyfile: None,
        });
        assert!(run_helper(&helper, keyslots)
            .unwrap_err()
            .contains("does not support keyfiles"));
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
