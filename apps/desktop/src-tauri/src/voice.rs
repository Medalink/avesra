//! Native media producer. Qualified readiness remains mandatory and defaults off.
use crate::{
    Runtime,
    connection::{PairingRecord, SessionIdentity},
};
use avesra_contracts::media::AudioPacket;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

pub struct Worker(tauri::async_runtime::JoinHandle<()>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub fn spawn(app: tauri::AppHandle, record: PairingRecord, generation: u64) -> Worker {
    Worker(tauri::async_runtime::spawn(async move {
        let mut attempted_epoch = None;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let state = app.state::<Runtime>();
            if state.connection_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let session = {
                let Ok(local) = state.local.lock() else {
                    return;
                };
                if !local.capture_allowed() || !local.voice_ready || local.enrollment_capture {
                    continue;
                }
                let Ok(session) = state.acknowledged_session.lock() else {
                    return;
                };
                session.filter(|value| {
                    value.generation == generation && value.epoch == local.capture_epoch
                })
            };
            let Some(session) = session else {
                continue;
            };
            // No continuous ambient windows without a qualified turn segmenter.
            // A fresh native readiness/epoch transition must admit each attempt.
            if attempted_epoch == Some(session.epoch) {
                continue;
            }
            attempted_epoch = Some(session.epoch);
            if window(&app, &record, session).await.is_err() {
                let state = app.state::<Runtime>();
                if let Ok(mut local) = state.local.lock()
                    && state.connection_generation.load(Ordering::SeqCst) == generation
                    && local.capture_epoch == session.epoch
                {
                    local.voice_ready = false;
                    local.capture_epoch = local.capture_epoch.saturating_add(1);
                    local.action_epoch = local.action_epoch.saturating_add(1);
                    local.refresh();
                    state.publish(&local);
                    let _ = app.emit("runtime-state", local.clone());
                    let _ = app.emit("runtime-error", "Voice media unavailable; listening stopped. Review service readiness before retrying.");
                }
            }
        }
    }))
}
fn current(app: &tauri::AppHandle, expected: SessionIdentity) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let session = state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?;
    if state.connection_generation.load(Ordering::SeqCst) != expected.generation
        || local.capture_epoch != expected.epoch
        || !local.capture_allowed()
        || !local.voice_ready
        || local.enrollment_capture
        || session.is_none_or(|value| {
            value.id != expected.id
                || value.epoch != expected.epoch
                || value.generation != expected.generation
        })
    {
        return Err("Voice media context changed".into());
    }
    Ok(())
}
async fn invalidated(
    app: &tauri::AppHandle,
    session: SessionIdentity,
    modes: &mut tokio::sync::watch::Receiver<crate::ModeSnapshot>,
) {
    loop {
        if modes.changed().await.is_err() || current(app, session).is_err() {
            return;
        }
    }
}
struct CaptureWindow {
    app: tauri::AppHandle,
    epoch: u64,
}
impl Drop for CaptureWindow {
    fn drop(&mut self) {
        self.app
            .state::<Runtime>()
            .media
            .close_voice_window(self.epoch);
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ack {
    version: u16,
    session_id: Uuid,
    capture_epoch: u64,
    utterance_id: Uuid,
    status: String,
    max_samples: u64,
    accepted_turn: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Processing {
    version: u16,
    session_id: Uuid,
    capture_epoch: u64,
    utterance_id: Uuid,
    sequence: u64,
    status: String,
    accepted_turn: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Abstain {
    version: u16,
    session_id: Uuid,
    capture_epoch: u64,
    utterance_id: Uuid,
    sequence: u64,
    asr_revision: String,
    speaker_revision: String,
    transcript: String,
    embedding: Option<Vec<f32>>,
    outcome: String,
    reason: String,
    accepted_turn: bool,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Reply {
    Processing(Processing),
    Abstain(Abstain),
}
async fn window(
    app: &tauri::AppHandle,
    record: &PairingRecord,
    session: SessionIdentity,
) -> Result<(), String> {
    current(app, session)?;
    let mut modes = app.state::<Runtime>().modes.subscribe();
    let mut socket = tokio::select! {
        biased;
        _=invalidated(app,session,&mut modes)=>return Err("Voice permission changed".into()),
        value=crate::connection::voice_socket(record)=>value?,
    };
    current(app, session)?;
    let utterance = Uuid::new_v4();
    let start = serde_json::json!({"version":1,"session_id":session.id,"capture_epoch":session.epoch,"utterance_id":utterance});
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(start.to_string().into())),
    )
    .await
    .map_err(|_| "Voice send timed out")?
    .map_err(|_| "Voice transport unavailable")?;
    let reply = tokio::select! {
        biased;
        _=invalidated(app,session,&mut modes)=>return Err("Voice permission changed".into()),
        value=tokio::time::timeout(Duration::from_secs(3),socket.next())=>value.map_err(|_| "Voice handshake timed out")?.ok_or("Voice disconnected")?.map_err(|_| "Voice transport unavailable")?,
    };
    let Message::Text(text) = reply else {
        return Err("Invalid capture acknowledgement".into());
    };
    let ack: Ack = serde_json::from_str(&text).map_err(|_| "Invalid capture acknowledgement")?;
    if ack.version != 1
        || ack.session_id != session.id
        || ack.capture_epoch != session.epoch
        || ack.utterance_id != utterance
        || ack.status != "capture_window_open"
        || ack.max_samples != 160_000
        || ack.accepted_turn
    {
        return Err("Capture acknowledgement context mismatch".into());
    }
    current(app, session)?;
    let opened = Instant::now();
    {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.capture_epoch != session.epoch
            || state.connection_generation.load(Ordering::SeqCst) != session.generation
        {
            return Err("Capture epoch changed".into());
        }
        state.media.open_voice_window(&local)?;
    }
    let capture = CaptureWindow {
        app: app.clone(),
        epoch: session.epoch,
    };
    let mut capture = Some(capture);
    let (mut writer, mut reader) = socket.split();
    let mut tick = tokio::time::interval(Duration::from_millis(10));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sent = 0;
    let mut received = 0;
    let mut last_reply = Instant::now();
    loop {
        current(app, session)?;
        if opened.elapsed() >= Duration::from_secs(25)
            || last_reply.elapsed() > Duration::from_secs(12)
        {
            return Err("Voice window expired".into());
        }
        tokio::select! {
            biased;
            _=invalidated(app,session,&mut modes)=>return Err("Voice permission changed".into()),
            reply=reader.next()=>{
                let message=reply.ok_or("Voice disconnected")?.map_err(|_| "Voice response unavailable")?;
                let Message::Text(text)=message else {return Err("Unexpected voice response".into());};
                let reply:Reply=serde_json::from_str(&text).map_err(|_| "Invalid voice response")?;
                current(app,session)?;
                match reply {
                    Reply::Processing(value)=>{
                        if value.version!=1||value.session_id!=session.id||value.capture_epoch!=session.epoch||value.utterance_id!=utterance||value.sequence!=received+1||value.sequence>sent||value.status!="processing"||value.accepted_turn {return Err("Stale voice response".into());}
                        received=value.sequence;
                    }
                    Reply::Abstain(value)=>{
                        if sent!=500||value.version!=1||value.session_id!=session.id||value.capture_epoch!=session.epoch||value.utterance_id!=utterance||value.sequence!=sent||value.sequence!=received+1||value.asr_revision!="ebe59e5a817142986528bbbee5dba8db7b38ed50"||value.speaker_revision!="0f99f2d0ebe89ac095bcc5903c4dd8f72b367286"||value.transcript.len()>8192||value.embedding.as_ref().is_some_and(|v|v.len()!=192||v.iter().any(|n|!n.is_finite()))||value.outcome!="abstain"||value.reason!="owner_overlap_directness_qualification_required"||value.accepted_turn {return Err("Invalid final voice analysis".into());}
                        // Unknown/unaccepted speech is neither UI content nor durable history.
                        drop(value);
                        return Ok(());
                    }
                }
                last_reply=Instant::now();
            }
            _=tick.tick(), if sent<500=>{
                let frame=app.state::<Runtime>().media.take_capture_frame(session.epoch)?;
                let Some(frame)=frame else {if opened.elapsed()>Duration::from_millis(sent*20+500){return Err("Capture frame late".into());} continue;};
                if frame.epoch!=session.epoch||frame.sequence!=sent+1||frame.captured<opened||frame.captured>Instant::now()||frame.captured.elapsed()>Duration::from_millis(500){return Err("Capture sequence or freshness lost".into());}
                let packet=AudioPacket{device_id:record.device_id,session_id:session.id,utterance_id:utterance,capture_epoch:session.epoch,sequence:frame.sequence,sample_offset:sent*320,start:sent==0,end:sent==499,pcm:frame.samples.iter().flat_map(|sample|sample.to_le_bytes()).collect()};
                current(app,session)?;
                tokio::time::timeout(Duration::from_millis(500),writer.send(Message::Binary(packet.encode().map_err(|_| "Invalid native frame")?.into()))).await.map_err(|_| "Voice media send expired")?.map_err(|_| "Voice media send failed")?;
                sent+=1;
                if sent==500 {capture.take();}
            }
            _=tokio::time::sleep(Duration::from_millis(20)), if sent==500=>{}
        }
    }
}
