//! Explicit voice preview output. Never selects a voice or starts microphone input.
use super::{Shared, active, authenticate_headers, voice_setup};
use avesra_contracts::{
    ErrorCode,
    preview::{self, PreviewControl, PreviewEvent, PreviewMessage, PreviewRequest},
};
use axum::{
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, mpsc};
use uuid::Uuid;

pub(super) async fn upgrade(
    auth: Shared,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    greeting: bool,
) -> Result<Response, StatusCode> {
    let device = authenticate_headers(auth.clone(), &headers).await?;
    if auth.tts.is_none() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let connection = auth
        .connections
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let admission = auth
        .voice_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(ws
        .max_message_size(2048)
        .max_frame_size(2048)
        .write_buffer_size(1024)
        .max_write_buffer_size(32768)
        .on_upgrade(move |socket| session(socket, auth, device, connection, admission, greeting)))
}
async fn session(
    mut socket: WebSocket,
    auth: Shared,
    device: Uuid,
    _connection: OwnedSemaphorePermit,
    _admission: OwnedSemaphorePermit,
    greeting: bool,
) {
    let _ = tokio::time::timeout(
        Duration::from_secs(70),
        start(&mut socket, auth, device, greeting),
    )
    .await;
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Close(None)),
    )
    .await;
}
async fn read(socket: &mut WebSocket, deadline: Duration) -> Result<String, ErrorCode> {
    match tokio::time::timeout(deadline, socket.recv())
        .await
        .map_err(|_| ErrorCode::Expired)?
        .ok_or(ErrorCode::Unavailable)?
        .map_err(|_| ErrorCode::Malformed)?
    {
        Message::Text(text) => Ok(text.to_string()),
        _ => Err(ErrorCode::Unavailable),
    }
}
async fn send(
    socket: &mut WebSocket,
    request: &PreviewRequest,
    message: PreviewMessage,
) -> Result<(), ErrorCode> {
    let value = serde_json::to_string(&PreviewEvent {
        context: request.clone(),
        message,
    })
    .map_err(|_| ErrorCode::Malformed)?;
    if value.len() > 8192 {
        return Err(ErrorCode::TooLarge);
    }
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(value.into())),
    )
    .await
    .map_err(|_| ErrorCode::Expired)?
    .map_err(|_| ErrorCode::Unavailable)
}
async fn start(
    socket: &mut WebSocket,
    auth: Shared,
    device: Uuid,
    greeting: bool,
) -> Result<(), ErrorCode> {
    let request: PreviewRequest =
        serde_json::from_str(&read(socket, Duration::from_secs(3)).await?)
            .map_err(|_| ErrorCode::Malformed)?;
    request.validate()?;
    if request.greeting.is_some() != greeting {
        return Err(ErrorCode::Denied);
    }
    let mut permission = voice_setup::admit(
        &auth,
        device,
        request.session_id,
        request.playback_epoch,
        request.request_id,
        true,
    )
    .map_err(|_| ErrorCode::Denied)?;
    let monitor = async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !voice_setup::current(
                &auth,
                device,
                request.session_id,
                request.playback_epoch,
                true,
            ) || !active(auth.clone(), device).await
            {
                return;
            }
        }
    };
    tokio::select! {
        biased;
        _=permission.changed()=>Err(ErrorCode::Stale),
        _=monitor=>Err(ErrorCode::Stale),
        result=stream(socket,&auth,device,&request)=>result,
    }
}
async fn stream(
    socket: &mut WebSocket,
    auth: &Shared,
    device: Uuid,
    request: &PreviewRequest,
) -> Result<(), ErrorCode> {
    if request.greeting.is_some() || request.test_text.is_some() {
        return synthesized_stream(socket, auth, device, request).await;
    }
    let lane = auth.tts.as_ref().ok_or(ErrorCode::Unavailable)?;
    let pcm = tokio::select! {
        biased;
        _=socket.recv()=>return Err(ErrorCode::Stale),
        value=tokio::time::timeout(Duration::from_secs(31),lane.preview_voice(1,&request.voice))=>value.map_err(|_|ErrorCode::Expired)??,
    };
    let samples = pcm.len() as u64;
    send(
        socket,
        request,
        PreviewMessage::Ready {
            sample_rate: 24000,
            samples,
        },
    )
    .await?;
    let control: PreviewControl =
        serde_json::from_str(&read(socket, Duration::from_secs(3)).await?)
            .map_err(|_| ErrorCode::Malformed)?;
    if !matches!(control,PreviewControl::Play{request_id} if request_id==request.request_id) {
        return Err(ErrorCode::Stale);
    }
    let started = tokio::time::Instant::now();
    let mut chunks = 0;
    for (index, chunk) in pcm.chunks(480).enumerate() {
        let due = started + Duration::from_millis(index as u64 * 20);
        tokio::select! { biased; _=socket.recv()=>return Err(ErrorCode::Stale), _=tokio::time::sleep_until(due)=>{} }
        if tokio::time::Instant::now().saturating_duration_since(due) > Duration::from_millis(100) {
            return Err(ErrorCode::Expired);
        }
        chunks = index as u64 + 1;
        send(
            socket,
            request,
            PreviewMessage::Audio {
                sequence: chunks,
                sample_offset: index as u64 * 480,
                samples: chunk.to_vec(),
            },
        )
        .await?;
    }
    drop(pcm);
    send(socket, request, PreviewMessage::End { chunks, samples }).await?;
    let control: PreviewControl =
        serde_json::from_str(&read(socket, Duration::from_secs(2)).await?)
            .map_err(|_| ErrorCode::Malformed)?;
    if !matches!(control,PreviewControl::Submitted{request_id,samples:count} if request_id==request.request_id&&count==samples)
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}

