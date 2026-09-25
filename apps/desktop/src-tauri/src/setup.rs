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
pub struct ManagementProof(Proof);
impl ManagementProof {
    pub fn current(&self, state: &Runtime) -> bool {
        let Ok(local) = state.local.lock() else {
            return false;
        };
        self.current_locked(state, &local)
    }
    pub(crate) fn current_locked(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
    ) -> bool {
        !local.locked
            && local.connected
            && self.0.verified_at.elapsed() < Duration::from_secs(60)
            && self.0.challenge == state.setup.generation.load(Ordering::SeqCst)
            && self.0.epoch == local.capture_epoch
            && self.0.generation == state.connection_generation.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
pub struct Setup {
    pub(crate) recording: tokio::sync::Mutex<()>,
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
    pub fn enrollment_active(&self) -> Result<bool, String> {
        self.enrollment
            .lock()
            .map(|value| value.is_some())
            .map_err(|_| "Enrollment unavailable".into())
    }
    pub fn challenge(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    pub fn management_proof(
        &self,
        local: &avesra_core::state::LocalState,
        generation: u64,
    ) -> Result<ManagementProof, String> {
        let mut proof = self.proof.lock().map_err(|_| "Setup unavailable")?;
        if local.locked
            || !local.connected
            || !proof.as_ref().is_some_and(|proof| {
                proof.challenge == self.generation.load(Ordering::SeqCst)
                    && proof.generation == generation
                    && proof.epoch == local.capture_epoch
                    && proof.verified_at.elapsed() < Duration::from_secs(60)
            })
        {
            return Err("Verify Windows Hello again for this management action".into());
        }
        Ok(ManagementProof(proof.take().ok_or("Setup unavailable")?))
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
                if local.enrollment_capture {
                    "recording"
                } else {
                    "prepared"
                },
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
        reason: "Each explicit recording first checks the paired speaker service, then records eight seconds. Collected candidates remain quality unqualified and cannot enable normal listening.",
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
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Finish the current voice recording first")?;
    let proof = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        state
            .setup
            .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?
    };
    let owner_actor = crate::owner::current_actor(&app).await?;
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !proof.current_locked(&state, &local)
            || local.microphone_check
            || !local.connected
            || local.locked
            || local.settings.paused
            || local.settings.deafened
            || local.settings.explicit_mute
        {
            return Err(
                "Connect Spark and restore local listening controls before enrollment".into(),
            );
        }
        let microphone = local
            .settings
            .microphone
            .clone()
            .ok_or("Select a microphone first")?;
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
                owner_actor,
            )
            .map_err(|e| e.to_string())?,
        );
    }
    setup_status(window, app.state::<Runtime>()).await
}

