use crate::{Runtime, enqueue};
use avesra_core::{
    shortcuts::{Chord, ShortcutAction},
    state::LocalControl,
};
use avesra_windows::shortcuts::ShortcutStatus;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use uuid::Uuid;

pub struct Recording {
    id: Uuid,
    expires: Instant,
}
#[derive(Clone, serde::Serialize)]
struct Recorded {
    id: Uuid,
    chord: Chord,
}
pub fn cancel_recording(app: &tauri::AppHandle) {
    let state = app.state::<Runtime>();
    if let Ok(mut slot) = state.shortcut_recording.lock() {
        state
            .shortcut_focus_generation
            .fetch_add(1, Ordering::SeqCst);
        *slot = None;
    }
}
#[tauri::command]
pub fn begin_shortcut_recording(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
) -> Result<Uuid, String> {
    let generation = state.shortcut_focus_generation.load(Ordering::SeqCst);
    if window.label() != "settings"
        || !window.is_visible().unwrap_or(false)
        || !window.is_focused().unwrap_or(false)
    {
        return Err("Use focused Settings".into());
    }
    let local = state.local.lock().map_err(|_| "Settings unavailable")?;
    if local.locked {
        return Err("Unlock Windows first".into());
    }
    let id = Uuid::new_v4();
    let mut slot = state
        .shortcut_recording
        .lock()
        .map_err(|_| "Shortcut editor unavailable")?;
    if generation != state.shortcut_focus_generation.load(Ordering::SeqCst) {
        return Err("Settings focus changed".into());
    }
    *slot = Some(Recording {
        id,
        expires: Instant::now() + Duration::from_secs(10),
    });
    Ok(id)
}
#[tauri::command]
pub fn end_shortcut_recording(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
    id: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    let mut slot = state
        .shortcut_recording
        .lock()
        .map_err(|_| "Shortcut editor unavailable")?;
    if slot.as_ref().is_some_and(|v| v.id == id) {
        *slot = None;
    }
    Ok(())
}
fn record_owned(app: &tauri::AppHandle, action: ShortcutAction) -> bool {
    let state = app.state::<Runtime>();
    let id = state.shortcut_recording.lock().ok().and_then(|v| {
        v.as_ref()
            .filter(|v| Instant::now() < v.expires)
            .map(|v| v.id)
    });
    let Some(id) = id else {
        return false;
    };
    let Some(window) = app.get_webview_window("settings") else {
        return false;
    };
    if !window.is_visible().unwrap_or(false) || !window.is_focused().unwrap_or(false) {
        cancel_recording(app);
        return false;
    }
    let Ok(local) = state.local.lock() else {
        return false;
    };
    if local.locked {
        cancel_recording(app);
        return false;
    }
    let chord = state
        .hotkeys
        .lock()
        .ok()
        .and_then(|v| v.as_ref().and_then(|v| v.snapshot().ok()))
        .and_then(|v| {
            v.into_iter()
                .find(|v| v.action == action)
                .and_then(|v| v.binding)
        });
    let Some(chord) = chord else {
        return false;
    };
    let Ok(mut slot) = state.shortcut_recording.lock() else {
        return false;
    };
    if !slot
        .as_ref()
        .is_some_and(|v| v.id == id && Instant::now() < v.expires)
    {
        return false;
    }
    *slot = None;
    drop(slot);
    drop(local);
    let _ = window.emit("shortcut-recorded", Recorded { id, chord });
    true
}

pub fn pressed(app: &tauri::AppHandle, action: ShortcutAction) {
    if record_owned(app, action) {
        return;
    }
    if action == ShortcutAction::Overlay {
        if let Some(window) = app.get_webview_window("overlay") {
            let _ = window.show();
            let _ = window.set_focus();
        }
        return;
    }
    let state = app.state::<Runtime>();
    if let Ok(mut local) = state.local.lock() {
        let control = match action {
            ShortcutAction::Mute => {
                if local.settings.explicit_mute {
                    LocalControl::Unmute
                } else {
                    LocalControl::Mute
                }
            }
            ShortcutAction::Deafen => {
                if local.settings.deafened {
                    LocalControl::Undeafen
                } else {
                    LocalControl::Deafen
                }
            }
            ShortcutAction::Overlay => return,
        };
        local.apply(control);
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
        if enqueue(&state, local.settings.clone()).is_err() {
            let _ = app.emit(
                "runtime-error",
                "Shortcut applied; saving the preference is unavailable.",
            );
        }
    }
}
#[tauri::command]
pub fn shortcut_status(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
) -> Result<Vec<ShortcutStatus>, String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    state
        .hotkeys
        .lock()
        .map_err(|_| "Shortcut owner unavailable")?
        .as_ref()
        .ok_or("Shortcut registration unavailable")?
        .snapshot()
        .map_err(|_| "Shortcut status unavailable".into())
}
#[tauri::command]
pub async fn set_shortcut(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    action: ShortcutAction,
    binding: Option<Chord>,
) -> Result<(), String> {
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use visible Settings to change a shortcut".into());
    }
    if let Some(chord) = binding {
        chord
            .validate()
            .map_err(|_| "Use Ctrl or Alt with A-Z, 0-9 or F1-F11")?;
    }
    // The owned coordinator finishes registration + settings publication even
    // if the IPC caller disappears after submitting this explicit change.
    tauri::async_runtime::spawn(async move {
        let state = app.state::<Runtime>();
        let _owner = state
            .shortcut_edit
            .try_lock()
            .map_err(|_| "Another shortcut change is pending")?;
        {
            let local = state.local.lock().map_err(|_| "Settings unavailable")?;
            if local.locked {
                return Err("Unlock Windows before changing shortcuts".to_string());
            }
            let mut desired = local.settings.shortcuts.clone();
            desired.set(action, binding);
            desired
                .validate()
                .map_err(|_| "Shortcut duplicates another Avesra control")?;
        }
        let reply = state
            .hotkeys
            .lock()
            .map_err(|_| "Shortcut owner unavailable")?
            .as_ref()
            .ok_or("Shortcut registration unavailable")?
            .set(action, binding)
            .map_err(|_| "Shortcut request unavailable")?;
        let result = tokio::task::spawn_blocking(move || reply.recv())
            .await
            .map_err(|_| "Shortcut owner stopped")?
            .map_err(|_| "Shortcut owner stopped")?;
        result.map_err(|error| {
            if error == avesra_contracts::ErrorCode::Denied {
                "Shortcut is already in use; the previous binding remains."
            } else {
                "Shortcut registration failed. Refresh its actual status."
            }
        })?;
        let pending = {
            let mut local = state.local.lock().map_err(|_| "Settings unavailable")?;
            local.settings.shortcuts.set(action, binding);
            local.refresh();
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
            enqueue(&state, local.settings.clone())?
        };
        pending
            .await
            .map_err(|_| "Shortcut changed but settings writer stopped")??;
        Ok(())
    })
    .await
    .map_err(|_| "Shortcut coordinator stopped")?
}
