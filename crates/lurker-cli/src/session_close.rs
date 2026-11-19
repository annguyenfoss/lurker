use lurker_core::{
    effective_uid, run_configured_close, trusted_privileged_executable,
    trusted_privileged_storage_source, AppError, AppResult, Output, PrivilegeMode, UnmountCommand,
    VolumeType,
};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::FileExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn close(
    mut command: UnmountCommand,
    timeout: Option<Duration>,
    output: &Output,
) -> AppResult<()> {
    let login_directory =
        command.volume_type.is_directory() && command.volume_type != VolumeType::Pefs;
    let unprivileged = command.privilege_mode == PrivilegeMode::Unprivileged;
    if unprivileged && !cfg!(target_os = "linux") {
        return Err(AppError::new("Unprivileged mode requires Linux."));
    }
    if unprivileged && !login_directory && command.volume_type != VolumeType::Veracrypt {
        return Err(AppError::new(
            "Unprivileged session close requires VeraCrypt or a login-user directory backend.",
        ));
    }
    let root = effective_uid() == 0;
    if root == (login_directory || unprivileged) {
        return Err(AppError::new(if root {
            "Run this hook as the login user that owns the vault session."
        } else {
            "Run this native session hook with existing administrator authority."
        }));
    }

    let Some(timeout) = timeout else {
        if root {
            command.target = trusted_privileged_storage_source(&command.target)?;
        }
        let metadata = fs::metadata(&command.target)
            .map_err(|error| AppError::io("Inspect the configured vault source", error))?;
        let is_source = if command.volume_type.is_directory() {
            metadata.is_dir()
        } else {
            metadata.is_file()
        };
        if !is_source {
            return Err(AppError::new(
                "Configure the encrypted directory or container file as the vault source.",
            ));
        }
        return run_configured_close(command, output.clone());
    };
    let executable = std::env::current_exe()
        .map_err(|error| AppError::io("Find the installed Lurker CLI", error))?;
    let executable = if root {
        trusted_privileged_executable(&executable)?
    } else {
        executable
    };

    // Reuse the actual CLI/core close path. An anonymous regular file keeps
    // the worker's output away from caller pipes even if it outlives the hook.
    let log =
        tempfile::tempfile().map_err(|error| AppError::io("Open session-close log", error))?;
    let mut process = Command::new(executable);
    process
        .args([
            "__session-close-worker",
            "--color=never",
            "--type",
            command.volume_type.as_str(),
        ])
        .current_dir("/")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(
            log.try_clone()
                .map_err(|error| AppError::io("Open close stdout", error))?,
        )
        .stderr(
            log.try_clone()
                .map_err(|error| AppError::io("Open close stderr", error))?,
        );
    if unprivileged {
        process.arg("--unprivileged");
    }
    let mut child = process
        .arg("--")
        .arg(command.target)
        .spawn()
        .map_err(|error| AppError::io("Start normal vault close", error))?;
    let status = wait_for_close(&mut child, timeout)
        .map_err(|error| AppError::io("Observe vault close", error))?;
    replay_log(&log, status.is_some_and(|status| status.success()), output);
    match status {
        Some(status) if status.success() => Ok(()),
        Some(status) => Err(AppError {
            message: "Normal vault close did not complete. Check the diagnostic and vault state.".into(),
            exit_code: status.code().unwrap_or(1),
        }),
        None => Err(AppError {
            message: format!(
                "Timed out waiting for vault close (PID {}). Close may still be running. Keep the session available and check its state before retrying.",
                child.id()
            ),
            exit_code: 124,
        }),
    }
}

fn wait_for_close(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(None);
        }
        thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

fn replay_log(log: &File, success: bool, output: &Output) {
    let mut bytes = [0; 8192];
    if let Ok(length) = log.read_at(&mut bytes, 0) {
        let text = String::from_utf8_lossy(&bytes[..length]);
        if success {
            output.raw_stdout(&text);
        } else {
            output.raw_stderr(&text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_leaves_worker_alive_to_complete() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 1"])
            .spawn()
            .unwrap();
        let timed_out = wait_for_close(&mut child, Duration::ZERO)
            .unwrap()
            .is_none();
        let status = child.wait().unwrap();
        assert!(timed_out);
        assert!(status.success());
    }
}
