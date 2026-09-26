//! Native measured qualification, protected review, and revocable live admission.
#[path = "activity_calibration.rs"]
pub mod activity;
#[path = "personal_voice.rs"]
mod personal;
use crate::Runtime;
use avesra_contracts::actors;
use avesra_core::voice::qualification::COLLECTION_LIFETIME;
pub use personal::{learn_personal, start_personal};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    OwnerDirected,
    OwnerDirectedWithoutName,
    SpeakerSwitch,
    AmbiguousApproval,
    OtherSpeaker,
    OwnerConversation,
    Recorded,
    AssistantPlayback,
    Overlap,
    Silence,
}
const CONDITIONS: [Condition; 10] = [
    Condition::OwnerDirected,
    Condition::OwnerDirectedWithoutName,
    Condition::SpeakerSwitch,
    Condition::AmbiguousApproval,
    Condition::OtherSpeaker,
    Condition::OwnerConversation,
    Condition::Recorded,
    Condition::AssistantPlayback,
    Condition::Overlap,
    Condition::Silence,
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    pub session: Uuid,
    pub condition: Condition,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub candidate: Uuid,
    pub revision: Uuid,
    pub actor: Uuid,
    pub owner_revision: Uuid,
    pub registration: actors::Binding,
    pub microphone: String,
    pub device: Uuid,
    pub session: Uuid,
    pub generation: u64,
    pub action_epoch: u64,
    pub asr_revision: &'static str,
    pub speaker_revision: &'static str,
    pub activity_revision: Option<&'static str>,
    pub activity_streaming: bool,
}
impl Binding {
    fn current(&self, state: &Runtime, local: &avesra_core::state::LocalState) -> bool {
        use std::sync::atomic::Ordering;
        local.connected
            && !local.locked
            && !local.settings.paused
            && !local.settings.deafened
            && !local.settings.explicit_mute
            && local.action_epoch == self.action_epoch
            && local.settings.microphone.as_ref() == Some(&self.microphone)
            && state.connection_generation.load(Ordering::SeqCst) == self.generation
            && state.acknowledged_session.lock().is_ok_and(|ack| {
                ack.is_some_and(|ack| {
                    ack.device == self.device
                        && ack.id == self.session
                        && ack.generation == self.generation
                        && ack.action_epoch == self.action_epoch
                })
            })
    }
}

/// Read native ownership and the exact reconciled registration, never UI state.
pub async fn binding(
    app: &tauri::AppHandle,
    candidate: &avesra_core::enrollment::Candidate,
    pairing: &crate::connection::PairingRecord,
    session: crate::connection::SessionIdentity,
    activity: bool,
    activity_streaming: bool,
) -> Result<Binding, String> {
    let state = app.state::<Runtime>();
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let server = pairing.server_fingerprint()?;
    let expected_server: String = session
        .server_fingerprint
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    if pairing.device_id != session.device || server != expected_server {
        return Err("Calibration pairing differs from the current session".into());
    }
    let device = session.device;
    let (actor, owner_revision, registered_by, _owner) = tokio::task::spawn_blocking(move || {
        // The actual reader owns admission if the invoking calibration future
        // is withdrawn while blocking storage/principal work is still running.
        let (actor, owner_revision) =
            crate::owner::identity(&directory).map_err(|_| "Owner identity unavailable")?;
        let identity = avesra_core::actor_intents::Identity {
            server,
            device,
            actor,
            owner_revision,
        };
        let intent =
            avesra_core::actor_intents::load(&directory.join("actor-intents.db"), &identity)
                .map_err(|_| "Registration intent unavailable")?
                .ok_or("Register this owner on Spark first")?;
        Ok::<_, String>((actor, owner_revision, intent.request, owner))
    })
    .await
    .map_err(|_| "Calibration owner reader stopped")??;
    let request = actors::Request {
        version: actors::VERSION,
        request: Uuid::new_v4(),
        attempt: Uuid::new_v4(),
        session: session.id,
        action_epoch: session.action_epoch,
        command: actors::Command::Status,
    };
    let reply = crate::connection::actor_operation(pairing, &request).await?;
    let registration = reply
        .binding
        .filter(|v| {
            !v.revoked
                && v.actor == actor
                && v.owner_revision == owner_revision
                && v.registered_by == registered_by
        })
        .ok_or("Refresh and reconcile this owner's Spark registration before calibration")?;
    let binding = Binding {
        candidate: candidate.id,
        revision: candidate.revision,
        actor,
        owner_revision,
        registration,
        microphone: candidate.microphone.clone(),
        device,
        session: session.id,
        generation: session.generation,
        action_epoch: session.action_epoch,
        asr_revision: "ebe59e5a817142986528bbbee5dba8db7b38ed50",
        speaker_revision: "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286",
        activity_revision: activity.then_some(avesra_contracts::activity::REVISION),
        activity_streaming,
    };
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if !binding.current(&state, &local) {
        return Err("Calibration context changed".into());
    }
    Ok(binding)
}

