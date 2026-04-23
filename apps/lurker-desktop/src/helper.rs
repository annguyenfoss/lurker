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
