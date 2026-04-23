mod helper;
mod logic;

use crate::helper::{resolve_helper_path, run_helper};
use crate::logic::{
    active_volume_items, build_create_command, build_mount_command, build_unmount_command,
    response_error, suggested_unmount_target,
};
use lurker_core::{list_active_volumes, ActiveVolume, CommandAction};
use slint::{BackendSelector, ComponentHandle, ModelRc, VecModel, Weak};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

slint::include_modules!();

#[derive(Clone)]
struct AppState {
    window: Weak<MainWindow>,
    helper_path: Option<PathBuf>,
    helper_error: Option<String>,
    active_volumes: Arc<Mutex<Vec<ActiveVolume>>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;

    let window = MainWindow::new()?;
    let helper_path = resolve_helper_path();
    let state = AppState {
        window: window.as_weak(),
        helper_path: helper_path.as_ref().ok().cloned(),
        helper_error: helper_path.err(),
        active_volumes: Arc::new(Mutex::new(Vec::new())),
    };

    bind_callbacks(&window, &state);
    state.refresh_active_volumes(false);
    window.run()?;
    Ok(())
}

fn bind_callbacks(window: &MainWindow, state: &AppState) {
    let refresh_state = state.clone();
    window.on_refresh_volumes(move || refresh_state.refresh_active_volumes(true));

    let selection_state = state.clone();
    window.on_active_volume_picked(move |index| selection_state.prefill_unmount_target(index));

    let create_state = state.clone();
    window.on_submit_create(move || {
        let Some(window) = create_state.window.upgrade() else {
            return;
        };

        match build_create_command(
            &window.get_create_target(),
            &window.get_create_size_gb(),
            window.get_create_force(),
            &window.get_create_source_kind(),
            &window.get_create_volume_type(),
            &window.get_create_cipher(),
            &window.get_create_passphrase(),
            &window.get_create_passphrase_confirm(),
        ) {
            Ok(command) => create_state.run_operation(
                "Creating volume…",
                "Volume created.",
                CommandAction::Create(command),
                ResetKind::CreateSecrets,
            ),
            Err(message) => {
                clear_create_secrets(&window);
                set_error(&window, &message);
            }
        }
    });

    let mount_state = state.clone();
    window.on_submit_mount(move || {
        let Some(window) = mount_state.window.upgrade() else {
            return;
        };

        match build_mount_command(
            &window.get_mount_source(),
            &window.get_mount_mountpoint(),
            &window.get_mount_tag(),
            &window.get_mount_volume_type(),
            &window.get_mount_passphrase(),
        ) {
            Ok(command) => mount_state.run_operation(
                "Mounting volume…",
                "Volume mounted.",
                CommandAction::Mount(command),
                ResetKind::MountSecrets,
            ),
            Err(message) => {
                clear_mount_secrets(&window);
                set_error(&window, &message);
            }
        }
    });

    let unmount_state = state.clone();
    window.on_submit_unmount(move || {
        let Some(window) = unmount_state.window.upgrade() else {
            return;
        };

        match build_unmount_command(
            &window.get_unmount_target(),
            &window.get_unmount_tag(),
            &window.get_unmount_volume_type(),
        ) {
            Ok(command) => unmount_state.run_operation(
                "Unmounting volume…",
                "Volume unmounted.",
                CommandAction::Unmount(command),
                ResetKind::None,
            ),
            Err(message) => set_error(&window, &message),
        }
    });
}

impl AppState {
    fn refresh_active_volumes(&self, announce: bool) {
        if let Some(window) = self.window.upgrade() {
            if announce {
                set_busy(&window, "Refreshing active volumes…");
            } else {
                window.set_error_message("".into());
            }
        }

        let window = self.window.clone();
        let store = Arc::clone(&self.active_volumes);
        thread::spawn(move || {
            let result = list_active_volumes().map_err(|err| err.message);
            let _ = window.upgrade_in_event_loop(move |ui| {
                ui.set_busy(false);
                ui.set_busy_label("".into());

                match result {
                    Ok(volumes) => {
                        replace_active_volumes(&store, &ui, volumes);
                        ui.set_error_message("".into());
                        if announce {
                            ui.set_status_message("Active volumes refreshed.".into());
                        }
                    }
                    Err(message) => {
                        if announce {
                            ui.set_status_message("".into());
                        }
                        ui.set_error_message(message.into());
                    }
                }
            });
        });
    }

