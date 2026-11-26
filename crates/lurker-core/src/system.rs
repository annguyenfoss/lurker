use crate::api::{CommandAction, CreateCommand, SystemProbe, ToolStatus};
use crate::error::{AppError, AppResult};
use crate::linux::mapper_device_path;
use crate::model::{SourceKind, VolumeType};
use crate::output::Output;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output as ProcessOutput, Stdio};

#[derive(Clone, Debug)]
pub struct ToolPaths {
    pub cryptsetup: PathBuf,
    pub formatter: Option<PathBuf>,
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
    loop_lease: Option<crate::linux_loop::LoopLease>,
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
            loop_lease: None,
        })
    }

    pub fn cryptsetup_path(&self) -> &Path {
        &self.tools.cryptsetup
    }

    pub fn formatter_path(&self) -> AppResult<&Path> {
        self.tools
            .formatter
            .as_deref()
            .ok_or_else(|| AppError::new("Required filesystem formatter was not resolved"))
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

    pub(crate) fn set_loop_lease(&mut self, lease: crate::linux_loop::LoopLease) {
        self.loop_lease = Some(lease);
    }

    pub fn release_cleanup_loop(&mut self) -> AppResult<()> {
        if let Some(lease) = self.loop_lease.as_mut() {
            lease.release()?;
        }
        self.loop_lease = None;
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
        // Closing our control connection releases only the descriptor opened
        // by the trusted lease helper. Never detach a reusable device pathname.
        self.loop_lease.take();
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
            formatter: None,
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
                filesystem,
                ..
            }) => {
                if *filesystem == Some(crate::api::CreateFilesystem::Ufs1)
                    && matches!(volume_type, VolumeType::Auto | VolumeType::Luks)
                {
                    return Err(AppError::new("Linux UFS1 requires 512-byte logical sectors; default LUKS2 creation can select 4096. Select luks1 or veracrypt for UFS1."));
                }
                tools.formatter = Some(require_command(linux_formatter(*filesystem)?)?);
                let module = linux_filesystem_module(*filesystem);
                if let Some(module) = module {
                    if !native_filesystem_available(module) {
                        return Err(AppError::new(format!("Writable {module} creation requires its matching native kernel module; refusing before storage mutation.")));
                    }
                }
                if *filesystem == Some(crate::api::CreateFilesystem::Ufs2) {
                    crate::trusted_system_executable("fuse-ufs")?;
                }
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

pub(crate) fn linux_filesystem_module(
    filesystem: Option<crate::api::CreateFilesystem>,
) -> Option<&'static str> {
    match filesystem {
        Some(crate::api::CreateFilesystem::Apfs) => Some("apfs"),
        Some(crate::api::CreateFilesystem::Hammer2) => Some("hammer2"),
        Some(crate::api::CreateFilesystem::Zfs) => Some("zfs"),
        Some(crate::api::CreateFilesystem::Ufs1) => Some("ufs"),
        Some(crate::api::CreateFilesystem::Ntfs) => match linux_ntfs_mount_driver() {
            "ntfs3" => Some("ntfs3"),
            "ntfs" => Some("ntfs"),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn linux_ntfs_mount_driver() -> &'static str {
    if native_filesystem_available("ntfs3") {
        return "ntfs3";
    }
    // The newer ntfs module is writable, unlike the legacy reader. Check its
    // explicit capability identity rather than assuming every ntfs module
    // is writable, including older kernels and vendor backports.
    if let Ok((program, arguments)) = linux_module_invocation(
        "modinfo",
        &["-F".into(), "description".into(), "ntfs".into()],
    ) {
        if Command::new(program)
            .args(arguments)
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && writable_ntfs_description(&String::from_utf8_lossy(&output.stdout))
            })
        {
            return "ntfs";
        }
    }
    "ntfs-3g"
}

fn writable_ntfs_description(description: &str) -> bool {
    description
        .lines()
        .any(|line| line.trim() == "NTFS read-write filesystem driver")
}

