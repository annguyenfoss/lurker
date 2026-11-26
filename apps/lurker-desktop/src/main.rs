mod devices;
mod helper;
mod logic;

use crate::devices::list_block_devices;
use crate::helper::{list_desktop_volumes, resolve_helper_path, run_helper};
use crate::logic::{
    build_create_command, build_keyslots_command, build_mount_command,
    build_unmount_command_for_volume, response_error, volume_rows,
};
use lurker_core::{ActiveVolume, CommandAction, PrivilegeMode};
use slint::{BackendSelector, ComponentHandle, ModelRc, Timer, VecModel, Weak};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
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
    operation_busy: Arc<AtomicBool>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;

    let window = MainWindow::new()?;
    configure_platform(&window, std::env::consts::OS);
    if let Some(home) = std::env::var_os("HOME") {
        window.set_create_file_path(home.to_string_lossy().as_ref().into());
    }
    let helper_path = resolve_helper_path();
    let state = AppState {
        window: window.as_weak(),
        helper_path: helper_path.as_ref().ok().cloned(),
        helper_error: helper_path.err(),
        active_volumes: Arc::new(Mutex::new(Vec::new())),
        toasts: Arc::new(Mutex::new(Vec::new())),
        operation_busy: Arc::new(AtomicBool::new(false)),
    };

    bind_callbacks(&window, &state);
    state.refresh_active_volumes(false);
    if cfg!(target_os = "linux") {
        state.rescan_devices();
    }
    window.run()?;
    Ok(())
}

fn bind_callbacks(window: &MainWindow, state: &AppState) {
    let access = state.window.clone();
    window.on_change_privilege_mode(move |value| {
        if let Some(window) = access.upgrade() {
            apply_privilege_mode(&window, value.as_str());
        }
    });
    // ── Volumes refresh ──
    let rs = state.clone();
    window.on_refresh_volumes(move || rs.refresh_active_volumes(true));

    let gs = state.clone();
    window.on_generate_passphrase(move || {
        if let Some(window) = gs.window.upgrade() {
            match logic::generate_passphrase() {
                Ok(passphrase) => {
                    window.set_create_passphrase(passphrase.into());
                    window.set_create_confirm("".into());
                }
                Err(message) => gs.push_toast("err", "Passphrase generation failed", &message),
            }
        }
    });

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
            &w.get_create_auth_method(),
            &w.get_create_key_file(),
        )
        .and_then(|mut command| {
            if cfg!(target_os = "dragonfly")
                && command.volume_type == lurker_core::VolumeType::Veracrypt
            {
                command.filesystem = Some(
                    lurker_core::CreateFilesystem::parse(&w.get_create_filesystem())
                        .ok_or_else(|| "Choose HAMMER2 or FAT32.".to_string())?,
                );
            }
            logic::with_create_members(command, &w.get_create_peer_source(), std::env::consts::OS)
        }) {
            Ok(command) => {
                let mode = selected_privilege_mode(&w);
                if let Err(message) =
                    logic::validate_create_platform(&command, std::env::consts::OS, mode)
                {
                    clear_create_secrets(&w);
                    cs.push_toast("err", "Create failed", &message);
                    return;
                }
                cs.run_operation(
                    "Encrypted storage created",
                    CommandAction::Create(command).with_privilege_mode(mode),
                    ResetKind::CreateSecrets,
                );
            }
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
            &w.get_mount_source_kind(),
        )
        .and_then(|command| {
            logic::with_mount_members(
                command,
                w.get_mount_mirrored(),
                &w.get_mount_peer_source(),
                w.get_mount_degraded(),
                std::env::consts::OS,
            )
        }) {
            Ok(command) => ms.run_operation(
                "Volume mounted",
                CommandAction::Mount(command).with_privilege_mode(selected_privilege_mode(&w)),
                ResetKind::MountSecrets,
            ),
            Err(message) => {
                clear_mount_secrets(&w);
                ms.push_toast("err", "Mount failed", &message);
            }
        }
    });

    let ks = state.clone();
    window.on_submit_keyslots(move || {
        let Some(w) = ks.window.upgrade() else { return };
        if !cfg!(target_os = "linux") || selected_privilege_mode(&w) == PrivilegeMode::Unprivileged
        {
            clear_keyslot_secrets(&w);
            ks.push_toast(
                "err",
                "LUKS keys unavailable",
                "Switch to Linux Privileged mode to manage LUKS credentials.",
            );
            return;
        }
        match build_keyslots_command(
            &w.get_keys_source(),
            &w.get_keys_operation(),
            &w.get_keys_slot(),
            &w.get_keys_auth_method(),
            &w.get_keys_passphrase(),
            &w.get_keys_key_file(),
            &w.get_keys_new_auth_method(),
            &w.get_keys_new_passphrase(),
            &w.get_keys_confirm(),
            &w.get_keys_new_key_file(),
        ) {
            Ok(command) => ks.run_operation(
                "LUKS keyslot operation complete",
                CommandAction::Keyslots(command),
                ResetKind::KeyslotSecrets,
            ),
            Err(message) => {
                clear_keyslot_secrets(&w);
                w.set_keys_result(message.clone().into());
                ks.push_toast("err", "LUKS keys failed", &message);
            }
        }
    });

    // ── Reset Mount form ──
    let rms = window.as_weak();
    window.on_reset_mount(move || {
        if let Some(w) = rms.upgrade() {
            w.set_mount_source("".into());
            w.set_mount_point(mountpoint_for_mode(selected_privilege_mode(&w)).into());
            w.set_mount_passphrase("".into());
            w.set_mount_key_file("".into());
            w.set_mount_auth_method("pass".into());
            w.set_mount_mirrored(false);
            w.set_mount_peer_source("".into());
            w.set_mount_degraded(false);
        }
    });

    // ── Unmount (from volume row index) ──
    let us = state.clone();
    window.on_unmount_volume(move |index| us.unmount_volume(index));

    // ── Device rescan ──
    let ds = state.clone();
    window.on_rescan_devices(move || ds.rescan_devices());

    // ── Device picked → mirror label for the trigger meta text ──
    let lc = window.as_weak();
    window.on_device_picked_for_create(move |info| {
        if let Some(w) = lc.upgrade() {
            w.set_create_partition_label(info.label);
        }
    });
    let lm = window.as_weak();
    window.on_device_picked_for_mount(move |info| {
        if let Some(w) = lm.upgrade() {
            w.set_mount_source_label(info.label);
        }
    });
}

