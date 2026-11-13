use crate::api::{CommandAction, CreateCommand, SystemProbe, ToolStatus};
use crate::error::{AppError, AppResult};
use crate::linux::mapper_device_path;
use crate::model::{SourceKind, VolumeType};
use crate::output::Output;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output as ProcessOutput, Stdio};

#[derive(Clone, Debug)]
pub struct ToolPaths {
    pub cryptsetup: PathBuf,
    pub mkfs_btrfs: Option<PathBuf>,
    pub mount: Option<PathBuf>,
    pub umount: Option<PathBuf>,
    pub blkid: Option<PathBuf>,
    pub lsblk: Option<PathBuf>,
    pub losetup: Option<PathBuf>,
    pub veracrypt: Option<PathBuf>,
}

pub struct AppContext {
    pub output: Output,
    pub tools: ToolPaths,
    is_root: bool,
    sudo_ready: bool,
    cleanup_mapper: Option<String>,
    cleanup_loop: Option<PathBuf>,
}

impl AppContext {
    pub fn new(command: &CommandAction, output: Output) -> AppResult<Self> {
        let tools = ToolPaths::resolve(command)?;
        let is_root = effective_uid()? == 0;
        Ok(Self {
            output,
            tools,
            is_root,
            sudo_ready: false,
            cleanup_mapper: None,
            cleanup_loop: None,
        })
    }

    pub fn cryptsetup_path(&self) -> &Path {
        &self.tools.cryptsetup
    }

    pub fn mkfs_btrfs_path(&self) -> AppResult<&Path> {
        self.tools
            .mkfs_btrfs
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: mkfs.btrfs"))
    }

    pub fn mount_path(&self) -> AppResult<&Path> {
        self.tools
            .mount
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: mount"))
    }

    pub fn umount_path(&self) -> AppResult<&Path> {
        self.tools
            .umount
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: umount"))
    }

    pub fn lsblk_path(&self) -> AppResult<&Path> {
        self.tools
            .lsblk
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: lsblk"))
    }

    pub fn losetup_path(&self) -> AppResult<&Path> {
        self.tools
            .losetup
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: losetup"))
    }

    pub fn blkid_path(&self) -> Option<&Path> {
        self.tools.blkid.as_deref()
    }

    pub fn veracrypt_path(&self) -> AppResult<&Path> {
        self.tools
            .veracrypt
            .as_deref()
            .ok_or_else(|| AppError::new("Bundled VeraCrypt creator is missing. Build scripts/build-linux-userspace.py --only-creator or install the complete Lurker package."))
    }

    pub fn set_cleanup_mapper(&mut self, mapper_name: impl Into<String>) {
        self.cleanup_mapper = Some(mapper_name.into());
    }

    pub fn clear_cleanup_mapper(&mut self) {
        self.cleanup_mapper = None;
    }

    pub fn set_cleanup_loop(&mut self, device: PathBuf) {
        self.cleanup_loop = Some(device);
    }

    pub fn release_cleanup_loop(&mut self) -> AppResult<()> {
        let Some(device) = self.cleanup_loop.as_ref().cloned() else {
            return Ok(());
        };
        let losetup = self.losetup_path()?.to_path_buf();
        self.run_command(
            &losetup,
            &["--detach".into(), device.as_os_str().into()],
            true,
            false,
            "Failed to release the owned source loop device",
        )?;
        // An open mapper keeps the loop alive with LO_FLAGS_AUTOCLEAR. Otherwise
        // detach removes it now, including on failed authentication.
        self.cleanup_loop = None;
        Ok(())
    }

    pub fn prepare_command(
        &mut self,
        program: &Path,
        args: &[OsString],
        privileged: bool,
        interactive: bool,
    ) -> AppResult<Command> {
        let program = if privileged || self.is_root {
            crate::trusted_privileged_executable(program)?
        } else {
            program.to_path_buf()
        };
        let mut command = if privileged && !self.is_root {
            self.ensure_sudo()?;
            let sudo = crate::trusted_system_executable("sudo")?;
            let mut command = Command::new(sudo);
            command.arg(&program);
            command.args(args);
            command
        } else {
            let mut command = Command::new(&program);
            command.args(args);
            command
        };

        if interactive {
            command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }

        Ok(command)
    }

