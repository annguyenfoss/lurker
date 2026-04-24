mod helper;
mod logic;

use crate::helper::{resolve_helper_path, run_helper};
use crate::logic::{
    build_create_command, build_mount_command, build_unmount_command_for_volume, response_error,
    volume_rows,
};
use lurker_core::{list_active_volumes, ActiveVolume, CommandAction};
use slint::{BackendSelector, ComponentHandle, ModelRc, Timer, VecModel, Weak};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

slint::include_modules!();

const TOAST_DWELL_MS: u64 = 2800;
static TOAST_ID: AtomicI32 = AtomicI32::new(1);

#[derive(Clone)]
struct AppState {
    window: Weak<MainWindow>,
    helper_path: Option<PathBuf>,
    helper_error: Option<String>,
    active_volumes: Arc<Mutex<Vec<ActiveVolume>>>,
    toasts: Arc<Mutex<Vec<ToastData>>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("skia".into())
        .select()?;

    let window = MainWindow::new()?;
    let helper_path = resolve_helper_path();
    let state = AppState {
        window: window.as_weak(),
        helper_path: helper_path.as_ref().ok().cloned(),
        helper_error: helper_path.err(),
        active_volumes: Arc::new(Mutex::new(Vec::new())),
        toasts: Arc::new(Mutex::new(Vec::new())),
    };

    bind_callbacks(&window, &state);
    state.refresh_active_volumes(false);
    window.run()?;
    Ok(())
}

fn bind_callbacks(window: &MainWindow, state: &AppState) {
    // ── Volumes refresh ──
    let rs = state.clone();
    window.on_refresh_volumes(move || rs.refresh_active_volumes(true));

    // ── Submit Create ──
    let cs = state.clone();
    window.on_submit_create(move || {
        let Some(w) = cs.window.upgrade() else { return };
        match build_create_command(
            &w.get_create_target_kind(),
            &w.get_create_format(),
            &w.get_create_file_path(),
            &w.get_create_file_name(),
            &w.get_create_partition(),
            &w.get_create_size(),
            &w.get_create_size_unit(),
            &w.get_create_cipher(),
            &w.get_create_passphrase(),
            &w.get_create_confirm(),
        ) {
            Ok(command) => cs.run_operation(
                "Container created",
                CommandAction::Create(command),
                ResetKind::CreateSecrets,
            ),
            Err(message) => {
                clear_create_secrets(&w);
                cs.push_toast("err", "Create failed", &message);
            }
        }
    });

    // ── Submit Mount ──
    let ms = state.clone();
    window.on_submit_mount(move || {
        let Some(w) = ms.window.upgrade() else { return };
        match build_mount_command(
            &w.get_mount_source(),
            &w.get_mount_point(),
            &w.get_mount_auth_method(),
            &w.get_mount_passphrase(),
            &w.get_mount_key_file(),
            w.get_mount_readonly(),
            &w.get_mount_source_kind(),
        ) {
            Ok(command) => ms.run_operation(
                "Volume mounted",
                CommandAction::Mount(command),
                ResetKind::MountSecrets,
            ),
            Err(message) => {
                clear_mount_secrets(&w);
                ms.push_toast("err", "Mount failed", &message);
            }
        }
    });

    // ── Reset Mount form ──
    let rms = window.as_weak();
    window.on_reset_mount(move || {
        if let Some(w) = rms.upgrade() {
            w.set_mount_source("".into());
            w.set_mount_point("/mnt/".into());
            w.set_mount_passphrase("".into());
            w.set_mount_key_file("".into());
            w.set_mount_auth_method("pass".into());
            w.set_mount_readonly(false);
        }
    });

    // ── Unmount (from volume row index) ──
    let us = state.clone();
    window.on_unmount_volume(move |index| us.unmount_volume(index));
}

impl AppState {
    fn refresh_active_volumes(&self, announce: bool) {
        let window = self.window.clone();
        let store = Arc::clone(&self.active_volumes);
        let state = self.clone();
        thread::spawn(move || {
            let result = list_active_volumes().map_err(|err| err.message);
            let _ = window.upgrade_in_event_loop(move |ui| match result {
                Ok(volumes) => {
                    replace_active_volumes(&store, &ui, volumes);
                    if announce {
                        state.push_toast("info", "Volumes refreshed", "");
                    }
                }
                Err(message) => {
                    state.push_toast("err", "Refresh failed", &message);
                }
            });
        });
    }

    fn unmount_volume(&self, index: i32) {
        if index < 0 {
            return;
        }
        let Ok(guard) = self.active_volumes.lock() else { return };
        let Some(volume) = guard.get(index as usize).cloned() else { return };
        drop(guard);
        let command = build_unmount_command_for_volume(&volume);
        self.run_operation(
            "Volume unmounted",
            CommandAction::Unmount(command),
            ResetKind::None,
        );
    }

