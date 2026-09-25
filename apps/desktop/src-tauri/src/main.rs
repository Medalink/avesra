#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod connection;
use avesra_core::{
    state::{LocalControl, LocalState, Settings},
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
    action_epoch: u64,
    muted: bool,
    deafened: bool,
    paused: bool,
}
impl From<&LocalState> for ModeSnapshot {
    fn from(local: &LocalState) -> Self {
        Self {
            capture_epoch: local.capture_epoch,
            action_epoch: local.action_epoch,
            muted: local.settings.explicit_mute,
            deafened: local.settings.deafened,
            paused: local.settings.paused,
        }
    }
}
struct WriteSettings {
    settings: Settings,
    reply: oneshot::Sender<Result<(), String>>,
}
struct Runtime {
    local: Mutex<LocalState>,
    writes: SyncSender<WriteSettings>,
    modes: tokio::sync::watch::Sender<ModeSnapshot>,
    connection: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    connection_generation: AtomicU64,
    pairing: tokio::sync::Mutex<()>,
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
#[tauri::command]
async fn audio_devices() -> Result<Vec<avesra_windows::AudioDevice>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        avesra_windows::audio_devices().map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Device enumeration failed")?
}
#[tauri::command]
async fn local_control(
    control: LocalControl,
    app: tauri::AppHandle,
    state: tauri::State<'_, Runtime>,
) -> Result<LocalState, String> {
    let (snapshot, pending) = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        local.apply(control);
        let snapshot = local.clone();
        state.modes.send_replace(ModeSnapshot::from(&snapshot));
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
    let (snapshot, pending) = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if settings.explicit_mute != local.settings.explicit_mute
            || settings.deafened != local.settings.deafened
            || settings.paused != local.settings.paused
        {
            return Err("Use local controls to change listening modes".into());
        }
        let pending = enqueue(&state, settings.clone())?;
        local.settings = settings;
        local.refresh();
        (local.clone(), pending)
    };
    app.emit("runtime-state", &snapshot)
        .map_err(|_| "Unable to notify windows")?;
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_always_on_top(snapshot.settings.always_on_top)
            .map_err(|_| "Preference changed but window update failed")?;
    }
    pending.await.map_err(|_| "Settings writer stopped")??;
    runtime_snapshot(state)
}
fn start_connection(
    app: &tauri::AppHandle,
    record: connection::PairingRecord,
) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let mut slot = state
        .connection
        .lock()
        .map_err(|_| "Connection manager unavailable")?;
    let generation = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let generation = state.connection_generation.fetch_add(1, Ordering::SeqCst) + 1;
        local.apply(LocalControl::Disconnect);
        state.modes.send_replace(ModeSnapshot::from(&*local));
        app.emit("runtime-state", local.clone())
            .map_err(|_| "Unable to notify windows")?;
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
            state.modes.send_replace(ModeSnapshot::from(&*local));
            let _ = app_handle.emit("runtime-state", local.clone());
            if let Err(error) = outcome {
                let _ = app_handle.emit("runtime-error", error);
            }
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
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    let record = connection::pair(input, &directory).await?;
    start_connection(&app, record)
}
#[tauri::command]
async fn connect_spark(app: tauri::AppHandle) -> Result<(), String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    let record = tauri::async_runtime::spawn_blocking(move || connection::load(&directory))
        .await
        .map_err(|_| "Credential loading failed")??;
    start_connection(&app, record)
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
    state.modes.send_replace(ModeSnapshot::from(&*local));
    let snapshot = local.clone();
    drop(local);
    if let Some(task) = slot.take() {
        task.abort();
    }
    app.emit("runtime-state", snapshot)
        .map_err(|_| "Unable to notify windows".into())
}
fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let mut store = Store::open(&directory.join("avesra.db"))?;
            let settings = store.settings()?;
            if let Some(window) = app.get_webview_window("overlay") {
                window.set_always_on_top(settings.always_on_top)?;
                if let Some(monitor) = window.current_monitor()? {
                    let area = monitor.work_area();
                    let scale = monitor.scale_factor();
                    let x = area.position.x
                        + ((f64::from(area.size.width) - 464.0 * scale).max(0.0)) as i32;
                    let y = area.position.y + (24.0 * scale) as i32;
                    window.set_position(tauri::PhysicalPosition::new(x, y))?;
                }
            }
            if let Some(window) = app.get_webview_window("settings") {
                window.center()?;
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
            let local = LocalState::new(settings);
            let (modes, _) = tokio::sync::watch::channel(ModeSnapshot::from(&local));
            app.manage(Runtime {
                local: Mutex::new(local),
                writes,
                modes,
                connection: Mutex::new(None),
                connection_generation: AtomicU64::new(0),
                pairing: tokio::sync::Mutex::new(()),
            });
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
                            state.modes.send_replace(ModeSnapshot::from(&snapshot));
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
                        if let Ok(mut local) = app.state::<Runtime>().local.lock() {
                            local.apply(LocalControl::Quit);
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            runtime_snapshot,
            audio_devices,
            local_control,
            save_settings,
            pair_spark,
            connect_spark,
            disconnect_spark
        ])
        .run(tauri::generate_context!())
        .expect("Unable to run Avesra");
}