    pub fn run_command(
        &mut self,
        program: &Path,
        args: &[OsString],
        privileged: bool,
        interactive: bool,
        context: &str,
    ) -> AppResult<()> {
        let mut command = self.prepare_command(program, args, privileged, interactive)?;
        run_status(&mut command, context)
    }

    pub fn capture_command(
        &mut self,
        program: &Path,
        args: &[OsString],
        privileged: bool,
        context: &str,
    ) -> AppResult<ProcessOutput> {
        let mut command = self.prepare_command(program, args, privileged, false)?;
        command.stdin(Stdio::null());
        let output = command.output().map_err(|err| AppError::io(context, err))?;
        Ok(output)
    }

    pub fn run_command_with_input(
        &mut self,
        program: &Path,
        args: &[OsString],
        privileged: bool,
        stdin_data: &[u8],
        context: &str,
    ) -> AppResult<()> {
        let mut command = self.prepare_command(program, args, privileged, false)?;
        command.stdin(Stdio::piped());
        command.stdout(Stdio::null());
        command.stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|err| AppError::io(context, err))?;
        let write_result = if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_data)
        } else {
            Ok(())
        };
        // Reap the child even when it exits before consuming its input.
        let result = child
            .wait_with_output()
            .map_err(|err| AppError::io(context, err))?;
        if result.status.success() {
            write_result.map_err(|err| AppError::io(context, err))
        } else {
            let detail: String = String::from_utf8_lossy(&result.stderr)
                .trim()
                .chars()
                .take(8192)
                .collect();
            Err(AppError::new(if detail.is_empty() {
                format!("{context} failed.")
            } else {
                format!("{context}: {detail}")
            }))
        }
    }

    fn ensure_sudo(&mut self) -> AppResult<()> {
        if self.is_root || self.sudo_ready {
            return Ok(());
        }

        let sudo = crate::trusted_system_executable("sudo")?;
        self.output.msg2("Refreshing sudo credentials");
        let status = Command::new(sudo)
            .arg("-v")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|err| AppError::io("Failed to refresh sudo credentials", err))?;
        if !status.success() {
            return Err(AppError::new("Failed to refresh sudo credentials."));
        }
        self.sudo_ready = true;
        Ok(())
    }

    fn try_ensure_sudo(&mut self) -> bool {
        if self.is_root || self.sudo_ready {
            return true;
        }

        let Ok(sudo) = crate::trusted_system_executable("sudo") else {
            return false;
        };
        let Ok(status) = Command::new(sudo)
            .arg("-v")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        else {
            return false;
        };
        if status.success() {
            self.sudo_ready = true;
            true
        } else {
            false
        }
    }

    fn try_run_privileged_silent(&mut self, program: &Path, args: &[OsString]) -> bool {
        let Ok(program) = crate::trusted_privileged_executable(program) else {
            return false;
        };
        let mut command = if self.is_root {
            let mut command = Command::new(program);
            command.args(args);
            command
        } else {
            if !self.try_ensure_sudo() {
                return false;
            }
            let Ok(sudo) = crate::trusted_system_executable("sudo") else {
                return false;
            };
            let mut command = Command::new(sudo);
            command.arg(program);
            command.args(args);
            command
        };

        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        command
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

impl Drop for AppContext {
    fn drop(&mut self) {
        if let Some(mapper_name) = self.cleanup_mapper.take() {
            if mapper_device_path(&mapper_name).exists() {
                self.output.warn(&format!(
                    "Closing mapper {} after failed action.",
                    mapper_name
                ));
                let cryptsetup = self.tools.cryptsetup.clone();
                if !self.try_run_privileged_silent(
                    &cryptsetup,
                    &[OsString::from("close"), OsString::from(&mapper_name)],
                ) {
                    self.output.warn(&format!(
                        "Failed to close mapper {} during cleanup.",
                        mapper_name
                    ));
                }
            }
        }
        if let Some(device) = self.cleanup_loop.take() {
            let released = self.tools.losetup.clone().is_some_and(|losetup| {
                self.try_run_privileged_silent(
                    &losetup,
                    &["--detach".into(), device.as_os_str().into()],
                )
            });
            if !released {
                self.output.warn(&format!(
                    "Failed to release owned loop {} during cleanup.",
                    device.display()
                ));
            }
        }
    }
}

impl ToolPaths {
    fn resolve(command: &CommandAction) -> AppResult<Self> {
        let cryptsetup = require_command("cryptsetup")?;
        let blkid = find_executable("blkid");
        let veracrypt = if matches!(command, CommandAction::Create(c) if c.volume_type == VolumeType::Veracrypt)
        {
            Some(resolve_veracrypt_creator(
                matches!(command, CommandAction::Create(c) if c.source_kind == SourceKind::Block),
            )?)
        } else {
            None
        };
        let mut tools = Self {
            cryptsetup,
            mkfs_btrfs: None,
            mount: None,
            umount: None,
            blkid,
            lsblk: find_executable("lsblk"),
            losetup: find_executable("losetup"),
            veracrypt,
        };

        match command {
            CommandAction::Create(CreateCommand {
                source_kind,
                volume_type,
                ..
            }) => {
                tools.mkfs_btrfs = Some(require_command("mkfs.btrfs")?);
                if *source_kind == SourceKind::Block && tools.lsblk.is_none() {
                    tools.lsblk = Some(require_command("lsblk")?);
                }
                if *volume_type == VolumeType::Veracrypt && tools.veracrypt.is_none() {
                    return Err(AppError::new("Bundled VeraCrypt creator is missing. Build scripts/build-linux-userspace.py --only-creator or install the complete Lurker package."));
                }
            }
            CommandAction::Mount(_) => {
                tools.mount = Some(require_command("mount")?);
            }
            CommandAction::Unmount(_) => {
                tools.umount = Some(require_command("umount")?);
            }
            CommandAction::Keyslots(_) => {}
        }

        Ok(tools)
    }
}

/// Private backend discovery never searches PATH for an unrelated VeraCrypt app.
fn creator_candidates(root: bool) -> Vec<PathBuf> {
    if !root {
        if let Some(directory) = env::var_os("LURKER_BACKEND_DIR") {
            return vec![PathBuf::from(directory).join("lurker-veracrypt-create")];
        }
    }
    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join("backends/lurker-veracrypt-create"));
            candidates.push(parent.join("../libexec/lurker/backends/lurker-veracrypt-create"));
            candidates.push(parent.join("userspace/lurker-veracrypt-create"));
        }
    }
    candidates.push("/usr/lib/lurker/backends/lurker-veracrypt-create".into());
    candidates.push("/usr/lib/lurker/userspace/lurker-veracrypt-create".into());
    candidates
}

