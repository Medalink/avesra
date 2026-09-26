//! Explicit Settings voice preview; independent of microphone and identity-management proof lifetime.
use crate::performance::{self, Operation, Outcome, Stage};
use crate::{
    Runtime,
    connection::{self, MediaEndpoint, PairingRecord, SessionIdentity},
};
use avesra_contracts::{
    preview::{self, Greeting, PreviewControl, PreviewEvent, PreviewMessage, PreviewRequest},
    voice::VoiceIdentity,
};
use avesra_windows::audio::{PlaybackFrame, PlaybackRate};
use futures_util::{SinkExt, StreamExt};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

struct Lease {
    app: tauri::AppHandle,
    session: SessionIdentity,
    epoch: u64,
    id: Uuid,
    panel: Option<Uuid>,
    check: Option<Uuid>,
    withdrawn: Arc<AtomicBool>,
}
impl Lease {
    fn current(&self, acknowledged: bool) -> Result<(), String> {
        if let Some(check) = self.check {
            crate::voice_check::current(&self.app, check)?;
        }
        let state = self.app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if self.withdrawn.load(Ordering::SeqCst)
            || self.panel.is_some_and(|panel| {
                !state
                    .voice_panel
                    .lock()
                    .is_ok_and(|value| *value == Some(panel))
            })
            || !local.connected
            || local.locked
            || local.settings.deafened
            || local.settings.paused
            || local.playback_epoch != self.epoch
            || state.connection_generation.load(Ordering::SeqCst) != self.session.generation
        {
            return Err("Preview permission changed".into());
        }
        if acknowledged
            && !state.acknowledged_session.lock().is_ok_and(|v| {
                v.is_some_and(|v| {
                    v.id == self.session.id
                        && v.generation == self.session.generation
                        && v.playback_epoch == self.epoch
                })
            })
        {
            return Err("Preview output epoch is not acknowledged".into());
        }
        Ok(())
    }
    fn stop(&self) {
        let state = self.app.state::<Runtime>();
        if let Ok(mut local) = state.local.lock()
            && local.playback_epoch == self.epoch
            && state.connection_generation.load(Ordering::SeqCst) == self.session.generation
        {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
            local.refresh();
            state.publish(&local);
            let _ = self.app.emit("runtime-state", local.clone());
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.stop();
    }
}

#[tauri::command]
pub async fn preview_voice(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    voice: VoiceIdentity,
    panel: Uuid,
    text: Option<String>,
) -> Result<String, String> {
    let withdrawn = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(withdrawn.clone());
    tauri::async_runtime::spawn(async move {
        preview_owned(window, app, voice, panel, text, withdrawn).await
    })
    .await
    .map_err(|_| "Preview coordinator stopped")?
}
async fn preview_owned(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    voice: VoiceIdentity,
    panel: Uuid,
    text: Option<String>,
    withdrawn: Arc<AtomicBool>,
) -> Result<String, String> {
    let admission_epoch = app
        .state::<Runtime>()
        .local
        .lock()
        .map_err(|_| "Local state unavailable")?
        .capture_epoch;
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Open Settings to preview a generated voice".into());
    }
    voice
        .validate()
        .map_err(|_| "Invalid generated voice identity")?;
    if text
        .as_deref()
        .is_some_and(|text| !PreviewRequest::valid_test_text(text))
    {
        return Err("Voice test text must be nonblank, at most 512 UTF-8 bytes, and contain no controls except newline or tab".into());
    }
    let state = app.state::<Runtime>();
    let mut owner = crate::output::Owner::reserve(&app)?;
    let lease = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !state
            .voice_panel
            .lock()
            .is_ok_and(|value| *value == Some(panel))
            || local.capture_epoch != admission_epoch
            || !local.connected
            || local.locked
            || local.settings.deafened
            || local.settings.paused
            || local.settings.speaker.is_none()
        {
            return Err("Choose output and enable assistant sound before previewing".into());
        }
        let generation = state.connection_generation.load(Ordering::SeqCst);
        let session = state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .filter(|v| v.generation == generation && v.playback_epoch == local.playback_epoch)
            .ok_or("Wait for the current Spark session")?;
        local.playback_epoch = local.playback_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        let id = Uuid::new_v4();
        owner.bind(local.playback_epoch, session.generation);
        state.media.open_preview(&local, id, withdrawn.clone())?;
        let lease = Lease {
            app: app.clone(),
            session,
            epoch: local.playback_epoch,
            id,
            panel: Some(panel),
            check: None,
            withdrawn,
        };
        let _ = app.emit("runtime-state", local.clone());
        lease
    };
    play_owned(app, voice, lease, owner, None, text).await
}