/// Pending observations become missing measurements even if their caller drops.
pub struct Attempt {
    app: tauri::AppHandle,
    session: Uuid,
    request: Uuid,
}
impl Attempt {
    pub fn new(app: tauri::AppHandle, session: Uuid, request: Uuid) -> Self {
        Self {
            app,
            session,
            request,
        }
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        self.app
            .state::<Runtime>()
            .qualification
            .fail(self.session, self.request);
    }
}
struct Measurement {
    request: Uuid,
    condition: Condition,
    held_out: bool,
    completed: bool,
    similarity: Option<f64>,
    clipped_samples: u32,
    voiced_samples: Option<u32>,
    development_check: (bool, bool),
}
struct Session {
    id: Uuid,
    binding: Binding,
    created: Instant,
    saved_held_out: Vec<f32>,
    threshold: Option<f64>,
    observations: Vec<Measurement>,
    activity: activity::Calibration,
    gate: Option<avesra_core::voice::qualification::Qualification>,
    consent: Option<Uuid>,
    directed: Option<crate::connection::directedness::Binding>,
    profile: Option<avesra_core::voice::QualifiedProfile>,
    capture: Option<GateCapture>,
}
struct GateCapture {
    request: Uuid,
    owner: avesra_core::voice::utterance::Owner,
    sequence: u64,
    endpoint: Option<avesra_core::voice::utterance::Completed>,
    ambiguous: bool,
}
#[derive(Default)]
struct Sessions {
    active: Option<Session>,
    retired: HashSet<Uuid>,
    full: bool,
    revoked: bool,
    suspended: bool,
}
impl Sessions {
    fn retire(&mut self, id: Uuid) {
        if self.retired.len() < 128 {
            self.retired.insert(id);
        } else {
            self.full = true;
        }
    }
    fn clear(&mut self) {
        if let Some(session) = self.active.take() {
            self.retire(session.id);
        }
    }
}
#[derive(Default)]
pub struct State(Mutex<Sessions>, std::sync::Arc<tokio::sync::Mutex<()>>);
#[derive(Serialize)]
pub struct Counts {
    condition: Condition,
    attempts: usize,
    measured: usize,
    failed_or_missing: usize,
    speaker_matches: usize,
    minimum_similarity: Option<f64>,
    maximum_similarity: Option<f64>,
    clipped_samples: u64,
}
#[derive(Serialize)]
pub struct Summary {
    session: Uuid,
    phase: &'static str,
    threshold: Option<f64>,
    calibration_attempts: usize,
    held_out: Vec<Counts>,
    calibration: Vec<Counts>,
    seconds_remaining: u64,
    activity: activity::Summary,
    gate: Option<avesra_core::voice::qualification::Summary>,
    qualified: bool,
    admission: Option<avesra_core::voice::AdmissionKind>,
}
impl Session {
    fn context(&self, epoch: u64) -> avesra_core::voice::Context {
        avesra_core::voice::Context {
            device: self.binding.device,
            session: self.binding.session,
            capture_epoch: epoch,
            action_epoch: self.binding.action_epoch,
            microphone: self.binding.microphone.clone(),
            grant_revision: self.consent,
            actor: Some(self.binding.actor),
        }
    }
    fn summary(&self) -> Summary {
        let counts = |held_out| {
            CONDITIONS
                .into_iter()
                .map(|condition| {
                    let values: Vec<_> = self
                        .observations
                        .iter()
                        .filter(|v| v.condition == condition && v.held_out == held_out)
                        .collect();
                    let scores: Vec<_> = values.iter().filter_map(|v| v.similarity).collect();
                    Counts {
                        condition,
                        attempts: values.len(),
                        measured: scores.len(),
                        failed_or_missing: values.len() - scores.len(),
                        speaker_matches: self.threshold.map_or(0, |threshold| {
                            scores.iter().filter(|score| **score >= threshold).count()
                        }),
                        minimum_similarity: scores.iter().copied().reduce(f64::min),
                        maximum_similarity: scores.iter().copied().reduce(f64::max),
                        clipped_samples: values.iter().map(|v| u64::from(v.clipped_samples)).sum(),
                    }
                })
                .collect()
        };
        Summary {
            session: self.id,
            phase: if self.threshold.is_some() {
                "held_out"
            } else {
                "calibration"
            },
            threshold: self.threshold,
            calibration_attempts: self.observations.iter().filter(|v| !v.held_out).count(),
            held_out: counts(true),
            calibration: counts(false),
            seconds_remaining: COLLECTION_LIFETIME
                .as_secs()
                .saturating_sub(self.created.elapsed().as_secs()),
            activity: self.activity.summary(),
            gate: self.gate.as_ref().map(|v| v.summary()),
            qualified: self.profile.as_ref().is_some_and(|v| {
                v.valid() && v.kind() == avesra_core::voice::AdmissionKind::ReleaseQualified
            }),
            admission: self
                .profile
                .as_ref()
                .filter(|v| v.valid())
                .map(|v| v.kind()),
        }
    }
}
fn gate_condition(value: Condition) -> Option<avesra_core::voice::qualification::Condition> {
    use avesra_core::voice::qualification::Condition as G;
    Some(match value {
        Condition::OwnerDirected => G::OwnerDirected,
        Condition::OwnerDirectedWithoutName => G::OwnerDirectedWithoutName,
        Condition::OtherSpeaker => G::OtherSpeaker,
        Condition::Recorded => G::Recorded,
        Condition::AssistantPlayback => G::AssistantPlayback,
        Condition::Overlap => G::Overlap,
        Condition::OwnerConversation => G::OwnerConversation,
        Condition::SpeakerSwitch => G::SpeakerSwitch,
        Condition::AmbiguousApproval => G::AmbiguousApproval,
        Condition::Silence => return None,
    })
}
pub const OUTPUT_REVISION: &str = "native-output-observation-v2";
pub const SIGNAL_REVISION: &str =
    "sortformer-frame-signal-v1-cd03eee90fbec18297ac31b8c21546e596b7f71c";
