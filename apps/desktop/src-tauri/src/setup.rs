//! Local management proof stays native and never grants voice/action authority.
use crate::Runtime;
use serde::Serialize;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

struct Proof {
    challenge: u64,
    generation: u64,
    epoch: u64,
    verified_at: Instant,
}
#[derive(Default)]
pub struct Setup {
    enrollment: Mutex<Option<avesra_core::enrollment::Enrollment>>,
    context: Mutex<Option<(u64, u64)>>,
    operation: Mutex<Option<avesra_windows::authentication::Verification>>,
    generation: AtomicU64,
    proof: Mutex<Option<Proof>>,
    pending: tokio::sync::Mutex<()>,
}
#[derive(Serialize)]
pub struct SetupStatus {
    local_authentication: &'static str,
    authentication_seconds_remaining: u64,
    enrollment: &'static str,
    reason: &'static str,
    completed_segments: usize,
    next_segment: Option<avesra_core::enrollment::SegmentKind>,
}
impl Setup {
    fn consume_proof(
        &self,
        local: &avesra_core::state::LocalState,
        generation: u64,
    ) -> Result<(), String> {
        let mut proof = self.proof.lock().map_err(|_| "Setup unavailable")?;
        if !proof.as_ref().is_some_and(|proof| {
            proof.challenge == self.generation.load(Ordering::SeqCst)
                && proof.generation == generation
                && proof.epoch == local.capture_epoch
                && proof.verified_at.elapsed() < Duration::from_secs(60)
        }) {
            return Err("Verify Windows Hello again for this management action".into());
        }
        *proof = None;
        Ok(())
    }
    pub fn observe(&self, epoch: u64, connection: u64) {
        if let Ok(mut context) = self.context.lock()
            && *context != Some((epoch, connection))
        {
            *context = Some((epoch, connection));
            self.invalidate();
        }
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut proof) = self.proof.lock() {
            *proof = None;
        }
        if let Ok(mut enrollment) = self.enrollment.lock() {
            *enrollment = None;
        }
        if let Ok(operation) = self.operation.lock()
            && let Some(operation) = operation.as_ref()
        {
            operation.cancel();
        }
    }
}
fn settings_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use the local Settings window".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn setup_status(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
) -> Result<SetupStatus, String> {
    settings_only(&window)?;
    let available = avesra_windows::authentication::available()
        .await
        .map_err(|e| e.to_string())?;
    // All native reads/mutations below share a fresh context after the await.
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let epoch = local.capture_epoch;
    let generation = state.connection_generation.load(Ordering::SeqCst);
    let remaining = {
        let mut stored = state.setup.proof.lock().map_err(|_| "Setup unavailable")?;
        if stored.as_ref().is_some_and(|proof| {
            proof.epoch != epoch
                || proof.challenge != state.setup.generation.load(Ordering::SeqCst)
                || proof.generation != generation
                || proof.verified_at.elapsed() >= Duration::from_secs(60)
        }) {
            *stored = None;
        }
        stored.as_ref().map_or(0, |proof| {
            60u64.saturating_sub(proof.verified_at.elapsed().as_secs())
        })
    };
    let authentication = if remaining > 0 {
        "verified"
    } else if available {
        "available"
    } else {
        "unavailable"
    };
    let (enrollment, completed_segments, next_segment) = {
        let mut active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        if active
            .as_ref()
            .is_some_and(|session| !session.current(epoch))
        {
            *active = None;
        }
        active.as_ref().map_or(("unavailable", 0, None), |session| {
            (
                "waiting_for_service",
                session.completed(),
                session.next_kind(),
            )
        })
    };
    Ok(SetupStatus {
        local_authentication: authentication,
        authentication_seconds_remaining: remaining,
        enrollment,
        completed_segments,
        next_segment,
        reason: "Speaker enrollment capture and held-out quality validation are not ready. Verification does not enable listening.",
    })
}

#[tauri::command]
pub async fn begin_enrollment(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<SetupStatus, String> {
    settings_only(&window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Open Settings to enroll".into());
    }
    let state = app.state::<Runtime>();
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected || local.locked || local.settings.paused || local.settings.deafened {
            return Err(
                "Connect Spark and restore local listening controls before enrollment".into(),
            );
        }
        let microphone = local
            .settings
            .microphone
            .clone()
            .ok_or("Select a microphone first")?;
        let mut proof = state.setup.proof.lock().map_err(|_| "Setup unavailable")?;
        let valid = proof.as_ref().is_some_and(|proof| {
            proof.challenge == state.setup.generation.load(Ordering::SeqCst)
                && proof.generation == state.connection_generation.load(Ordering::SeqCst)
                && proof.epoch == local.capture_epoch
                && proof.verified_at.elapsed() < Duration::from_secs(60)
        });
        if !valid {
            return Err("Verify Windows Hello again before enrollment".into());
        }
        let mut active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        if active.is_some() {
            return Err("Cancel the existing enrollment first".into());
        }
        // This is the configured artifact identity, never a claim of readiness.
        *active = Some(
            avesra_core::enrollment::Enrollment::new(
                local.capture_epoch,
                "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286".into(),
                microphone,
            )
            .map_err(|e| e.to_string())?,
        );
        *proof = None;
    }
    setup_status(window, app.state::<Runtime>()).await
}

