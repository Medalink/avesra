//! Read-only current controlled-load inspection; no model or permission changes.
use crate::{Runtime, connection};
use serde::Serialize;
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Default)]
struct State {
    reader: Arc<tokio::sync::Mutex<()>>,
}
pub fn init(app: &tauri::AppHandle) {
    app.manage(State::default());
}
#[derive(Serialize)]
pub struct Observation {
    state: &'static str,
    artifact_revision: String,
    engine_incarnation: String,
    quality_fingerprint_available: bool,
}
#[tauri::command]
pub async fn reasoning_health(window: tauri::WebviewWindow) -> Result<Observation, String> {
    let started = Instant::now();
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to inspect reasoning".into());
    }
    let app = window.app_handle();
    let state = app.state::<Runtime>();
    let reader = app
        .state::<State>()
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Reasoning inspection is already active")?;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let (session, microphone, challenge) =
        {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if !local.connected || local.locked {
                return Err("Connect Spark before inspecting reasoning".into());
            }
            let session = state
                .acknowledged_session
                .lock()
                .map_err(|_| "Session unavailable")?
                .ok_or("Session unavailable")?;
            if session.epoch != local.capture_epoch || session.action_epoch != local.action_epoch {
                return Err("Session acknowledgement pending".into());
            }
            (
                session,
                local.settings.microphone.clone().ok_or(
                    "Select the input device before inspecting the voice reasoning adapter",
                )?,
                state.setup.challenge(),
            )
        };
    let current = || -> Result<(), String> {
        if !window.is_visible().unwrap_or(false) {
            return Err("Settings closed during reasoning inspection".into());
        }
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if started.elapsed() >= Duration::from_secs(30)
            || !local.connected
            || local.locked
            || local.capture_epoch != session.epoch
            || local.action_epoch != session.action_epoch
            || local.settings.microphone.as_ref() != Some(&microphone)
            || state.setup.challenge() != challenge
            || state.connection_generation.load(Ordering::SeqCst) != session.generation
            || !state.acknowledged_session.lock().is_ok_and(|value| {
                value.is_some_and(|value| {
                    value.id == session.id
                        && value.device == session.device
                        && value.epoch == session.epoch
                        && value.action_epoch == session.action_epoch
                        && value.generation == session.generation
                })
            })
        {
            return Err("Reasoning inspection context changed".into());
        }
        Ok(())
    };
    current()?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let (_reader, record, actor) = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        let record = connection::load(&directory)?;
        let actor = crate::owner::identity(&directory)
            .map_err(|_| "Owner unavailable")?
            .0;
        Ok::<_, String>((reader, record, actor))
    })
    .await
    .map_err(|_| "Reasoning reader stopped")??;
    current()?;
    let context = avesra_core::voice::Context {
        device: session.device,
        session: session.id,
        capture_epoch: session.epoch,
        action_epoch: session.action_epoch,
        microphone: microphone.clone(),
        actor: Some(actor),
        grant_revision: None,
    };
    let binding = tokio::time::timeout_at(
        (started + Duration::from_secs(30)).into(),
        connection::directedness::metadata(&record, session, &context, current),
    )
    .await
    .map_err(|_| "Reasoning inspection timed out")??;
    current()?;
    Ok(Observation {
        state: "loaded_unqualified",
        artifact_revision: binding.artifact_revision,
        engine_incarnation: binding.engine_incarnation,
        quality_fingerprint_available: binding.quality_fingerprint.is_some(),
    })
}
