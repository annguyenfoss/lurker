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
    pub veracrypt: Option<PathBuf>,
    pub sudo: Option<PathBuf>,
}

pub struct AppContext {
    pub output: Output,
    pub tools: ToolPaths,
    is_root: bool,
    sudo_ready: bool,
    cleanup_mapper: Option<String>,
    veracrypt_fallback_noticed: bool,
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
            veracrypt_fallback_noticed: false,
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

    pub fn blkid_path(&self) -> Option<&Path> {
        self.tools.blkid.as_deref()
    }

    pub fn veracrypt_path(&self) -> AppResult<&Path> {
        self.tools
            .veracrypt
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: veracrypt"))
    }

    pub fn have_veracrypt(&self) -> bool {
        self.tools.veracrypt.is_some()
    }

    pub fn notice_veracrypt_fallback(&mut self) {
        if !self.veracrypt_fallback_noticed {
            self.output
                .msg2("veracrypt not found; falling back to cryptsetup for veracrypt container");
            self.veracrypt_fallback_noticed = true;
        }
    }

    pub fn set_cleanup_mapper(&mut self, mapper_name: impl Into<String>) {
        self.cleanup_mapper = Some(mapper_name.into());
    }

    pub fn clear_cleanup_mapper(&mut self) {
        self.cleanup_mapper = None;
    }

    pub fn prepare_command(
        &mut self,
        program: &Path,
        args: &[OsString],
        privileged: bool,
        interactive: bool,
    ) -> AppResult<Command> {
        let mut command = if privileged && !self.is_root {
            self.ensure_sudo()?;
            let sudo = self
                .tools
                .sudo
                .as_deref()
                .ok_or_else(|| AppError::new("Required command not found: sudo"))?;
            let mut command = Command::new(sudo);
            command.arg(program);
            command.args(args);
            command
        } else {
            let mut command = Command::new(program);
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
        command.stderr(Stdio::null());

        let mut child = command.spawn().map_err(|err| AppError::io(context, err))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_data)
                .map_err(|err| AppError::io(context, err))?;
        }
        let status = child.wait().map_err(|err| AppError::io(context, err))?;
        if status.success() {
            Ok(())
        } else {
            Err(AppError::new(format!("{context} failed.")))
        }
    }

    fn ensure_sudo(&mut self) -> AppResult<()> {
        if self.is_root || self.sudo_ready {
            return Ok(());
        }

        let sudo = self
            .tools
            .sudo
            .as_deref()
            .ok_or_else(|| AppError::new("Required command not found: sudo"))?;
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

        let Some(sudo) = self.tools.sudo.as_deref() else {
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
        let mut command = if self.is_root {
            let mut command = Command::new(program);
            command.args(args);
            command
        } else {
            if !self.try_ensure_sudo() {
                return false;
            }
            let Some(sudo) = self.tools.sudo.as_deref() else {
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
        let Some(mapper_name) = self.cleanup_mapper.take() else {
            return;
        };
        let mapper_path = mapper_device_path(&mapper_name);
        if !mapper_path.exists() {
            return;
        }

        self.output.warn(&format!(
            "Closing mapper {} after failed action.",
            mapper_name
        ));

        let cryptsetup = self.tools.cryptsetup.clone();
        let closed = self.try_run_privileged_silent(
            &cryptsetup,
            &[OsString::from("close"), OsString::from(&mapper_name)],
        );
        if !closed {
            self.output.warn(&format!(
                "Failed to close mapper {} during cleanup.",
                mapper_name
            ));
        }
    }
}

impl ToolPaths {
    fn resolve(command: &CommandAction) -> AppResult<Self> {
        let cryptsetup = require_command("cryptsetup")?;
        let sudo = find_executable("sudo");
        let blkid = find_executable("blkid");
        let veracrypt = find_executable("veracrypt");
        let mut tools = Self {
            cryptsetup,
            mkfs_btrfs: None,
            mount: None,
            umount: None,
            blkid,
            lsblk: find_executable("lsblk"),
            veracrypt,
            sudo,
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
                    return Err(AppError::new("Required command not found: veracrypt"));
                }
            }
            CommandAction::Mount(_) => {
                tools.mount = Some(require_command("mount")?);
            }
            CommandAction::Unmount(_) => {
                tools.umount = Some(require_command("umount")?);
            }
        }

        Ok(tools)
    }
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
                name: "blkid".into(),
                path: find_executable("blkid"),
                required: false,
            },
            ToolStatus {
                name: "veracrypt".into(),
                path: find_executable("veracrypt"),
                required: false,
            },
            ToolStatus {
                name: "sudo".into(),
                path: find_executable("sudo"),
                required: false,
            },
            ToolStatus {
                name: "pkexec".into(),
                path: find_executable("pkexec"),
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
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|err| AppError::io("Failed to read /proc/self/status", err))?;
    for line in status.lines() {
        if let Some(values) = line.strip_prefix("Uid:") {
            let mut parts = values.split_whitespace();
            let _real = parts.next();
            let effective = parts
                .next()
                .ok_or_else(|| AppError::new("Failed to parse effective uid."))?;
            return effective
                .parse::<u32>()
                .map_err(|_| AppError::new("Failed to parse effective uid."));
        }
    }
    Err(AppError::new("Failed to locate effective uid."))
}

#[cfg(test)]
mod tests {
    use super::effective_uid;

    #[test]
    fn effective_uid_is_available() {
        let _ = effective_uid().unwrap();
    }
}