#[tauri::command]
pub async fn finish_enrollment(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    settings_only(&window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Open Settings to finish".into());
    }
    let state = app.state::<Runtime>();
    let candidate = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        if !active.as_ref().is_some_and(|session| {
            session.current(local.capture_epoch)
                && session.completed() == avesra_core::enrollment::SEGMENTS
        }) {
            return Err("Complete actual prompted and held-out collection first".into());
        }
        active
            .take()
            .ok_or("Enrollment unavailable")?
            .candidate(local.capture_epoch)
            .map_err(|e| e.to_string())?
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::save_candidate(&directory, &candidate)
    })
    .await
    .map_err(|_| "Profile writer stopped")??;
    // Candidate storage deliberately cannot set enrolled or voice_ready.
    Ok(())
}

#[tauri::command]
pub async fn speaker_candidates(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<crate::profiles::CandidateSummary>, String> {
    settings_only(&window)?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || crate::profiles::list_candidates(&directory))
        .await
        .map_err(|_| "Profile reader stopped")?
}
#[tauri::command]
pub async fn delete_speaker_candidate(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    id: uuid::Uuid,
    revision: uuid::Uuid,
) -> Result<(), String> {
    settings_only(&window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Open Settings to remove a candidate".into());
    }
    let state = app.state::<Runtime>();
    {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        state
            .setup
            .consume_proof(&local, state.connection_generation.load(Ordering::SeqCst))?;
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        local.action_epoch = local.action_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::remove_candidate(&directory, id, revision)
    })
    .await
    .map_err(|_| "Profile writer stopped")?
}
#[tauri::command]
pub async fn verify_setup(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<SetupStatus, String> {
    settings_only(&window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Open Settings to verify".into());
    }
    let state = app.state::<Runtime>();
    let _pending = state
        .setup
        .pending
        .try_lock()
        .map_err(|_| "Local verification is already pending")?;
    state.setup.invalidate();
    if state
        .setup
        .operation
        .lock()
        .map_err(|_| "Setup unavailable")?
        .as_ref()
        .is_some_and(|operation| operation.running())
    {
        return Err("Previous Windows verification is still closing".into());
    }
    let challenge = state.setup.generation.load(Ordering::SeqCst);
    let (epoch, generation) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected || local.locked {
            return Err("Connect Spark before voice setup".into());
        }
        (
            local.capture_epoch,
            state.connection_generation.load(Ordering::SeqCst),
        )
    };
    let operation = avesra_windows::authentication::request_for_settings(
        window.hwnd().map_err(|_| "Settings window unavailable")?.0 as isize,
    )
    .map_err(|e| e.to_string())?;
    struct CancelOnDrop(avesra_windows::authentication::Verification);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            self.0.cancel();
        }
    }
    let cancel = CancelOnDrop(operation.clone());
    *state
        .setup
        .operation
        .lock()
        .map_err(|_| "Setup unavailable")? = Some(operation.clone());
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if state.setup.generation.load(Ordering::SeqCst) != challenge
            || state.connection_generation.load(Ordering::SeqCst) != generation
            || local.capture_epoch != epoch
            || !local.connected
            || local.locked
            || !window
                .is_visible()
                .map_err(|_| "Settings window unavailable")?
        {
            operation.cancel();
            return Err("Setup changed while Windows verification opened".into());
        }
    }
    tokio::time::timeout(Duration::from_secs(60), operation.verified())
        .await
        .map_err(|_| "Local verification timed out")?
        .map_err(|_| "Windows did not verify this user")?;
    drop(cancel);
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Settings closed during verification".into());
    }
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected
            || local.locked
            || local.capture_epoch != epoch
            || state.connection_generation.load(Ordering::SeqCst) != generation
            || state.setup.generation.load(Ordering::SeqCst) != challenge
        {
            return Err("Setup changed during verification; verify again when ready".into());
        }
        *state.setup.proof.lock().map_err(|_| "Setup unavailable")? = Some(Proof {
            challenge,
            generation,
            epoch,
            verified_at: Instant::now(),
        });
    }
    setup_status(window, app.state::<Runtime>()).await
}
#[tauri::command]
pub fn cancel_setup(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
) -> Result<(), String> {
    settings_only(&window)?;
    state.setup.invalidate();
    Ok(())
}