async fn restore_checked(
    app: &tauri::AppHandle,
    pairing: &crate::connection::PairingRecord,
    acknowledged: crate::connection::SessionIdentity,
    explicit: bool,
    authorize: impl Fn() -> Result<(), String>,
) -> Result<bool, String> {
    authorize()?;
    let state = app.state::<Runtime>();
    {
        let slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Qualification unavailable")?;
        if slot.revoked || slot.active.is_some() || (slot.suspended && !explicit) {
            return Ok(false);
        }
    }
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let loaded = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        crate::profiles::read_qualification(&directory)
    })
    .await
    .map_err(|_| "Qualification reader stopped")??;
    let Some((mut record, candidate)) = loaded else {
        return Ok(false);
    };
    authorize()?;
    let expected = binding(app, &candidate, pairing, acknowledged, true, true).await?;
    crate::connection::voice_analysis_available(pairing).await?;
    crate::connection::voice_activity_available(pairing, true).await?;
    if record.owner_revision != expected.owner_revision
        || record.registration != expected.registration
        || record.server != pairing.server_fingerprint()?
        || candidate.microphone != expected.microphone
        || candidate.model_revision != expected.speaker_revision
        || record.report.point().asr_revision != expected.asr_revision
        || record.report.point().signal_revision != SIGNAL_REVISION
        || record.report.point().overlap_revision != SIGNAL_REVISION
        || record.report.point().echo_revision != OUTPUT_REVISION
    {
        return Err(
            "Reviewed qualification no longer matches this native owner or adapters".into(),
        );
    }
    let consent = record.report.point().grant_revision;
    let context = avesra_core::voice::Context {
        device: expected.device,
        session: expected.session,
        capture_epoch: acknowledged.epoch,
        action_epoch: expected.action_epoch,
        microphone: expected.microphone.clone(),
        actor: Some(expected.actor),
        grant_revision: Some(consent),
    };
    let directed =
        crate::connection::directedness::metadata(pairing, acknowledged, &context, || {
            authorize()?;
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != acknowledged.epoch || !expected.current(&state, &local) {
                return Err("Qualification restore context changed".into());
            }
            Ok(())
        })
        .await?;
    if directed.artifact_revision != record.directed_artifact {
        return Err(
            "Reviewed directedness artifact changed; collect new qualification evidence".into(),
        );
    }
    if directed.adapter_revision != record.report.point().directed_revision
        || directed.engine_incarnation != record.directed_incarnation
    {
        let (Some(stored), Some(observed)) = (
            record.directed_quality.as_deref(),
            directed.quality_fingerprint.as_deref(),
        ) else {
            return Err(
                "Reviewed directedness incarnation changed without complete quality evidence"
                    .into(),
            );
        };
        record
            .report
            .rebind_directed(stored, observed, &directed.adapter_revision)
            .map_err(|_| "Reviewed directedness deployment or policy changed")?;
    }
    let profile = record
        .report
        .restore(candidate, context)
        .map_err(|_| "Protected qualification evidence did not pass revalidation")?;
    let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
    authorize()?;
    if local.capture_epoch != acknowledged.epoch || !expected.current(&state, &local) {
        return Err("Qualification restore withdrawn".into());
    }
    {
        let mut slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Qualification unavailable")?;
        if slot.revoked || slot.active.is_some() {
            return Ok(false);
        }
        slot.active = Some(Session {
            id: Uuid::new_v4(),
            binding: expected,
            created: Instant::now(),
            saved_held_out: Vec::new(),
            threshold: None,
            observations: Vec::new(),
            activity: activity::Calibration::default(),
            gate: None,
            consent: Some(consent),
            directed: Some(directed),
            profile: Some(profile),
            capture: None,
        });
        slot.suspended = false;
    }
    local.enrolled = true;
    local.voice_ready = true;
    local.capture_epoch = local.capture_epoch.saturating_add(1);
    local.refresh();
    state.publish(&local);
    use tauri::Emitter;
    let _ = app.emit("runtime-state", local.clone());
    Ok(true)
}

