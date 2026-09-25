//! Explicit read-only comparison. No qualified profile or action is produced.
use crate::{Runtime, connection::SessionIdentity};
use serde::Serialize;
use std::{
    sync::{Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

#[derive(Clone, Copy)]
struct Attempt {
    id: Uuid,
    epoch: u64,
    connection: u64,
    action: u64,
}
#[derive(Default)]
pub struct State(Mutex<Option<Attempt>>);
impl State {
    fn current(&self, app: &tauri::AppHandle, id: Uuid) -> Result<Attempt, String> {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if state.media.setup_output_owned(local.playback_epoch) {
            return Err("Stop the voice preview before checking your saved voice".into());
        }
        let attempt = self
            .0
            .lock()
            .map_err(|_| "Voice check unavailable")?
            .filter(|a| a.id == id)
            .ok_or("Voice check cancelled")?;
        if local.capture_epoch != attempt.epoch
            || local.action_epoch != attempt.action
            || state.connection_generation.load(Ordering::SeqCst) != attempt.connection
            || !local.connected
            || local.locked
            || local.settings.paused
            || local.settings.deafened
            || local.settings.explicit_mute
        {
            return Err(local.capture_error.clone().unwrap_or_else(|| {
                "Voice check cancelled because the connection or listening controls changed".into()
            }));
        }
        Ok(attempt)
    }
}
pub fn stop(app: &tauri::AppHandle, id: Uuid) {
    let state = app.state::<Runtime>();
    let attempt = state.voice_check.0.lock().ok().and_then(|mut slot| {
        if slot.is_some_and(|a| a.id == id) {
            slot.take()
        } else {
            None
        }
    });
    if let Some(attempt) = attempt
        && let Ok(mut local) = state.local.lock()
        && local.capture_epoch == attempt.epoch
    {
        // This read-only check owns capture only, never a concurrently started preview.
        local.enrollment_capture = false;
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
}
struct Guard {
    app: tauri::AppHandle,
    id: Uuid,
}
impl Drop for Guard {
    fn drop(&mut self) {
        stop(&self.app, self.id);
    }
}
#[derive(Clone, Serialize)]
struct Progress {
    request: Uuid,
    phase: &'static str,
    seconds: u32,
}
fn progress(app: &tauri::AppHandle, request: Uuid, phase: &'static str, seconds: u32) {
    let _ = app.emit_to(
        "settings",
        "voice-check-progress",
        Progress {
            request,
            phase,
            seconds,
        },
    );
}
#[derive(Serialize)]
pub struct ResultSummary {
    request: Uuid,
    candidate: Uuid,
    revision: Uuid,
    transcript: String,
    similarity: Option<f64>,
    held_out_similarities: Vec<f32>,
    seconds: u32,
    clipped_samples: u32,
    total_samples: u32,
}
#[tauri::command]
pub fn cancel_voice_check(window: tauri::WebviewWindow, request: Uuid) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings to cancel the voice check".into());
    }
    stop(window.app_handle(), request);
    Ok(())
}
#[tauri::command]
pub async fn check_saved_voice(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    request: Uuid,
    id: Uuid,
    revision: Uuid,
) -> Result<ResultSummary, String> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) || request.is_nil() {
        return Err("Open Settings to check your saved voice".into());
    }
    let state = app.state::<Runtime>();
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "A voice recording is already active")?;
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if state.setup.enrollment_active()? || local.microphone_check || local.capture_allowed() {
            return Err("Finish or cancel the current enrollment or microphone check first".into());
        }
        let mut slot = state
            .voice_check
            .0
            .lock()
            .map_err(|_| "Voice check unavailable")?;
        if slot.is_some() {
            return Err("A voice check is already active".into());
        }
        *slot = Some(Attempt {
            id: request,
            epoch: local.capture_epoch,
            connection: state.connection_generation.load(Ordering::SeqCst),
            action: local.action_epoch,
        });
    }
    let _guard = Guard {
        app: app.clone(),
        id: request,
    };
    state.voice_check.current(&app, request)?;
    progress(&app, request, "checking", 0);
    let work = async {
        let actor = crate::owner::current_actor(&app).await?;
        state.voice_check.current(&app, request)?;
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "Profile directory unavailable")?;
        let directory_read = directory.clone();
        let (candidate, pairing) = tauri::async_runtime::spawn_blocking(move || {
            Ok::<_, String>((
                crate::profiles::read_candidate(&directory_read, id, revision)?,
                crate::connection::load(&directory_read)?,
            ))
        })
        .await
        .map_err(|_| "Profile reader stopped")??;
        let before = state.voice_check.current(&app, request)?;
        {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.settings.microphone.as_ref() != Some(&candidate.microphone)
                || candidate.model_revision != "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286"
            {
                return Err(
                    "Choose the microphone used for these saved recordings before checking them"
                        .into(),
                );
            }
        }
        crate::connection::voice_analysis_available(&pairing).await?;
        state.voice_check.current(&app, request)?;
        if !window.is_visible().unwrap_or(false) {
            return Err("Settings closed".into());
        }
        let epoch = {
            let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
            let mut slot = state
                .voice_check
                .0
                .lock()
                .map_err(|_| "Voice check unavailable")?;
            let attempt = slot
                .as_mut()
                .filter(|a| a.id == request && a.epoch == before.epoch)
                .ok_or("Voice check cancelled")?;
            if local.capture_epoch != before.epoch
                || !local.connected
                || local.locked
                || local.settings.paused
                || local.settings.deafened
                || local.settings.explicit_mute
            {
                return Err("Voice check cancelled".into());
            }
            local.capture_epoch = local.capture_epoch.saturating_add(1);
            attempt.epoch = local.capture_epoch;
            drop(slot);
            local.capture_error = None;
            local.refresh();
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
            local.capture_epoch
        };
        let ack_started = Instant::now();
        let session: SessionIdentity = loop {
            let attempt = state.voice_check.current(&app, request)?;
            let acknowledged = *state
                .acknowledged_session
                .lock()
                .map_err(|_| "Session unavailable")?;
            if let Some(session) = acknowledged
                && session.epoch == epoch
                && session.generation == attempt.connection
                && session.action_epoch == attempt.action
            {
                break session;
            }
            if ack_started.elapsed() >= Duration::from_secs(3) {
                return Err("Spark did not acknowledge this voice check".into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        if !window.is_visible().unwrap_or(false) {
            return Err("Settings closed".into());
        }
        {
            let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != epoch {
                return Err("Voice check cancelled".into());
            }
            local.enrollment_capture = true;
            local.refresh();
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
        }
        progress(&app, request, "recording", 0);
        let phrase = crate::phrase_capture::collect(
            &app,
            epoch,
            || state.voice_check.current(&app, request).map(|_| ()),
            |seconds| progress(&app, request, "recording", seconds),
        )
        .await?;
        {
            let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != epoch {
                return Err("Voice check cancelled".into());
            }
            local.enrollment_capture = false;
            local.refresh();
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
        }
        progress(&app, request, "processing", 8);
        let analysis =
            crate::connection::analyze_voice(&pairing, session, request, phrase.pcm).await?;
        state.voice_check.current(&app, request)?;
        let current_actor = crate::owner::current_actor(&app).await?;
        state.voice_check.current(&app, request)?;
        if actor != current_actor || !window.is_visible().unwrap_or(false) {
            return Err("Voice check owner or Settings changed".into());
        }
        let similarity = analysis
            .embedding
            .as_ref()
            .map(|vector| {
                let norm = vector
                    .iter()
                    .map(|v| f64::from(*v).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if !norm.is_finite() || norm < 1e-12 {
                    return Err("Speaker returned an invalid comparison".to_string());
                }
                Ok(vector
                    .iter()
                    .zip(&candidate.representation)
                    .map(|(a, b)| f64::from(*a) * f64::from(*b) / norm)
                    .sum::<f64>()
                    .clamp(-1.0, 1.0))
            })
            .transpose()?;
        Ok(ResultSummary {
            request,
            candidate: id,
            revision,
            transcript: analysis.transcript,
            similarity,
            held_out_similarities: candidate.held_out_similarities,
            seconds: 8,
            clipped_samples: phrase.clipped_samples,
            total_samples: 128_000,
        })
    };
    let cancelled = async {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if let Err(error) = state.voice_check.current(&app, request) {
                return error;
            }
        }
    };
    tokio::select! {
        biased;
        error=cancelled=>Err(error),
        value=tokio::time::timeout(Duration::from_secs(50),work)=>value.map_err(|_| "Voice check timed out; no recording or result was saved")?,
    }
}