enum Piece {
    Audio(Vec<i16>),
    Complete { samples: usize },
}

async fn produce(
    auth: &Shared,
    device: Uuid,
    request: &PreviewRequest,
    tx: mpsc::Sender<Piece>,
) -> Result<(), ErrorCode> {
    use avesra_server::audio::synthesis::{Completion, SpeechEvent};
    let text = match (&request.greeting, &request.test_text) {
        (Some(greeting), None) => greeting.text()?,
        (None, Some(text)) => text.clone(),
        _ => return Err(ErrorCode::Malformed),
    };
    let lane = auth.tts.as_ref().ok_or(ErrorCode::Unavailable)?;
    let status = lane.voice_status(1).await?;
    if status.selection_state != "available"
        || status.active_state != "available"
        || status.selected.as_ref() != Some(&request.voice)
        || status.active_voice.as_ref() != Some(&request.voice)
    {
        return Err(ErrorCode::Unavailable);
    }
    let mut stream = lane
        .synthesize(
            1,
            request.request_id,
            &request.voice,
            &text,
            None,
            || async {
                if active(auth.clone(), device).await
                    && voice_setup::current(
                        auth,
                        device,
                        request.session_id,
                        request.playback_epoch,
                        true,
                    )
                {
                    Ok(())
                } else {
                    Err(ErrorCode::Stale)
                }
            },
        )
        .await?;
    let mut received = 0usize;
    loop {
        match stream.next().await? {
            SpeechEvent::Audio { samples, .. } => {
                if samples.len() != 1920 || received + samples.len() > preview::MAX_SAMPLES as usize
                {
                    return Err(ErrorCode::TooLarge);
                }
                received += samples.len();
                tx.send(Piece::Audio(samples))
                    .await
                    .map_err(|_| ErrorCode::Stale)?;
            }
            SpeechEvent::End {
                outcome: Completion::Complete,
                samples,
                ..
            } if samples == received && received != 0 => {
                tx.send(Piece::Complete { samples })
                    .await
                    .map_err(|_| ErrorCode::Stale)?;
                return Ok(());
            }
            _ => return Err(ErrorCode::Unavailable),
        }
    }
}

async fn synthesized_stream(
    socket: &mut WebSocket,
    auth: &Shared,
    device: Uuid,
    request: &PreviewRequest,
) -> Result<(), ErrorCode> {
    // Includes status, private preparation and blocked queue sends. Neither
    // backpressure nor a later private start renews this original budget.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    // 64 * 1920 samples = 245760 PCM bytes, with one producer chunk and
    // one consumer chunk plus its held final frame outside the queue.
    let (tx, rx) = mpsc::channel(64);
    let producer = async {
        tokio::time::timeout_at(deadline, produce(auth, device, request, tx))
            .await
            .map_err(|_| ErrorCode::Expired)?
    };
    // No detached producer: either branch's failure drops the private stream,
    // whose Drop retains its actual lane permit through cancellation settlement.
    tokio::try_join!(producer, consume(socket, request, rx))?;
    Ok(())
}

