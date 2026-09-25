//! Explicit, bounded local level check through the shared media worker.
use crate::Runtime;
use avesra_core::state::LocalState;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

pub fn stop(app: &tauri::AppHandle, expected: Option<u64>) {
    let state = app.state::<Runtime>();
    if let Ok(mut local) = state.local.lock()
        && local.microphone_check
        && expected.is_none_or(|epoch| epoch == local.capture_epoch)
    {
        local.microphone_check = false;
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
}

#[tauri::command]
pub fn begin_microphone_check(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<LocalState, String> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to check the microphone".into());
    }
    let snapshot = {
        let state = app.state::<Runtime>();
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.locked
            || local.settings.explicit_mute
            || local.settings.deafened
            || local.settings.paused
        {
            return Err("Unmute the microphone and resume local audio controls first".into());
        }
        if local.settings.microphone.is_none() {
            return Err("Select a microphone first".into());
        }
        if local.microphone_check
            || local.enrollment_capture
            || local.capture_allowed()
            || state.setup.enrollment_active()?
        {
            return Err("Finish or cancel the current capture or enrollment first".into());
        }
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        local.microphone_check = true;
        local.microphone_check_epoch = Some(local.capture_epoch);
        local.capture_error = None;
        local.refresh();
        state.publish(&local);
        let snapshot = local.clone();
        let _ = app.emit("runtime-state", snapshot.clone());
        snapshot
    };
    let epoch = snapshot.capture_epoch;
    tauri::async_runtime::spawn(async move {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let current = app
                .state::<Runtime>()
                .local
                .lock()
                .is_ok_and(|local| local.microphone_check && local.capture_epoch == epoch);
            if !current {
                return;
            }
            if Instant::now() >= deadline || !window.is_visible().unwrap_or(false) {
                stop(&app, Some(epoch));
                return;
            }
        }
    });
    Ok(snapshot)
}

#[tauri::command]
pub fn stop_microphone_check(window: tauri::WebviewWindow, epoch: u64) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings to stop the check".into());
    }
    stop(window.app_handle(), Some(epoch));
    Ok(())
}