/// Native launch-only entry. No webview command accepts greeting text.
pub(crate) async fn greet(
    app: tauri::AppHandle,
    session: SessionIdentity,
    voice: VoiceIdentity,
    greeting: Greeting,
) -> Result<(), String> {
    greeting.validate().map_err(|_| "Invalid remembered name")?;
    let withdrawn = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(withdrawn.clone());
    let state = app.state::<Runtime>();
    let mut owner = crate::output::Owner::reserve(&app)?;
    let lease = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected
            || local.locked
            || local.settings.deafened
            || local.settings.paused
            || local.settings.speaker.is_none()
            || local.playback_epoch != session.playback_epoch
            || state.connection_generation.load(Ordering::SeqCst) != session.generation
            || !state
                .acknowledged_session
                .lock()
                .is_ok_and(|ack| ack.is_some_and(|ack| ack.id == session.id))
        {
            return Err("Startup greeting cancelled".into());
        }
        local.playback_epoch = local.playback_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        let id = Uuid::new_v4();
        owner.bind(local.playback_epoch, session.generation);
        state.media.open_greeting(&local, id, withdrawn.clone())?;
        let lease = Lease {
            app: app.clone(),
            session,
            epoch: local.playback_epoch,
            id,
            panel: None,
            check: None,
            withdrawn,
        };
        let _ = app.emit("runtime-state", local.clone());
        lease
    };
    play_owned(app, voice, lease, owner, Some(greeting), None)
        .await
        .map(|_| ())
}

/// Only the explicit whole-gate playback trial calls this native output path.
/// The coordinator and output owner survive cancellation of its waiting caller.
pub(crate) async fn play_for_check(
    app: tauri::AppHandle,
    session: SessionIdentity,
    check: Uuid,
    output: Uuid,
    voice: VoiceIdentity,
) -> Result<(), String> {
    let withdrawn = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(withdrawn.clone());
    tauri::async_runtime::spawn(async move {
        crate::voice_check::current(&app, check)?;
        let state = app.state::<Runtime>();
        let mut owner = crate::output::Owner::reserve(&app)?;
        let lease = {
            let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if withdrawn.load(Ordering::SeqCst)
                || local.capture_epoch != session.epoch
                || local.action_epoch != session.action_epoch
                || !local.enrollment_capture
                || !local.capture_allowed()
                || local.playback_epoch != session.playback_epoch
                || state.connection_generation.load(Ordering::SeqCst) != session.generation
            {
                return Err("Playback measurement capture changed".into());
            }
            local.playback_epoch = local.playback_epoch.saturating_add(1);
            local.refresh();
            state.publish(&local);
            owner.bind(local.playback_epoch, session.generation);
            state
                .media
                .open_preview(&local, output, withdrawn.clone())?;
            let lease = Lease {
                app: app.clone(),
                session,
                epoch: local.playback_epoch,
                id: output,
                panel: None,
                check: Some(check),
                withdrawn,
            };
            let _ = app.emit("runtime-state", local.clone());
            lease
        };
        play_owned(
            app,
            voice,
            lease,
            owner,
            None,
            Some("Avesra, tell me the time.".into()),
        )
        .await
        .map(|_| ())
    })
    .await
    .map_err(|_| "Playback measurement coordinator stopped")?
}

