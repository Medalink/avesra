#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod actor_registration;
mod browser;
mod catalog;
mod comparison;
mod connection;
mod discovery;
mod media;
mod microphone_check;
mod model_health;
mod notifications;
mod output;
mod owner;
mod performance;
mod phrase_capture;
pub mod planner;
mod playback_signal;
mod preview;
mod profiles;
mod qualification;
mod setup;
mod shortcuts;
mod sound;
pub mod speech;
mod startup_greeting;
pub mod tasks;
mod traces;
mod voice;
mod voice_check;
mod voices;
mod window_positions;
use avesra_core::{
    state::{ConnectionPhase, LocalControl, LocalState, Settings},
    store::Store,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{
    Mutex,
    mpsc::{SyncSender, sync_channel},
};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tokio::sync::oneshot;
#[derive(Clone)]
struct ModeSnapshot {
    capture_epoch: u64,
    playback_epoch: u64,
    action_epoch: u64,
    muted: bool,
    deafened: bool,
    paused: bool,
}
impl From<&LocalState> for ModeSnapshot {
    fn from(local: &LocalState) -> Self {
        Self {
            capture_epoch: local.capture_epoch,
            playback_epoch: local.playback_epoch,
            action_epoch: local.action_epoch,
            muted: local.settings.explicit_mute || local.locked,
            deafened: local.settings.deafened,
            paused: local.settings.paused || local.locked,
        }
    }
}
struct WriteSettings {
    settings: Settings,
    reply: oneshot::Sender<Result<(), String>>,
}
struct Runtime {
    performance: std::sync::Arc<performance::State>,
    discovery: discovery::Discovery,
    browser: browser::BrowserSetup,
    catalog: catalog::CatalogSetup,
    tasks: tasks::State,
    turns: Mutex<avesra_core::voice::TurnGate>,
    planner: planner::NativePlanner,
    acknowledged_session: Mutex<Option<connection::SessionIdentity>>,
    setup: setup::Setup,
    media: media::MediaWorker,
    effects: avesra_windows::effects::NativeEffects,
    local: Mutex<LocalState>,
    writes: SyncSender<WriteSettings>,
    modes: tokio::sync::watch::Sender<ModeSnapshot>,
    connection: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    connection_generation: AtomicU64,
    startup_connection_attempted: std::sync::atomic::AtomicBool,
    pairing: tokio::sync::Mutex<()>,
    health: tokio::sync::Mutex<()>,
    voice_check: voice_check::State,
    qualification: qualification::State,
    preview: std::sync::Arc<tokio::sync::Mutex<()>>,
    voice_panel: Mutex<Option<uuid::Uuid>>,
    hotkeys: Mutex<Option<avesra_windows::shortcuts::Hotkeys>>,
    shortcut_edit: tokio::sync::Mutex<()>,
    owner_setup: std::sync::Arc<tokio::sync::Mutex<()>>,
    shortcut_recording: Mutex<Option<shortcuts::Recording>>,
    shortcut_focus_generation: std::sync::atomic::AtomicU64,
}
impl Runtime {
    fn publish(&self, local: &LocalState) {
        if local.locked || !local.connected {
            self.tasks.invalidate();
            self.catalog.invalidate();
            self.browser.invalidate();
        }
        self.browser.observe(local);
        if local.locked
            && let Ok(mut slot) = self.shortcut_recording.lock()
        {
            self.shortcut_focus_generation
                .fetch_add(1, Ordering::SeqCst);
            *slot = None;
        }
        if !local.connected
            && let Ok(mut session) = self.acknowledged_session.lock()
        {
            *session = None;
        }
        self.setup.observe(
            local.capture_epoch,
            self.connection_generation.load(Ordering::SeqCst),
        );
        self.qualification.observe(self, local);
        self.media.publish(local);
        self.effects.observe(
            local.action_epoch,
            local.connected && local.enrolled && !local.locked && !local.settings.paused,
        );
        self.modes.send_replace(ModeSnapshot::from(local));
    }
}
fn enqueue(
    runtime: &Runtime,
    settings: Settings,
) -> Result<oneshot::Receiver<Result<(), String>>, String> {
    let (reply, receive) = oneshot::channel();
    runtime
        .writes
        .try_send(WriteSettings { settings, reply })
        .map_err(|_| "Settings writer is unavailable or busy")?;
    Ok(receive)
}
#[tauri::command]
fn runtime_snapshot(state: tauri::State<'_, Runtime>) -> Result<LocalState, String> {
    Ok(state
        .local
        .lock()
        .map_err(|_| "Local state unavailable")?
        .clone())
}
fn invalidate_settings(app: &tauri::AppHandle) {
    app.state::<Runtime>().tasks.invalidate();
    app.state::<Runtime>().qualification.settings_hidden();
    microphone_check::stop(app, None);
    app.state::<Runtime>().discovery.invalidate();
    app.state::<Runtime>().catalog.invalidate();
    app.state::<Runtime>().browser.settings_hidden();
    setup::cancel_native(app);
    let _ = app.emit("settings-hidden", ());
}
#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    match window.label() {
        "settings" => invalidate_settings(window.app_handle()),
        "overlay" => {}
        _ => return Err("Unknown Avesra window".into()),
    }
    window_positions::save(window.app_handle());
    window
        .hide()
        .map_err(|_| "Window could not be hidden".into())
}
#[tauri::command]
async fn audio_devices() -> Result<Vec<avesra_windows::AudioDevice>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        avesra_windows::audio_devices().map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Device enumeration failed")?
}
#[tauri::command]
async fn audio_lane_health(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<connection::AudioLaneHealth, String> {
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Open Settings to inspect deployments".into());
    }
    let state = app.state::<Runtime>();
    let _guard = state
        .health
        .try_lock()
        .map_err(|_| "A deployment probe is already active")?;
    let (epoch, generation) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected || local.locked {
            return Err("Connect Spark before inspecting deployments".into());
        }
        (
            local.capture_epoch,
            state.connection_generation.load(Ordering::SeqCst),
        )
    };
    let current = || {
        state.local.lock().is_ok_and(|local| {
            local.connected
                && !local.locked
                && local.capture_epoch == epoch
                && state.connection_generation.load(Ordering::SeqCst) == generation
        })
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    let record = tauri::async_runtime::spawn_blocking(move || connection::load(&directory))
        .await
        .map_err(|_| "Pairing unavailable")??;
    if !current() {
        return Err("Deployment probe context changed".into());
    }
    let value = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        connection::audio_lane_health(&record),
    )
    .await
    .map_err(|_| "Deployment probe timed out")??;
    if !window.is_visible().map_err(|_| "Settings unavailable")? || !current() {
        return Err("Deployment probe context changed".into());
    }
    Ok(value)
}
#[tauri::command]
async fn local_control(
    control: LocalControl,
    app: tauri::AppHandle,
    state: tauri::State<'_, Runtime>,
) -> Result<LocalState, String> {
    let (snapshot, pending) = {
        // Every terminal control, including direct IPC, cancels the same native
        // connection generation as the pairing page. A late acknowledgement or
        // startup credential load cannot revive it.
        let mut connection = if matches!(
            control,
            LocalControl::Lock | LocalControl::Disconnect | LocalControl::Quit
        ) {
            Some(
                state
                    .connection
                    .lock()
                    .map_err(|_| "Connection manager unavailable")?,
            )
        } else {
            None
        };
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if let Some(slot) = connection.as_mut() {
            state.connection_generation.fetch_add(1, Ordering::SeqCst);
            if let Some(task) = slot.take() {
                task.abort();
            }
        }
        if matches!(control, LocalControl::Unmute) {
            state.qualification.resume_personal();
        }
        local.apply(control);
        let snapshot = local.clone();
        state.publish(&snapshot);
        let pending = enqueue(&state, snapshot.settings.clone());
        (snapshot, pending)
    };
    // The control and event precede any disk wait; the local lock never covers I/O.
    app.emit("runtime-state", &snapshot)
        .map_err(|_| "Unable to notify windows")?;
    pending?.await.map_err(|_| "Settings writer stopped")??;
    runtime_snapshot(state)
}
#[tauri::command]
async fn save_settings(
    settings: Settings,
    app: tauri::AppHandle,
    state: tauri::State<'_, Runtime>,
) -> Result<LocalState, String> {
    settings.validate().map_err(|e| e.to_string())?;
    let (snapshot, pending, scale_changed) = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if settings.explicit_mute != local.settings.explicit_mute
            || settings.deafened != local.settings.deafened
            || settings.paused != local.settings.paused
        {
            return Err("Use local controls to change listening modes".into());
        }
        if settings.owner_name != local.settings.owner_name {
            return Err("Use the remembered name control to update your name".into());
        }
        if settings.sound != local.settings.sound
            || settings.speech_volume != local.settings.speech_volume
        {
            return Err(
                "Sound settings changed. Refresh and use the Voice atmosphere controls.".into(),
            );
        }
        if settings.shortcuts != local.settings.shortcuts {
            return Err(
                "Shortcuts changed. Refresh settings and use the native shortcut editor.".into(),
            );
        }
        let pending = enqueue(&state, settings.clone())?;
        let scale_changed = settings.interface_scale != local.settings.interface_scale;
        // Only these two profiles share the unchanged Spark inference route.
        let same_spark_route = matches!(
            (local.settings.profile, settings.profile),
            (
                avesra_contracts::Profile::SingleSpark,
                avesra_contracts::Profile::Gaming
            ) | (
                avesra_contracts::Profile::Gaming,
                avesra_contracts::Profile::SingleSpark
            )
        );
        let route_changed = settings.profile != local.settings.profile && !same_spark_route;
        if settings.profile != local.settings.profile
            && settings.profile == avesra_contracts::Profile::Gaming
        {
            tasks::teaching::defer_passive(&state);
        }
        if settings.speaker != local.settings.speaker || route_changed {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
            local.action_epoch = local.action_epoch.saturating_add(1);
        }
        if settings.microphone != local.settings.microphone
            || settings.speaker != local.settings.speaker
            || route_changed
        {
            local.microphone_check = false;
            local.enrollment_capture = false;
            local.capture_epoch = local.capture_epoch.saturating_add(1);
        }
        local.settings = settings;
        local.refresh();
        state.publish(&local);
        (local.clone(), pending, scale_changed)
    };
    app.emit("runtime-state", &snapshot)
        .map_err(|_| "Unable to notify windows")?;
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_always_on_top(snapshot.settings.always_on_top)
            .map_err(|_| "Preference changed but window update failed")?;
    }
    if scale_changed {
        apply_interface_scale(&app, snapshot.settings.interface_scale)
            .map_err(|_| "Preference changed but window update failed")?;
    }
    pending.await.map_err(|_| "Settings writer stopped")??;
    runtime_snapshot(state)
}
fn start_connection(
    app: &tauri::AppHandle,
    record: connection::PairingRecord,
    expected_generation: u64,
) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let mut slot = state
        .connection
        .lock()
        .map_err(|_| "Connection manager unavailable")?;
    let generation = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.locked || state.connection_generation.load(Ordering::SeqCst) != expected_generation
        {
            return Err("Connection request cancelled or superseded".into());
        }
        let generation = state.connection_generation.fetch_add(1, Ordering::SeqCst) + 1;
        local.apply(LocalControl::Disconnect);
        local.connection_phase = ConnectionPhase::Connecting;
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
        generation
    };
    if let Some(previous) = slot.take() {
        previous.abort();
    }
    let app_handle = app.clone();
    let modes = state.modes.subscribe();
    *slot = Some(tauri::async_runtime::spawn(async move {
        let outcome = connection::run(app_handle.clone(), record, modes, generation).await;
        let state = app_handle.state::<Runtime>();
        if let Ok(mut local) = state.local.lock()
            && state.connection_generation.load(Ordering::SeqCst) == generation
        {
            local.apply(LocalControl::Disconnect);
            if let Err(error) = outcome {
                local.connection_phase = ConnectionPhase::Error;
                local.connection_error = Some(error);
                local.refresh();
            }
            state.publish(&local);
            let _ = app_handle.emit("runtime-state", local.clone());
        }
    }));
    Ok(())
}
#[tauri::command]
async fn pair_spark(
    input: connection::PairInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, Runtime>,
) -> Result<(), String> {
    let _guard = state
        .pairing
        .try_lock()
        .map_err(|_| "Pairing already in progress")?;
    let generation = state.connection_generation.load(Ordering::SeqCst);
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    let record = connection::pair(input, &directory).await?;
    start_connection(&app, record, generation)
}
#[tauri::command]
async fn connect_spark(app: tauri::AppHandle) -> Result<(), String> {
    let generation = app
        .state::<Runtime>()
        .connection_generation
        .load(Ordering::SeqCst);
    connect_saved(app, generation, false).await
}
async fn connect_saved(
    app: tauri::AppHandle,
    generation: u64,
    startup: bool,
) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let _guard = state
        .pairing
        .try_lock()
        .map_err(|_| "Pairing management already in progress")?;
    {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.locked || state.connection_generation.load(Ordering::SeqCst) != generation {
            return Err("Connection request cancelled or superseded".into());
        }
        if local.connected || matches!(local.connection_phase, ConnectionPhase::Connecting) {
            return Ok(());
        }
        local.connection_phase = ConnectionPhase::Connecting;
        local.connection_error = None;
        local.refresh();
        let _ = app.emit("runtime-state", local.clone());
    }
    let outcome: Result<(), String> = async {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "Local data directory unavailable")?;
        let record = tauri::async_runtime::spawn_blocking(move || {
            if startup
                && !directory
                    .join("spark-pairing.dpapi")
                    .try_exists()
                    .map_err(|_| "Saved pairing unavailable")?
            {
                return Ok(None);
            }
            connection::load(&directory).map(Some)
        })
        .await
        .map_err(|_| "Credential loading failed")??;
        if let Some(record) = record {
            start_connection(&app, record, generation)?;
        } else {
            connection_attempt_finished(&app, generation, None);
        }
        Ok(())
    }
    .await;
    if let Err(ref error) = outcome {
        connection_attempt_finished(&app, generation, Some(error.clone()));
    }
    outcome
}
fn connection_attempt_finished(app: &tauri::AppHandle, generation: u64, error: Option<String>) {
    let state = app.state::<Runtime>();
    if let Ok(mut local) = state.local.lock()
        && state.connection_generation.load(Ordering::SeqCst) == generation
    {
        local.connection_phase = if error.is_some() {
            ConnectionPhase::Error
        } else {
            ConnectionPhase::Disconnected
        };
        local.connection_error = error;
        local.refresh();
        let _ = app.emit("runtime-state", local.clone());
    }
}
#[tauri::command]
async fn saved_pairing(app: tauri::AppHandle) -> Result<Option<connection::SavedPairing>, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || connection::saved_pairing(&directory))
        .await
        .map_err(|_| "Pairing inspection failed")?
}
#[tauri::command]
async fn forget_spark(
    app: tauri::AppHandle,
    file_revision: String,
    understand_revocation: bool,
) -> Result<(), String> {
    if !understand_revocation {
        return Err("Review the server revocation requirement first".into());
    }
    let state = app.state::<Runtime>();
    let _guard = state
        .pairing
        .try_lock()
        .map_err(|_| "Pairing management already in progress")?;
    disconnect_spark(app.clone(), app.state::<Runtime>())?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || connection::forget(&directory, &file_revision))
        .await
        .map_err(|_| "Pairing removal failed")?
}
#[tauri::command]
fn disconnect_spark(app: tauri::AppHandle, state: tauri::State<'_, Runtime>) -> Result<(), String> {
    let mut slot = state
        .connection
        .lock()
        .map_err(|_| "Connection manager unavailable")?;
    let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state.connection_generation.fetch_add(1, Ordering::SeqCst);
    local.apply(LocalControl::Disconnect);
    state.publish(&local);
    let snapshot = local.clone();
    drop(local);
    if let Some(task) = slot.take() {
        task.abort();
    }
    app.emit("runtime-state", snapshot)
        .map_err(|_| "Unable to notify windows".into())
}
fn session_changed(app: &tauri::AppHandle, locked: bool) {
    let state = app.state::<Runtime>();
    // Match connection-start/disconnect lock order. No window getter, disk or
    // network operation occurs while these native state locks are held.
    let Ok(mut slot) = state.connection.lock() else {
        return;
    };
    let Ok(mut local) = state.local.lock() else {
        return;
    };
    if locked {
        state.connection_generation.fetch_add(1, Ordering::SeqCst);
        local.apply(LocalControl::Lock);
        local.voice_ready = false;
        if let Some(task) = slot.take() {
            task.abort();
        }
    } else {
        local.locked = false;
        local.refresh();
    }
    state.publish(&local);
    let snapshot = local.clone();
    drop(local);
    drop(slot);
    let _ = app.emit("signal-clear", ());
    let _ = app.emit("runtime-state", snapshot);
    if !locked
        && !state
            .startup_connection_attempted
            .swap(true, Ordering::SeqCst)
    {
        let generation = state.connection_generation.load(Ordering::SeqCst);
        let Ok(epoch) = state.local.lock().map(|local| local.playback_epoch) else {
            return;
        };
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if connect_saved(app.clone(), generation, true).await.is_ok() {
                // Saved connection start advances both once. Any intervening
                // Stop/output change or replacement connection suppresses hello.
                startup_greeting::run(app, generation.saturating_add(1), epoch.saturating_add(1))
                    .await;
            }
        });
    }
}
/// Reference window sizes at 100%, matching the approved mockups in `design/`.
const OVERLAY_SIZE: (f64, f64) = (440.0, 124.0);
const SETTINGS_SIZE: (f64, f64) = (880.0, 640.0);
/// Zooms both webviews uniformly and scales the settings window to match. The
/// overlay's height depends on its expanded state, so its webview resizes itself.
fn apply_interface_scale(app: &tauri::AppHandle, percent: u16) -> tauri::Result<()> {
    let zoom = f64::from(percent) / 100.0;
    for label in ["overlay", "settings"] {
        if let Some(window) = app.get_webview_window(label) {
            window.set_zoom(zoom)?;
        }
    }
    if let Some(window) = app.get_webview_window("settings") {
        let size = tauri::LogicalSize::new(SETTINGS_SIZE.0 * zoom, SETTINGS_SIZE.1 * zoom);
        window.set_min_size(Some(size))?;
        window.set_size(size)?;
    }
    Ok(())
}
fn main() {
    #[cfg(windows)]
    if let Err(error) = avesra_windows::output_recording::configure_from_args() {
        eprintln!("{error}");
        std::process::exit(2);
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Diagnostic launch arguments never turn an existing audible process
            // into a recording process or activate a window on another desktop.
            #[cfg(windows)]
            if avesra_windows::output_recording::enabled()
                || args.iter().any(|arg| arg.starts_with("--capture-output"))
            {
                return;
            }
            // Keep explicitly headless inspection configurations invisible.
            if !app
                .config()
                .app
                .windows
                .iter()
                .any(|window| window.label == "settings" && window.visible)
            {
                return;
            }
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::POSITION)
                .skip_initial_state("overlay")
                .skip_initial_state("settings")
                .build(),
        )
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            // Backstop simultaneous launches before the plugin's message window exists.
            // The OS releases this lock on exit, including a crash; never delete the file.
            let instance_lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(directory.join("desktop-instance.lock"))?;
            instance_lock.try_lock().map_err(|_| {
                std::io::Error::other(
                    "Avesra is already running or its startup lock is unavailable",
                )
            })?;
            app.manage(instance_lock);
            let _ = avesra_core::trace::initialize(&directory, avesra_core::trace::Host::Native);
            avesra_windows::resources::start();
            let mut store = Store::open(&directory.join("avesra.db"))?;
            let mut settings = store.settings()?;
            if (settings.microphone.is_none() || settings.speaker.is_none())
                && let Ok(devices) = avesra_windows::audio_devices()
            {
                if settings.microphone.is_none() {
                    settings.microphone = devices
                        .iter()
                        .find(|v| v.direction == "input" && v.is_default)
                        .map(|v| v.id.clone());
                }
                if settings.speaker.is_none() {
                    settings.speaker = devices
                        .iter()
                        .find(|v| v.direction == "output" && v.is_default)
                        .map(|v| v.id.clone());
                }
                store.save_settings(&settings)?;
            }
            let zoom = f64::from(settings.interface_scale) / 100.0;
            apply_interface_scale(app.handle(), settings.interface_scale)?;
            if let Some(window) = app.get_webview_window("overlay") {
                window.set_always_on_top(settings.always_on_top)?;
                window.set_size(tauri::LogicalSize::new(
                    OVERLAY_SIZE.0 * zoom,
                    OVERLAY_SIZE.1 * zoom,
                ))?;
                if let Some(monitor) = window.current_monitor()? {
                    let area = monitor.work_area();
                    let scale = monitor.scale_factor();
                    let width = (OVERLAY_SIZE.0 * zoom + 24.0) * scale;
                    let x =
                        area.position.x + ((f64::from(area.size.width) - width).max(0.0)) as i32;
                    let y = area.position.y + (24.0 * scale) as i32;
                    window.set_position(tauri::PhysicalPosition::new(x, y))?;
                }
            }
            if let Some(window) = app.get_webview_window("settings") {
                window.center()?;
            }
            for window in app.webview_windows().values() {
                window_positions::restore(window)?;
            }
            let (writes, receiver) = sync_channel::<WriteSettings>(32);
            let app_handle = app.handle().clone();
            std::thread::Builder::new()
                .name("avesra-storage".into())
                .spawn(move || {
                    while let Ok(write) = receiver.recv() {
                        let result = store.save_settings(&write.settings).map_err(|_| {
                            "Preference applied in memory but could not be saved".to_string()
                        });
                        if let Err(ref error) = result {
                            let _ = app_handle.emit("runtime-error", error);
                        }
                        let _ = write.reply.send(result);
                    }
                })?;
            let mut local = LocalState::new(settings);
            // Closed until own-session WTS registration and initial query succeed.
            local.locked = true;
            local.refresh();
            let (modes, _) = tokio::sync::watch::channel(ModeSnapshot::from(&local));
            let media = media::MediaWorker::spawn(app.handle().clone())?;
            media.publish(&local);
            // Volume targets and accepted-intent ingress remain closed. Native
            // authenticated Settings can register immutable application aliases.
            let effects = avesra_windows::effects::NativeEffects::spawn(
                directory.join("native-actions.db"),
                Vec::new(),
            )?;
            let initial_shortcuts = local.settings.shortcuts.clone();
            app.manage(Runtime {
                performance: std::sync::Arc::new(performance::State::default()),
                discovery: discovery::Discovery::default(),
                browser: browser::BrowserSetup::default(),
                catalog: catalog::CatalogSetup::default(),
                tasks: tasks::State::default(),
                turns: Mutex::new(avesra_core::voice::TurnGate::default()),
                acknowledged_session: Mutex::new(None),
                setup: setup::Setup::default(),
                media,
                effects,
                local: Mutex::new(local),
                writes,
                modes,
                connection: Mutex::new(None),
                connection_generation: AtomicU64::new(0),
                startup_connection_attempted: std::sync::atomic::AtomicBool::new(false),
                pairing: tokio::sync::Mutex::new(()),
                health: tokio::sync::Mutex::new(()),
                voice_check: voice_check::State::default(),
                qualification: qualification::State::default(),
                preview: std::sync::Arc::new(tokio::sync::Mutex::new(())),
                voice_panel: Mutex::new(None),
                hotkeys: Mutex::new(None),
                shortcut_edit: tokio::sync::Mutex::new(()),
                planner: planner::NativePlanner::default(),
                owner_setup: std::sync::Arc::new(tokio::sync::Mutex::new(())),
                shortcut_recording: Mutex::new(None),
                shortcut_focus_generation: std::sync::atomic::AtomicU64::new(1),
            });
            notifications::start(app.handle().clone());
            tasks::teaching::start_passive(app.handle().clone());
            model_health::init(app.handle());
            // Receiver is independent of the selected transport. No worker
            // emits an accepted-read offer until the full dispatch is wired.
            browser::start_read_preparation(app.handle().clone())
                .map_err(|_| "Native browser preparation unavailable")?;
            let hotkey_app = app.handle().clone();
            let hotkeys =
                avesra_windows::shortcuts::Hotkeys::spawn(initial_shortcuts, move |action| {
                    shortcuts::pressed(&hotkey_app, action)
                });
            match hotkeys {
                Ok(value) => {
                    if let Ok(mut slot) = app.state::<Runtime>().hotkeys.lock() {
                        *slot = Some(value);
                    }
                }
                Err(_) => {
                    let _ = app.emit(
                        "runtime-error",
                        "Global shortcuts unavailable; overlay and tray controls remain available.",
                    );
                }
            }
            if let Some(window) = app.get_webview_window("settings") {
                let handle = window.hwnd()?;
                let app_handle = app.handle().clone();
                if avesra_windows::session::install(handle.0 as isize, move |locked| {
                    session_changed(&app_handle, locked);
                })
                .is_err()
                {
                    let _ = app.emit(
                        "runtime-error",
                        "Windows session monitoring unavailable; capture remains closed.",
                    );
                }
            }
            let show = MenuItem::with_id(app, "show", "Show Avesra", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause / resume", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Avesra", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &pause, &quit])?;
            let mut pixels = vec![0u8; 32 * 32 * 4];
            for y in 4..28 {
                for x in 13..19 {
                    let i = (y * 32 + x) * 4;
                    pixels[i..i + 4].copy_from_slice(&[224, 17, 95, 255]);
                }
            }
            TrayIconBuilder::new()
                .icon(tauri::image::Image::new_owned(pixels, 32, 32))
                .tooltip("Avesra")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" | "settings" => {
                        let label = if event.id.as_ref() == "show" {
                            "overlay"
                        } else {
                            "settings"
                        };
                        if let Some(window) = app.get_webview_window(label) {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "pause" => {
                        let state = app.state::<Runtime>();
                        if let Ok(mut local) = state.local.lock() {
                            let control = if local.settings.paused {
                                LocalControl::Resume
                            } else {
                                LocalControl::Pause
                            };
                            local.apply(control);
                            let snapshot = local.clone();
                            state.publish(&snapshot);
                            let pending = enqueue(&state, snapshot.settings.clone());
                            drop(local);
                            let _ = app.emit("runtime-state", snapshot);

                            if pending.is_err() {
                                let _ = app.emit(
                                    "runtime-error",
                                    "Pause applied but settings writer is unavailable",
                                );
                            }
                        }
                    }
                    "quit" => {
                        window_positions::save(app);
                        if let Ok(mut owner) = app.state::<Runtime>().hotkeys.lock() {
                            owner.take();
                        }
                        if let Ok(mut local) = app.state::<Runtime>().local.lock() {
                            local.apply(LocalControl::Quit);
                            app.state::<Runtime>().publish(&local);
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "settings"
                && matches!(
                    event,
                    tauri::WindowEvent::Focused(false)
                        | tauri::WindowEvent::CloseRequested { .. }
                        | tauri::WindowEvent::Destroyed
                )
            {
                shortcuts::cancel_recording(window.app_handle());
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window_positions::save(window.app_handle());
                if window.label() == "settings" {
                    invalidate_settings(window.app_handle());
                }
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            microphone_check::begin_microphone_check,
            microphone_check::stop_microphone_check,
            voice_check::check_saved_voice,
            voice_check::cancel_voice_check,
            qualification::freeze_voice_calibration,
            qualification::discard_voice_calibration,
            qualification::voice_calibration_status,
            qualification::annotate_voice_activity,
            qualification::review_voice_admission,
            qualification::review_development_voice,
            qualification::revalidate_voice_admission,
            qualification::revoke_voice_admission,
            notifications::notification_status,
            model_health::reasoning_health,
            discovery::discover_sparks,
            discovery::cancel_spark_discovery,
            discovery::pair_discovered_spark,
            browser::begin_browser_pairing,
            browser::connect_selected_browser,
            browser::documents::inspect_browser_documents,
            browser::documents::browser_documents_status,
            browser::documents::cancel_browser_documents,
            browser::documents::select_browser_document,
            browser::scopes::propose_browser_scope,
            browser::scopes::saved_browser_scopes,
            browser::scopes::cancel_browser_scope,
            browser::scopes::revoke_browser_scope,
            browser::browser_pairing_status,
            browser::approve_browser_pairing,
            browser::cancel_browser_pairing,
            browser::release_browser_management,
            browser::saved_browser_pairings,
            browser::revoke_browser_pairing,
            browser::browser_selections,
            browser::select_browser_pairing,
            browser::clear_browser_selection,
            hide_window,
            catalog::open_app_catalog,
            catalog::close_app_catalog,
            catalog::scan_app_catalog,
            catalog::choose_app_folder,
            catalog::choose_app_executable,
            catalog::observe_app_windows,
            catalog::choose_app_window,
            catalog::app_aliases,
            catalog::remember_app,
            catalog::forget_app_alias,
            tasks::open_action_panel,
            performance::performance_snapshot,
            comparison::trace_cohorts,
            traces::save_trace_cohort,
            traces::accepted_traces,
            traces::export_accepted_traces,
            traces::set_trace_retention,
            tasks::close_action_panel,
            tasks::action_status,
            tasks::grant_app_action,
            tasks::grant_volume_action,
            tasks::grant_diagnostic_action,
            tasks::inspect_vpn_profile,
            tasks::grant_vpn_action,
            tasks::download::grant_download_diagnosis,
            tasks::download::grant_download_fix,
            tasks::download::approve_download_fix,
            tasks::grant_browser_read_action,
            tasks::change_private_memory,
            tasks::teaching::teaching_status,
            tasks::teaching::teaching_progress,
            tasks::teaching::set_teaching_scope,
            tasks::teaching::passive::set_passive_teaching,
            tasks::teaching::start_demonstration,
            tasks::teaching::stop_demonstration,
            tasks::teaching::change_demonstration,
            tasks::memory::memory_forget_status,
            tasks::memory::approve_memory_forget,
            tasks::memory::cancel_memory_forget,
            tasks::inspect_prompt_surface,
            tasks::bind_prompt_project,
            tasks::revoke_action_permission,
            tasks::cancel_action_task,
            shortcuts::shortcut_status,
            owner::owner_status,
            owner::create_owner,
            owner::remember_owner_name,
            actor_registration::actor_registration_status,
            actor_registration::register_owner_with_spark,
            actor_registration::revoke_owner_registration,
            shortcuts::set_shortcut,
            shortcuts::begin_shortcut_recording,
            shortcuts::end_shortcut_recording,
            preview::preview_voice,
            voices::open_voice_panel,
            voices::close_voice_panel,
            voices::voice_operation,
            setup::setup_status,
            setup::verify_setup,
            setup::cancel_setup,
            setup::begin_enrollment,
            setup::finish_enrollment,
            setup::speaker_candidates,
            setup::delete_speaker_candidate,
            setup::select_speaker_candidate,
            setup::clear_speaker_selection,
            setup::record_enrollment,
            runtime_snapshot,
            playback_signal::playback_signal_clock,
            audio_devices,
            audio_lane_health,
            local_control,
            save_settings,
            sound::update_sound,
            sound::sound_output_channels,
            sound::sound_diagnostics,
            pair_spark,
            connect_spark,
            saved_pairing,
            forget_spark,
            disconnect_spark
        ])
        .run(tauri::generate_context!())
        .expect("Unable to run Avesra");
}
