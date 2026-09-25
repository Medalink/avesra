//! Explicit Settings reference preview; independent of microphone and identity-management proof lifetime.
use crate::{
    Runtime,
    connection::{self, MediaEndpoint, PairingRecord, SessionIdentity},
};
use avesra_contracts::{
    preview::{PreviewControl, PreviewEvent, PreviewMessage, PreviewRequest},
    voice::VoiceIdentity,
};
use avesra_windows::audio::{PlaybackFrame, PlaybackRate};
use futures_util::{SinkExt, StreamExt};
use std::{
    sync::atomic::Ordering,
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
    panel: Uuid,
}
impl Lease {
    fn current(&self, acknowledged: bool) -> Result<(), String> {
        let state = self.app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !state
            .voice_panel
            .lock()
            .is_ok_and(|value| *value == Some(self.panel))
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
    let state = app.state::<Runtime>();
    let _slot = state
        .preview
        .try_lock()
        .map_err(|_| "A preview is already active")?;
    let (lease, gain) = {
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
        state.media.open_preview(&local, id)?;
        let lease = Lease {
            app: app.clone(),
            session,
            epoch: local.playback_epoch,
            id,
            panel,
        };
        let _ = app.emit("runtime-state", local.clone());
        (lease, f32::from(local.settings.speech_volume) / 100.0)
    };
    // Device opening and current output acknowledgment precede any Play request.
    let ready = async {
        loop {
            lease.current(false)?;
            if lease.current(true).is_ok() && state.media.preview_state(lease.epoch, lease.id)?.0 {
                return Ok::<_, String>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
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
    let mut modes = state.modes.subscribe();
    let monitor = async {
        loop {
            if modes.changed().await.is_err() || lease.current(true).is_err() {
                return;
            }
        }
    };
    let result = tokio::select! {
        biased;
        _=monitor=>Err("Preview permission changed".into()),
        value=tokio::time::timeout(Duration::from_secs(65),play(&lease,&record,voice,gain))=>value.map_err(|_|"Preview expired".to_string())?,
    };
    lease.stop();
    result.map(|_| "Final preview samples submitted to the output device".into())
}
async fn play(
    lease: &Lease,
    record: &PairingRecord,
    voice: VoiceIdentity,
    gain: f32,
) -> Result<(), String> {
    let mut socket = connection::voice_socket(record, MediaEndpoint::Preview).await?;
    lease.current(true)?;
    let request = PreviewRequest {
        version: 1,
        session_id: lease.session.id,
        playback_epoch: lease.epoch,
        request_id: lease.id,
        voice,
    };
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
            let timeout = if total.is_none() {
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
            if event.context.version != 1
                || event.context.session_id != request.session_id
                || event.context.playback_epoch != request.playback_epoch
                || event.context.request_id != request.request_id
                || event.context.voice != request.voice
            {
                return Err("Stale preview response".into());
            }
            match event.message {
                PreviewMessage::Ready {
                    sample_rate,
                    samples,
                } if total.is_none()
                    && sample_rate == 24000
                    && (24000..=720000).contains(&samples) =>
                {
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
                    let total = total.ok_or("Preview audio before Ready")?;
                    let origin = opened.ok_or("Preview clock unavailable")?;
                    let now = Instant::now();
                    let due = Duration::from_secs_f64(sample_offset as f64 / 24000.0);
                    if sequence != expected
                        || sample_offset != offset
                        || samples.is_empty()
                        || samples.len() > 480
                        || offset + samples.len() as u64 > total
                        || (samples.len() != 480 && offset + samples.len() as u64 != total)
                        || now.duration_since(origin) > due + Duration::from_millis(500)
                        || due > now.duration_since(origin) + Duration::from_millis(100)
                    {
                        return Err("Preview lost continuity or freshness".into());
                    }
                    if let Some(frame) = pending.take() {
                        lease.app.state::<Runtime>().media.preview_frame(frame)?;
                    }
                    let mut pcm = [0i16; 480];
                    for (dst, src) in pcm.iter_mut().zip(&samples) {
                        *dst = (f32::from(*src) * gain).round() as i16;
                    }
                    pending = Some(PlaybackFrame {
                        epoch: lease.epoch,
                        utterance: lease.id,
                        sequence,
                        captured: (origin + due).min(now),
                        deadline: origin
                            + Duration::from_secs_f64(total as f64 / 24000.0)
                            + Duration::from_secs(2),
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
                    if total == Some(samples) && offset == samples && chunks == expected - 1 =>
                {
                    let mut frame = pending.take().ok_or("Preview ended without audio")?;
                    frame.final_frame = true;
                    lease.app.state::<Runtime>().media.preview_frame(frame)?;
                    let finish = async {
                        loop {
                            lease.current(true)?;
                            if lease
                                .app
                                .state::<Runtime>()
                                .media
                                .preview_state(lease.epoch, lease.id)?
                                .1
                            {
                                return Ok::<_, String>(());
                            }
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    };
                    tokio::time::timeout(Duration::from_millis(1500), finish)
                        .await
                        .map_err(|_| "Output did not confirm final submission")??;
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
                                .preview_state(lease.epoch, lease.id)?
                                .2
                            {
                                return Ok::<_, String>(());
                            }
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    };
                    tokio::time::timeout(Duration::from_millis(1200), drain)
                        .await
                        .map_err(|_| "Output drain expired")??;
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
