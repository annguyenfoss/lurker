use lurker_core::{run_buffered, CommandAction, OperationResponse, OutputEntry};
use std::env;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

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

    if as_root || is_root() {
        return run_direct(&payload);
    }

    rerun_with_pkexec(&payload)
}

fn run_direct(payload: &[u8]) -> OperationResponse {
    match serde_json::from_slice::<CommandAction>(payload) {
        Ok(command) => run_buffered(command),
        Err(err) => operation_error(format!("Failed to decode helper request: {err}")),
    }
}

fn rerun_with_pkexec(payload: &[u8]) -> OperationResponse {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(err) => return operation_error(format!("Failed to locate helper executable: {err}")),
    };

    let mut child = match Command::new("pkexec")
        .arg(&current_exe)
        .arg("--as-root")
        .arg("run")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return operation_error(format!(
                "Failed to start pkexec for privileged action: {err}"
            ));
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(err) = stdin.write_all(payload) {
            return operation_error(format!("Failed to forward helper request to pkexec: {err}"));
        }
    }

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(err) => return operation_error(format!("Failed waiting for pkexec helper: {err}")),
    };

    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!(
                "Privileged helper failed with exit status {:?}.",
                output.status.code()
            )
        } else {
            stderr
        };
        return operation_error(message);
    }

    match serde_json::from_slice::<OperationResponse>(&output.stdout) {
        Ok(response) => response,
        Err(err) => operation_error(format!(
            "Failed to decode privileged helper response: {err}"
        )),
    }
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
    env::var("EUID")
        .ok()
        .or_else(|| env::var("UID").ok())
        .and_then(|value| value.parse::<u32>().ok())
        .map(|uid| uid == 0)
        .unwrap_or(false)
}

fn operation_error(message: impl Into<String>) -> OperationResponse {
    let message = message.into();
    OperationResponse {
        ok: false,
        logs: vec![OutputEntry::error(message.clone())],
        error: Some(message),
    }
}