    fn run_operation(
        &self,
        success_message: &'static str,
        command: CommandAction,
        reset: ResetKind,
    ) {
        let Some(helper_path) = self.helper_path.clone() else {
            let message = self
                .helper_error
                .clone()
                .unwrap_or_else(|| "Failed to locate lurker-helper.".into());
            if let Some(window) = self.window.upgrade() {
                apply_reset(&window, reset);
            }
            self.push_toast("err", "Helper missing", &message);
            return;
        };

        let window = self.window.clone();
        let store = Arc::clone(&self.active_volumes);
        let state = self.clone();
        thread::spawn(move || {
            let helper_result = run_helper(&helper_path, command);
            let refreshed_volumes = match &helper_result {
                Ok(response) if response.ok => list_active_volumes().ok(),
                _ => None,
            };

            let _ = window.upgrade_in_event_loop(move |ui| {
                apply_reset(&ui, reset);
                match helper_result {
                    Ok(response) if response.ok => {
                        if let Some(volumes) = refreshed_volumes {
                            replace_active_volumes(&store, &ui, volumes);
                        }
                        state.push_toast("ok", success_message, "");
                    }
                    Ok(response) => {
                        state.push_toast("err", "Operation failed", &response_error(&response));
                    }
                    Err(message) => {
                        state.push_toast("err", "Helper error", &message);
                    }
                }
            });
        });
    }

    fn push_toast(&self, kind: &str, title: &str, detail: &str) {
        let id = TOAST_ID.fetch_add(1, Ordering::SeqCst);
        let data = ToastData {
            id,
            kind: kind.into(),
            title: title.into(),
            detail: detail.into(),
            opacity: 1.0,
        };
        if let Ok(mut toasts) = self.toasts.lock() {
            toasts.push(data);
        }
        self.publish_toasts();

        let state = self.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let state = state.clone();
            Timer::single_shot(Duration::from_millis(TOAST_DWELL_MS), move || {
                state.remove_toast(id);
            });
        });
    }

    fn remove_toast(&self, id: i32) {
        if let Ok(mut toasts) = self.toasts.lock() {
            toasts.retain(|t| t.id != id);
        }
        self.publish_toasts();
    }

    fn publish_toasts(&self) {
        let toasts = self
            .toasts
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_default();
        let weak = self.window.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = weak.upgrade() {
                let model = Rc::new(VecModel::from(toasts));
                window.set_toasts(ModelRc::from(model));
            }
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
    let rows = volume_rows(&volumes)
        .into_iter()
        .map(|row| VolumeRowData {
            name: row.name.into(),
            source: row.source.into(),
            mount: row.mount.into(),
            readonly: row.readonly,
            cipher: row.cipher.into(),
            source_kind: row.source_kind.into(),
        })
        .collect::<Vec<_>>();
    let model = Rc::new(VecModel::from(rows));

    if let Ok(mut guard) = store.lock() {
        *guard = volumes;
    }

    window.set_volumes(ModelRc::from(model));
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
    window.set_create_confirm("".into());
}

fn clear_mount_secrets(window: &MainWindow) {
    window.set_mount_passphrase("".into());
    window.set_mount_key_file("".into());
}

#[cfg(test)]
mod tests {
    use slint::platform::software_renderer::{
        MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel,
    };
    use slint::platform::{Platform, PlatformError, WindowAdapter};
    use slint::ComponentHandle;
    use slint::PhysicalSize;
    use std::rc::Rc;
    use std::sync::Mutex;

    slint::slint! {
        export component InlineZoomProbe inherits Window {
            in-out property <length> size: 16px;
            out property <length> text_width: label.preferred-width;
            out property <length> text_ascent: label.font-metrics.ascent;

            label := Text {
                text: "Hello";
                font-size: root.size;
            }
        }
    }

    thread_local! {
        static TEST_WINDOW: Rc<MinimalSoftwareWindow> =
            MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    }

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    struct TestPlatform;

    impl Platform for TestPlatform {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(TEST_WINDOW.with(|window| window.clone()))
        }
    }

    fn setup_test_window() -> Rc<MinimalSoftwareWindow> {
        slint::platform::set_platform(Box::new(TestPlatform)).ok();
        let window = TEST_WINDOW.with(|test_window| test_window.clone());
        window.set_size(PhysicalSize::new(1280, 900));
        window
    }

    fn render(window: &Rc<MinimalSoftwareWindow>) {
        window.request_redraw();
        let _ = window.draw_if_needed(|renderer| {
            let mut buffer = vec![Rgb565Pixel::default(); 1280 * 900];
            renderer.render(buffer.as_mut_slice(), 1280);
        });
    }

    #[test]
    fn inline_probe_font_size_updates_text_layout() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = setup_test_window();
        let ui = InlineZoomProbe::new().unwrap();
        ui.show().unwrap();
        render(&window);

        let before_width = ui.get_text_width();
        let before_ascent = ui.get_text_ascent();

        ui.set_size(24.0);
        render(&window);

        let after_width = ui.get_text_width();
        let after_ascent = ui.get_text_ascent();

        assert!(
            after_width > before_width,
            "inline probe width did not grow: before={before_width} after={after_width}"
        );
        assert!(
            after_ascent > before_ascent,
            "inline probe ascent did not grow: before={before_ascent} after={after_ascent}"
        );
    }
}