async fn play_owned(
    app: tauri::AppHandle,
    voice: VoiceIdentity,
    lease: Lease,
    owner: crate::output::Owner,
    greeting: Option<Greeting>,
    test_text: Option<String>,
) -> Result<String, String> {
    let state = app.state::<Runtime>();
    let operation = if greeting.is_some() {
        Operation::Greeting
    } else {
        Operation::Preview
    };
    let span = state
        .performance
        .begin(operation, Stage::OutputSession, lease.id);
    let result = async {
    // Device opening and current output acknowledgment precede any Play request.
    let ready = async {
        loop {
            lease.current(false)?;
            if lease.current(true).is_ok() && state.media.output_state(lease.epoch, lease.id)?.0 {
                return Ok::<_, String>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    let preparation = state.performance.begin(operation, Stage::Preparation, lease.id);
    let prepared = async {
    tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .map_err(|_| "Preview device/session was not ready in time")??;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Pairing unavailable")?;
    let record = tokio::task::spawn_blocking(move || connection::load(&directory))
        .await
        .map_err(|_| "Pairing reader stopped")??;
    lease.current(true)?;
    Ok::<_, String>(record)
    }.await;
    preparation.finish(if prepared.is_ok() { Outcome::Complete } else { Outcome::Failed });
    let record = prepared?;
    let mut modes = state.modes.subscribe();
    let monitor = async {
        loop {
            let disconnected = tokio::select! {
                value = modes.changed() => value.is_err(),
                _ = tokio::time::sleep(Duration::from_millis(50)), if lease.check.is_some() => false,
            };
            if disconnected || lease.current(true).is_err() {
                return;
            }
        }
    };
    let result = tokio::select! {
        biased;
        _=monitor=>Err("Preview permission changed".into()),
        value=tokio::time::timeout(Duration::from_secs(65),play(&lease,&record,voice,greeting,test_text,operation))=>value.map_err(|_|"Preview expired".to_string())?,
    };
    lease.stop();
    owner.finish().await?;
    result.map(|_| "Final preview samples submitted to the output device".into())
    }.await;
    span.finish(if result.is_ok() {
        Outcome::Complete
    } else if lease.withdrawn.load(Ordering::SeqCst) {
        Outcome::Withdrawn
    } else {
        Outcome::Failed
    });
    result
}
async fn play(
    lease: &Lease,
    record: &PairingRecord,
    voice: VoiceIdentity,
    greeting: Option<Greeting>,
    test_text: Option<String>,
    operation: Operation,
) -> Result<(), String> {
    let performance = lease.app.state::<Runtime>().performance.clone();
    let mut remote = Some(performance.begin(operation, Stage::RemoteReady, lease.id));
    let observed = async {
        let endpoint = if greeting.is_some() {
            MediaEndpoint::Greeting
        } else {
            MediaEndpoint::Preview
        };
        let mut socket = connection::voice_socket(record, endpoint).await?;
        lease.current(true)?;
        let request = PreviewRequest {
            version: preview::VERSION,
            session_id: lease.session.id,
            playback_epoch: lease.epoch,
            request_id: lease.id,
            voice,
            greeting,
            test_text,
        };
        request.validate().map_err(|_| "Invalid preview request")?;
        let synthesized = request.greeting.is_some() || request.test_text.is_some();
        let start = serde_json::to_string(&request).map_err(|_| "Preview encoding failed")?;
        tokio::time::timeout(
            Duration::from_millis(500),
            socket.send(Message::Text(start.into())),
        )
        .await
        .map_err(|_| "Preview send expired")?
        .map_err(|_| "Preview disconnected")?;
        let result = async {
            let mut expected = 1u64;
            let mut offset = 0u64;
            let mut total = None;
            let mut opened = None;
            let mut pending: Option<PlaybackFrame> = None;
            loop {
                let timeout = if opened.is_none() {
                    Duration::from_secs(32)
                } else {
                    Duration::from_millis(500)
                };
                let message = tokio::time::timeout(timeout, socket.next())
                    .await
                    .map_err(|_| "Preview receive expired")?
                    .ok_or("Preview disconnected")?
                    .map_err(|_| "Preview transport failed")?;
                lease.current(true)?;
                let Message::Text(text) = message else {
                    return Err("Unexpected preview frame".to_string());
                };
                let event: PreviewEvent =
                    serde_json::from_str(&text).map_err(|_| "Invalid preview response")?;
                if event.context.version != preview::VERSION
                    || event.context.session_id != request.session_id
                    || event.context.playback_epoch != request.playback_epoch
                    || event.context.request_id != request.request_id
                    || event.context.voice != request.voice
                    || event.context.greeting != request.greeting
                    || event.context.test_text != request.test_text
                {
                    return Err("Stale preview response".into());
                }
                match event.message {
                    PreviewMessage::StreamingReady {
                        sample_rate,
                        frame_samples,
                        max_samples,
                    } if synthesized
                        && opened.is_none()
                        && sample_rate == 24000
                        && frame_samples == preview::FRAME_SAMPLES
                        && max_samples == preview::MAX_SAMPLES =>
                    {
                        if let Some(span) = remote.take() {
                            span.finish(Outcome::Complete);
                        }
                        // The ceiling is not an expected length. Only the actual
                        // Complete terminal establishes the final sample count.
                        opened = Some(Instant::now());
                        let value = serde_json::to_string(&PreviewControl::Play {
                            request_id: lease.id,
                        })
                        .map_err(|_| "Preview encoding failed")?;
                        tokio::time::timeout(
                            Duration::from_millis(500),
                            socket.send(Message::Text(value.into())),
                        )
                        .await
                        .map_err(|_| "Preview send expired")?
                        .map_err(|_| "Preview disconnected")?;
                    }
                    PreviewMessage::Ready {
                        sample_rate,
                        samples,
                    } if !synthesized
                        && opened.is_none()
                        && sample_rate == 24000
                        && (24000..=preview::MAX_SAMPLES).contains(&samples) =>
                    {
                        if let Some(span) = remote.take() {
                            span.finish(Outcome::Complete);
                        }
                        total = Some(samples);
                        opened = Some(Instant::now());
                        let value = serde_json::to_string(&PreviewControl::Play {
                            request_id: lease.id,
                        })
                        .map_err(|_| "Preview encoding failed")?;
                        tokio::time::timeout(
                            Duration::from_millis(500),
                            socket.send(Message::Text(value.into())),
                        )
                        .await
                        .map_err(|_| "Preview send expired")?
                        .map_err(|_| "Preview disconnected")?;
                    }
                    PreviewMessage::Audio {
                        sequence,
                        sample_offset,
                        samples,
                    } => {
                        let origin = opened.ok_or("Preview clock unavailable")?;
                        let ceiling = total.unwrap_or(preview::MAX_SAMPLES);
                        let now = Instant::now();
                        let due = Duration::from_secs_f64(sample_offset as f64 / 24000.0);
                        if sequence != expected
                            || sample_offset != offset
                            || samples.is_empty()
                            || samples.len() > 480
                            || offset + samples.len() as u64 > ceiling
                            || (samples.len() != 480
                                && (synthesized || Some(offset + samples.len() as u64) != total))
                            || now.duration_since(origin) >= Duration::from_secs(32)
                            || now.duration_since(origin) > due + Duration::from_millis(500)
                            || due > now.duration_since(origin) + Duration::from_millis(100)
                        {
                            return Err("Preview lost continuity or freshness".into());
                        }
                        if let Some(frame) = pending.take() {
                            lease.app.state::<Runtime>().media.output_frame(frame)?;
                        }
                        let mut pcm = [0i16; 480];
                        for (dst, src) in pcm.iter_mut().zip(&samples) {
                            *dst = *src;
                        }
                        pending = Some(PlaybackFrame {
                            epoch: lease.epoch,
                            utterance: lease.id,
                            sequence,
                            captured: (origin + due).min(now),
                            deadline: origin
                                + (Duration::from_secs_f64(ceiling as f64 / 24000.0)
                                    + Duration::from_secs(4))
                                .min(Duration::from_secs(32)),
                            device_time: None,
                            rate: PlaybackRate::Pcm24000,
                            samples: pcm,
                            valid_samples: samples.len(),
                            final_frame: false,
                        });
                        offset += samples.len() as u64;
                        expected += 1;
                    }
                    PreviewMessage::End { chunks, samples }
                        if opened.is_some()
                            && samples != 0
                            && samples <= preview::MAX_SAMPLES
                            && total.is_none_or(|expected| expected == samples)
                            && offset == samples
                            && chunks == expected - 1 =>
                    {
                        let mut frame = pending.take().ok_or("Preview ended without audio")?;
                        frame.final_frame = true;
                        lease.app.state::<Runtime>().media.output_frame(frame)?;
                        let finish = async {
                            loop {
                                lease.current(true)?;
                                if lease
                                    .app
                                    .state::<Runtime>()
                                    .media
                                    .output_state(lease.epoch, lease.id)?
                                    .1
                                {
                                    return Ok::<_, String>(());
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        };
                        performance::measure(
                            &performance,
                            operation,
                            Stage::FinalSubmission,
                            lease.id,
                            async {
                                tokio::time::timeout(Duration::from_millis(1500), finish)
                                    .await
                                    .map_err(|_| "Output did not confirm final submission")??;
                                Ok(())
                            },
                        )
                        .await?;
                        let value = serde_json::to_string(&PreviewControl::Submitted {
                            request_id: lease.id,
                            samples,
                        })
                        .map_err(|_| "Preview encoding failed")?;
                        tokio::time::timeout(
                            Duration::from_millis(500),
                            socket.send(Message::Text(value.into())),
                        )
                        .await
                        .map_err(|_| "Preview completion expired")?
                        .map_err(|_| "Preview disconnected")?;
                        // Retain the device after submission until the bounded
                        // scheduled-output estimate drains; controls still revoke it.
                        let drain = async {
                            loop {
                                lease.current(true)?;
                                if lease
                                    .app
                                    .state::<Runtime>()
                                    .media
                                    .output_state(lease.epoch, lease.id)?
                                    .2
                                {
                                    return Ok::<_, String>(());
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        };
                        performance::measure(
                            &performance,
                            operation,
                            Stage::EstimatedDrain,
                            lease.id,
                            async {
                                tokio::time::timeout(Duration::from_millis(3100), drain)
                                    .await
                                    .map_err(|_| "Output drain expired")??;
                                Ok(())
                            },
                        )
                        .await?;
                        return Ok(());
                    }
                    _ => return Err("Unexpected preview response".into()),
                }
            }
        }
        .await;
        // Stop local output first; explicit output invalidation also reaches the
        // control-session watch if socket closure races the server's private read.
        lease.stop();
        let _ = tokio::time::timeout(Duration::from_millis(500), socket.close(None)).await;
        result
    }
    .await;
    if let Some(span) = remote {
        span.finish(if observed.is_ok() {
            Outcome::Complete
        } else {
            Outcome::Failed
        });
    }
    observed
}