#[tauri::command]
pub async fn revalidate_voice_admission(window: tauri::WebviewWindow) -> Result<(), String> {
    let started = Instant::now();
    let app = window.app_handle();
    let state = app.state::<Runtime>();
    let challenge = state.setup.challenge();
    let withdrawn = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _caller = crate::output::Caller(withdrawn.clone());
    // The original challenge precedes even synchronous window inspection.
    // Atomic invalidation includes Settings hide; never call a window getter
    // under Runtime.local at final publication.
    let authorize = || {
        if withdrawn.load(std::sync::atomic::Ordering::SeqCst)
            || state.setup.challenge() != challenge
            || started.elapsed() >= Duration::from_secs(60)
        {
            Err("Voice permission revalidation was withdrawn or expired".into())
        } else {
            Ok(())
        }
    };
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to revalidate saved voice permission".into());
    }
    authorize()?;
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Finish the current recording first")?;
    let mutation = state
        .qualification
        .1
        .clone()
        .try_lock_owned()
        .map_err(|_| "Voice permission management is busy")?;
    let acknowledged = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.voice_ready {
            return Err("Automatic voice permission is already active".into());
        }
        if !local.connected
            || local.locked
            || local.settings.paused
            || local.settings.deafened
            || local.settings.explicit_mute
        {
            return Err(
                "Connect Spark and explicitly unmute/resume before revalidating voice permission"
                    .into(),
            );
        }
        let acknowledged = state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .ok_or("Spark disconnected")?;
        if local.capture_epoch != acknowledged.epoch
            || local.action_epoch != acknowledged.action_epoch
        {
            return Err("Wait for the current Spark session acknowledgement".into());
        }
        let mut slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Qualification unavailable")?;
        if slot.revoked {
            return Err("Voice permission was explicitly revoked; complete qualification before enabling it again".into());
        }
        if slot.active.as_ref().is_some_and(|v| v.profile.is_none()) {
            return Err("Finish or discard the current unreviewed qualification first".into());
        }
        authorize()?;
        slot.suspended = true;
        slot.clear();
        acknowledged
    };
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let (_mutation, pairing) = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        let pairing = crate::connection::load(&directory)?;
        Ok::<_, String>((mutation, pairing))
    })
    .await
    .map_err(|_| "Pairing reader stopped")??;
    authorize()?;
    if !restore_checked(app, &pairing, acknowledged, true, authorize).await? {
        return Err(
            "No reviewed voice permission was saved; complete voice qualification first".into(),
        );
    }
    Ok(())
}
pub struct Admission {
    pub kind: avesra_core::voice::AdmissionKind,
    pub session: Uuid,
    pub context: avesra_core::voice::Context,
    pub policy: avesra_core::voice::utterance::Policy,
    pub directed: Option<crate::connection::directedness::Binding>,
    pub registration: actors::Binding,
}
impl State {
    pub fn playback_ready(&self, session: Uuid, request: Uuid) -> bool {
        self.0.lock().is_ok_and(|slot| {
            slot.active.as_ref().is_some_and(|value| {
                value.id == session
                    && value.gate.is_some()
                    && value.observations.iter().any(|v| {
                        v.request == request
                            && !v.completed
                            && v.condition == Condition::AssistantPlayback
                    })
                    && value.capture.as_ref().is_some_and(|v| {
                        v.request == request
                            && v.endpoint.is_none()
                            && !v.ambiguous
                            && v.owner.ready_for_speech()
                    })
            })
        })
    }
    pub fn can_restore(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|v| !v.revoked && !v.suspended && v.active.is_none())
    }
    pub fn resume_personal(&self) {
        if let Ok(mut slot) = self.0.lock()
            && slot.active.is_none()
        {
            slot.revoked = false;
            slot.suspended = false;
        }
    }
    pub fn suspend_personal(&self, session: Uuid) {
        if let Ok(mut slot) = self.0.lock()
            && slot.active.as_ref().is_some_and(|v| {
                v.id == session
                    && v.profile
                        .as_ref()
                        .is_some_and(|p| p.kind() == avesra_core::voice::AdmissionKind::Personal)
            })
        {
            slot.clear();
            slot.suspended = true;
        }
    }
    pub fn capture_gate_pcm(
        &self,
        id: Uuid,
        request: Uuid,
        pcm: &[u8],
        captured: Instant,
    ) -> Result<(), String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id)
            .ok_or("Qualification changed")?;
        if let Some(capture) = session.capture.as_mut().filter(|v| v.request == request) {
            if pcm.len() != 6400 {
                return Err("Qualification capture chunk changed".into());
            }
            for (index, chunk) in pcm.chunks_exact(640).enumerate() {
                let samples: Vec<_> = chunk
                    .chunks_exact(2)
                    .map(|v| i16::from_le_bytes([v[0], v[1]]))
                    .collect();
                capture.sequence += 1;
                capture
                    .owner
                    .capture(
                        capture.sequence,
                        captured + Duration::from_millis(index as u64 * 20),
                        &samples,
                    )
                    .map_err(|_| "Qualification capture lost continuity")?;
            }
        }
        Ok(())
    }
    pub fn take_gate_endpoint(
        &self,
        id: Uuid,
        request: Uuid,
    ) -> Result<Option<avesra_core::voice::utterance::Completed>, String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id)
            .ok_or("Qualification changed")?;
        if session.gate.is_none() {
            return Ok(None);
        }
        let capture = session
            .capture
            .take()
            .filter(|v| v.request == request)
            .ok_or("Qualification capture owner lost")?;
        if capture.ambiguous || capture.owner.unfinished() {
            return Err("Whole-gate check contained multiple endpoints; record one request".into());
        }
        capture
            .endpoint
            .map(Some)
            .ok_or("No complete measured utterance; cutoff is not an endpoint".into())
    }
    pub fn admission(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
    ) -> Option<Admission> {
        let slot = self.0.lock().ok()?;
        let session = slot
            .active
            .as_ref()
            .filter(|v| v.binding.current(state, local))?;
        let profile = session.profile.as_ref().filter(|v| v.valid())?;
        if profile.kind() != avesra_core::voice::AdmissionKind::Personal
            && session.directed.is_none()
        {
            return None;
        }
        Some(Admission {
            kind: profile.kind(),
            session: session.id,
            context: session.context(local.capture_epoch),
            policy: profile.endpoint_policy(),
            directed: session.directed.clone(),
            registration: session.binding.registration.clone(),
        })
    }
    pub fn with_profile<T>(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        expected: &avesra_core::voice::Context,
        work: impl FnOnce(&avesra_core::voice::QualifiedProfile) -> T,
    ) -> Result<T, String> {
        let slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        let session = slot
            .active
            .as_ref()
            .filter(|v| {
                v.binding.current(state, local) && v.context(local.capture_epoch) == *expected
            })
            .ok_or("Qualification context changed")?;
        let profile = session
            .profile
            .as_ref()
            .filter(|v| v.valid())
            .ok_or("Voice qualification is unavailable")?;
        Ok(work(profile))
    }
    pub fn directed_current(
        &self,
        id: Uuid,
        directed: &crate::connection::directedness::Binding,
    ) -> Result<(), String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        if !slot
            .active
            .as_ref()
            .is_some_and(|v| v.id == id && v.directed.as_ref() == Some(directed))
        {
            slot.clear();
            return Err("Directedness adapter changed; qualification revoked".into());
        }
        Ok(())
    }
    fn promote(&self, id: Uuid, epoch: u64) -> Result<Summary, String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Qualification expired")?;
        if !session
            .gate
            .as_ref()
            .is_some_and(|v| v.summary().reviewable)
        {
            return Err("Required independent whole-gate evidence has not passed review".into());
        }
        let context = session.context(epoch);
        let gate = session
            .gate
            .take()
            .ok_or("Qualification is not pending review")?;
        session.profile = Some(
            gate.review(&context)
                .map_err(|_| "Native qualification review refused activation")?,
        );
        slot.revoked = false;
        slot.suspended = false;
        Ok(slot.active.as_ref().ok_or("Qualification lost")?.summary())
    }
    pub fn gate_context(
        &self,
        id: Uuid,
        epoch: u64,
    ) -> Option<(
        avesra_core::voice::Context,
        crate::connection::directedness::Binding,
    )> {
        let slot = self.0.lock().ok()?;
        let session = slot.active.as_ref().filter(|v| {
            v.id == id && v.gate.is_some() && v.created.elapsed() < COLLECTION_LIFETIME
        })?;
        Some((session.context(epoch), session.directed.clone()?))
    }
    pub fn gate_observe(
        &self,
        id: Uuid,
        current: &avesra_core::voice::Context,
        request: Uuid,
        observation: avesra_core::voice::Observation,
    ) -> Result<(), String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Qualification expired")?;
        session
            .gate
            .as_mut()
            .ok_or("Freeze the whole-gate operating point first")?
            .observe(current, request, observation)
            .map_err(|_| "Qualification observation changed".into())
    }
    fn freeze_gate(
        &self,
        id: Uuid,
        epoch: u64,
        candidate: avesra_core::enrollment::Candidate,
        directed: crate::connection::directedness::Binding,
    ) -> Result<Summary, String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Calibration expired")?;
        if session.gate.is_some()
            || session.observations.iter().any(|v| !v.completed)
            || candidate.id != session.binding.candidate
            || candidate.revision != session.binding.revision
        {
            return Err("Qualification is already frozen, busy or changed".into());
        }
        let threshold = session
            .threshold
            .ok_or("Freeze the measured speaker threshold first")? as f32;
        let policy = session
            .activity
            .policy()
            .ok_or("Freeze the measured activity policy first")?
            .segmenter()?;
        if !session.binding.activity_streaming {
            return Err("Whole-gate qualification requires live activity ownership; begin a live activity session".into());
        }
        let owner: Vec<_> = session
            .observations
            .iter()
            .filter(|v| {
                v.condition == Condition::OwnerDirected
                    && v.completed
                    && v.voiced_samples.is_some_and(|n| n > 0)
            })
            .collect();
        let minimum_voiced_samples = owner
            .iter()
            .filter_map(|v| v.voiced_samples)
            .min()
            .ok_or("Record owner speech under the frozen activity policy first")?;
        let maximum_clipped_fraction = owner
            .iter()
            .map(|v| v.clipped_samples as f32 / 128000.0)
            .reduce(f32::max)
            .ok_or("Owner signal observations are missing")?;
        let margin = candidate
            .held_out_similarities
            .iter()
            .copied()
            .reduce(f32::min)
            .ok_or("Saved held-out measurements missing")?
            - threshold;
        if margin < 0.0 {
            return Err("Saved held-out margin does not separate".into());
        }
        let consent = Uuid::new_v4();
        let mut context = session.context(epoch);
        context.grant_revision = Some(consent);
        let gate = avesra_core::voice::qualification::Qualification::new(
            candidate,
            context,
            avesra_core::voice::qualification::OperatingPoint {
                actor: session.binding.actor,
                grant_revision: consent,
                asr_revision: session.binding.asr_revision.into(),
                threshold,
                held_out_margin: if margin > 0.0 {
                    margin.next_down()
                } else {
                    0.0
                },
                signal_revision: SIGNAL_REVISION.into(),
                directed_revision: directed.adapter_revision.clone(),
                overlap_revision: SIGNAL_REVISION.into(),
                echo_revision: OUTPUT_REVISION.into(),
                endpoint_policy: policy,
                minimum_voiced_samples,
                maximum_clipped_fraction,
            },
        )
        .map_err(|_| "Measured whole-gate operating point is invalid")?;
        session.consent = Some(consent);
        session.directed = Some(directed);
        session.gate = Some(gate);
        Ok(session.summary())
    }
    pub fn begin(
        &self,
        recording: &Recording,
        request: Uuid,
        binding: Binding,
        saved_held_out: &[f32],
        epoch: u64,
    ) -> Result<(), String> {
        if recording.session.is_nil() || request.is_nil() {
            return Err("Invalid calibration recording".into());
        }
        let mut slot = self.0.lock().map_err(|_| "Calibration unavailable")?;
        if slot.full || slot.retired.contains(&recording.session) {
            return Err("Calibration session was discarded or session capacity was reached".into());
        }
        if slot.active.is_none() {
            slot.active = Some(Session {
                id: recording.session,
                binding: binding.clone(),
                created: Instant::now(),
                saved_held_out: saved_held_out.to_vec(),
                threshold: None,
                observations: Vec::new(),
                activity: activity::Calibration::default(),
                gate: None,
                consent: None,
                directed: None,
                profile: None,
                capture: None,
            });
        }
        let session = slot.active.as_mut().ok_or("Calibration unavailable")?;
        if session.id != recording.session
            || session.binding != binding
            || session.created.elapsed() >= COLLECTION_LIFETIME
        {
            return Err(
                "Calibration context changed or expired. Discard it before starting again.".into(),
            );
        }
        if session.profile.is_some()
            || session.observations.len() >= 1024
            || session
                .observations
                .iter()
                .any(|v| !v.completed || v.request == request)
        {
            return Err("Calibration is busy, full or the recording was already used".into());
        }
        if session.threshold.is_none()
            && session.binding.activity_revision.is_none()
            && !matches!(
                recording.condition,
                Condition::OwnerDirected | Condition::OtherSpeaker
            )
        {
            return Err(
                "Calibrate with owner and other live speaker samples before freezing".into(),
            );
        }
        let context = session.context(epoch);
        if let Some(gate) = &mut session.gate {
            let condition =
                gate_condition(recording.condition).ok_or("Choose a whole-gate condition")?;
            gate.begin(&context, request, condition)
                .map_err(|_| "Gate measurement is unavailable")?;
            session.capture = Some(GateCapture {
                request,
                owner: avesra_core::voice::utterance::Owner::new(
                    context,
                    session
                        .activity
                        .policy()
                        .ok_or("Measured policy missing")?
                        .segmenter()?,
                )
                .map_err(|_| "Invalid qualification utterance owner")?,
                sequence: 0,
                endpoint: None,
                ambiguous: false,
            });
        }
        session.observations.push(Measurement {
            request,
            condition: recording.condition,
            held_out: session.threshold.is_some(),
            completed: false,
            similarity: None,
            clipped_samples: 0,
            voiced_samples: None,
            development_check: (false, false),
        });
        if session.binding.activity_revision.is_some() {
            session.activity.begin(request);
        }
        Ok(())
    }
    pub fn complete(
        &self,
        id: Uuid,
        request: Uuid,
        similarity: Option<f64>,
        clipped_samples: u32,
        activity: Option<&avesra_contracts::activity::Activity>,
        development_check: (bool, bool),
    ) -> Result<Summary, String> {
        if similarity.is_some_and(|v| !v.is_finite() || !(-1.0..=1.0).contains(&v)) {
            return Err("Invalid calibration measurement".into());
        }
        let mut slot = self.0.lock().map_err(|_| "Calibration unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Calibration discarded or expired")?;
        let value = session
            .observations
            .iter_mut()
            .find(|v| v.request == request && !v.completed)
            .ok_or("Calibration recording is no longer pending")?;
        if session.binding.activity_revision.is_some() {
            value.voiced_samples = activity.and_then(|v| session.activity.signal(v).map(|s| s.0));
            session.activity.complete(request, activity)?;
        }
        value.completed = true;
        value.similarity = similarity;
        value.clipped_samples = clipped_samples;
        value.development_check = development_check;
        Ok(session.summary())
    }
    pub fn fail(&self, id: Uuid, request: Uuid) {
        let _ = self.complete(id, request, None, 0, None, (false, false));
        if let Ok(mut slot) = self.0.lock()
            && let Some(session) = slot.active.as_mut().filter(|v| v.id == id)
            && let Some(gate) = &mut session.gate
        {
            gate.missing(request);
            session.capture = None;
        }
    }
    pub fn discard(&self, id: Uuid) -> Result<Option<Uuid>, String> {
        let mut slot = self.0.lock().map_err(|_| "Qualification unavailable")?;
        if slot
            .active
            .as_ref()
            .is_some_and(|v| v.id == id && v.profile.is_some())
        {
            return Err(
                "Revoke automatic voice permission to remove reviewed qualification".into(),
            );
        }
        slot.retire(id);
        if slot.active.as_ref().is_some_and(|v| v.id == id) {
            return Ok(slot.active.take().and_then(|v| {
                v.observations
                    .iter()
                    .find(|v| !v.completed)
                    .map(|v| v.request)
            }));
        }
        Ok(None)
    }
    fn snapshot(&self, id: Uuid) -> Result<Option<Summary>, String> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "Calibration unavailable")?
            .active
            .as_ref()
            .filter(|v| v.id == id)
            .map(Session::summary))
    }
    fn binding(&self, id: Uuid) -> Result<Binding, String> {
        self.0
            .lock()
            .map_err(|_| "Calibration unavailable")?
            .active
            .as_ref()
            .filter(|v| v.id == id)
            .map(|v| v.binding.clone())
            .ok_or("Calibration discarded or expired".into())
    }
    pub fn settings_hidden(&self) {
        if let Ok(mut slot) = self.0.lock()
            && slot.active.as_ref().is_some_and(|v| v.profile.is_none())
        {
            slot.clear();
        }
    }
    pub fn activity_chunk(
        &self,
        id: Uuid,
        request: Uuid,
        chunk: &avesra_contracts::activity::Chunk,
    ) -> Result<Option<activity::Diagnostic>, String> {
        let mut slot = self.0.lock().map_err(|_| "Calibration unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Calibration discarded or expired")?;
        if let Some(capture) = session.capture.as_mut().filter(|v| v.request == request) {
            for endpoint in capture
                .owner
                .observe(u64::from(chunk.frame_offset), &chunk.frames, chunk.r#final)
                .map_err(|_| "Qualification utterance evidence changed")?
            {
                if capture.endpoint.is_some() || capture.ambiguous {
                    capture.endpoint = None;
                    capture.ambiguous = true;
                } else {
                    capture.endpoint = Some(endpoint);
                }
            }
        }
        session.activity.observe(request, chunk)
    }
    fn annotate_activity(
        &self,
        id: Uuid,
        request: Uuid,
        spans: Vec<activity::Span>,
    ) -> Result<Summary, String> {
        let mut slot = self.0.lock().map_err(|_| "Calibration unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Calibration discarded or expired")?;
        if session.observations.iter().any(|v| !v.completed) {
            return Err("Finish recording before annotating".into());
        }
        session.activity.annotate(request, spans)?;
        Ok(session.summary())
    }
    fn freeze_activity(&self, id: Uuid) -> Result<Summary, String> {
        let mut slot = self.0.lock().map_err(|_| "Calibration unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Calibration discarded or expired")?;
        session.activity.freeze()?;
        Ok(session.summary())
    }
    pub fn observe(&self, state: &Runtime, local: &avesra_core::state::LocalState) {
        if let Ok(mut slot) = self.0.lock()
            && slot.active.as_ref().is_some_and(|v| {
                !v.binding.current(state, local)
                    || if let Some(profile) = &v.profile {
                        !profile.valid()
                    } else {
                        v.created.elapsed() >= COLLECTION_LIFETIME
                    }
            })
        {
            slot.clear();
        }
    }
    fn freeze(&self, id: Uuid) -> Result<Summary, String> {
        let mut slot = self.0.lock().map_err(|_| "Calibration unavailable")?;
        let session = slot
            .active
            .as_mut()
            .filter(|v| v.id == id && v.created.elapsed() < COLLECTION_LIFETIME)
            .ok_or("Calibration discarded or expired")?;
        if session.threshold.is_some() || session.observations.iter().any(|v| !v.completed) {
            return Err(
                "Finish recording first; an already frozen threshold cannot be changed".into(),
            );
        }
        let minimum_owner = session
            .observations
            .iter()
            .filter(|v| v.condition == Condition::OwnerDirected)
            .filter_map(|v| v.similarity)
            .reduce(f64::min)
            .ok_or("Record an owner calibration sample first")?;
        let maximum_other = session
            .observations
            .iter()
            .filter(|v| v.condition == Condition::OtherSpeaker)
            .filter_map(|v| v.similarity)
            .reduce(f64::max)
            .ok_or("Record an explicitly participating other live speaker first")?;
        if minimum_owner <= maximum_other {
            return Err(
                "The observed speaker scores overlap. No separating operating point was found."
                    .into(),
            );
        }
        let threshold = maximum_other + (minimum_owner - maximum_other) / 2.0;
        if session.saved_held_out.len() != 2
            || session
                .saved_held_out
                .iter()
                .any(|v| !v.is_finite() || f64::from(*v) < threshold)
        {
            return Err(
                "The saved held-out comparisons do not meet this measured operating point".into(),
            );
        }
        session.threshold = Some(threshold);
        Ok(session.summary())
    }
}

