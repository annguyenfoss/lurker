mod helper;
mod logic;
mod preferences;

use crate::helper::{resolve_helper_path, run_helper};
use crate::logic::{
    active_volume_cards, build_create_command, build_mount_command, build_unmount_command,
    response_error, suggested_unmount_target,
};
use crate::preferences::{load_preferences, save_preferences, TaskMode, ThemeMode, UiPreferences};
use lurker_core::{list_active_volumes, ActiveVolume, CommandAction};
use slint::{BackendSelector, ComponentHandle, ModelRc, VecModel, Weak};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

slint::include_modules!();

const DEFAULT_UI_SCALE: f32 = 1.0;
const MIN_UI_SCALE: f32 = 0.8;
const MAX_UI_SCALE: f32 = 2.0;
const UI_SCALE_STEP: f32 = 0.1;

#[derive(Clone)]
struct AppState {
    window: Weak<MainWindow>,
    helper_path: Option<PathBuf>,
    helper_error: Option<String>,
    active_volumes: Arc<Mutex<Vec<ActiveVolume>>>,
    preferences: Arc<Mutex<UiPreferences>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;

    let window = MainWindow::new()?;
    let helper_path = resolve_helper_path();
    let preferences = load_preferences();
    let state = AppState {
        window: window.as_weak(),
        helper_path: helper_path.as_ref().ok().cloned(),
        helper_error: helper_path.err(),
        active_volumes: Arc::new(Mutex::new(Vec::new())),
        preferences: Arc::new(Mutex::new(preferences.clone())),
    };

    apply_preferences(&window, &preferences);
    bind_callbacks(&window, &state);
    state.refresh_active_volumes(false);
    window.run()?;
    Ok(())
}

fn bind_callbacks(window: &MainWindow, state: &AppState) {
    let zoom_in_window = window.as_weak();
    let zoom_in_state = state.clone();
    window.on_zoom_in(move || {
        with_zoom(&zoom_in_window, |scale| {
            (scale + UI_SCALE_STEP).min(MAX_UI_SCALE)
        });
        zoom_in_state.sync_preferences_from_window();
    });

    let zoom_out_window = window.as_weak();
    let zoom_out_state = state.clone();
    window.on_zoom_out(move || {
        with_zoom(&zoom_out_window, |scale| {
            (scale - UI_SCALE_STEP).max(MIN_UI_SCALE)
        });
        zoom_out_state.sync_preferences_from_window();
    });

    let zoom_reset_window = window.as_weak();
    let zoom_reset_state = state.clone();
    window.on_zoom_reset(move || {
        if let Some(window) = zoom_reset_window.upgrade() {
            apply_zoom(&window, DEFAULT_UI_SCALE);
        }
        zoom_reset_state.sync_preferences_from_window();
    });

    let refresh_state = state.clone();
    window.on_refresh_volumes(move || refresh_state.refresh_active_volumes(true));

    let theme_state = state.clone();
    window.on_theme_mode_changed(move |_| theme_state.sync_preferences_from_window());

    let task_state = state.clone();
    window.on_task_mode_changed(move |_| task_state.sync_preferences_from_window());

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
    fn sync_preferences_from_window(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };

        let preferences = UiPreferences {
            ui_scale: window.get_ui_scale(),
            theme_mode: ThemeMode::parse_ui_value(&window.get_theme_mode()).unwrap_or_default(),
            last_task: TaskMode::parse_ui_value(&window.get_current_mode()).unwrap_or_default(),
        }
        .normalized();

        if let Ok(mut guard) = self.preferences.lock() {
            *guard = preferences.clone();
        }

        let _ = save_preferences(&preferences);
    }

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
            window.set_current_mode(TaskMode::Unmount.as_ui_value().into());
            window.set_unmount_target(target.into());
            window.set_error_message("".into());
            window.set_status_message("Unmount target loaded from active volumes.".into());
        }

        self.sync_preferences_from_window();
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
    let cards = active_volume_cards(&volumes)
        .into_iter()
        .map(|card| ActiveVolumeCardData {
            title: card.title.into(),
            detail: card.detail.into(),
            meta: card.meta.into(),
            badge: card.badge.into(),
        })
        .collect::<Vec<_>>();
    let model = Rc::new(VecModel::from(cards));

    if let Ok(mut guard) = store.lock() {
        *guard = volumes;
    }

    window.set_active_volumes(ModelRc::from(model));
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

fn with_zoom(window: &Weak<MainWindow>, adjust: impl FnOnce(f32) -> f32) {
    let Some(window) = window.upgrade() else {
        return;
    };
    let next = adjust(window.get_ui_scale());
    apply_zoom(&window, next);
}

fn apply_zoom(window: &MainWindow, scale: f32) {
    let scale = scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
    window.set_ui_scale(scale);
    window.set_zoom_label(format!("{}%", (scale * 100.0).round() as i32).into());
}

fn apply_preferences(window: &MainWindow, preferences: &UiPreferences) {
    window.set_theme_mode(preferences.theme_mode.as_ui_value().into());
    window.set_current_mode(preferences.last_task.as_ui_value().into());
    apply_zoom(window, preferences.ui_scale);
}

#[cfg(test)]
mod tests {
    use super::{ZoomMetricsWindow, DEFAULT_UI_SCALE, MAX_UI_SCALE, MIN_UI_SCALE, UI_SCALE_STEP};
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

