//! Explicit Audio & Voice context; no identity-management authentication gate.
use crate::{
    Runtime,
    connection::{self, SessionIdentity},
};
use avesra_contracts::voices::VoiceCommand;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{Emitter, Manager};
use uuid::Uuid;

fn visible(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use the visible Audio & Voice settings panel".into());
    }
    Ok(())
}
#[tauri::command]
pub fn open_voice_panel(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Uuid, String> {
    let state = app.state::<Runtime>();
    let epoch = state
        .local
        .lock()
        .map_err(|_| "Local state unavailable")?
        .capture_epoch;
    visible(&window)?;
    let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if local.capture_epoch != epoch || local.locked {
        return Err("Settings context changed".into());
    }
    let mut panel = state
        .voice_panel
        .lock()
        .map_err(|_| "Voice panel unavailable")?;
    let replaced = panel.is_some();
    let id = Uuid::new_v4();
    *panel = Some(id);
    drop(panel);
    if replaced {
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        if state.media.setup_output_owned(local.playback_epoch) {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
        }
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
    Ok(id)
}
#[tauri::command]
pub fn close_voice_panel(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    let state = app.state::<Runtime>();
    let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut current = state
        .voice_panel
        .lock()
        .map_err(|_| "Voice panel unavailable")?;
    if *current != Some(panel) {
        return Ok(());
    }
    *current = None;
    drop(current);
    local.enrollment_capture = false;
    local.capture_epoch = local.capture_epoch.saturating_add(1);
    if state.media.setup_output_owned(local.playback_epoch) {
        local.playback_epoch = local.playback_epoch.saturating_add(1);
    }
    local.refresh();
    state.publish(&local);
    let _ = app.emit("runtime-state", local.clone());
    Ok(())
}
fn current(state: &Runtime, panel: Uuid, session: SessionIdentity) -> Result<(), String> {
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if !local.connected
        || local.locked
        || local.capture_epoch != session.epoch
        || state.connection_generation.load(Ordering::SeqCst) != session.generation
        || !state.voice_panel.lock().is_ok_and(|v| *v == Some(panel))
        || !state.acknowledged_session.lock().is_ok_and(|v| {
            v.is_some_and(|v| {
                v.id == session.id && v.epoch == session.epoch && v.generation == session.generation
            })
        })
    {
        return Err("Voice setup context changed".into());
    }
    Ok(())
}
struct OperationLease {
    app: tauri::AppHandle,
    panel: Uuid,
    session: SessionIdentity,
    complete: bool,
}
impl Drop for OperationLease {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        let state = self.app.state::<Runtime>();
        if let Ok(mut local) = state.local.lock()
            && local.capture_epoch == self.session.epoch
            && state.connection_generation.load(Ordering::SeqCst) == self.session.generation
            && state
                .voice_panel
                .lock()
                .is_ok_and(|v| *v == Some(self.panel))
        {
            local.enrollment_capture = false;
            local.capture_epoch = local.capture_epoch.saturating_add(1);
            local.refresh();
            state.publish(&local);
            let _ = self.app.emit("runtime-state", local.clone());
        }
    }
}
#[tauri::command]
pub async fn voice_operation(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    command: VoiceCommand,
) -> Result<connection::VoiceResult, String> {
    let state = app.state::<Runtime>();
    let epoch = state
        .local
        .lock()
        .map_err(|_| "Local state unavailable")?
        .capture_epoch;
    visible(&window)?;
    let _slot = state
        .preview
        .try_lock()
        .map_err(|_| "Another voice operation is still active")?;
    let session = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.capture_epoch != epoch
            || !local.connected
            || local.locked
            || !state.voice_panel.lock().is_ok_and(|v| *v == Some(panel))
        {
            return Err("Voice setup is unavailable".into());
        }
        let generation = state.connection_generation.load(Ordering::SeqCst);
        let session = state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .filter(|v| v.epoch == epoch && v.generation == generation)
            .ok_or("Wait for the Spark session")?;
        if matches!(
            command,
            VoiceCommand::Select { .. } | VoiceCommand::Clear { .. }
        ) {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
            local.refresh();
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
        }
        session
    };
    let mut lease = OperationLease {
        app: app.clone(),
        panel,
        session,
        complete: false,
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Pairing unavailable")?;
    let record = tokio::task::spawn_blocking(move || connection::load(&directory))
        .await
        .map_err(|_| "Pairing reader stopped")??;
    current(&state, panel, session)?;
    let mut modes = state.modes.subscribe();
    let monitor = async {
        loop {
            if modes.changed().await.is_err() || current(&state, panel, session).is_err() {
                return;
            }
        }
    };
    let result = tokio::select! { biased; _=monitor=>Err("Voice operation context changed; refresh status before retrying".into()), value=tokio::time::timeout(Duration::from_secs(36),connection::voice_operation(&record,session,&command))=>value.map_err(|_|"Voice operation timed out; refresh status before retrying")?, };
    current(&state, panel, session)?;
    lease.complete = result.is_ok();
    result
}