fn trusted_root_program(path: &Path) -> AppResult<PathBuf> {
    crate::trusted_privileged_executable(path)
}

fn resolve_veracrypt_creator(will_elevate: bool) -> AppResult<PathBuf> {
    let root = crate::effective_uid() == 0;
    for candidate in creator_candidates(root) {
        if !is_executable(&candidate) {
            continue;
        }
        let program = if root || will_elevate {
            trusted_root_program(&candidate)?
        } else {
            fs::canonicalize(&candidate)
                .map_err(|err| AppError::io("Failed to resolve creator", err))?
        };
        let output = Command::new(&program)
            .arg("--lurker-creator-version")
            .stdin(Stdio::null())
            .output()
            .map_err(|err| AppError::io("Failed to inspect private VeraCrypt creator", err))?;
        if !output.status.success() || output.stdout != b"lurker-creator-1\n" {
            return Err(AppError::new(
                "Incompatible private VeraCrypt creator. Rebuild the bundled backend.",
            ));
        }
        return Ok(program);
    }
    Err(AppError::new("Bundled VeraCrypt creator is missing. Build scripts/build-linux-userspace.py --only-creator or install the complete Lurker package."))
}

pub fn probe_system() -> AppResult<SystemProbe> {
    Ok(SystemProbe {
        is_root: effective_uid()? == 0,
        tools: vec![
            ToolStatus {
                name: "cryptsetup".into(),
                path: find_executable("cryptsetup"),
                required: true,
            },
            ToolStatus {
                name: "mkfs.btrfs".into(),
                path: find_executable("mkfs.btrfs"),
                required: true,
            },
            ToolStatus {
                name: "mount".into(),
                path: find_executable("mount"),
                required: true,
            },
            ToolStatus {
                name: "umount".into(),
                path: find_executable("umount"),
                required: true,
            },
            ToolStatus {
                name: "lsblk".into(),
                path: find_executable("lsblk"),
                required: true,
            },
            ToolStatus {
                name: "losetup".into(),
                path: find_executable("losetup"),
                required: true,
            },
            ToolStatus {
                name: "blkid".into(),
                path: find_executable("blkid"),
                required: false,
            },
            ToolStatus {
                name: "lurker-veracrypt-create".into(),
                path: creator_candidates(false)
                    .into_iter()
                    .find(|path| is_executable(path)),
                required: false,
            },
            ToolStatus {
                name: "sudo".into(),
                path: crate::trusted_system_executable("sudo").ok(),
                required: false,
            },
            ToolStatus {
                name: "pkexec".into(),
                path: crate::trusted_system_executable("pkexec").ok(),
                required: false,
            },
        ],
    })
}