async fn review(
    window: &tauri::WebviewWindow,
    session: Uuid,
) -> Result<
    (
        avesra_core::enrollment::Candidate,
        crate::connection::PairingRecord,
    ),
    String,
> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to review calibration".into());
    }
    let state = window.state::<Runtime>();
    let expected = state.qualification.binding(session)?;
    let acknowledged = state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?
        .ok_or("No acknowledged Spark session")?;
    let directory = window
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let id = expected.candidate;
    let revision = expected.revision;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let (candidate, pairing) = tokio::task::spawn_blocking(move || {
        // Guard acquisition precedes scheduling, so repeated native Freeze
        // calls cannot accumulate unbounded profile readers behind a busy disk.
        let _owner = owner;
        Ok::<_, String>((
            crate::profiles::read_candidate(&directory, id, revision)?,
            crate::connection::load(&directory)?,
        ))
    })
    .await
    .map_err(|_| "Calibration profile reader stopped")??;
    if binding(
        window.app_handle(),
        &candidate,
        &pairing,
        acknowledged,
        expected.activity_revision.is_some(),
        expected.activity_streaming,
    )
    .await?
        != expected
    {
        let _ = state.qualification.discard(session);
        return Err("Calibration owner or registration changed".into());
    }
    if !window.is_visible().unwrap_or(false) {
        return Err("Settings closed".into());
    }
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state.qualification.observe(&state, &local);
    Ok((candidate, pairing))
}