fn native_filesystem_available(name: &str) -> bool {
    if fs::read_to_string("/proc/filesystems").is_ok_and(|text| {
        text.lines()
            .any(|line| line.split_whitespace().last() == Some(name))
    }) {
        return true;
    }
    linux_module_invocation("modinfo", &[name.into()])
        .ok()
        .is_some_and(|(program, arguments)| {
            Command::new(program)
                .args(arguments)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
}

/// kmod selects its applet from argv[0], which canonical executable resolution
/// intentionally removes. A trusted env launcher preserves that name even when
/// sudo starts the canonical executable on our behalf.
pub(crate) fn linux_module_invocation(
    applet: &str,
    arguments: &[OsString],
) -> AppResult<(PathBuf, Vec<OsString>)> {
    if !matches!(applet, "modinfo" | "modprobe") {
        return Err(AppError::new("Unsupported Linux module applet."));
    }
    let program = crate::trusted_system_executable(applet)?;
    let launcher = crate::trusted_system_executable("env")?;
    let mut args = vec![
        format!("--argv0={applet}").into(),
        "--".into(),
        program.into_os_string(),
    ];
    args.extend_from_slice(arguments);
    Ok((launcher, args))
}

/// Explicit native Linux formatters. Unsupported selections fail before staging
/// or mutating either a file container or a physical target.
pub(crate) fn linux_formatter(
    filesystem: Option<crate::api::CreateFilesystem>,
) -> AppResult<&'static str> {
    use crate::api::CreateFilesystem::*;
    match filesystem.unwrap_or(Btrfs) {
        Btrfs => Ok("mkfs.btrfs"),
        Ext4 => Ok("mkfs.ext4"),
        Xfs => Ok("mkfs.xfs"),
        Ntfs => Ok("mkntfs"),
        Exfat => Ok("mkfs.exfat"),
        Fat32 => Ok("mkfs.fat"),
        Zfs => Ok("zpool"),
        Apfs => Ok("mkapfs"),
        Hammer2 => Ok("newfs_hammer2"),
        // The NetBSD makefs shipped by some Linux distributions writes a
        // different FFSv2 layout than the FreeBSD-only FUSE driver accepts.
        Ufs1 | Ufs2 => Ok("makefs-freebsd"),
    }
}

pub(crate) fn linux_format_args(
    filesystem: crate::api::CreateFilesystem,
    owner: crate::new_filesystem::Owner,
) -> Vec<OsString> {
    use crate::api::CreateFilesystem::*;
    match filesystem {
        Btrfs => vec!["-q".into()],
        Ext4 => vec![
            "-t".into(),
            "ext4".into(),
            "-q".into(),
            "-E".into(),
            format!("root_owner={}:{}", owner.uid, owner.gid).into(),
        ],
        Xfs => vec!["-q".into()],
        Ntfs => vec!["-Q".into(), "-F".into()],
        Exfat => vec![],
        Fat32 => vec!["-F".into(), "32".into()],
        Hammer2 => vec!["-L".into(), "DATA".into()],
        _ => vec![], // linux_formatter rejects unsupported selections first.
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

// The dynamic loader follows the creator's $ORIGIN/lib before main(). A
// trusted executable alone is insufficient when its companions are writable.
fn trusted_backend_file(path: &Path, limit: u64) -> AppResult<Vec<u8>> {
    crate::trust::trusted_runtime_file(path, limit)
}

fn creator_library_inventory(manifest: &serde_json::Value) -> AppResult<Vec<(String, String)>> {
    let invalid = || {
        AppError::new(
            "Invalid private creator library manifest; reinstall the complete Lurker package.",
        )
    };
    if manifest["schema"].as_u64() != Some(1) {
        return Err(invalid());
    }
    let libraries = manifest["libraries"].as_array().ok_or_else(invalid)?;
    if libraries.is_empty() || libraries.len() > 64 {
        return Err(invalid());
    }
    let mut inventory = Vec::new();
    for entry in libraries {
        let name = entry["name"].as_str().ok_or_else(invalid)?;
        let digest = entry["sha256"].as_str().ok_or_else(invalid)?;
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
            || name == "."
            || name == ".."
            || entry["soname"].as_str() != Some(name)
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || manifest["files"][format!("lib/{name}")]["sha256"].as_str() != Some(digest)
            || inventory.iter().any(|(existing, _)| existing == name)
        {
            return Err(invalid());
        }
        inventory.push((name.to_owned(), digest.to_owned()));
    }
    Ok(inventory)
}

fn validate_creator_libraries(program: &Path) -> AppResult<()> {
    let parent = program
        .parent()
        .ok_or_else(|| AppError::new("Invalid private creator path."))?;
    let directory = parent.join("lib");
    let manifest = parent.join("private-libraries.json");
    // Inspect the entry itself: try_exists follows links, so a dangling
    // $ORIGIN/lib symlink must not be mistaken for an older host-only build.
    let entry_exists = |path: &Path| match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(AppError::io("Cannot inspect private backend entry", err)),
    };
    // Older administrator-installed builds use only trusted host libraries.
    if !entry_exists(&directory)? && !entry_exists(&manifest)? {
        return Ok(());
    }
    let bytes = trusted_backend_file(&manifest, 1024 * 1024)?;
    let information: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::new(format!("Invalid private creator library manifest: {err}")))?;
    let inventory = creator_library_inventory(&information)?;
    let directory_metadata = fs::symlink_metadata(&directory)
        .map_err(|err| AppError::io("Cannot inspect private library directory", err))?;
    if !directory_metadata.is_dir()
        || directory_metadata.uid() != 0
        || directory_metadata.permissions().mode() & 0o022 != 0
    {
        return Err(AppError::new(
            "Private library directory is an untrusted path.",
        ));
    }
    for entry in fs::read_dir(&directory)
        .map_err(|err| AppError::io("Cannot list private libraries", err))?
    {
        let entry = entry.map_err(|err| AppError::io("Cannot inspect private library", err))?;
        if !inventory
            .iter()
            .any(|(name, _)| entry.file_name() == name.as_str())
        {
            return Err(AppError::new(
                "Uninventoried private library; reinstall the complete Lurker package.",
            ));
        }
    }
    for (name, expected) in inventory {
        let bytes = trusted_backend_file(&directory.join(name), 64 * 1024 * 1024)?;
        if crate::sha256::hex_digest(&bytes) != expected {
            return Err(AppError::new(
                "Private library hash mismatch; reinstall the complete Lurker package.",
            ));
        }
    }
    Ok(())
}

fn resolve_veracrypt_creator(will_elevate: bool) -> AppResult<PathBuf> {
    let root = crate::effective_uid() == 0;
    for candidate in creator_candidates(root) {
        if !is_executable(&candidate) {
            continue;
        }
        let program = if root || will_elevate {
            let program = trusted_root_program(&candidate)?;
            validate_creator_libraries(&program)?;
            program
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
    find_executable_with_path(name, env::var_os("PATH"))
}

fn find_executable_with_path(name: &str, paths: Option<OsString>) -> Option<PathBuf> {
    // An explicitly empty PATH is also used by callers to disable tool lookup.
    if paths.as_deref().is_some_and(|p| p.is_empty()) {
        return None;
    }
    let path = Path::new(name);
    if path.components().count() > 1 {
        return is_executable(path).then(|| path.to_path_buf());
    }

    for dir in paths
        .into_iter()
        .flat_map(|p| env::split_paths(&p).collect::<Vec<_>>())
        .chain(
            [
                "/usr/local/sbin",
                "/usr/local/bin",
                "/usr/sbin",
                "/sbin",
                "/usr/bin",
                "/bin",
            ]
            .into_iter()
            .map(PathBuf::from),
        )
    {
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

    #[test]
    fn linux_system_lookup_works_without_sbin_in_login_path() {
        let path =
            super::find_executable_with_path("true", Some("/nonexistent/lurker-path".into()))
                .unwrap();
        assert!(path.is_absolute() && super::is_executable(&path));
        assert!(super::find_executable_with_path("true", None).is_some());
        assert!(super::find_executable_with_path("true", Some("".into())).is_none());
        assert!(super::find_executable_with_path("lurker-no-such-tool", None).is_none());
    }

    #[test]
    fn native_ntfs_selection_rejects_legacy_reader_descriptions() {
        assert!(super::writable_ntfs_description(
            "NTFS read-write filesystem driver\n"
        ));
        for description in [
            "NTFS filesystem driver",
            "NTFS 1.2/3.x driver",
            "NTFS read-only filesystem driver",
            "",
        ] {
            assert!(!super::writable_ntfs_description(description));
        }
    }

    #[test]
    #[ignore = "requires native Linux kmod and an env launcher supporting --argv0"]
    fn canonical_linux_module_tools_keep_the_requested_applet() {
        for applet in ["modinfo", "modprobe"] {
            let (program, arguments) =
                super::linux_module_invocation(applet, &["--version".into()]).unwrap();
            let output = std::process::Command::new(program)
                .args(arguments)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{applet}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("kmod"));
        }
    }

    #[test]
    #[ignore = "formats a private image with the installed native mke2fs"]
    fn canonical_ext4_formatter_creates_a_journaled_ext4_superblock() {
        use std::os::unix::fs::{FileExt, OpenOptionsExt, PermissionsExt};
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("lurker-ext4-{nonce}"));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let image = directory.join("native.img");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&image)
            .unwrap()
            .set_len(64 * 1024 * 1024)
            .unwrap();
        let owner = crate::new_filesystem::Owner {
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
        };
        let mut arguments = super::linux_format_args(crate::api::CreateFilesystem::Ext4, owner);
        arguments.extend(["-F".into(), image.as_os_str().to_os_string()]);
        // Execute the canonical multicall binary, as privileged execution does.
        let program = crate::trusted_system_executable("mke2fs").unwrap();
        let output = std::process::Command::new(program)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut features = [0; 8];
        std::fs::File::open(&image)
            .unwrap()
            .read_exact_at(&mut features, 1024 + 92)
            .unwrap();
        let compatible = u32::from_le_bytes(features[..4].try_into().unwrap());
        let incompatible = u32::from_le_bytes(features[4..].try_into().unwrap());
        assert_ne!(
            compatible & 0x4,
            0,
            "ext4 journal absent: {}",
            image.display()
        );
        assert_ne!(
            incompatible & 0x40,
            0,
            "ext4 extents absent: {}",
            image.display()
        );
        std::fs::remove_file(image).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    fn subprocess_context() -> super::AppContext {
        super::AppContext {
            output: crate::output::Output::buffered().0,
            tools: super::ToolPaths {
                cryptsetup: "/bin/false".into(),
                formatter: None,
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
            loop_lease: None,
        }
    }

    #[test]
    fn creator_manifest_rejects_escaping_duplicate_or_unbound_library_paths() {
        let digest = "a".repeat(64);
        let valid = serde_json::json!({"schema": 1, "libraries": [
            {"name":"libtest.so.1", "soname":"libtest.so.1", "sha256":digest}],
            "files":{"lib/libtest.so.1":{"sha256":digest}}});
        assert_eq!(super::creator_library_inventory(&valid).unwrap().len(), 1);
        let mut traversal = valid.clone();
        traversal["libraries"][0]["name"] = "../outside.so".into();
        assert!(super::creator_library_inventory(&traversal).is_err());
        let mut duplicate = valid.clone();
        duplicate["libraries"]
            .as_array_mut()
            .unwrap()
            .push(valid["libraries"][0].clone());
        assert!(super::creator_library_inventory(&duplicate).is_err());
        let mut unbound = valid;
        unbound["files"]["lib/libtest.so.1"]["sha256"] = "b".repeat(64).into();
        assert!(super::creator_library_inventory(&unbound).is_err());
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
