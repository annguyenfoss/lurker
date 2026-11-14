use lurker_core::{CommandAction, OperationResponse};
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

pub fn run_helper(helper_path: &Path, command: CommandAction) -> Result<OperationResponse, String> {
    let directory_backend =
        lurker_core::request_requires_user_session(&command).map_err(|e| e.message)?;
    // Directory requests have their own login-session capability. A row's
    // Unprivileged mode must not require Linux's separate VeraCrypt transport.
    let unprivileged =
        !directory_backend && command.privilege_mode() == lurker_core::PrivilegeMode::Unprivileged;
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
    if privileged_tools || directory_backend || unprivileged || keyfile || keyslots || raid1c {
        let result = Command::new(helper_path)
            .arg("capabilities")
            .stdin(Stdio::null())
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
        if unprivileged && capabilities["unprivileged_veracrypt"] != true {
            return Err("Installed lurker-helper does not support unprivileged operation. Update it together with the desktop app.".into());
        }
        if unprivileged
            && (capabilities["protocol_version"] != 3
                || capabilities["userspace_backend_protocol"] != 3)
        {
            return Err("Installed lurker-helper does not support the source-locking userspace backend protocol. Update the helper and native components together.".into());
        }
        if (keyfile && capabilities["keyfile_authentication"] != true)
            || (keyslots && capabilities["luks_keyslots"] != true)
        {
            return Err("Installed lurker-helper does not support keyfiles or LUKS keyslots. Update it together with the desktop app.".into());
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

    #[test]
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn bsd_native_requests_require_terminal_free_authorization_support() {
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
    #[cfg(target_os = "linux")]
    fn privileged_linux_requests_require_owned_loop_lease_capability() {
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
        let directory =
            std::env::temp_dir().join(format!("lurker-folder-capability-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(
            &helper,
            r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"directory_backends":true,"unprivileged_veracrypt":false}'
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
    fn userspace_requests_reject_helpers_without_the_source_locking_protocol() {
        let directory = std::env::temp_dir().join(format!(
            "lurker-transport-capability-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let helper = directory.join("helper");
        fs::write(&helper, r#"#!/bin/sh
if [ "$1" = capabilities ]; then
  printf '%s\n' '{"protocol_version":3,"unprivileged_veracrypt":true,"userspace_backend_protocol":2}'
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
            .contains("source-locking userspace backend protocol"));
        assert!(!directory.join("helper.ran").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn plain_privileged_requests_require_the_trusted_tool_contract() {
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
            volume_type: VolumeType::Auto,
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