#[tauri::command]
pub async fn review_development_voice(
    window: tauri::WebviewWindow,
    session: Uuid,
    request: Uuid,
    start: u32,
    end: u32,
    consent: bool,
) -> Result<Summary, String> {
    use avesra_core::voice::qualification::{DevelopmentMeasurement, Report};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let started = Instant::now();
    let state = window.state::<Runtime>();
    let challenge = state.setup.challenge();
    let cancelled = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(cancelled.clone());
    let authorize = || {
        if !consent
            || cancelled.load(Ordering::SeqCst)
            || state.setup.challenge() != challenge
            || started.elapsed() >= Duration::from_secs(60)
        {
            Err("Development voice consent was withdrawn or expired".to_string())
        } else {
            Ok(())
        }
    };
    authorize()?;
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Finish recording before enabling development voice")?;
    let mutation = state
        .qualification
        .1
        .clone()
        .try_lock_owned()
        .map_err(|_| "Voice permission management is busy")?;
    let (candidate, pairing) = review(&window, session).await?;
    authorize()?;
    let acknowledged = state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?
        .ok_or("Spark disconnected")?;
    let grant = Uuid::new_v4();
    let (expected, context, measurement) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Voice admission unavailable")?;
        let current = slot
            .active
            .as_ref()
            .filter(|v| {
                v.id == session
                    && v.profile.is_none()
                    && v.gate.is_none()
                    && v.threshold.is_none()
                    && v.created.elapsed() < COLLECTION_LIFETIME
                    && v.binding.current(&state, &local)
                    && v.binding.activity_streaming
                    && !v.observations.iter().any(|m| !m.completed)
            })
            .ok_or("Start a fresh short live setup with the selected saved voice")?;
        if local.capture_epoch != acknowledged.epoch || local.voice_ready {
            return Err("Voice setup context changed".into());
        }
        let observed = current
            .observations
            .last()
            .filter(|v| {
                v.request == request
                    && v.condition == Condition::OwnerDirected
                    && v.completed
                    && !v.held_out
            })
            .ok_or("Use the latest completed owner check")?;
        let mut context = current.context(local.capture_epoch);
        context.grant_revision = Some(grant);
        (
            current.binding.clone(),
            context,
            DevelopmentMeasurement {
                request,
                policy_revision: Uuid::new_v4(),
                similarity: observed
                    .similarity
                    .ok_or("Live owner comparison unavailable")? as f32,
                speech_start: start,
                speech_end: end,
                frames: current.activity.development_frames(request)?,
                clipped_samples: observed.clipped_samples,
                recognized_speech: observed.development_check.0,
                no_output: observed.development_check.1,
            },
        )
    };
    let directed =
        crate::connection::directedness::metadata(&pairing, acknowledged, &context, || {
            authorize()?;
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != acknowledged.epoch
                || !expected.current(&state, &local)
                || state.qualification.binding(session)? != expected
            {
                return Err("Development voice review context changed".into());
            }
            Ok(())
        })
        .await?;
    authorize()?;
    let candidate_digest = crate::profiles::candidate_digest(&candidate)?;
    let (report, profile) = Report::development(candidate, context, [
        expected.asr_revision.into(), SIGNAL_REVISION.into(), directed.adapter_revision.clone(),
        SIGNAL_REVISION.into(), OUTPUT_REVISION.into(),
    ], measurement).map_err(|_| "The short check did not separate quiet and speech, match the saved owner, or contain one complete utterance. Adjust the speech interval or record again.")?;
    let record = crate::profiles::QualificationRecord {
        version: 1,
        owner_revision: expected.owner_revision,
        registration: expected.registration.clone(),
        server: pairing.server_fingerprint()?,
        candidate_digest,
        directed_artifact: directed.artifact_revision.clone(),
        directed_incarnation: directed.engine_incarnation.clone(),
        directed_quality: directed.quality_fingerprint.clone(),
        report,
    };
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let app = window.app_handle().clone();
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    // Actual disk ownership outlives a dropped command. Only the locked final
    // publication can install admission, and caller loss is checked there.
    tokio::task::spawn_blocking(move || {
        let _mutation = mutation;
        let _owner = owner;
        let mut profile = Some(profile);
        let mut summary = None;
        crate::profiles::save_qualification(&directory, &record, &mut |temporary, destination| {
            let state = app.state::<Runtime>();
            let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
            let mut slot = state
                .qualification
                .0
                .lock()
                .map_err(|_| "Voice admission unavailable")?;
            if cancelled.load(Ordering::SeqCst)
                || state.setup.challenge() != challenge
                || started.elapsed() >= Duration::from_secs(60)
                || local.capture_epoch != acknowledged.epoch
                || local.voice_ready
                || !expected.current(&state, &local)
                || !slot.active.as_ref().is_some_and(|v| {
                    v.id == session
                        && v.binding == expected
                        && v.profile.is_none()
                        && v.created.elapsed() < COLLECTION_LIFETIME
                        && v.observations
                            .last()
                            .is_some_and(|m| m.request == request && m.completed)
                })
            {
                return Err("Development voice publication withdrawn".into());
            }
            std::fs::hard_link(temporary, destination)
                .map_err(|_| "Revoke the previous voice permission before replacing it")?;
            let current = slot.active.as_mut().ok_or("Voice setup disappeared")?;
            current.consent = Some(grant);
            current.directed = Some(directed.clone());
            current.profile = profile.take();
            summary = Some(current.summary());
            slot.revoked = false;
            slot.suspended = false;
            local.enrolled = true;
            local.voice_ready = true;
            local.capture_epoch = local.capture_epoch.saturating_add(1);
            local.refresh();
            state.publish(&local);
            use tauri::Emitter;
            let _ = app.emit("runtime-state", local.clone());
            Ok(())
        })?;
        summary.ok_or("Development voice was not published".into())
    })
    .await
    .map_err(|_| "Development voice writer stopped")?
}