impl AppState {
    fn rescan_devices(&self) {
        let window = self.window.clone();
        thread::spawn(move || {
            let list = list_block_devices();
            let _ = window.upgrade_in_event_loop(move |ui| {
                let items = list
                    .into_iter()
                    .map(|d| DeviceInfo {
                        path: d.path.into(),
                        label: d.label.into(),
                        sub: d.sub.into(),
                        kind: d.kind.into(),
                    })
                    .collect::<Vec<_>>();
                let model = Rc::new(VecModel::from(items));
                ui.set_devices(ModelRc::from(model));
            });
        });
    }

    fn refresh_active_volumes(&self, announce: bool) {
        let window = self.window.clone();
        let store = Arc::clone(&self.active_volumes);
        let state = self.clone();
        thread::spawn(move || {
            let result = list_desktop_volumes(state.helper_path.as_deref());
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
        let Ok(guard) = self.active_volumes.lock() else {
            return;
        };
        let Some(volume) = guard.get(index as usize).cloned() else {
            return;
        };
        drop(guard);
        if self
            .window
            .upgrade()
            .is_some_and(|w| selected_privilege_mode(&w) == PrivilegeMode::Unprivileged)
            && volume.privilege_mode == PrivilegeMode::Privileged
        {
            self.push_toast(
                "info",
                "Privileged volume",
                "Switch to Privileged mode to unmount this volume.",
            );
            return;
        }
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
        let keyslots = matches!(&command, CommandAction::Keyslots(_));
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
        if self
            .operation_busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.push_toast(
                "info",
                "Operation in progress",
                "Wait for the current operation to finish.",
            );
            return;
        }
        if let Some(ui) = window.upgrade() {
            ui.set_operation_busy(true);
        }
        let store = Arc::clone(&self.active_volumes);
        let state = self.clone();
        thread::spawn(move || {
            let helper_result = run_helper(&helper_path, command);
            let refreshed_volumes = match &helper_result {
                Ok(response) if response.ok => Some(list_desktop_volumes(Some(&helper_path))),
                _ => None,
            };
            state.operation_busy.store(false, Ordering::SeqCst);

            let _ = window.upgrade_in_event_loop(move |ui| {
                ui.set_operation_busy(false);
                apply_reset(&ui, reset);
                match helper_result {
                    Ok(response) if response.ok => {
                        if keyslots {
                            ui.set_keys_result(
                                response
                                    .logs
                                    .iter()
                                    .map(|entry| entry.message.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                                    .into(),
                            );
                        }
                        match refreshed_volumes {
                            Some(Ok(volumes)) => replace_active_volumes(&store, &ui, volumes),
                            Some(Err(message)) => {
                                state.push_toast("err", "Volume refresh failed", &message)
                            }
                            None => {}
                        }
                        state.push_toast("ok", success_message, "");
                    }
                    Ok(response) => {
                        if keyslots {
                            ui.set_keys_result(response_error(&response).into());
                        }
                        state.push_toast("err", "Operation failed", &response_error(&response));
                    }
                    Err(message) => {
                        if keyslots {
                            ui.set_keys_result(message.clone().into());
                        }
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
    KeyslotSecrets,
}

fn replace_active_volumes(
    store: &Arc<Mutex<Vec<ActiveVolume>>>,
    window: &MainWindow,
    volumes: Vec<ActiveVolume>,
) {
    let rows = volume_rows(&volumes)
        .into_iter()
        .map(|row| VolumeRowData {
            unprivileged: row.unprivileged,
            name: row.name.into(),
            source: row.source.into(),
            mount: row.mount.into(),
            readonly: row.readonly,
            filesystem: row.filesystem.into(),
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
        ResetKind::KeyslotSecrets => clear_keyslot_secrets(window),
    }
}

fn clear_create_secrets(window: &MainWindow) {
    window.set_create_passphrase("".into());
    window.set_create_confirm("".into());
    window.set_create_key_file("".into());
}

fn clear_keyslot_secrets(window: &MainWindow) {
    window.set_keys_passphrase("".into());
    window.set_keys_key_file("".into());
    window.set_keys_new_passphrase("".into());
    window.set_keys_confirm("".into());
    window.set_keys_new_key_file("".into());
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
    use slint::platform::{EventLoopProxy, Platform, PlatformError, WindowAdapter};
    use slint::ComponentHandle;
    use slint::PhysicalSize;
    use std::rc::Rc;
    use std::sync::Mutex;

    slint::slint! {
        export component InlineZoomProbe inherits Window {
            default-font-family: "Inter Tight";
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
            MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    }

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    type TestEvent = Box<dyn FnOnce() + Send>;
    static TEST_EVENTS: Mutex<Vec<TestEvent>> = Mutex::new(Vec::new());
    struct TestProxy;
    impl EventLoopProxy for TestProxy {
        fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
            Ok(())
        }
        fn invoke_from_event_loop(&self, event: TestEvent) -> Result<(), slint::EventLoopError> {
            TEST_EVENTS.lock().unwrap().push(event);
            Ok(())
        }
    }

    struct TestPlatform {
        with_proxy: bool,
    }

    impl Platform for TestPlatform {
        fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
            self.with_proxy
                .then(|| Box::new(TestProxy) as Box<dyn EventLoopProxy>)
        }
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(TEST_WINDOW.with(|window| window.clone()))
        }
    }

    fn setup_test_window() -> Rc<MinimalSoftwareWindow> {
        setup_test_window_with_proxy(false)
    }

    fn setup_test_window_with_proxy(with_proxy: bool) -> Rc<MinimalSoftwareWindow> {
        slint::platform::set_platform(Box::new(TestPlatform { with_proxy })).ok();
        let window = TEST_WINDOW.with(|test_window| test_window.clone());
        window.set_size(PhysicalSize::new(1280, 900));
        window
    }

    fn render(window: &Rc<MinimalSoftwareWindow>) {
        let events = std::mem::take(&mut *TEST_EVENTS.lock().unwrap());
        for event in events {
            event();
        }
        slint::platform::update_timers_and_animations();
        slint::private_unstable_api::re_exports::ChangeTracker::run_change_handlers();
        window.request_redraw();
        let _ = window.draw_if_needed(|renderer| {
            let mut buffer = vec![Rgb565Pixel::default(); 1280 * 900];
            renderer.render(buffer.as_mut_slice(), 1280);
        });
        slint::platform::update_timers_and_animations();
        slint::private_unstable_api::re_exports::ChangeTracker::run_change_handlers();
    }

    fn snapshot(window: &Rc<MinimalSoftwareWindow>, name: &str) {
        let Some(directory) = std::env::var_os("LURKER_UI_SNAPSHOT_DIR") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            let mut pixels = vec![Rgb565Pixel::default(); 1280 * 900];
            renderer.render(&mut pixels, 1280);
            let mut ppm = b"P6\n1280 900\n255\n".to_vec();
            for pixel in pixels {
                let rgb: slint::Rgb8Pixel = pixel.into();
                ppm.extend_from_slice(&[rgb.r, rgb.g, rgb.b]);
            }
            std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
        });
    }

    /// Real helper/backend test: opt in only on a disposable remote test host.
    /// The normal workspace suite never starts encryption or mount operations.
    #[test]
    #[ignore = "requires an explicitly configured remote native fixture and real helper"]
    fn native_gui_callbacks_create_mount_reopen_and_unmount() {
        use std::os::unix::fs::DirBuilderExt;
        use std::sync::atomic::Ordering;
        use std::time::{Duration, Instant};
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(
            std::env::var("LURKER_DESKTOP_NATIVE_TEST").as_deref(),
            Ok("1")
        );
        let mut entropy = [0_u8; 16];
        std::io::Read::read_exact(
            &mut std::fs::File::open("/dev/urandom").expect("native fixture entropy source"),
            &mut entropy,
        )
        .expect("native fixture entropy");
        let suffix = entropy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let fixture_password = format!("Desktop 密碼 9!{suffix}");
        assert!(fixture_password.len() < 64);
        let fixture_parent = std::path::PathBuf::from(
            std::env::var_os("LURKER_DESKTOP_NATIVE_ROOT").expect("explicit /tmp fixture root"),
        );
        let fixture_parent = fixture_parent.canonicalize().unwrap();
        let temporary_root = std::path::Path::new("/tmp").canonicalize().unwrap();
        assert!(fixture_parent.starts_with(&temporary_root) && fixture_parent != temporary_root);
        let helper = std::path::PathBuf::from(
            std::env::var_os("LURKER_DESKTOP_NATIVE_HELPER").expect("explicit real helper path"),
        )
        .canonicalize()
        .unwrap();
        let format =
            std::env::var("LURKER_DESKTOP_NATIVE_FORMAT").unwrap_or_else(|_| "gocryptfs".into());
        let backend = lurker_core::VolumeType::parse(&format).expect("backend name");
        assert!(backend.supports_platform(std::env::consts::OS));
        let mirrored = backend == lurker_core::VolumeType::Raid1c;
        let uid = lurker_core::effective_uid();
        if let Ok(expected) = std::env::var("LURKER_DESKTOP_NATIVE_EXPECT_UID") {
            assert_eq!(uid, expected.parse::<u32>().expect("expected test uid"));
        }
        eprintln!(
            "native GUI callback uid={uid} backend={format} helper={}",
            helper.display()
        );
        if !backend.is_directory() && uid != 0 {
            eprintln!(
                "native authorization tools: pkexec={:?} sudo={:?}",
                lurker_core::trusted_system_executable("pkexec"),
                lurker_core::trusted_system_executable("sudo")
            );
        }
        let fixture = fixture_parent.join(format!("gui-{}", std::process::id()));
        std::fs::create_dir(&fixture).unwrap();
        let partition = std::env::var_os("LURKER_DESKTOP_NATIVE_PARTITION");
        let physical_partition = partition.is_some();
        let source = if let Some(partition) = partition {
            use std::os::unix::fs::FileTypeExt;
            assert!(cfg!(target_os = "macos") && backend == lurker_core::VolumeType::Veracrypt);
            let partition = std::path::PathBuf::from(partition).canonicalize().unwrap();
            assert!(partition.starts_with("/dev") && std::fs::metadata(&partition).unwrap().file_type().is_block_device());
            partition
        } else {
            fixture.join("encrypted")
        };
        let peer = fixture.join("encrypted-peer");
        let mountpoint = fixture.join("mount");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&mountpoint)
            .unwrap();
        let owns_volume = |volume: &lurker_core::ActiveVolume| {
            volume
                .source_path
                .as_ref()
                .map(|path| path == &source)
                .unwrap_or_else(|| volume.mountpoint.as_ref() == Some(&mountpoint))
        };
        let window = setup_test_window_with_proxy(true);
        let ui = super::MainWindow::new().unwrap();
        super::configure_platform(&ui, std::env::consts::OS);
        let state = super::AppState {
            window: ui.as_weak(),
            helper_path: Some(helper.clone()),
            helper_error: None,
            active_volumes: std::sync::Arc::new(Mutex::new(Vec::new())),
            toasts: std::sync::Arc::new(Mutex::new(Vec::new())),
            operation_busy: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        super::bind_callbacks(&ui, &state);
        ui.show().unwrap();
        let wait = || {
            let deadline = Instant::now() + Duration::from_secs(120);
            loop {
                render(&window);
                if !state.operation_busy.load(Ordering::SeqCst) && !ui.get_operation_busy() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "native GUI operation timed out; fixture {}",
                    fixture.display()
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            let toasts = state.toasts.lock().unwrap();
            assert!(
                !toasts.iter().any(|toast| toast.kind == "err"),
                "GUI operation failed: {:?}",
                toasts
                    .iter()
                    .map(|toast| (&toast.title, &toast.detail))
                    .collect::<Vec<_>>()
            );
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_create_format(if backend == lurker_core::VolumeType::Veracrypt {
                "vera"
            } else {
                format.as_str()
            }.into());
            render(&window);
            ui.set_create_target_kind(
                if physical_partition {
                    "partition"
                } else if backend.is_directory() {
                    "directory"
                } else {
                    "file"
                }
                .into(),
            );
            ui.set_create_file_path(fixture.to_string_lossy().as_ref().into());
            ui.set_create_file_name("encrypted".into());
            if physical_partition {
                ui.set_create_partition(source.to_string_lossy().as_ref().into());
            }
            if mirrored {
                ui.set_create_peer_source(peer.to_string_lossy().as_ref().into());
            }
            ui.set_create_size(
                if cfg!(target_os = "dragonfly") && backend == lurker_core::VolumeType::Veracrypt {
                    "260"
                } else if cfg!(target_os = "linux") {
                    "256"
                } else {
                    "128"
                }
                .into(),
            );
            ui.set_create_size_unit("MB".into());
            ui.set_create_passphrase(fixture_password.as_str().into());
            ui.set_create_confirm(fixture_password.as_str().into());
            ui.invoke_submit_create();
            wait();
            assert!(source.exists());
            assert_eq!(ui.get_create_passphrase(), "");
            ui.set_current_view("manage".into());
            render(&window);
            let mount = |include_peer| {
                ui.set_mount_source_kind(
                    if physical_partition {
                        "partition"
                    } else if backend.is_directory() {
                        "directory"
                    } else {
                        "file"
                    }
                    .into(),
                );
                ui.set_mount_source(source.to_string_lossy().as_ref().into());
                ui.set_mount_point(mountpoint.to_string_lossy().as_ref().into());
                if mirrored {
                    ui.set_mount_mirrored(true);
                    render(&window);
                    ui.set_mount_peer_source(if include_peer {
                        peer.to_string_lossy().as_ref().into()
                    } else {
                        "".into()
                    });
                }
                ui.set_mount_passphrase(fixture_password.as_str().into());
                ui.invoke_submit_mount();
                wait();
                assert_eq!(ui.get_mount_passphrase(), "");
            };
            let unmount = || {
                let index = state
                    .active_volumes
                    .lock()
                    .unwrap()
                    .iter()
                    .position(|volume| owns_volume(volume))
                    .expect("own native session listed");
                ui.invoke_unmount_volume(index as i32);
                wait();
                assert!(!state
                    .active_volumes
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|volume| owns_volume(volume)));
            };
            mount(true);
            if !backend.is_directory() && !cfg!(target_os = "macos") {
                use std::os::unix::fs::{MetadataExt, PermissionsExt};
                let image = std::fs::metadata(&source).unwrap();
                let filesystem = std::fs::metadata(&mountpoint).unwrap();
                assert_eq!(image.uid(), uid, "new image belongs to the invoking user");
                assert_eq!(image.permissions().mode() & 0o777, 0o600);
                if mirrored {
                    let peer_image = std::fs::metadata(&peer).unwrap();
                    assert_eq!(peer_image.uid(), uid);
                    assert_eq!(peer_image.permissions().mode() & 0o777, 0o600);
                }
                assert_eq!(
                    filesystem.uid(),
                    uid,
                    "new filesystem belongs to the invoking user"
                );
                assert_eq!(filesystem.permissions().mode() & 0o777, 0o700);
            }
            let payload = (0..1048576)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>();
            let mut file = std::fs::File::create(mountpoint.join("payload.bin")).unwrap();
            std::io::Write::write_all(&mut file, &payload).unwrap();
            file.sync_all().unwrap();
            drop(file);
            unmount();
            mount(false);
            assert_eq!(
                std::fs::read(mountpoint.join("payload.bin")).unwrap(),
                payload
            );
            unmount();
        }));
        // Prefer the listed source identity. Linux native rows expose only the
        // mapping, so accept this test's exact private mountpoint when absent.
        if let Ok(volumes) = super::list_desktop_volumes(Some(&helper)) {
            if let Some(volume) = volumes.iter().find(|volume| owns_volume(volume)) {
                let response = super::run_helper(
                    &helper,
                    lurker_core::CommandAction::Unmount(
                        super::logic::build_unmount_command_for_volume(volume),
                    ),
                );
                eprintln!("native GUI fixture cleanup: {response:?}");
            }
        }
        ui.hide().unwrap();
        if let Err(failure) = outcome {
            eprintln!("retained native GUI fixture: {}", fixture.display());
            std::panic::resume_unwind(failure);
        }
        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[test]
    fn macos_veracrypt_partition_forms_render_at_native_scales() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        super::configure_platform(&ui, "macos");
        ui.show().unwrap();
        ui.set_create_format("vera".into());
        render(&window);
        ui.set_create_target_kind("partition".into());
        ui.set_create_partition("/dev/disk42s2".into());
        ui.set_mount_source_kind("partition".into());
        ui.set_mount_source("/dev/disk42s2".into());
        ui.set_mount_point("/Volumes/Work".into());
        for scale in [1.0, 1.2] {
            ui.set_ui_scale(scale);
            ui.set_current_view("create".into());
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(1000.0, 500.0), delta_x: 0.0, delta_y: 10000.0,
            });
            render(&window);
            assert_eq!(ui.get_create_target_kind(), "partition");
            snapshot(&window, &format!("macos-vc-partition-create-{scale}"));
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(1000.0, 500.0), delta_x: 0.0, delta_y: -10000.0,
            });
            render(&window);
            snapshot(&window, &format!("macos-vc-partition-create-bottom-{scale}"));
            ui.set_current_view("manage".into());
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(1000.0, 500.0), delta_x: 0.0, delta_y: 10000.0,
            });
            render(&window);
            assert_eq!(ui.get_mount_source_kind(), "partition");
            snapshot(&window, &format!("macos-vc-partition-open-{scale}"));
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(1000.0, 500.0), delta_x: 0.0, delta_y: -10000.0,
            });
            render(&window);
            snapshot(&window, &format!("macos-vc-partition-open-bottom-{scale}"));
        }
        ui.hide().unwrap();
    }

    #[test]
    fn openbsd_mirror_fields_render_and_clear_when_ordinary_storage_is_selected() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        super::configure_platform(&ui, "openbsd");
        ui.show().unwrap();
        ui.set_create_format("raid1c".into());
        render(&window);
        ui.set_create_peer_source("/tmp/mirror-peer".into());
        ui.set_create_auth_method("key".into());
        ui.set_create_cipher("twofish".into());
        ui.set_create_format("softraid".into());
        render(&window);
        assert_eq!(ui.get_create_peer_source(), "");
        assert_eq!(ui.get_create_auth_method(), "pass");
        assert_eq!(ui.get_create_cipher(), "aes");
        ui.set_current_view("manage".into());
        render(&window);
        ui.set_mount_key_file("/tmp/stale-key".into());
        ui.set_mount_auth_method("key".into());
        ui.set_mount_mirrored(true);
        render(&window);
        assert_eq!(ui.get_mount_auth_method(), "pass");
        assert_eq!(ui.get_mount_key_file(), "");
        ui.set_mount_peer_source("/tmp/mirror-peer".into());
        ui.set_mount_degraded(true);
        for scale in [0.8, 1.0, 1.2] {
            ui.set_ui_scale(scale);
            render(&window);
        }
        ui.set_mount_mirrored(false);
        render(&window);
        assert_eq!(ui.get_mount_peer_source(), "");
        assert!(!ui.get_mount_degraded());
        ui.hide().unwrap();
    }

    #[test]
    fn selected_platform_backends_render_and_reset_incompatible_form_fields() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        ui.show().unwrap();
        for os in [
            "linux",
            "macos",
            "freebsd",
            "netbsd",
            "openbsd",
            "dragonfly",
        ] {
            super::configure_platform(&ui, os);
            render(&window);
            assert_eq!(ui.get_is_linux(), os == "linux");
            assert_eq!(
                ui.get_create_format().as_str(),
                super::logic::create_formats(os, false)[0]
            );
            for format in super::logic::create_formats(os, false) {
                ui.set_current_view("create".into());
                ui.set_create_format("vera".into());
                render(&window);
                ui.set_create_target_kind("partition".into());
                ui.set_create_cipher("twofish".into());
                ui.set_create_auth_method("key".into());
                render(&window);
                ui.set_create_key_file("/tmp/stale-key".into());
                ui.set_create_format(format.into());
                render(&window);
                let backend = lurker_core::VolumeType::parse(if format == "vera" {
                    "veracrypt"
                } else {
                    format
                })
                .unwrap();
                // Selecting the already-active VeraCrypt format retains its existing target.
                if format != "vera" {
                    assert_eq!(
                        ui.get_create_target_kind(),
                        if backend.is_directory() {
                            "directory"
                        } else {
                            "file"
                        }
                    );
                }
                if !matches!(format, "luks" | "luks1" | "vera") {
                    assert_eq!(ui.get_create_cipher(), "aes");
                }
                if !matches!(format, "luks" | "luks1" | "vera" | "geli" | "pefs") {
                    assert_eq!(ui.get_create_auth_method(), "pass");
                    assert_eq!(ui.get_create_key_file(), "");
                }
                for scale in [0.8, 1.0, 1.2] {
                    ui.set_ui_scale(scale);
                    render(&window);
                    snapshot(&window, &format!("{os}-create-{format}-{scale}"));
                }
                ui.set_current_view("manage".into());
                ui.set_mount_source_kind("directory".into());
                ui.set_mount_source("/tmp/encrypted-directory".into());
                render(&window);
            }
        }
        ui.hide().unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_privilege_selection_keeps_forms_and_command_routing_consistent() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        ui.set_create_target_kind("partition".into());
        ui.set_create_passphrase("temporary secret".into());
        ui.set_mount_source_kind("partition".into());
        ui.set_mount_source("/dev/not-a-real-device".into());
        super::apply_privilege_mode(&ui, "unprivileged");
        assert_eq!(
            super::selected_privilege_mode(&ui),
            lurker_core::PrivilegeMode::Unprivileged
        );
        assert_eq!(ui.get_create_format(), "vera");
        assert_eq!(ui.get_create_target_kind(), "file");
        assert_eq!(ui.get_mount_source_kind(), "file");
        assert_eq!(ui.get_mount_source(), "");
        assert_eq!(ui.get_create_passphrase(), "");
        ui.show().unwrap();
        ui.set_ui_scale(1.0);
        render(&window);
        snapshot(&window, "linux-unprivileged");
        for scale in [0.8, 1.0, 1.2] {
            ui.set_ui_scale(scale);
            for view in ["create", "manage"] {
                ui.set_current_view(view.into());
                render(&window);
            }
        }
        super::apply_privilege_mode(&ui, "privileged");
        assert_eq!(ui.get_create_format(), "luks");
        assert_eq!(
            super::selected_privilege_mode(&ui),
            lurker_core::PrivilegeMode::Privileged
        );
        render(&window);
        snapshot(&window, "linux-privileged");
        ui.hide().unwrap();
    }

    #[test]
    fn credential_views_render_and_clear_inactive_secrets() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        ui.show().unwrap();
        render(&window);
        for format in ["luks", "vera"] {
            ui.set_create_format(format.into());
            ui.set_create_passphrase("temporary secret".into());
            ui.set_create_confirm("temporary secret".into());
            ui.set_create_auth_method("key".into());
            render(&window);
            assert_eq!(ui.get_create_passphrase(), "");
            assert_eq!(ui.get_create_confirm(), "");
            ui.set_create_key_file("/tmp/key, with spaces".into());
            render(&window);
            snapshot(&window, &format!("create-{format}-keyfile"));
            ui.set_create_auth_method("pass".into());
            render(&window);
            assert_eq!(ui.get_create_key_file(), "");
        }
        ui.set_create_auth_method("key".into());
        ui.set_create_key_file("/tmp/key".into());
        ui.set_create_format("apple".into());
        render(&window);
        assert_eq!(ui.get_create_auth_method(), "pass");
        assert_eq!(ui.get_create_key_file(), "");
        ui.set_current_view("manage".into());
        render(&window);
        ui.set_mount_passphrase("temporary secret".into());
        ui.set_mount_auth_method("key".into());
        render(&window);
        assert_eq!(ui.get_mount_passphrase(), "");
        ui.set_mount_source("/tmp/vault".into());
        ui.set_mount_key_file("/tmp/key".into());
        ui.set_mount_point("/tmp/mount".into());
        render(&window);
        snapshot(&window, "mount-keyfile");
        ui.set_current_view("keys".into());
        ui.set_keys_source("/tmp/vault.luks".into());
        ui.set_keys_result("LUKS2 active keyslots: 0, 1, 31".into());
        for scale in [0.8, 1.0, 1.2] {
            ui.set_ui_scale(scale);
            for operation in ["list", "add", "remove"] {
                ui.set_keys_operation(operation.into());
                ui.set_keys_slot("1".into());
                for method in ["pass", "key"] {
                    ui.set_keys_auth_method(method.into());
                    ui.set_keys_new_auth_method(method.into());
                    render(&window);
                    snapshot(&window, &format!("keys-{operation}-{method}-{scale}"));
                }
            }
        }
        ui.set_keys_passphrase("old secret".into());
        ui.set_keys_new_passphrase("new secret".into());
        ui.set_keys_confirm("new secret".into());
        super::apply_reset(&ui, super::ResetKind::KeyslotSecrets);
        assert_eq!(ui.get_keys_passphrase(), "");
        assert_eq!(ui.get_keys_new_passphrase(), "");
        assert_eq!(ui.get_keys_confirm(), "");
        super::apply_privilege_mode(&ui, "unprivileged");
        // macOS intentionally has no Linux privilege selector; it is a no-op.
        assert_eq!(
            ui.get_current_view(),
            if cfg!(target_os = "linux") {
                "create"
            } else {
                "keys"
            }
        );
        ui.hide().unwrap();
    }

    #[test]
    fn macos_form_switches_between_apple_and_veracrypt() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        ui.set_is_macos(true);
        ui.set_create_format("vera".into());
        ui.set_create_cipher("serpent".into());
        ui.show().unwrap();
        render(&window);
        ui.set_create_format("apple".into());
        render(&window);
        let command = super::logic::build_create_command(
            "file",
            &ui.get_create_format(),
            "/tmp",
            "test.dmg",
            "",
            "32",
            "MB",
            &ui.get_create_cipher(),
            "test password",
            "test password",
            "pass",
            "",
        )
        .unwrap();
        assert_eq!(command.cipher, lurker_core::CreateCipher::Aes);
        ui.set_current_view("manage".into());
        render(&window);
        ui.set_current_view("create".into());
        render(&window);
        assert_eq!(ui.get_create_format().as_str(), "apple");
        ui.hide().unwrap();
    }

    #[test]
    fn main_window_renders_all_views_with_software_renderer() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let window = setup_test_window();
        let ui = super::MainWindow::new().unwrap();
        ui.show().unwrap();
        for scale in [1.0, 1.25, 1.5] {
            ui.set_ui_scale(scale);
            for view in ["create", "mount", "manage"] {
                ui.set_current_view(view.into());
                for format in ["luks", "vera"] {
                    ui.set_create_format(format.into());
                    render(&window);
                }
            }
        }
        ui.hide().unwrap();
    }

    #[test]
    fn inline_probe_font_size_updates_text_layout() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = setup_test_window();
        let ui = InlineZoomProbe::new().unwrap();
        // The standalone zoom probe must work before MainWindow imports its
        // fonts, including on BSD test hosts without a system font collection.
        WindowAdapter::renderer(window.as_ref())
            .register_font_from_memory(include_bytes!("../ui/fonts/InterTight-Regular.ttf"))
            .unwrap();
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

