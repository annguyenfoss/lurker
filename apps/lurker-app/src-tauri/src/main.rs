#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lurker_core::{
    list_active_volumes as core_list_active_volumes, probe_system as core_probe_system,
    ActiveVolume, CommandAction, CreateCommand, MountCommand, OperationResponse, SystemProbe,
    UnmountCommand,
};
use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

#[tauri::command]
fn probe_system() -> Result<SystemProbe, String> {
    core_probe_system().map_err(|err| err.message)
}

#[tauri::command]
fn list_active_volumes() -> Result<Vec<ActiveVolume>, String> {
    core_list_active_volumes().map_err(|err| err.message)
}

#[tauri::command]
async fn create_volume(
    app: AppHandle,
    request: CreateCommand,
) -> Result<OperationResponse, String> {
    run_helper(app, CommandAction::Create(request)).await
}

#[tauri::command]
async fn mount_volume(app: AppHandle, request: MountCommand) -> Result<OperationResponse, String> {
    run_helper(app, CommandAction::Mount(request)).await
}

#[tauri::command]
async fn unmount_volume(
    app: AppHandle,
    request: UnmountCommand,
) -> Result<OperationResponse, String> {
    run_helper(app, CommandAction::Unmount(request)).await
}

async fn run_helper(app: AppHandle, command: CommandAction) -> Result<OperationResponse, String> {
    let payload = serde_json::to_vec(&command)
        .map_err(|err| format!("Failed to encode helper request: {err}"))?;

    let sidecar = app
        .shell()
        .sidecar("lurker-helper")
        .map_err(|err| format!("Failed to resolve lurker-helper sidecar: {err}"))?;

    let (mut rx, mut child) = sidecar
        .args(["run"])
        .spawn()
        .map_err(|err| format!("Failed to spawn lurker-helper: {err}"))?;

    child
        .write(&payload)
        .map_err(|err| format!("Failed to write helper request: {err}"))?;
    drop(child);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_code = Some(0);

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                stdout.extend_from_slice(&line);
                stdout.push(b'\n');
            }
            CommandEvent::Stderr(line) => {
                stderr.extend_from_slice(&line);
                stderr.push(b'\n');
            }
            CommandEvent::Error(err) => {
                return Err(format!("lurker-helper execution failed: {err}"));
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code;
            }
            _ => {}
        }
    }

    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(stderr);
        }
        return Err(format!(
            "lurker-helper returned no output (exit code {:?}).",
            exit_code
        ));
    }

    serde_json::from_slice::<OperationResponse>(&stdout)
        .map_err(|err| format!("Failed to decode helper response: {err}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            probe_system,
            list_active_volumes,
            create_volume,
            mount_volume,
            unmount_volume
        ])
        .run(tauri::generate_context!())
        .expect("failed to run lurker app");
}
