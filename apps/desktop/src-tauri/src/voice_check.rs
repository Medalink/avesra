//! Explicit read-only comparison. No qualified profile or action is produced.
#[path = "voice_check_activity.rs"]
pub(crate) mod activity_stream;
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
    output: Option<Uuid>,
}
#[derive(Default)]
pub struct State(Mutex<Option<Attempt>>);
impl State {
    fn current(&self, app: &tauri::AppHandle, id: Uuid) -> Result<Attempt, String> {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let attempt = self
            .0
            .lock()
            .map_err(|_| "Voice check unavailable")?
            .filter(|a| a.id == id)
            .ok_or("Voice check cancelled")?;
        if state.media.setup_output_owned(local.playback_epoch)
            && attempt.output.is_none_or(|output| {
                state
                    .media
                    .output_state(local.playback_epoch, output)
                    .is_err()
            })
        {
            return Err("Stop the voice preview before checking your saved voice".into());
        }
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
pub(crate) fn current(app: &tauri::AppHandle, request: Uuid) -> Result<(), String> {
    let state = app.state::<Runtime>();
    state.voice_check.current(app, request)?;
    if !state
        .local
        .lock()
        .is_ok_and(|local| local.enrollment_capture)
    {
        return Err("Playback measurement capture ended".into());
    }
    Ok(())
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
    calibration: Option<crate::qualification::Summary>,
    activity: Option<avesra_contracts::activity::Activity>,
    assistant_output_overlap: bool,
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
    request: Uuid,
    id: Uuid,
    revision: Uuid,
    calibration: Option<crate::qualification::Recording>,
    activity: Option<bool>,
    activity_streaming: Option<bool>,
) -> Result<ResultSummary, String> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) || request.is_nil() {
        return Err("Open Settings to check your saved voice".into());
    }
    let app = window.app_handle().clone();
    let state = app.state::<Runtime>();
    let activity = activity.unwrap_or(false);
    let activity_streaming = activity_streaming.unwrap_or(false);
    if activity_streaming && !activity {
        return Err("Enable activity observations for a live check".into());
    }
    let playback_trial = calibration
        .as_ref()
        .is_some_and(|v| v.condition == crate::qualification::Condition::AssistantPlayback);
    if playback_trial && (!activity || !activity_streaming) {
        return Err(
            "Assistant-playback measurements require the frozen live whole-gate policy".into(),
        );
    }
    let trial_output = playback_trial.then(Uuid::new_v4);
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
            output: trial_output,
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
        if activity {
            crate::connection::voice_activity_available(&pairing, activity_streaming).await?;
        }
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
        let measurement_binding = if let Some(recording) = &calibration {
            let binding = crate::qualification::binding(
                &app,
                &candidate,
                &pairing,
                session,
                activity,
                activity_streaming,
            )
            .await?;
            state.voice_check.current(&app, request)?;
            state.qualification.begin(
                recording,
                request,
                binding.clone(),
                &candidate.held_out_similarities,
                epoch,
            )?;
            Some(binding)
        } else {
            None
        };
        let _measurement = calibration
            .as_ref()
            .map(|v| crate::qualification::Attempt::new(app.clone(), v.session, request));
        let playback_voice = if playback_trial {
            let recording = calibration.as_ref().ok_or("Playback measurement missing")?;
            if state
                .qualification
                .gate_context(recording.session, epoch)
                .is_none()
            {
                return Err(
                    "Freeze the whole-gate policy before an assistant-playback trial".into(),
                );
            }
            let status = crate::connection::greeting_voice_status(&pairing, session).await?;
            let crate::connection::VoiceResult::Status(status) = status else {
                return Err("Selected playback voice unavailable".into());
            };
            if status.selection_state != "available"
                || status.active_state != "available"
                || status.selected != status.active_voice
            {
                return Err("Select and load the exact voice before measuring its playback".into());
            }
            Some(
                status
                    .selected
                    .ok_or("Selected playback voice unavailable")?,
            )
        } else {
            None
        };
        let activity_stream = if activity_streaming {
            Some(activity_stream::Stream::open(&pairing, session).await?)
        } else {
            None
        };
        state.voice_check.current(&app, request)?;
        if !window.is_visible().unwrap_or(false) {
            return Err("Settings closed".into());
        }
        let output_observation = trial_output
            .map(|output| state.media.observe_output(output))
            .transpose()?;
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
        let no_output = {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            state.media.no_output(&local)
        };
        progress(&app, request, "recording", 0);
        let (phrase, streamed_activity) = if let Some(stream) = activity_stream {
            let (sender, receiver) = tokio::sync::mpsc::channel(2);
            let capture = crate::phrase_capture::collect_observed(
                &app,
                epoch,
                || state.voice_check.current(&app, request).map(|_| ()),
                |seconds| progress(&app, request, "recording", seconds),
                |pcm, captured, final_chunk| {
                    if let Some(recording) = &calibration {
                        state.qualification.capture_gate_pcm(
                            recording.session,
                            request,
                            pcm,
                            captured,
                        )?;
                    }
                    sender
                        .try_send(activity_stream::Captured {
                            pcm: pcm.to_vec(),
                            captured,
                            final_chunk,
                        })
                        .map_err(|_| "Activity processing fell behind recording".into())
                },
            );
            let capture = async {
                let phrase = capture.await?;
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
                Ok(phrase)
            };
            let playback = async {
                if let Some(voice) = playback_voice {
                    let recording = calibration.as_ref().ok_or("Playback measurement missing")?;
                    let started = Instant::now();
                    while !state
                        .qualification
                        .playback_ready(recording.session, request)
                    {
                        state.voice_check.current(&app, request)?;
                        if started.elapsed() >= Duration::from_secs(8) {
                            return Err("No measured quiet interval preceded playback".into());
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    crate::preview::play_for_check(
                        app.clone(),
                        session,
                        request,
                        trial_output.ok_or("Playback owner missing")?,
                        voice,
                    )
                    .await?;
                }
                Ok::<_, String>(())
            };
            let (phrase, activity, ()) = tokio::try_join!(
                capture,
                stream.run(
                    &app,
                    request,
                    calibration.as_ref().map(|v| v.session),
                    receiver
                ),
                playback
            )?;
            (phrase, Some(activity))
        } else {
            (
                crate::phrase_capture::collect(
                    &app,
                    epoch,
                    || state.voice_check.current(&app, request).map(|_| ()),
                    |seconds| progress(&app, request, "recording", seconds),
                )
                .await?,
                None,
            )
        };
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
        let echo_absent = {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            no_output
                .as_ref()
                .is_some_and(|v| state.media.no_output_current(v, &local))
        };
        let endpoint = if let Some(recording) = &calibration {
            state
                .qualification
                .take_gate_endpoint(recording.session, request)?
        } else {
            None
        };
        let utterance = endpoint.as_ref().map_or(request, |v| v.id());
        let assistant_output_overlap = endpoint.as_ref().is_some_and(|endpoint| {
            output_observation
                .as_ref()
                .is_some_and(|observed| observed.overlaps(endpoint.started(), endpoint.completed()))
        });
        let endpoint_signal = endpoint.as_ref().map(|v| {
            (
                v.started(),
                v.samples(),
                v.voiced_samples(),
                v.overlapping_samples(),
                v.clipped_samples(),
            )
        });
        let analysis_pcm = endpoint.map_or(phrase.pcm, |v| v.into_pcm());
        let analysis = crate::connection::analyze_voice(
            &pairing,
            session,
            utterance,
            analysis_pcm,
            activity && !activity_streaming,
        )
        .await?;
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
        let gate_observation = if let Some(recording) = &calibration
            && let Some((context, expected)) =
                state.qualification.gate_context(recording.session, epoch)
        {
            use avesra_core::voice::{
                AudioCondition, DirectedIntent, DirectedKind, SignalEvidence,
            };
            let directed = crate::connection::directedness::classify(
                &pairing,
                session,
                utterance,
                &context,
                &analysis.transcript,
                || state.voice_check.current(&app, request).map(|_| ()),
            )
            .await?;
            state
                .qualification
                .directed_current(recording.session, &directed.binding)?;
            if expected != directed.binding {
                return Err("Directedness adapter changed".into());
            }
            let directed = match directed.category {
                avesra_contracts::directedness::Category::Request => DirectedIntent::Directed {
                    adapter_revision: directed.binding.adapter_revision,
                    utterance,
                    context: context.clone(),
                    kind: DirectedKind::Request,
                },
                avesra_contracts::directedness::Category::FollowUp => DirectedIntent::Directed {
                    adapter_revision: directed.binding.adapter_revision,
                    utterance,
                    context: context.clone(),
                    kind: DirectedKind::FollowUp,
                },
                avesra_contracts::directedness::Category::Rejected => DirectedIntent::Rejected {
                    adapter_revision: directed.binding.adapter_revision,
                    utterance,
                    context: context.clone(),
                },
                avesra_contracts::directedness::Category::Unknown => DirectedIntent::Unknown,
            };
            let (
                utterance_started,
                total_samples,
                voiced_samples,
                overlapping_samples,
                clipped_samples,
            ) = endpoint_signal.ok_or("Measured endpoint missing")?;
            let signal = Some((voiced_samples, overlapping_samples));
            let overlap = signal.map_or(AudioCondition::Unknown, |(_, overlap)| {
                AudioCondition::Measured {
                    adapter_revision: crate::qualification::SIGNAL_REVISION.into(),
                    utterance,
                    context: context.clone(),
                    detected: overlap > 0,
                }
            });
            let signal = signal.map_or(SignalEvidence::Unknown, |(voiced, _)| {
                SignalEvidence::Measured {
                    adapter_revision: crate::qualification::SIGNAL_REVISION.into(),
                    voiced_samples: voiced,
                    total_samples,
                    clipped_samples,
                }
            });
            let echo = if assistant_output_overlap || (echo_absent && !playback_trial) {
                AudioCondition::Measured {
                    adapter_revision: crate::qualification::OUTPUT_REVISION.into(),
                    utterance,
                    context: context.clone(),
                    detected: assistant_output_overlap,
                }
            } else {
                AudioCondition::Unknown
            };
            Some((
                context.clone(),
                avesra_core::voice::Observation {
                    utterance,
                    context,
                    asr_revision: measurement_binding
                        .as_ref()
                        .ok_or("Measurement binding lost")?
                        .asr_revision
                        .into(),
                    speaker_revision: candidate.model_revision.clone(),
                    started: utterance_started,
                    completed: Instant::now(),
                    transcript: analysis.transcript.clone(),
                    embedding: analysis.embedding.clone(),
                    overlap,
                    echo,
                    signal,
                    directed,
                },
            ))
        } else {
            None
        };
        let calibration_summary = if let Some(recording) = &calibration {
            let binding = crate::qualification::binding(
                &app,
                &candidate,
                &pairing,
                session,
                activity,
                activity_streaming,
            )
            .await?;
            state.voice_check.current(&app, request)?;
            if Some(&binding) != measurement_binding.as_ref() {
                let _ = state.qualification.discard(recording.session);
                return Err("Calibration owner or registration changed".into());
            }
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            state.qualification.observe(&state, &local);
            if let Some((context, observation)) = gate_observation {
                state.qualification.gate_observe(
                    recording.session,
                    &context,
                    request,
                    observation,
                )?;
            }
            Some(
                state.qualification.complete(
                    recording.session,
                    request,
                    similarity,
                    phrase.clipped_samples,
                    streamed_activity.as_ref().or(analysis.activity.as_ref()),
                    (
                        !analysis.transcript.trim().is_empty(),
                        no_output
                            .as_ref()
                            .is_some_and(|v| state.media.no_output_current(v, &local)),
                    ),
                )?,
            )
        } else {
            None
        };
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
            calibration: calibration_summary,
            activity: streamed_activity.or(analysis.activity),
            assistant_output_overlap,
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
