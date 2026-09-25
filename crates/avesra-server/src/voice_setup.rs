//! Paired explicit voice setup. Microphone permission is independent of output.
use super::{Shared, active, authenticate_headers};
use avesra_contracts::{ErrorCode, voices::VoiceCommand as Command};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u16,
    request_id: Uuid,
    session_id: Uuid,
    capture_epoch: u64,
    #[serde(default)]
    playback_epoch: Option<u64>,
    command: Command,
}
pub(super) fn current(
    auth: &Shared,
    device: Uuid,
    session: Uuid,
    epoch: u64,
    output: bool,
) -> bool {
    auth.sessions.lock().is_ok_and(|sessions| {
        sessions.get(&session).is_some_and(|live| {
            live.device == device
                && (if output {
                    live.output_epoch
                } else {
                    live.epoch
                }) == epoch
                && live.updated.elapsed() < Duration::from_secs(30)
                && (!output || live.output_enabled)
        })
    })
}
pub(super) fn admit(
    auth: &Shared,
    device: Uuid,
    session: Uuid,
    epoch: u64,
    request: Uuid,
    output: bool,
) -> Result<tokio::sync::watch::Receiver<(u64, bool)>, StatusCode> {
    if request.is_nil() || session.is_nil() || epoch == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut sessions = auth
        .sessions
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let live = sessions.get_mut(&session).ok_or(StatusCode::UNAUTHORIZED)?;
    if live.device != device
        || (if output {
            live.output_epoch
        } else {
            live.epoch
        }) != epoch
        || live.updated.elapsed() >= Duration::from_secs(30)
        || (output && !live.output_enabled)
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    while live
        .seen
        .front()
        .is_some_and(|(_, created)| created.elapsed() > Duration::from_secs(61))
    {
        live.seen.pop_front();
    }
    if live.seen.len() >= 128 || live.seen.iter().any(|(id, _)| *id == request) {
        return Err(StatusCode::CONFLICT);
    }
    live.seen.push_back((request, Instant::now()));
    Ok(if output {
        live.output_permission.subscribe()
    } else {
        live.permission.subscribe()
    })
}
pub(super) async fn operation(
    auth: Shared,
    headers: HeaderMap,
    body: serde_json::Value,
) -> Result<serde_json::Value, StatusCode> {
    let device = authenticate_headers(auth.clone(), &headers).await?;
    let request: Request = serde_json::from_value(body).map_err(|_| StatusCode::BAD_REQUEST)?;
    request
        .command
        .validate()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if request.version != 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if request.playback_epoch.is_some() && !matches!(request.command, Command::Status) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let output = request.playback_epoch.is_some();
    let epoch = request.playback_epoch.unwrap_or(request.capture_epoch);
    let _permit = auth
        .voice_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let mut permission = admit(
        &auth,
        device,
        request.session_id,
        epoch,
        request.request_id,
        output,
    )?;
    let execute = async {
        let tts = auth.tts.as_ref().ok_or(ErrorCode::Unavailable)?;
        let value = match request.command {
            Command::Status => serde_json::to_value(tts.voice_status(1).await?)
                .map_err(|_| ErrorCode::Malformed)?,
            Command::Generate { text, description } => {
                let lane = auth.voice_design.as_ref().ok_or(ErrorCode::Unavailable)?;
                serde_json::to_value(lane.create_voice(1, &text, &description).await?)
                    .map_err(|_| ErrorCode::Malformed)?
            }
            Command::Select {
                voice,
                expected_selection,
            } => {
                tts.select_voice(1, &voice, expected_selection.as_deref())
                    .await?;
                serde_json::json!({"outcome":"selected","voice":voice})
            }
            Command::Clear { expected_selection } => {
                tts.clear_voice(1, expected_selection.as_deref()).await?;
                serde_json::json!({"outcome":"cleared"})
            }
            Command::Discard { id, revision } => {
                tts.discard_voice(1, id, revision).await?;
                serde_json::json!({"outcome":"discarded"})
            }
        };
        Ok::<_, ErrorCode>(value)
    };
    let monitor = async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !current(&auth, device, request.session_id, epoch, output)
                || !active(auth.clone(), device).await
            {
                return;
            }
        }
    };
    let result = tokio::select! {
        biased;
        _=permission.changed()=>return Err(StatusCode::CONFLICT),
        _=monitor=>return Err(StatusCode::CONFLICT),
        value=tokio::time::timeout(Duration::from_secs(31),execute)=>value.map_err(|_|StatusCode::GATEWAY_TIMEOUT)?.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?,
    };
    if !current(&auth, device, request.session_id, epoch, output)
        || !active(auth.clone(), device).await
        || !current(&auth, device, request.session_id, epoch, output)
    {
        return Err(StatusCode::CONFLICT);
    }
    let mut reply = serde_json::json!({"version":1,"request_id":request.request_id,"session_id":request.session_id,"capture_epoch":request.capture_epoch,"result":result});
    if let Some(epoch) = request.playback_epoch {
        reply["playback_epoch"] = epoch.into();
    }
    Ok(reply)
}
