#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use avesra_core::{
    state::{LocalControl, LocalState, Settings},
    store::Store,
};
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
struct WriteSettings {
    settings: Settings,
    reply: oneshot::Sender<Result<(), String>>,
}
struct Runtime {
    local: Mutex<LocalState>,
    writes: SyncSender<WriteSettings>,
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
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let mut store = Store::open(&directory.join("avesra.db"))?;
            let settings = store.settings()?;
            if let Some(window) = app.get_webview_window("overlay") {
                window.set_always_on_top(settings.always_on_top)?;
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
            app.manage(Runtime {
                local: Mutex::new(LocalState::new(settings)),
                writes,
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
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("Unable to run Avesra");
}