    fn prefill_unmount_target(&self, index: i32) {
        if index < 0 {
            return;
        }

        let Ok(guard) = self.active_volumes.lock() else {
            return;
        };
        let Some(volume) = guard.get(index as usize) else {
            return;
        };
        let target = suggested_unmount_target(volume);

        if let Some(window) = self.window.upgrade() {
            window.set_unmount_target(target.into());
        }
    }

    fn run_operation(
        &self,
        busy_label: &'static str,
        success_message: &'static str,
        command: CommandAction,
        reset: ResetKind,
    ) {
        let Some(helper_path) = self.helper_path.clone() else {
            let Some(window) = self.window.upgrade() else {
                return;
            };
            let message = self
                .helper_error
                .clone()
                .unwrap_or_else(|| "Failed to locate lurker-helper.".into());
            apply_reset(&window, reset);
            set_error(&window, &message);
            return;
        };

        if let Some(window) = self.window.upgrade() {
            set_busy(&window, busy_label);
        }

        let window = self.window.clone();
        let store = Arc::clone(&self.active_volumes);
        thread::spawn(move || {
            let helper_result = run_helper(&helper_path, command);
            let refreshed_volumes = match &helper_result {
                Ok(response) if response.ok => list_active_volumes().ok(),
                _ => None,
            };

            let _ = window.upgrade_in_event_loop(move |ui| {
                ui.set_busy(false);
                ui.set_busy_label("".into());
                apply_reset(&ui, reset);

                match helper_result {
                    Ok(response) if response.ok => {
                        if let Some(volumes) = refreshed_volumes {
                            replace_active_volumes(&store, &ui, volumes);
                        }
                        ui.set_error_message("".into());
                        ui.set_status_message(success_message.into());
                    }
                    Ok(response) => {
                        ui.set_status_message("".into());
                        ui.set_error_message(response_error(&response).into());
                    }
                    Err(message) => {
                        ui.set_status_message("".into());
                        ui.set_error_message(message.into());
                    }
                }
            });
        });
    }
}

#[derive(Clone, Copy)]
enum ResetKind {
    None,
    CreateSecrets,
    MountSecrets,
}

fn replace_active_volumes(
    store: &Arc<Mutex<Vec<ActiveVolume>>>,
    window: &MainWindow,
    volumes: Vec<ActiveVolume>,
) {
    let items = active_volume_items(&volumes);
    let model = Rc::new(VecModel::from(items));

    if let Ok(mut guard) = store.lock() {
        *guard = volumes;
    }

    window.set_active_volume_items(ModelRc::from(model));
}

fn set_busy(window: &MainWindow, message: &str) {
    window.set_busy(true);
    window.set_busy_label(message.into());
    window.set_status_message("".into());
    window.set_error_message("".into());
}

fn set_error(window: &MainWindow, message: &str) {
    window.set_busy(false);
    window.set_busy_label("".into());
    window.set_status_message("".into());
    window.set_error_message(message.into());
}

fn apply_reset(window: &MainWindow, reset: ResetKind) {
    match reset {
        ResetKind::None => {}
        ResetKind::CreateSecrets => clear_create_secrets(window),
        ResetKind::MountSecrets => clear_mount_secrets(window),
    }
}

fn clear_create_secrets(window: &MainWindow) {
    window.set_create_passphrase("".into());
    window.set_create_passphrase_confirm("".into());
}

fn clear_mount_secrets(window: &MainWindow) {
    window.set_mount_passphrase("".into());
}
