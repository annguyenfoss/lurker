use lurker_core::{run_buffered, CommandAction, OperationResponse, OutputEntry};
use std::env;
use std::io::{IsTerminal, Read, Write};
use std::process::{Command, Output, Stdio};

fn main() {
    if let Some(code) = lurker_core::run_folder_supervisor_if_requested() {
        std::process::exit(code);
    }
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
        Some("capabilities") => {
            println!(
                "{}",
                serde_json::json!({
                    "protocol_version": 3,
                    "directory_backends": true,
                    "folder_supervision_protocol": 1,
                    "raid1c_members": cfg!(target_os = "openbsd"),
                    "noninteractive_sudo": !cfg!(target_os = "macos"),
                    "trusted_privileged_tools": !cfg!(target_os = "macos"),
                    "unprivileged_veracrypt": cfg!(target_os = "linux"),
                    "userspace_backend_protocol": if cfg!(target_os = "linux") { Some(3) } else { None },
                    "linux_loop_lease_protocol": if cfg!(target_os = "linux") { Some(1) } else { None },
                    "keyfile_authentication": true,
                    "luks_keyslots": cfg!(target_os = "linux"),
                })
            );
            0
        }
        #[cfg(target_os = "linux")]
        Some("loop-lease") => {
            let Some(endpoint) = args.next().filter(|_| !as_root) else {
                return 64;
            };
            if args.next().is_some() {
                return 64;
            }
            match lurker_core::serve_loop_lease(std::path::Path::new(&endpoint)) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("{}", error.message);
                    error.exit_code
                }
            }
        }
        Some("run") => {
            let response = handle_run(as_root);
            let mut stdout = std::io::stdout().lock();
            let _ = serde_json::to_writer(&mut stdout, &response);
            let _ = stdout.write_all(b"\n");
            let _ = stdout.flush();
            0
        }
        _ => {
            eprintln!("usage: lurker-helper [--as-root] run | capabilities");
            64
        }
    }
}

fn handle_run(as_root: bool) -> OperationResponse {
    let payload = match read_stdin() {
        Ok(payload) => payload,
        Err(message) => return operation_error(message),
    };
    handle_payload(&payload, as_root)
}

fn handle_payload(payload: &[u8], as_root: bool) -> OperationResponse {
    let command = match serde_json::from_slice::<CommandAction>(payload) {
        Ok(command) => command,
        Err(error) => return operation_error(format!("Failed to decode helper request: {error}")),
    };
    if let Err(error) = lurker_core::validate_request(&command) {
        return operation_error(error.message);
    }

    let user_session = match lurker_core::request_requires_user_session(&command) {
        Ok(value) => value,
        Err(error) => return operation_error(error.message),
    };
    // macOS disk images and VeraCrypt run in the login user session.
    // VeraCrypt owns any platform authorization it needs.
    if user_session
        || command.privilege_mode() == lurker_core::PrivilegeMode::Unprivileged
        || cfg!(target_os = "macos")
        || as_root
        || is_root()
    {
        return run_direct(payload);
    }

    rerun_with_elevation(payload)
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
        Err(pkexec_error) => match rerun_with_sudo(payload) {
            Ok(response) => response,
            Err(sudo_error) => operation_error(format!("{pkexec_error}\n\n{sudo_error}")),
        },
    }
}

fn rerun_with_pkexec(payload: &[u8]) -> Result<OperationResponse, String> {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(err) => return Err(format!("Failed to locate helper executable: {err}")),
    };

    let current_exe =
        lurker_core::trusted_privileged_executable(&current_exe).map_err(|e| e.message)?;
    let output = spawn_reexec("pkexec", &current_exe, payload)?;
    decode_reexec_output("pkexec", output)
}

fn rerun_with_sudo(payload: &[u8]) -> Result<OperationResponse, String> {
    let current_exe =
        env::current_exe().map_err(|err| format!("Failed to locate helper executable: {err}"))?;
    let current_exe =
        lurker_core::trusted_privileged_executable(&current_exe).map_err(|e| e.message)?;
    let output = spawn_reexec("sudo", &current_exe, payload)?;
    decode_reexec_output("sudo", output)
}

fn spawn_reexec(
    program: &str,
    current_exe: &std::path::Path,
    payload: &[u8],
) -> Result<Output, String> {
    let executable = lurker_core::trusted_system_executable(program).map_err(|e| e.message)?;
    let mut command = reexec_command(&executable, program, current_exe, has_terminal_for_sudo());

    let mut child = command
        .spawn()
        .map_err(|err| format!("Failed to start {program} for privileged action: {err}"))?;

    let write_error = child
        .stdin
        .take()
        .and_then(|mut stdin| stdin.write_all(payload).err());
    let output = child
        .wait_with_output()
        .map_err(|err| format!("Failed waiting for {program} helper: {err}"))?;
    // An authorization denial can close stdin before the JSON is delivered.
    // Preserve sudo/polkit's actual diagnosis and always reap the child.
    if let Some(error) = write_error {
        if output.stdout.is_empty() && output.stderr.is_empty() {
            return Err(format!(
                "Failed to forward helper request to {program}: {error}"
            ));
        }
    }
    Ok(output)
}

fn reexec_command(
    executable: &std::path::Path,
    program: &str,
    current_exe: &std::path::Path,
    terminal: bool,
) -> Command {
    let mut command = Command::new(executable);
    if program == "sudo" && !terminal {
        command.arg("-n");
    }
    command
        .arg(current_exe)
        .arg("--as-root")
        .arg("run")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if program == "sudo" && terminal {
        command.stderr(Stdio::inherit());
    } else {
        command.stderr(Stdio::piped());
    }
    command
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
    fn nonterminal_sudo_cannot_prompt_or_consume_a_password_from_json() {
        let helper = std::path::Path::new("/usr/libexec/lurker/lurker-helper");
        for (program, terminal, expected) in [
            (
                "sudo",
                false,
                vec![
                    "-n",
                    "/usr/libexec/lurker/lurker-helper",
                    "--as-root",
                    "run",
                ],
            ),
            (
                "sudo",
                true,
                vec!["/usr/libexec/lurker/lurker-helper", "--as-root", "run"],
            ),
            (
                "pkexec",
                false,
                vec!["/usr/libexec/lurker/lurker-helper", "--as-root", "run"],
            ),
        ] {
            let executable = std::path::Path::new("/usr/bin").join(program);
            let command = reexec_command(&executable, program, helper, terminal);
            assert_eq!(command.get_program(), executable);
            assert_eq!(command.get_args().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn noninteractive_authorization_denial_is_preserved() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: b"sudo: a password is required".to_vec(),
        };
        assert_eq!(
            decode_reexec_output("sudo", output).unwrap_err(),
            "sudo: a password is required"
        );
    }

    #[test]
    fn malformed_payload_is_a_structured_error() {
        let response = run_direct(b"{bad json}");
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("decode helper request"));
    }

    #[test]
    fn unprivileged_requests_fail_directly_without_trying_elevation() {
        let response = handle_payload(br#"{"action":"create","privilege_mode":"unprivileged","target":"unused","size_gb":"1","force":false,"source_kind":"file","volume_type":"luks","cipher":"aes","passphrase":"private"}"#, false);
        assert!(!response.ok);
        #[cfg(target_os = "linux")]
        assert!(response.error.unwrap().contains("supports VeraCrypt"));
        #[cfg(target_os = "macos")]
        assert!(response.error.unwrap().contains("not supported on macos"));
        assert!(handle_payload(b"{invalid", false)
            .error
            .unwrap()
            .contains("decode helper request"));
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
