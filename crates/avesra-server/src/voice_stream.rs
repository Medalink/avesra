//! Paired media ingress. This route never accepts intent or publishes history.
use super::{Shared, active, authenticate_headers, session_current};
use avesra_contracts::{
    ErrorCode,
    media::{AudioPacket, MediaSession},
};
use avesra_server::audio::{AudioClient, AudioInput, AudioOutput};
use axum::{
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    version: u16,
    session_id: Uuid,
    capture_epoch: u64,
    utterance_id: Uuid,
}
pub(super) async fn upgrade(
    auth: Shared,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let device = authenticate_headers(auth.clone(), &headers).await?;
    if auth.asr.is_none() || auth.speaker.is_none() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let connection = auth
        .connections
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let media = auth
        .speaker_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(ws
        .max_message_size(1024)
        .max_frame_size(1024)
        .write_buffer_size(1024)
        .max_write_buffer_size(32768)
        .on_upgrade(move |socket| session(socket, auth, device, connection, media)))
}
async fn send(socket: &mut WebSocket, value: serde_json::Value) -> Result<(), ErrorCode> {
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| ErrorCode::Expired)?
    .map_err(|_| ErrorCode::Unavailable)
}
async fn session(
    mut socket: WebSocket,
    auth: Shared,
    device: Uuid,
    _connection: OwnedSemaphorePermit,
    _media: OwnedSemaphorePermit,
) {
    // Bounds all retained media even when a final auth/storage check stalls.
    let _ = tokio::time::timeout(Duration::from_secs(25), run(&mut socket, auth, device)).await;
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Close(None)),
    )
    .await;
}
struct CancelSpeaker {
    client: Arc<AudioClient>,
    id: Uuid,
    complete: bool,
}
impl Drop for CancelSpeaker {
    fn drop(&mut self) {
        if !self.complete {
            let client = self.client.clone();
            let id = self.id;
            tokio::spawn(async move {
                let _ = client.cancel(id).await;
            });
        }
    }
}
async fn run(socket: &mut WebSocket, auth: Shared, device: Uuid) -> Result<(), ErrorCode> {
    let frame = tokio::time::timeout(Duration::from_secs(3), socket.recv())
        .await
        .map_err(|_| ErrorCode::Expired)?
        .ok_or(ErrorCode::Unavailable)?
        .map_err(|_| ErrorCode::Malformed)?;
    let Message::Text(text) = frame else {
        return Err(ErrorCode::Malformed);
    };
    let start: Start = serde_json::from_str(&text).map_err(|_| ErrorCode::Malformed)?;
    if start.version != 1
        || start.session_id.is_nil()
        || start.utterance_id.is_nil()
        || start.capture_epoch == 0
    {
        return Err(ErrorCode::Malformed);
    }
    let mut permission = {
        let mut sessions = auth.sessions.lock().map_err(|_| ErrorCode::Unavailable)?;
        let live = sessions
            .get_mut(&start.session_id)
            .ok_or(ErrorCode::Unauthenticated)?;
        if live.device != device
            || live.epoch != start.capture_epoch
            || !live.enabled
            || live.updated.elapsed() >= Duration::from_secs(30)
        {
            return Err(ErrorCode::Unauthenticated);
        }
        while live
            .seen
            .front()
            .is_some_and(|(_, created)| created.elapsed() > Duration::from_secs(61))
        {
            live.seen.pop_front();
        }
        if live.seen.len() >= 128 || live.seen.iter().any(|(id, _)| *id == start.utterance_id) {
            return Err(ErrorCode::Stale);
        }
        live.seen.push_back((start.utterance_id, Instant::now()));
        live.permission.subscribe()
    };
    let asr = auth.asr.as_ref().ok_or(ErrorCode::Unavailable)?;
    let speaker = auth.speaker.as_ref().ok_or(ErrorCode::Unavailable)?;
    // Worker session/epoch are private lane identity, not the paired PC identity.
    let mut stream = tokio::select! {
        biased;
        _=permission.changed()=>return Err(ErrorCode::Stale),
        result=asr.begin_stream(1,start.utterance_id)=>result?,
    };
    if !session_current(&auth, device, start.session_id, start.capture_epoch)
        || !active(auth.clone(), device).await
        || !session_current(&auth, device, start.session_id, start.capture_epoch)
    {
        return Err(ErrorCode::Stale);
    }
    let mut media = MediaSession::new(device, start.session_id, start.capture_epoch)?;
    media.enabled = true;
    // Native capture begins only after this acknowledgement. The first packet
    // must arrive within500ms; subsequent capture offsets share this same anchor.
    let opened = Instant::now();
    send(socket,serde_json::json!({"version":1,"session_id":start.session_id,"capture_epoch":start.capture_epoch,"utterance_id":start.utterance_id,"status":"capture_window_open","max_samples":160000,"accepted_turn":false})).await?;
    let mut raw = Vec::with_capacity(320_000);
    let mut last_auth = Instant::now();
    loop {
        if !session_current(&auth, device, start.session_id, start.capture_epoch) {
            return Err(ErrorCode::Stale);
        }
        let frame = tokio::select! {
            biased;
            _=permission.changed()=>return Err(ErrorCode::Stale),
            frame=tokio::time::timeout(Duration::from_millis(500),socket.recv())=>frame.map_err(|_|ErrorCode::Expired)?.ok_or(ErrorCode::Unavailable)?.map_err(|_|ErrorCode::Malformed)?,
        };
        let Message::Binary(bytes) = frame else {
            return Err(ErrorCode::Malformed);
        };
        let packet = AudioPacket::decode(&bytes)?;
        drop(bytes);
        if packet.utterance_id != start.utterance_id
            || packet.sample_offset + packet.pcm.len() as u64 / 2 > 160_000
        {
            return Err(ErrorCode::Malformed);
        }
        media.accept(&packet)?;
        let capture_offset =
            Duration::from_micros(packet.sample_offset.saturating_mul(1_000_000) / 16_000);
        let elapsed = opened.elapsed();
        if elapsed > capture_offset + Duration::from_millis(500)
            || capture_offset > elapsed + Duration::from_millis(100)
        {
            return Err(ErrorCode::Expired);
        }
        if last_auth.elapsed() > Duration::from_secs(1) {
            if !active(auth.clone(), device).await
                || !session_current(&auth, device, start.session_id, start.capture_epoch)
            {
                return Err(ErrorCode::Stale);
            }
            last_auth = Instant::now();
        }
        raw.extend_from_slice(&packet.pcm);
        let samples: Vec<i16> = packet
            .pcm
            .chunks_exact(2)
            .map(|s| i16::from_le_bytes([s[0], s[1]]))
            .collect();
        let captured = opened + capture_offset;
        let captured = captured.min(Instant::now());
        let result = tokio::select! {
            biased;
            _=permission.changed()=>return Err(ErrorCode::Stale),
            result=stream.push(&samples,packet.sequence,captured,packet.end)=>result?,
        };
        if !session_current(&auth, device, start.session_id, start.capture_epoch) {
            return Err(ErrorCode::Stale);
        }
        if !packet.end {
            // No unknown-speaker transcript crosses to UI/history while partial.
            send(socket,serde_json::json!({"version":1,"session_id":start.session_id,"capture_epoch":start.capture_epoch,"utterance_id":start.utterance_id,"sequence":packet.sequence,"status":"processing","accepted_turn":false})).await?;
            continue;
        }
        let worker = Uuid::new_v4();
        let mut cancel = CancelSpeaker {
            client: speaker.clone(),
            id: worker,
            complete: false,
        };
        let pcm_s16le = STANDARD.encode(&raw);
        drop(raw);
        let identity = tokio::select! {
            biased;
            _=permission.changed()=>return Err(ErrorCode::Stale),
            value=speaker.infer_with_budget(worker,1,start.utterance_id,AudioInput::Pcm {pcm_s16le},Duration::from_secs(10))=>value?,
        };
        cancel.complete = true;
        if !session_current(&auth, device, start.session_id, start.capture_epoch)
            || !active(auth.clone(), device).await
            || !session_current(&auth, device, start.session_id, start.capture_epoch)
        {
            return Err(ErrorCode::Stale);
        }
        let embedding = match identity.output {
            AudioOutput::Embedding { embedding, .. } => Some(embedding),
            AudioOutput::Insufficient { .. } => None,
            _ => return Err(ErrorCode::Malformed),
        };
        return send(socket,serde_json::json!({"version":1,"session_id":start.session_id,"capture_epoch":start.capture_epoch,"utterance_id":start.utterance_id,"sequence":packet.sequence,"asr_revision":asr.configured_revision(),"speaker_revision":speaker.configured_revision(),"transcript":result.text,"embedding":embedding,"outcome":"abstain","reason":"owner_overlap_directness_qualification_required","accepted_turn":false})).await;
    }
}
