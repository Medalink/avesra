//! Qualified continuous native capture; no serialized input creates admission.
use crate::{
    Runtime,
    connection::{PairingRecord, SessionIdentity},
    qualification::Admission,
};
use avesra_core::voice::{
    self,
    utterance::{Completed, Owner},
};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

pub struct Worker(tauri::async_runtime::JoinHandle<()>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub fn spawn(app: tauri::AppHandle, record: PairingRecord, generation: u64) -> Worker {
    Worker(tauri::async_runtime::spawn(async move {
        #[cfg(windows)]
        if avesra_windows::output_recording::enabled() {
            return;
        }
        let mut restore_attempt = None;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let state = app.state::<Runtime>();
            if state.connection_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let restore = (|| {
                let local = state.local.lock().ok()?;
                state.qualification.observe(&state, &local);
                if !state.qualification.can_restore()
                    || !local.connected
                    || local.locked
                    || local.settings.paused
                    || local.settings.deafened
                    || local.settings.explicit_mute
                    || local.enrollment_capture
                    || local.microphone_check
                {
                    return None;
                }
                let session = (*state.acknowledged_session.lock().ok()?)?;
                (session.generation == generation && session.epoch == local.capture_epoch)
                    .then_some(session)
            })();
            if let Some(session) = restore
                && restore_attempt != Some((session.id, session.epoch))
            {
                restore_attempt = Some((session.id, session.epoch));
                if let Err(error) = crate::qualification::restore(&app, &record, session).await {
                    let _ = app.emit(
                        "runtime-error",
                        format!("Saved qualification was not restored: {error}"),
                    );
                }
                continue;
            }
            let ready = (|| {
                let local = state.local.lock().ok()?;
                if !local.capture_allowed()
                    || !local.voice_ready
                    || local.enrollment_capture
                    || local.active_task
                {
                    return None;
                }
                let session = (*state.acknowledged_session.lock().ok()?)?;
                if session.generation != generation || session.epoch != local.capture_epoch {
                    return None;
                }
                let admission = state.qualification.admission(&state, &local)?;
                let quiet = state.media.no_output(&local)?;
                Some((session, admission, quiet))
            })();
            let Some((session, admission, quiet)) = ready else {
                continue;
            };
            let Some(_voice_owner) = crate::notifications::voice_owner(&app) else {
                continue;
            };
            let work = async {
                let observed = crate::connection::directedness::metadata(
                    &record,
                    session,
                    &admission.context,
                    || current(&app, session, &admission),
                )
                .await?;
                state
                    .qualification
                    .directed_current(admission.session, &observed)?;
                if observed != admission.directed {
                    return Err("Directedness incarnation changed".into());
                }
                if let Some(segment) = capture(&app, &record, session, &admission, &quiet).await? {
                    process(&app, &record, session, &admission, quiet, segment).await?;
                }
                Ok::<_, String>(())
            };
            if let Err(error) = work.await
                && let Ok(mut local) = state.local.lock()
                && state.connection_generation.load(Ordering::SeqCst) == generation
                && local.capture_epoch == session.epoch
            {
                local.voice_ready = false;
                local.capture_epoch = local.capture_epoch.saturating_add(1);
                local.refresh();
                state.publish(&local);
                let _ = app.emit("runtime-state", local.clone());
                let _ = app.emit("runtime-error", format!("Listening stopped: {error}"));
            }
        }
    }))
}
fn current(
    app: &tauri::AppHandle,
    expected: SessionIdentity,
    admission: &Admission,
) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let session = state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?
        .ok_or("Spark disconnected")?;
    if state.connection_generation.load(Ordering::SeqCst) != expected.generation
        || local.capture_epoch != expected.epoch
        || !local.capture_allowed()
        || !local.voice_ready
        || local.enrollment_capture
        || session.id != expected.id
        || session.epoch != expected.epoch
        || session.action_epoch != expected.action_epoch
        || local.action_epoch != expected.action_epoch
        || session.generation != expected.generation
    {
        return Err("Voice admission changed".into());
    }
    state
        .qualification
        .with_profile(&state, &local, &admission.context, |_| ())
}
struct Capture {
    app: tauri::AppHandle,
    epoch: u64,
    lease: Uuid,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.app
            .state::<Runtime>()
            .media
            .close_voice_window(self.epoch, self.lease);
    }
}
async fn capture(
    app: &tauri::AppHandle,
    pairing: &PairingRecord,
    session: SessionIdentity,
    admission: &Admission,
    quiet: &crate::media::NoOutput,
) -> Result<Option<Completed>, String> {
    use crate::voice_check::activity_stream::{Captured, Stream};
    current(app, session, admission)?;
    let mut stream = Stream::open(pairing, session).await?;
    current(app, session, admission)?;
    let lease = Uuid::new_v4();
    let opened = Instant::now();
    {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !state.media.no_output_current(quiet, &local) {
            return Err("Assistant output is not quiet".into());
        }
        state.media.open_voice_window(&local, lease)?;
    }
    let capture = Mutex::new(Some(Capture {
        app: app.clone(),
        epoch: session.epoch,
        lease,
    }));
    let finalize = AtomicBool::new(false);
    let owner = Mutex::new(
        Owner::new(admission.context.clone(), admission.policy.clone())
            .map_err(|_| "Invalid utterance owner")?,
    );
    let (send, mut receive) = tokio::sync::mpsc::channel::<Captured>(2);
    let produce = async {
        let mut sequence = 0;
        let mut pcm = Vec::with_capacity(6400);
        let mut captured = opened;
        loop {
            current(app, session, admission)?;
            if finalize.load(Ordering::SeqCst) && (!pcm.is_empty() || send.capacity() < 2) {
                if send.capacity() == 2 {
                    send.try_send(Captured {
                        pcm,
                        captured,
                        final_chunk: true,
                    })
                    .map_err(|_| "Final activity capture unavailable")?;
                }
                capture
                    .lock()
                    .map_err(|_| "Capture lease unavailable")?
                    .take();
                return Ok(());
            }
            let state = app.state::<Runtime>();
            {
                let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                if !state.media.no_output_current(quiet, &local) {
                    return Err("Assistant playback interrupted input".to_owned());
                }
            }
            if opened.elapsed() >= Duration::from_secs(11) {
                return Err("Capture deadline expired".into());
            }
            let frame = match state.media.take_voice_frame(session.epoch, lease) {
                Ok(value) => value,
                Err(_) if finalize.load(Ordering::SeqCst) => return Ok(()),
                Err(error) => return Err(error),
            };
            let Some(frame) = frame else {
                if opened.elapsed() > Duration::from_millis(sequence * 20 + 500) {
                    return Err("Capture lost freshness".into());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            };
            if frame.epoch != session.epoch
                || frame.sequence != sequence + 1
                || frame.captured < opened
            {
                return Err("Capture provenance changed".into());
            }
            owner
                .lock()
                .map_err(|_| "Utterance owner unavailable")?
                .capture(frame.sequence, frame.captured, &frame.samples)
                .map_err(|_| "Capture sequence or clock changed")?;
            if pcm.is_empty() {
                captured = frame.captured;
            }
            pcm.extend(frame.samples.iter().flat_map(|v| v.to_le_bytes()));
            sequence += 1;
            if sequence % 10 == 0 {
                send.try_send(Captured {
                    pcm: std::mem::replace(&mut pcm, Vec::with_capacity(6400)),
                    captured,
                    final_chunk: sequence == 400,
                })
                .map_err(|_| "Activity processing fell behind capture")?;
            }
            if sequence == 400 {
                capture
                    .lock()
                    .map_err(|_| "Capture lease unavailable")?
                    .take();
                return Ok::<_, String>(());
            }
        }
    };
    let consume = async {
        loop {
            let frame = tokio::time::timeout(Duration::from_millis(750), receive.recv())
                .await
                .map_err(|_| "Activity capture stalled")?
                .ok_or("Activity capture stopped")?;
            let observation = stream
                .exchange(app, lease, frame, || current(app, session, admission))
                .await?;
            let completed = owner
                .lock()
                .map_err(|_| "Utterance owner unavailable")?
                .observe(
                    u64::from(observation.frame_offset),
                    &observation.frames,
                    observation.r#final,
                )
                .map_err(|_| "Utterance evidence was discontinuous or expired")?;
            if let Some(segment) = completed.into_iter().next() {
                finalize.store(true, Ordering::SeqCst);
                if !observation.r#final {
                    let mut tail = tokio::time::timeout(Duration::from_millis(500), receive.recv())
                        .await
                        .map_err(|_| "Final activity capture timed out")?
                        .ok_or("Final activity capture missing")?;
                    tail.final_chunk = true;
                    capture
                        .lock()
                        .map_err(|_| "Capture lease unavailable")?
                        .take();
                    stream
                        .exchange(app, lease, tail, || current(app, session, admission))
                        .await?;
                }
                stream.settle().await?;
                let state = app.state::<Runtime>();
                let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                if !state.media.no_output_current(quiet, &local) {
                    return Err("Assistant output changed before endpoint completion".into());
                }
                return Ok(Some(segment));
            }
            if observation.r#final {
                stream.settle().await?;
                // Only the fully scored real final window can release idle
                // listening. EOF with active/unknown speech never grants it.
                if crate::notifications::gap_requested(app)
                    && owner
                        .lock()
                        .map_err(|_| "Utterance owner unavailable")?
                        .quiet_boundary()
                {
                    current(app, session, admission)?;
                    crate::notifications::grant_voice_gap(app);
                }
                return Ok::<_, String>(None);
            }
        }
    };
    tokio::pin!(produce);
    tokio::pin!(consume);
    // Normal endpoint completion sends a real contiguous final tail and waits
    // for terminal retirement. Cancellation still uses the private stream owner.
    tokio::select! {
        biased;
        value = &mut consume => value,
        value = &mut produce => { value?; consume.await }
    }
}
async fn process(
    app: &tauri::AppHandle,
    pairing: &PairingRecord,
    session: SessionIdentity,
    admission: &Admission,
    quiet: crate::media::NoOutput,
    segment: Completed,
) -> Result<(), String> {
    current(app, session, admission)?;
    let id = segment.id();
    let context = segment.context().clone();
    let started = segment.started();
    let endpoint = segment.completed();
    let samples = segment.samples();
    let voiced = segment.voiced_samples();
    let overlap = segment.overlapping_samples();
    let clipped = segment.clipped_samples();
    // Existing analysis backend has a one-second minimum; shorter segments
    // abstain rather than pad audio or weaken signal operating points.
    if samples < 16000 {
        return Ok(());
    }
    let analysis =
        crate::connection::analyze_voice(pairing, session, id, segment.into_pcm(), false).await?;
    app.state::<Runtime>().performance.record(
        crate::performance::Operation::Voice,
        crate::performance::Stage::EndpointToAnalysis,
        endpoint.elapsed(),
        crate::performance::Outcome::Complete,
    );
    current(app, session, admission)?;
    if analysis.transcript.trim().is_empty() {
        return Ok(());
    }
    let directed = crate::connection::directedness::classify(
        pairing,
        session,
        id,
        &context,
        &analysis.transcript,
        || current(app, session, admission),
    )
    .await?;
    app.state::<Runtime>().performance.record(
        crate::performance::Operation::Voice,
        crate::performance::Stage::EndpointToDirectedness,
        endpoint.elapsed(),
        crate::performance::Outcome::Complete,
    );
    app.state::<Runtime>()
        .qualification
        .directed_current(admission.session, &directed.binding)?;
    use avesra_contracts::directedness::Category;
    let directed = match directed.category {
        Category::Request | Category::FollowUp => voice::DirectedIntent::Directed {
            adapter_revision: directed.binding.adapter_revision,
            utterance: id,
            context: context.clone(),
            kind: if matches!(directed.category, Category::Request) {
                voice::DirectedKind::Request
            } else {
                voice::DirectedKind::FollowUp
            },
        },
        Category::Rejected => voice::DirectedIntent::Rejected {
            adapter_revision: directed.binding.adapter_revision,
            utterance: id,
            context: context.clone(),
        },
        Category::Unknown => voice::DirectedIntent::Unknown,
    };
    let observation = voice::Observation {
        utterance: id,
        context: context.clone(),
        asr_revision: "ebe59e5a817142986528bbbee5dba8db7b38ed50".into(),
        speaker_revision: "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286".into(),
        started,
        completed: Instant::now(),
        transcript: analysis.transcript,
        embedding: analysis.embedding,
        signal: voice::SignalEvidence::Measured {
            adapter_revision: crate::qualification::SIGNAL_REVISION.into(),
            voiced_samples: voiced,
            total_samples: samples,
            clipped_samples: clipped,
        },
        overlap: voice::AudioCondition::Measured {
            adapter_revision: crate::qualification::SIGNAL_REVISION.into(),
            utterance: id,
            context: context.clone(),
            detected: overlap > 0,
        },
        echo: voice::AudioCondition::Measured {
            adapter_revision: crate::qualification::OUTPUT_REVISION.into(),
            utterance: id,
            context: context.clone(),
            detected: false,
        },
        directed,
    };
    let state = app.state::<Runtime>();
    let accepted = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !state.media.no_output_current(&quiet, &local) {
            return Err("Assistant output changed before gate admission".into());
        }
        state
            .qualification
            .with_profile(&state, &local, &context, |profile| {
                let decision = state
                    .turns
                    .lock()
                    .map_err(|_| "Turn gate unavailable")?
                    .analyze(&context, Some(profile), observation, None);
                match decision {
                    voice::Decision::Accepted(value) => value
                        .consume(&context, profile)
                        .map(Some)
                        .map_err(|_| "Conversation provenance changed"),
                    voice::Decision::Abstain(_) => Ok(None),
                }
            })??
    };
    let Some(accepted) = accepted else {
        return Ok(());
    };
    let withdrawn = std::sync::Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(withdrawn.clone());
    let authority_app = app.clone();
    let authority_context = context.clone();
    let receiver = state
        .effects
        .accept_conversation(
            accepted,
            Box::new(move |value| {
                if withdrawn.load(Ordering::SeqCst)
                    || !value.precommit_current()
                    || value.context() != &authority_context
                {
                    return Err(avesra_contracts::ErrorCode::Stale);
                }
                let state = authority_app.state::<Runtime>();
                let local = state
                    .local
                    .lock()
                    .map_err(|_| avesra_contracts::ErrorCode::Unavailable)?;
                if !state.media.no_output_current(&quiet, &local) {
                    return Err(avesra_contracts::ErrorCode::Stale);
                }
                state
                    .qualification
                    .with_profile(&state, &local, &authority_context, |profile| {
                        value.profile_revision() == profile.candidate_revision()
                            && value.qualification_revision() == profile.qualification_revision()
                    })
                    .map_err(|_| avesra_contracts::ErrorCode::Stale)
                    .and_then(|valid| {
                        if valid {
                            Ok(())
                        } else {
                            Err(avesra_contracts::ErrorCode::Stale)
                        }
                    })
            }),
        )
        .map_err(|_| "Conversation admission is busy")?;
    let turn = tokio::task::spawn_blocking(move || receiver.recv())
        .await
        .map_err(|_| "Conversation worker stopped")?
        .map_err(|_| "Conversation worker stopped")?
        .map_err(|_| "Conversation was not durably accepted")?;
    let dispatch = avesra_core::ledger::DispatchSession {
        actor_id: context.actor.ok_or("Actor missing")?,
        device_id: context.device,
        session_id: context.session,
        capture_epoch: context.capture_epoch,
        action_epoch: context.action_epoch,
        active: true,
    };
    // This boundary owns an already durable turn. Failure here is not evidence
    // that speaker qualification failed, and must never replay that turn.
    let reply_result = async {
        match crate::tasks::accepted(app.clone(), turn, dispatch, admission.registration.clone())
            .await
            .map_err(|error| (ReplyStage::Task, error))?
        {
            crate::tasks::AcceptedResult::Action(_) => {}
            crate::tasks::AcceptedResult::NeedsInput { message } => {
                let _ = app.emit("runtime-error", message);
            }
            crate::tasks::AcceptedResult::Reply(reply) => {
                let status = tokio::time::timeout(
                    Duration::from_secs(2),
                    crate::connection::greeting_voice_status(pairing, session),
                )
                .await
                .map_err(|_| {
                    (
                        ReplyStage::VoiceStatus,
                        "Selected voice status expired".to_owned(),
                    )
                })?
                .map_err(|error| (ReplyStage::VoiceStatus, error))?;
                if let crate::connection::VoiceResult::Status(status) = status
                    && status.selection_state == "available"
                    && status.active_state == "available"
                    && status.selected == status.active_voice
                    && let Some(voice) = status.selected
                {
                    crate::speech::speak(app.clone(), *reply, voice)
                        .await
                        .map_err(|error| (ReplyStage::Playback, error))?;
                } else {
                    return Err((
                        ReplyStage::VoiceStatus,
                        "The selected voice is not available and active; the answer remains saved"
                            .to_owned(),
                    ));
                }
            }
        }
        Ok::<_, (ReplyStage, String)>(())
    }
    .await;
    if let Err((stage, error)) = reply_result {
        let _ = app.emit(
            "runtime-error",
            format!(
                "{}: {error}. This accepted turn will not be retried.",
                stage.label()
            ),
        );
        current(app, session, admission)?;
        let request = avesra_contracts::actors::Request {
            version: avesra_contracts::actors::VERSION,
            request: Uuid::new_v4(),
            attempt: Uuid::new_v4(),
            session: session.id,
            action_epoch: session.action_epoch,
            command: avesra_contracts::actors::Command::Status,
        };
        let status = tokio::time::timeout(
            Duration::from_secs(3),
            crate::connection::actor_operation(pairing, &request),
        )
        .await
        .map_err(|_| "Owner registration revalidation expired after accepted reply failure")??;
        if status.binding.as_ref() != Some(&admission.registration)
            || admission.registration.revoked
        {
            return Err("Owner registration changed after accepted reply failure".into());
        }
        current(app, session, admission)?;
    }
    Ok(())
}

enum ReplyStage {
    Task,
    VoiceStatus,
    Playback,
}
impl ReplyStage {
    fn label(&self) -> &'static str {
        match self {
            Self::Task => "Accepted task failed",
            Self::VoiceStatus => "Answer voice unavailable",
            Self::Playback => "Answer playback failed",
        }
    }
}
