//! Explicit reference-asset output. Never selects a voice or starts microphone input.
use super::{Shared, active, authenticate_headers, voice_setup};
use avesra_contracts::{
    ErrorCode,
    preview::{PreviewControl, PreviewEvent, PreviewMessage, PreviewRequest},
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
use tokio::sync::OwnedSemaphorePermit;
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
    let lane = auth.tts.as_ref().ok_or(ErrorCode::Unavailable)?;
    let pcm = tokio::select! {
        biased;
        _=socket.recv()=>return Err(ErrorCode::Stale),
        value=tokio::time::timeout(Duration::from_secs(31),async {
            if request.greeting.is_some() {
                greeting_pcm(auth,device,request).await
            } else {
                lane.preview_voice(1,&request.voice).await
            }
        })=>value.map_err(|_|ErrorCode::Expired)??,
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

async fn greeting_pcm(
    auth: &Shared,
    device: Uuid,
    request: &PreviewRequest,
) -> Result<Vec<i16>, ErrorCode> {
    use avesra_server::audio::synthesis::{Completion, SpeechEvent};
    let text = request
        .greeting
        .as_ref()
        .ok_or(ErrorCode::Malformed)?
        .text()?;
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
        .synthesize(1, request.request_id, &request.voice, &text, || async {
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
        })
        .await?;
    let mut pcm = Vec::new();
    loop {
        match stream.next().await? {
            SpeechEvent::Audio { samples, .. } => {
                if pcm.len() + samples.len() > 720_000 {
                    return Err(ErrorCode::TooLarge);
                }
                pcm.extend(samples);
            }
            SpeechEvent::End {
                outcome: Completion::Complete,
                samples,
                ..
            } if samples == pcm.len() && !pcm.is_empty() => return Ok(pcm),
            _ => return Err(ErrorCode::Unavailable),
        }
    }
}