fn set_create_formats(window: &MainWindow, os: &str, unprivileged: bool) {
    let installed_dragonfly_veracrypt = !cfg!(target_os = "dragonfly")
        || os != "dragonfly"
        || lurker_core::probe_system().is_ok_and(|probe| {
            probe.tools.iter().any(|tool| tool.name == "lurker-tcplay" && tool.path.is_some())
        });
    let formats = logic::create_formats(os, unprivileged)
        .into_iter()
        .filter(|format| *format != "vera" || installed_dragonfly_veracrypt)
        .map(Into::into)
        .collect::<Vec<slint::SharedString>>();
    window.set_create_formats(ModelRc::from(Rc::new(VecModel::from(formats))));
}

fn configure_platform(window: &MainWindow, os: &str) {
    window.set_platform_os(os.into());
    window.set_is_macos(os == "macos");
    window.set_backend_label(os.to_uppercase().into());
    set_create_formats(window, os, false);
    let format = logic::create_formats(os, false)
        .first()
        .copied()
        .unwrap_or("luks");
    window.set_create_format(format.into());
    window.set_create_file_name(
        match format {
            "apple" => "archive.dmg",
            "geli" => "archive.geli",
            "cgd" => "archive.cgd",
            "softraid" => "archive.softraid",
            _ => "archive.luks",
        }
        .into(),
    );
    window.set_mount_point(default_mountpoint().into());
}