/// Outside Runtime::publish only: closes media and publishes the epoch used by
/// the Spark session watch. Setup::invalidate itself never acquires local state.
pub fn cancel_native(app: &tauri::AppHandle) {
    cancel_native_epoch(app, None);
}
pub(crate) fn cancel_native_epoch(app: &tauri::AppHandle, expected: Option<u64>) {
    let state = app.state::<Runtime>();
    if let Ok(mut local) = state.local.lock() {
        if expected.is_some_and(|epoch| local.capture_epoch != epoch) {
            return;
        }
        local.enrollment_capture = false;
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        if state.media.setup_output_owned(local.playback_epoch) {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
        }
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
}

#[tauri::command]
pub async fn record_enrollment(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<SetupStatus, String> {
    settings_only(&window)?;
    let state = app.state::<Runtime>();
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Enrollment recording is already active")?;
    let admission = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        let session = active
            .as_ref()
            .filter(|session| session.current(local.capture_epoch))
            .ok_or("Prepare a verified enrollment first")?;
        (
            session.id,
            local.capture_epoch,
            state.setup.generation.load(Ordering::SeqCst),
            state.connection_generation.load(Ordering::SeqCst),
            session.prepared_by,
        )
    };
    let check_admission = || -> Result<(), String> {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.capture_epoch != admission.1
            || !local.connected
            || state.setup.generation.load(Ordering::SeqCst) != admission.2
            || state.connection_generation.load(Ordering::SeqCst) != admission.3
            || !state
                .setup
                .enrollment
                .lock()
                .map_err(|_| "Enrollment unavailable")?
                .as_ref()
                .is_some_and(|session| session.id == admission.0 && session.current(admission.1))
        {
            return Err("Original enrollment recording request was cancelled".into());
        }
        Ok(())
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Pairing directory unavailable")?;
    let owner_actor = crate::owner::current_actor(&app).await?;
    check_admission()?;
    if owner_actor != admission.4 {
        return Err("Enrollment belongs to a different owner identity".into());
    }
    let pairing = tauri::async_runtime::spawn_blocking(move || crate::connection::load(&directory))
        .await
        .map_err(|_| "Pairing reader stopped")??;
    check_admission()?;
    // No microphone is opened unless the configured paired service is reachable.
    crate::connection::speaker_available(&pairing).await?;
    check_admission()?;
    let visible = window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?;
    let (epoch, segment) = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected
            || local.locked
            || local.settings.explicit_mute
            || local.settings.deafened
            || local.settings.paused
            || !visible
        {
            return Err("Restore local listening controls and keep Settings open to record".into());
        }
        let mut active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        if !active.as_ref().is_some_and(|session| {
            session.id == admission.0 && session.current(local.capture_epoch)
        }) || local.capture_epoch != admission.1
            || state.setup.generation.load(Ordering::SeqCst) != admission.2
            || state.connection_generation.load(Ordering::SeqCst) != admission.3
        {
            return Err("Prepare a fresh verified enrollment first".into());
        }
        let mut session = active.take().ok_or("Enrollment unavailable")?;
        drop(active);
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        session
            .rebind_idle_epoch(local.capture_epoch)
            .map_err(|e| e.to_string())?;
        let segment = session
            .begin_segment(local.capture_epoch)
            .map_err(|e| e.to_string())?;
        local.refresh();
        state.publish(&local);
        *state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")? = Some(session);
        let _ = app.emit("runtime-state", local.clone());
        (local.capture_epoch, segment)
    };
    struct RecordingGuard {
        app: tauri::AppHandle,
        epoch: u64,
        complete: bool,
    }
    impl Drop for RecordingGuard {
        fn drop(&mut self) {
            if !self.complete {
                cancel_native_epoch(&self.app, Some(self.epoch));
            }
        }
    }
    let mut guard = RecordingGuard {
        app: app.clone(),
        epoch,
        complete: false,
    };
    let acknowledgement = Instant::now();
    let session = loop {
        {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != epoch || !local.connected {
                return Err("Enrollment cancelled".into());
            }
            let ack = *state
                .acknowledged_session
                .lock()
                .map_err(|_| "Session unavailable")?;
            if let Some(ack) = ack
                && ack.epoch == epoch
                && ack.generation == state.connection_generation.load(Ordering::SeqCst)
            {
                break ack;
            }
        }
        if acknowledgement.elapsed() > Duration::from_secs(3) {
            return Err("Spark did not acknowledge enrollment epoch".into());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    let visible = window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?;
    {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.capture_epoch != epoch || !visible {
            return Err("Enrollment cancelled".into());
        }
        local.capture_error = None;
        local.enrollment_capture = true;
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
    let started = Instant::now();
    let phrase = crate::phrase_capture::collect(
        &app,
        epoch,
        || {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != epoch {
                return Err(local
                    .capture_error
                    .clone()
                    .unwrap_or_else(|| "Enrollment cancelled".into()));
            }
            let active = state
                .setup
                .enrollment
                .lock()
                .map_err(|_| "Enrollment unavailable")?;
            let session = active
                .as_ref()
                .ok_or("Enrollment cancelled. Start enrollment again.")?;
            if !session.current(epoch) {
                return Err(
                    "The five-minute enrollment session expired. Start enrollment again.".into(),
                );
            }
            Ok(())
        },
        |_| {},
    )
    .await?;
    let pcm = phrase.pcm;
    {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.capture_epoch != epoch {
            return Err("Enrollment cancelled".into());
        }
        local.enrollment_capture = false;
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
    let remaining = Duration::from_secs(28)
        .checked_sub(started.elapsed())
        .ok_or("Enrollment media expired")?;
    let inference = tokio::time::timeout(
        remaining,
        crate::connection::enrollment_embedding(&pairing, session, segment, pcm),
    );
    let freshness = async {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if !state.setup.enrollment.lock().is_ok_and(|active| {
                active
                    .as_ref()
                    .is_some_and(|session| session.current(epoch))
            }) {
                break;
            }
        }
    };
    let embedding = tokio::select! {
        biased;
        _=freshness=>return Err("Enrollment cancelled or expired".into()),
        value=inference=>value.map_err(|_|"Enrollment media expired")??,
    };
    let visible = window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?;
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.capture_epoch != epoch || !local.connected || !visible {
            return Err("Enrollment result is stale".into());
        }
        state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?
            .as_mut()
            .ok_or("Enrollment cancelled")?
            .add_embedding(
                epoch,
                segment,
                "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286",
                128_000,
                &embedding,
            )
            .map_err(|e| e.to_string())?;
    }
    guard.complete = true;
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
    let admission = {
        let active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        active
            .as_ref()
            .map(|v| (v.id, v.prepared_by))
            .ok_or("Enrollment unavailable")?
    };
    let owner_actor = crate::owner::current_actor(&app).await?;
    if owner_actor != admission.1 {
        return Err("Enrollment owner changed".into());
    }
    let (candidate, context) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut active = state
            .setup
            .enrollment
            .lock()
            .map_err(|_| "Enrollment unavailable")?;
        if !active.as_ref().is_some_and(|session| {
            session.id == admission.0
                && session.prepared_by == owner_actor
                && session.current(local.capture_epoch)
                && session.completed() == avesra_core::enrollment::SEGMENTS
        }) {
            return Err("Complete actual prompted and held-out collection first".into());
        }
        let candidate = active
            .take()
            .ok_or("Enrollment unavailable")?
            .candidate(local.capture_epoch)
            .map_err(|e| e.to_string())?;
        (
            candidate,
            (
                local.capture_epoch,
                state.setup.challenge(),
                state.connection_generation.load(Ordering::SeqCst),
                Instant::now(),
            ),
        )
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::save_candidate(&directory, &candidate, &mut || {
            if !crate::owner::matches_actor(&directory, owner_actor) {
                return Err("Enrollment owner changed".into());
            }
            let state = app.state::<Runtime>();
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.locked
                || !local.connected
                || local.capture_epoch != context.0
                || state.setup.challenge() != context.1
                || state.connection_generation.load(Ordering::SeqCst) != context.2
                || context.3.elapsed() >= Duration::from_secs(30)
            {
                return Err("Candidate publication expired; refresh stored status".into());
            }
            Ok(())
        })
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
    let (_, proof) = authorize_profile_selection(&window, &app)?;
    let actor = crate::owner::current_actor(&app).await?;
    if !proof.current(&app.state::<Runtime>()) {
        return Err("Profile management expired".into());
    }
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::remove_candidate(&directory, id, revision, &mut || {
            authorize_profile_write(&app, &directory, actor, &proof)
        })
    })
    .await
    .map_err(|_| "Profile writer stopped")?
}
fn authorize_profile_selection(
    window: &tauri::WebviewWindow,
    app: &tauri::AppHandle,
) -> Result<(Option<String>, ManagementProof), String> {
    settings_only(window)?;
    if !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Open Settings to manage profile selection".into());
    }
    let state = app.state::<Runtime>();
    let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut proof = state
        .setup
        .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?;
    let microphone = local.settings.microphone.clone();
    local.enrollment_capture = false;
    local.voice_ready = false;
    local.enrolled = false;
    local.capture_epoch = local.capture_epoch.saturating_add(1);
    local.action_epoch = local.action_epoch.saturating_add(1);
    local.refresh();
    state.publish(&local);
    // This command's own serialized invalidation withdraws prior voice state.
    // Rebind only that transition, preserving the consumed proof's expiry and
    // connection identity; later invalidations still revoke this operation.
    proof.0.challenge = state.setup.challenge();
    proof.0.epoch = local.capture_epoch;
    let snapshot = local.clone();
    drop(local);
    let _ = app.emit("runtime-state", snapshot);
    Ok((microphone, proof))
}
fn authorize_profile_write(
    app: &tauri::AppHandle,
    directory: &std::path::Path,
    actor: uuid::Uuid,
    proof: &ManagementProof,
) -> Result<(), String> {
    if !crate::owner::matches_actor(directory, actor) || !proof.current(&app.state::<Runtime>()) {
        return Err("Profile management expired or owner changed; refresh stored status".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn select_speaker_candidate(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    id: uuid::Uuid,
    revision: uuid::Uuid,
) -> Result<(), String> {
    let (microphone, proof) = authorize_profile_selection(&window, &app)?;
    let microphone = microphone.ok_or("Select the enrollment microphone first")?;
    let actor = crate::owner::current_actor(&app).await?;
    if !proof.current(&app.state::<Runtime>()) {
        return Err("Profile management expired".into());
    }
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::select_candidate(&directory, id, revision, &microphone, &mut || {
            authorize_profile_write(&app, &directory, actor, &proof)
        })
    })
    .await
    .map_err(|_| "Profile writer stopped")?
}
#[tauri::command]
pub async fn clear_speaker_selection(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let (_, proof) = authorize_profile_selection(&window, &app)?;
    let actor = crate::owner::current_actor(&app).await?;
    if !proof.current(&app.state::<Runtime>()) {
        return Err("Profile management expired".into());
    }
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::clear_selection(&directory, &mut || {
            authorize_profile_write(&app, &directory, actor, &proof)
        })
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
    cancel_native(&app);
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
    let visible = window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?;
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if state.setup.generation.load(Ordering::SeqCst) != challenge
            || state.connection_generation.load(Ordering::SeqCst) != generation
            || local.capture_epoch != epoch
            || !local.connected
            || local.locked
            || !visible
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
pub fn cancel_setup(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<(), String> {
    settings_only(&window)?;
    cancel_native(&app);
    Ok(())
}