    #[derive(Clone, Copy, Debug)]
    struct ZoomSnapshot {
        title_ascent: f32,
        body_ascent: f32,
        title_font_size: f32,
        body_font_size: f32,
        title_width: f32,
        body_width: f32,
        button_ascent: f32,
        select_ascent: f32,
        checkbox_ascent: f32,
        row_ascent: f32,
        line_edit_font_size: f32,
        line_edit_height: f32,
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

    fn capture(ui: &ZoomMetricsWindow) -> ZoomSnapshot {
        ZoomSnapshot {
            title_ascent: ui.get_title_ascent(),
            body_ascent: ui.get_body_ascent(),
            title_font_size: ui.get_title_font_size_debug(),
            body_font_size: ui.get_body_font_size_debug(),
            title_width: ui.get_title_width(),
            body_width: ui.get_body_width(),
            button_ascent: ui.get_button_ascent(),
            select_ascent: ui.get_select_ascent(),
            checkbox_ascent: ui.get_checkbox_ascent(),
            row_ascent: ui.get_row_ascent(),
            line_edit_font_size: ui.get_line_edit_font_size(),
            line_edit_height: ui.get_line_edit_height(),
        }
    }

    #[test]
    fn zoom_constants_are_ordered() {
        assert!(MIN_UI_SCALE < DEFAULT_UI_SCALE);
        assert!(DEFAULT_UI_SCALE < MAX_UI_SCALE);
        assert!(UI_SCALE_STEP > 0.0);
    }

    #[test]
    fn zoom_increases_text_metrics_across_controls() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = setup_test_window();
        let ui = ZoomMetricsWindow::new().unwrap();
        ui.show().unwrap();
        render(&window);

        let before = capture(&ui);
        ui.set_ui_scale(1.5);
        render(&window);
        let after = capture(&ui);

        assert!(
            after.title_ascent > before.title_ascent,
            "title ascent did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.body_ascent > before.body_ascent,
            "body ascent did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.title_font_size > before.title_font_size,
            "title font size did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.body_font_size > before.body_font_size,
            "body font size did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.title_width > before.title_width,
            "title width did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.body_width > before.body_width,
            "body width did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.button_ascent > before.button_ascent,
            "button ascent did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.select_ascent > before.select_ascent,
            "select ascent did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.checkbox_ascent > before.checkbox_ascent,
            "checkbox ascent did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.row_ascent > before.row_ascent,
            "row ascent did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.line_edit_font_size > before.line_edit_font_size,
            "line edit font size did not increase: before={:?} after={:?}",
            before,
            after
        );
        assert!(
            after.line_edit_height > before.line_edit_height,
            "line edit height did not increase: before={:?} after={:?}",
            before,
            after
        );
    }

    #[test]
    fn zoom_keeps_window_geometry_stable() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = setup_test_window();
        let ui = ZoomMetricsWindow::new().unwrap();
        ui.show().unwrap();
        render(&window);

        let before_size = window.size();
        ui.set_ui_scale(2.0);
        render(&window);
        let after_size = window.size();

        assert_eq!(after_size, before_size);
    }

    #[test]
    fn zoom_out_reduces_text_metrics() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = setup_test_window();
        let ui = ZoomMetricsWindow::new().unwrap();
        ui.show().unwrap();
        ui.set_ui_scale(1.5);
        render(&window);
        let enlarged = capture(&ui);

        ui.set_ui_scale(0.9);
        render(&window);
        let reduced = capture(&ui);

        assert!(
            reduced.title_ascent < enlarged.title_ascent,
            "title ascent did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.body_ascent < enlarged.body_ascent,
            "body ascent did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.title_font_size < enlarged.title_font_size,
            "title font size did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.body_font_size < enlarged.body_font_size,
            "body font size did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.title_width < enlarged.title_width,
            "title width did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.body_width < enlarged.body_width,
            "body width did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.button_ascent < enlarged.button_ascent,
            "button ascent did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.select_ascent < enlarged.select_ascent,
            "select ascent did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.checkbox_ascent < enlarged.checkbox_ascent,
            "checkbox ascent did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.row_ascent < enlarged.row_ascent,
            "row ascent did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.line_edit_font_size < enlarged.line_edit_font_size,
            "line edit font size did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
        assert!(
            reduced.line_edit_height < enlarged.line_edit_height,
            "line edit height did not decrease: enlarged={:?} reduced={:?}",
            enlarged,
            reduced
        );
    }

    #[test]
    fn initial_scale_affects_text_layout() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = setup_test_window();
        let ui = ZoomMetricsWindow::new().unwrap();
        ui.set_ui_scale(1.5);
        ui.show().unwrap();
        render(&window);

        let scaled = capture(&ui);

        assert!(
            scaled.title_font_size > 30.0,
            "title font size did not initialize larger: {scaled:?}"
        );
        assert!(
            scaled.body_font_size > 16.0,
            "body font size did not initialize larger: {scaled:?}"
        );
        assert!(
            scaled.title_width > 77.0,
            "title width did not initialize larger: {scaled:?}"
        );
        assert!(
            scaled.body_width > 52.0,
            "body width did not initialize larger: {scaled:?}"
        );
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