pub fn run_status(command: &mut Command, context: &str) -> AppResult<()> {
    let status = command.status().map_err(|err| AppError::io(context, err))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(format!("{context} failed.")))
    }
}

pub fn trim_stdout(output: &ProcessOutput) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn require_command(name: &str) -> AppResult<PathBuf> {
    find_executable(name)
        .ok_or_else(|| AppError::new(format!("Required command not found: {name}")))
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

pub fn effective_uid() -> AppResult<u32> {
    // get euid from the OS rather than environment variables or Linux procfs.
    Ok(crate::effective_uid())
}

#[cfg(test)]
mod tests {
    use super::effective_uid;

    fn subprocess_context() -> super::AppContext {
        super::AppContext {
            output: crate::output::Output::buffered().0,
            tools: super::ToolPaths {
                cryptsetup: "/bin/false".into(),
                mkfs_btrfs: None,
                mount: None,
                umount: None,
                blkid: None,
                lsblk: None,
                losetup: None,
                veracrypt: None,
            },
            is_root: false,
            sudo_ready: false,
            cleanup_mapper: None,
            cleanup_loop: None,
        }
    }

    #[test]
    fn elevated_backend_rejects_writable_ancestors_and_user_builds() {
        let directory = std::env::temp_dir().join(format!("lurker-trust-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let program = directory.join("creator");
        std::fs::write(&program, b"#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(super::trusted_root_program(&program).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn privileged_commands_reject_user_programs_before_authorization_or_cleanup() {
        let directory =
            std::env::temp_dir().join(format!("lurker-path-trust-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let program = directory.join("cryptsetup");
        std::fs::write(&program, b"#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut context = subprocess_context();
        assert!(context
            .prepare_command(&program, &[], true, false)
            .unwrap_err()
            .message
            .contains("untrusted path"));
        assert!(!context.sudo_ready);
        assert!(!context.try_run_privileged_silent(&program, &[]));
        context.is_root = true;
        assert!(context
            .prepare_command(&program, &[], false, false)
            .is_err());
        assert!(!context.try_run_privileged_silent(&program, &[]));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn subprocess_stdin_errors_are_reaped_and_reported() {
        let mut context = subprocess_context();
        let error = context
            .run_command_with_input(
                std::path::Path::new("/bin/sh"),
                &[
                    "-c".into(),
                    "printf 'crypto test diagnostic' >&2; exit 7".into(),
                ],
                false,
                &vec![b'x'; 1024 * 1024],
                "Crypto test",
            )
            .unwrap_err();
        assert!(error
            .message
            .contains("Crypto test: crypto test diagnostic"));
    }

    #[test]
    fn subprocess_errors_are_bounded() {
        let mut context = subprocess_context();
        let error = context
            .run_command_with_input(
                std::path::Path::new("/bin/sh"),
                &[
                    "-c".into(),
                    "head -c 16384 /dev/zero | tr '\\0' x >&2; exit 1".into(),
                ],
                false,
                b"",
                "Crypto test",
            )
            .unwrap_err();
        assert_eq!(error.message.len(), "Crypto test: ".len() + 8192);
    }

    #[test]
    fn effective_uid_is_available() {
        let _ = effective_uid().unwrap();
    }
}