async fn next(socket: &mut WebSocket, rx: &mut mpsc::Receiver<Piece>) -> Result<Piece, ErrorCode> {
    tokio::select! { biased;
        _=socket.recv()=>Err(ErrorCode::Stale),
        value=rx.recv()=>value.ok_or(ErrorCode::Unavailable),
    }
}

async fn packet(
    socket: &mut WebSocket,
    request: &PreviewRequest,
    origin: tokio::time::Instant,
    sequence: u64,
    samples: Vec<i16>,
    last_sent: &mut Option<tokio::time::Instant>,
) -> Result<(), ErrorCode> {
    let due = origin + Duration::from_millis((sequence - 1) * 20);
    let send_at = last_sent.map_or(due, |last| due.max(last + Duration::from_millis(10)));
    tokio::select! { biased;
        _=socket.recv()=>return Err(ErrorCode::Stale),
        _=tokio::time::sleep_until(send_at)=>{},
    }
    if tokio::time::Instant::now().saturating_duration_since(due) > Duration::from_millis(100) {
        return Err(ErrorCode::Expired);
    }
    *last_sent = Some(tokio::time::Instant::now());
    send(
        socket,
        request,
        PreviewMessage::Audio {
            sequence,
            sample_offset: (sequence - 1) * preview::FRAME_SAMPLES,
            samples,
        },
    )
    .await
}

async fn consume(
    socket: &mut WebSocket,
    request: &PreviewRequest,
    mut rx: mpsc::Receiver<Piece>,
) -> Result<(), ErrorCode> {
    let Piece::Audio(first) = next(socket, &mut rx).await? else {
        return Err(ErrorCode::Malformed);
    };
    if first.len() != 1920 {
        return Err(ErrorCode::Malformed);
    }
    send(
        socket,
        request,
        PreviewMessage::StreamingReady {
            sample_rate: 24000,
            frame_samples: preview::FRAME_SAMPLES,
            max_samples: preview::MAX_SAMPLES,
        },
    )
    .await?;
    let control: PreviewControl =
        serde_json::from_str(&read(socket, Duration::from_secs(3)).await?)
            .map_err(|_| ErrorCode::Malformed)?;
    if !matches!(control, PreviewControl::Play { request_id } if request_id == request.request_id) {
        return Err(ErrorCode::Stale);
    }
    let origin = tokio::time::Instant::now();
    let output = async {
        let mut incoming = Some(first);
        let mut held = None;
        let mut received = 0usize;
        let mut chunks = 0u64;
        let mut last_sent = None;
        loop {
            if let Some(samples) = incoming.take() {
                if samples.len() != 1920 || received + samples.len() > preview::MAX_SAMPLES as usize
                {
                    return Err(ErrorCode::TooLarge);
                }
                received += samples.len();
                for frame in samples.chunks_exact(preview::FRAME_SAMPLES as usize) {
                    if let Some(previous) = held.replace(frame.to_vec()) {
                        chunks += 1;
                        packet(socket, request, origin, chunks, previous, &mut last_sent).await?;
                    }
                }
            }
            match next(socket, &mut rx).await? {
                Piece::Audio(samples) => incoming = Some(samples),
                Piece::Complete { samples } => {
                    if samples != received {
                        return Err(ErrorCode::Malformed);
                    }
                    let last = held.take().ok_or(ErrorCode::Malformed)?;
                    chunks += 1;
                    packet(socket, request, origin, chunks, last, &mut last_sent).await?;
                    send(
                        socket,
                        request,
                        PreviewMessage::End {
                            chunks,
                            samples: samples as u64,
                        },
                    )
                    .await?;
                    let control: PreviewControl =
                        serde_json::from_str(&read(socket, Duration::from_secs(2)).await?)
                            .map_err(|_| ErrorCode::Malformed)?;
                    if !matches!(control, PreviewControl::Submitted { request_id, samples: count }
                        if request_id == request.request_id && count == samples as u64)
                    {
                        return Err(ErrorCode::Stale);
                    }
                    return Ok(());
                }
            }
        }
    };
    tokio::time::timeout_at(origin + Duration::from_secs(32), output)
        .await
        .map_err(|_| ErrorCode::Expired)?
}