#[tauri::command]
pub async fn review_voice_admission(
    window: tauri::WebviewWindow,
    session: Uuid,
    activate: bool,
) -> Result<Summary, String> {
    let state = window.state::<Runtime>();
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Finish recording before reviewing voice admission")?;
    let (candidate, pairing) = review(&window, session).await?;
    let acknowledged = state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?
        .ok_or("Spark disconnected")?;
    let context = {
        let slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Qualification unavailable")?;
        slot.active
            .as_ref()
            .filter(|v| v.id == session)
            .ok_or("Qualification changed")?
            .context(acknowledged.epoch)
    };
    let directed =
        crate::connection::directedness::metadata(&pairing, acknowledged, &context, || {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            state.qualification.observe(&state, &local);
            if !window.is_visible().unwrap_or(false)
                || state.qualification.binding(session).is_err()
            {
                return Err("Qualification review cancelled".into());
            }
            Ok(())
        })
        .await?;
    if activate {
        let mutation = state
            .qualification
            .1
            .clone()
            .try_lock_owned()
            .map_err(|_| "Voice permission management is busy")?;
        state.qualification.directed_current(session, &directed)?;
        let (record, expected) = {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            state.qualification.observe(&state, &local);
            let slot = state
                .qualification
                .0
                .lock()
                .map_err(|_| "Qualification unavailable")?;
            let current = slot
                .active
                .as_ref()
                .filter(|v| v.id == session)
                .ok_or("Qualification changed")?;
            let report = current
                .gate
                .as_ref()
                .ok_or("Qualification is not awaiting review")?
                .report(&current.context(local.capture_epoch))
                .map_err(|_| "Whole-gate evidence is incomplete")?;
            (
                crate::profiles::QualificationRecord {
                    version: 1,
                    owner_revision: current.binding.owner_revision,
                    registration: current.binding.registration.clone(),
                    server: pairing.server_fingerprint()?,
                    candidate_digest: crate::profiles::candidate_digest(&candidate)?,
                    directed_artifact: directed.artifact_revision.clone(),
                    directed_incarnation: directed.engine_incarnation.clone(),
                    directed_quality: directed.quality_fingerprint.clone(),
                    report,
                },
                current.binding.clone(),
            )
        };
        let owner = state
            .owner_setup
            .clone()
            .try_lock_owned()
            .map_err(|_| "Owner management is busy")?;
        let directory = window
            .app_handle()
            .path()
            .app_data_dir()
            .map_err(|_| "Profile directory unavailable")?;
        let app = window.app_handle().clone();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let _caller = crate::output::Caller(cancelled.clone());
        tokio::task::spawn_blocking(move || {
            let _mutation = mutation;
            let _owner = owner;
            crate::profiles::save_qualification(
                &directory,
                &record,
                &mut |temporary, destination| {
                    let state = app.state::<Runtime>();
                    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                    let slot = state
                        .qualification
                        .0
                        .lock()
                        .map_err(|_| "Qualification unavailable")?;
                    if cancelled.load(std::sync::atomic::Ordering::SeqCst)
                        || local.capture_epoch != acknowledged.epoch
                        || !expected.current(&state, &local)
                        || !slot
                            .active
                            .as_ref()
                            .is_some_and(|v| v.id == session && v.binding == expected)
                    {
                        return Err("Qualification publication withdrawn".into());
                    }
                    // Publication stays under the same local and collection locks as
                    // the final authority check. Settings loss cannot interleave.
                    std::fs::hard_link(temporary, destination).map_err(|_| {
                        "Revoke the previous qualification before replacing its report".into()
                    })
                },
            )
        })
        .await
        .map_err(|_| "Qualification writer stopped")??;
    }
    let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state.qualification.observe(&state, &local);
    let summary = if activate {
        state.qualification.directed_current(session, &directed)?;
        let summary = state.qualification.promote(session, local.capture_epoch)?;
        local.enrolled = true;
        local.voice_ready = true;
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        use tauri::Emitter;
        let _ = window.app_handle().emit("runtime-state", local.clone());
        summary
    } else {
        state
            .qualification
            .freeze_gate(session, local.capture_epoch, candidate, directed)?
    };
    Ok(summary)
}

