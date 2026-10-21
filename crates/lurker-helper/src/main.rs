use lurker_core::{run_buffered, CommandAction, OperationResponse, OutputEntry};
use std::env;
use std::io::{IsTerminal, Read, Write};
use std::process::{Command, Output, Stdio};

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let mut args = env::args().skip(1);
    let mut as_root = false;
    let mut action = args.next();
    if action.as_deref() == Some("--as-root") {
        as_root = true;
        action = args.next();
    }

    match action.as_deref() {
        Some("run") => {
            let response = handle_run(as_root);
            let mut stdout = std::io::stdout().lock();
            let _ = serde_json::to_writer(&mut stdout, &response);
            let _ = stdout.write_all(b"\n");
            let _ = stdout.flush();
            0
        }
        _ => {
            eprintln!("usage: lurker-helper [--as-root] run");
            64
        }
    }
}

fn handle_run(as_root: bool) -> OperationResponse {
    let payload = match read_stdin() {
        Ok(payload) => payload,
        Err(message) => return operation_error(message),
    };

    // macOS disk images and VeraCrypt run in the login user session.
    // VeraCrypt owns any platform authorization it needs.
    if cfg!(target_os = "macos") || as_root || is_root() {
        return run_direct(&payload);
    }

    rerun_with_elevation(&payload)
}

fn run_direct(payload: &[u8]) -> OperationResponse {
    match serde_json::from_slice::<CommandAction>(payload) {
        Ok(command) => run_buffered(command),
        Err(err) => operation_error(format!("Failed to decode helper request: {err}")),
    }
}

fn rerun_with_elevation(payload: &[u8]) -> OperationResponse {
    match rerun_with_pkexec(payload) {
        Ok(response) => response,
        Err(pkexec_error) => {
            if !has_terminal_for_sudo() {
                return operation_error(pkexec_error);
            }

            match rerun_with_sudo(payload) {
                Ok(response) => response,
                Err(sudo_error) => operation_error(format!("{pkexec_error}\n\n{sudo_error}")),
            }
        }
    }
}

fn rerun_with_pkexec(payload: &[u8]) -> Result<OperationResponse, String> {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(err) => return Err(format!("Failed to locate helper executable: {err}")),
    };

    let output = spawn_reexec("pkexec", &current_exe, payload)?;
    decode_reexec_output("pkexec", output)
}

fn rerun_with_sudo(payload: &[u8]) -> Result<OperationResponse, String> {
    let current_exe =
        env::current_exe().map_err(|err| format!("Failed to locate helper executable: {err}"))?;
    let output = spawn_reexec("sudo", &current_exe, payload)?;
    decode_reexec_output("sudo", output)
}

fn spawn_reexec(
    program: &str,
    current_exe: &std::path::Path,
    payload: &[u8],
) -> Result<Output, String> {
    let mut command = Command::new(program);
    command
        .arg(current_exe)
        .arg("--as-root")
        .arg("run")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());

    if program == "sudo" && has_terminal_for_sudo() {
        command.stderr(Stdio::inherit());
    } else {
        command.stderr(Stdio::piped());
    }

    let mut child = command
        .spawn()
        .map_err(|err| format!("Failed to start {program} for privileged action: {err}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload)
            .map_err(|err| format!("Failed to forward helper request to {program}: {err}"))?;
    }

    child
        .wait_with_output()
        .map_err(|err| format!("Failed waiting for {program} helper: {err}"))
}

fn decode_reexec_output(program: &str, output: Output) -> Result<OperationResponse, String> {
    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(format!(
                "{program} helper failed with exit status {:?}.",
                output.status.code()
            ));
        }
        return Err(stderr);
    }

    serde_json::from_slice::<OperationResponse>(&output.stdout)
        .map_err(|err| format!("Failed to decode privileged helper response from {program}: {err}"))
}

fn read_stdin() -> Result<Vec<u8>, String> {
    let mut payload = Vec::new();
    std::io::stdin()
        .read_to_end(&mut payload)
        .map_err(|err| format!("Failed to read helper request: {err}"))?;
    if payload.is_empty() {
        return Err("Helper request was empty.".into());
    }
    Ok(payload)
}

fn is_root() -> bool {
    lurker_core::effective_uid() == 0
}

fn has_terminal_for_sudo() -> bool {
    std::io::stderr().is_terminal()
}

fn operation_error(message: impl Into<String>) -> OperationResponse {
    let message = message.into();
    OperationResponse {
        ok: false,
        logs: vec![OutputEntry::error(message.clone())],
        error: Some(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn malformed_payload_is_a_structured_error() {
        let response = run_direct(b"{bad json}");
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("decode helper request"));
    }

    #[test]
    fn elevated_output_must_be_valid_json() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: b"invalid".to_vec(),
            stderr: vec![],
        };
        assert!(decode_reexec_output("test", output).is_err());
        let response = operation_error("cancelled");
        let output = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: serde_json::to_vec(&response).unwrap(),
            stderr: vec![],
        };
        assert!(!decode_reexec_output("test", output).unwrap().ok);
    }

    #[test]
    fn elevation_failures_keep_the_error_message() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: b"authorization cancelled".to_vec(),
        };
        assert_eq!(
            decode_reexec_output("test", output).unwrap_err(),
            "authorization cancelled"
        );
    }

    #[test]
    fn root_detection_matches_effective_identity() {
        assert_eq!(is_root(), lurker_core::probe_system().unwrap().is_root);
    }
}
