//! Explicit paired activity observations, with no accepted-turn authority.
use super::{Shared, active, authenticate_headers, session_current};
use avesra_contracts::{
    ErrorCode,
    activity::{Acknowledgment, Packet, REVISION, Start, StreamReply},
};
use axum::{
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) async fn upgrade(
    auth: Shared,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let device = authenticate_headers(auth.clone(), &headers).await?;
    if auth.activity.is_none() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let connection = auth
        .connections
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(ws
        .max_message_size(10000)
        .max_frame_size(10000)
        .write_buffer_size(1024)
        .max_write_buffer_size(32768)
        .on_upgrade(move |mut socket| async move {
            let _connection = connection;
            let _ =
                tokio::time::timeout(Duration::from_secs(25), run(&mut socket, auth, device)).await;
            let _ = tokio::time::timeout(
                Duration::from_millis(500),
                socket.send(Message::Close(None)),
            )
            .await;
        }))
}
async fn send(socket: &mut WebSocket, value: impl serde::Serialize) -> Result<(), ErrorCode> {
    let text = serde_json::to_string(&value).map_err(|_| ErrorCode::Malformed)?;
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(text.into())),
    )
    .await
    .map_err(|_| ErrorCode::Expired)?
    .map_err(|_| ErrorCode::Unavailable)
}
async fn run(socket: &mut WebSocket, auth: Shared, device: Uuid) -> Result<(), ErrorCode> {
    let first = tokio::time::timeout(Duration::from_secs(3), socket.recv())
        .await
        .map_err(|_| ErrorCode::Expired)?
        .ok_or(ErrorCode::Unavailable)?
        .map_err(|_| ErrorCode::Malformed)?;
    let Message::Text(text) = first else {
        return Err(ErrorCode::Malformed);
    };
    let start: Start = serde_json::from_str(&text).map_err(|_| ErrorCode::Malformed)?;
    if start.version != 1
        || start.request_id.is_nil()
        || start.session_id.is_nil()
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
            .is_some_and(|(_, when)| when.elapsed() > Duration::from_secs(61))
        {
            live.seen.pop_front();
        }
        if live.seen.len() >= 128 || live.seen.iter().any(|(id, _)| *id == start.request_id) {
            return Err(ErrorCode::Stale);
        }
        live.seen.push_back((start.request_id, Instant::now()));
        live.permission.subscribe()
    };
    let client = auth.activity.as_ref().ok_or(ErrorCode::Unavailable)?;
    let mut stream = tokio::select! { biased; _=permission.changed()=>return Err(ErrorCode::Stale), value=client.begin_activity_stream(1,start.request_id)=>value? };
    if !active(auth.clone(), device).await
        || !session_current(&auth, device, start.session_id, start.capture_epoch)
    {
        return Err(ErrorCode::Stale);
    }
    send(
        socket,
        Acknowledgment {
            version: 1,
            session_id: start.session_id,
            capture_epoch: start.capture_epoch,
            request_id: start.request_id,
            model_revision: REVISION.into(),
        },
    )
    .await?;
    let opened = Instant::now();
    let mut next = 1;
    let mut total = 0u32;
    loop {
        if !session_current(&auth, device, start.session_id, start.capture_epoch) {
            return Err(ErrorCode::Stale);
        }
        let frame = tokio::select! { biased; _=permission.changed()=>return Err(ErrorCode::Stale), value=tokio::time::timeout(Duration::from_millis(750),socket.recv())=>value.map_err(|_|ErrorCode::Expired)?.ok_or(ErrorCode::Unavailable)?.map_err(|_|ErrorCode::Malformed)? };
        let Message::Text(text) = frame else {
            return Err(ErrorCode::Malformed);
        };
        let packet: Packet = serde_json::from_str(&text).map_err(|_| ErrorCode::Malformed)?;
        if packet.sequence != next || packet.sample_offset != total || packet.pcm_s16le.len() > 8536
        {
            return Err(ErrorCode::Malformed);
        }
        let raw = STANDARD
            .decode(&packet.pcm_s16le)
            .map_err(|_| ErrorCode::Malformed)?;
        if raw.is_empty()
            || raw.len() > 6400
            || !raw.len().is_multiple_of(320)
            || (!packet.r#final && raw.len() != 6400)
        {
            return Err(ErrorCode::Malformed);
        }
        let count = (raw.len() / 2) as u32;
        total = total
            .checked_add(count)
            .filter(|v| *v <= 160000)
            .ok_or(ErrorCode::TooLarge)?;
        let capture_end = Duration::from_micros(u64::from(total) * 1_000_000 / 16000);
        let capture_start =
            Duration::from_micros(u64::from(packet.sample_offset) * 1_000_000 / 16000);
        // The acknowledged stream clock is a conservative estimate; preserve
        // oldest-sample age rather than renewing it at the chunk's end.
        if opened.elapsed() > capture_start + Duration::from_millis(500)
            || capture_end > opened.elapsed() + Duration::from_millis(100)
        {
            return Err(ErrorCode::Expired);
        }
        let samples: Vec<i16> = raw
            .chunks_exact(2)
            .map(|v| i16::from_le_bytes([v[0], v[1]]))
            .collect();
        let captured = (opened + capture_start).min(Instant::now());
        let observation = tokio::select! { biased; _=permission.changed()=>return Err(ErrorCode::Stale), value=stream.push(&samples,next,captured,packet.r#final)=>value? };
        if !active(auth.clone(), device).await
            || !session_current(&auth, device, start.session_id, start.capture_epoch)
        {
            return Err(ErrorCode::Stale);
        }
        send(
            socket,
            StreamReply {
                version: 1,
                session_id: start.session_id,
                capture_epoch: start.capture_epoch,
                request_id: start.request_id,
                sequence: next,
                observation,
            },
        )
        .await?;
        if packet.r#final {
            return Ok(());
        }
        next += 1;
    }
}