#[tauri::command]
pub async fn freeze_voice_calibration(
    window: tauri::WebviewWindow,
    session: Uuid,
    activity: Option<bool>,
) -> Result<Summary, String> {
    let state = window.state::<Runtime>();
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Finish the current recording before reviewing calibration")?;
    review(&window, session).await?;
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state.qualification.observe(&state, &local);
    if activity.unwrap_or(false) {
        state.qualification.freeze_activity(session)
    } else {
        state.qualification.freeze(session)
    }
}

#[tauri::command]
pub async fn revoke_voice_admission(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to revoke automatic voice permission".into());
    }
    let app = window.app_handle().clone();
    let state = app.state::<Runtime>();
    let mutation = state
        .qualification
        .1
        .clone()
        .try_lock_owned()
        .map_err(|_| "Voice permission management is busy")?;
    let pending = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Qualification unavailable")?;
        slot.revoked = true;
        slot.clear();
        drop(slot);
        local.voice_ready = false;
        local.enrolled = false;
        local.settings.explicit_mute = true;
        local.capture_epoch = local.capture_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        use tauri::Emitter;
        let _ = app.emit("runtime-state", local.clone());
        crate::enqueue(&state, local.settings.clone())
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    // The actual revocation coordinator survives a disappearing Settings caller.
    let result = tauri::async_runtime::spawn(async move {
        let _mutation = mutation;
        let owner = app
            .state::<Runtime>()
            .owner_setup
            .clone()
            .lock_owned()
            .await;
        tokio::task::spawn_blocking(move || {
            let _owner = owner;
            crate::profiles::revoke_qualification(&directory)
        })
        .await
        .map_err(|_| "Qualification revocation writer stopped")?
    })
    .await
    .map_err(|_| "Qualification revocation coordinator stopped")?;
    pending?.await.map_err(|_| "Settings writer stopped")??;
    result
}

#[tauri::command]
pub async fn annotate_voice_activity(
    window: tauri::WebviewWindow,
    session: Uuid,
    request: Uuid,
    spans: Vec<activity::Span>,
) -> Result<Summary, String> {
    if request.is_nil() || spans.is_empty() || spans.len() > 32 {
        return Err("Invalid activity annotation".into());
    }
    let state = window.state::<Runtime>();
    let _recording = state
        .setup
        .recording
        .try_lock()
        .map_err(|_| "Finish the current recording before labelling calibration")?;
    review(&window, session).await?;
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state.qualification.observe(&state, &local);
    state
        .qualification
        .annotate_activity(session, request, spans)
}

#[tauri::command]
pub fn voice_calibration_status(
    window: tauri::WebviewWindow,
    session: Uuid,
) -> Result<Option<Summary>, String> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to inspect calibration".into());
    }
    let state = window.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state.qualification.observe(&state, &local);
    state.qualification.snapshot(session)
}

#[tauri::command]
pub fn discard_voice_calibration(
    window: tauri::WebviewWindow,
    session: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings to discard calibration".into());
    }
    if let Some(request) = window.state::<Runtime>().qualification.discard(session)? {
        crate::voice_check::stop(window.app_handle(), request);
    }
    Ok(())
}
