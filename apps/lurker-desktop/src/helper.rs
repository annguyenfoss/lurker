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
    if privileged_tools || directory_backend || unprivileged || keyfile || keyslots {
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