fn default_mountpoint() -> String {
    if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|home| format!("{home}/Lurker-mount"))
            .unwrap_or_else(|_| "/Volumes/Lurker".into())
    } else {
        "/mnt/".into()
    }
}

fn selected_privilege_mode(window: &MainWindow) -> PrivilegeMode {
    if cfg!(target_os = "linux") && window.get_privilege_mode() == "unprivileged" {
        PrivilegeMode::Unprivileged
    } else {
        PrivilegeMode::Privileged
    }
}

fn mountpoint_for_mode(mode: PrivilegeMode) -> String {
    if mode == PrivilegeMode::Unprivileged {
        std::env::var("HOME")
            .map(|home| format!("{home}/Lurker-mount"))
            .unwrap_or_else(|_| default_mountpoint())
    } else {
        default_mountpoint()
    }
}

fn apply_privilege_mode(window: &MainWindow, value: &str) {
    if !cfg!(target_os = "linux") || !window.get_is_linux() {
        return;
    }
    let mode = PrivilegeMode::parse(value).unwrap_or_default();
    let unprivileged = mode == PrivilegeMode::Unprivileged;
    window.set_privilege_mode(
        if unprivileged {
            "unprivileged"
        } else {
            "privileged"
        }
        .into(),
    );
    set_create_formats(window, "linux", unprivileged);
    window.set_create_target_kind("file".into());
    window.set_create_format(if unprivileged { "vera" } else { "luks" }.into());
    let name = window.get_create_file_name();
    if name == "archive.luks" || name == "archive.hc" {
        window.set_create_file_name(
            if unprivileged {
                "archive.hc"
            } else {
                "archive.luks"
            }
            .into(),
        );
    }
    if window.get_mount_source_kind() != "file" {
        window.set_mount_source("".into());
        window.set_mount_source_label("".into());
    }
    window.set_mount_source_kind("file".into());
    let mountpoint = window.get_mount_point();
    if mountpoint == default_mountpoint()
        || mountpoint == mountpoint_for_mode(PrivilegeMode::Unprivileged)
    {
        window.set_mount_point(mountpoint_for_mode(mode).into());
    }
    clear_create_secrets(window);
    clear_mount_secrets(window);
    clear_keyslot_secrets(window);
    if unprivileged && window.get_current_view() == "keys" {
        window.set_current_view("create".into());
    }
    window.set_mount_selected_index(-1);
}
