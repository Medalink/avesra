use crate::auth::{AuthStore, now_ms};
use avesra_contracts::{
    ConnectionStatus, ControlMessage, MAX_CONTROL_BYTES, ServerStatus, SessionContext,
    decode_control,
};
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;
struct ServerState {
    auth: Mutex<AuthStore>,
    admission: Arc<Semaphore>,
    connections: Arc<Semaphore>,
}
type Shared = Arc<ServerState>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairRequest {
    code: String,
}
#[derive(Serialize)]
struct PairResponse {
    version: u16,
    device_id: Uuid,
    credential: String,
}
#[derive(Serialize)]
struct SessionReply {
    version: u16,
    device_id: Uuid,
    session_id: Uuid,
    status: &'static str,
}
pub fn router(auth: AuthStore) -> Router {
    Router::new()
        .route("/pair", post(pair))
        .route("/control", get(control))
        .layer(DefaultBodyLimit::max(1024))
        .with_state(Arc::new(ServerState {
            auth: Mutex::new(auth),
            admission: Arc::new(Semaphore::new(8)),
            connections: Arc::new(Semaphore::new(4)),
        }))
}
async fn pair(
    State(auth): State<Shared>,
    Json(request): Json<PairRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    let permit = auth
        .admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        auth.auth
            .lock()
            .map_err(|_| "Unavailable".to_string())?
            .pair(&request.code)
    })
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let (device_id, credential) = result.map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(Json(PairResponse {
        version: 1,
        device_id,
        credential,
    }))
}
async fn control(
    State(auth): State<Shared>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let permit = auth
        .admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let connection = auth
        .connections
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_owned();
    let store = auth.clone();
    let device = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        store
            .auth
            .lock()
            .map_err(|_| "Unavailable".to_string())?
            .authenticate(&token)
    })
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(ws
        .max_message_size(MAX_CONTROL_BYTES)
        .max_frame_size(MAX_CONTROL_BYTES)
        .write_buffer_size(1024)
        .max_write_buffer_size(MAX_CONTROL_BYTES * 2)
        .on_upgrade(move |socket| session(socket, auth, device, connection)))
}
async fn active(auth: Shared, device: Uuid) -> bool {
    let Ok(permit) = auth.admission.clone().try_acquire_owned() else {
        return false;
    };
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        auth.auth
            .lock()
            .ok()
            .and_then(|s| s.active(device).ok())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}
async fn send(socket: &mut WebSocket, message: Message) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(3), socket.send(message)).await,
        Ok(Ok(()))
    )
}
async fn session(
    mut socket: WebSocket,
    auth: Shared,
    device_id: Uuid,
    _connection: OwnedSemaphorePermit,
) {
    let session_id = Uuid::new_v4();
    let reply = SessionReply {
        version: 1,
        device_id,
        session_id,
        status: "owner_setup_required",
    };
    let Ok(reply) = serde_json::to_string(&reply) else {
        return;
    };
    if !send(&mut socket, Message::Text(reply.into())).await {
        return;
    }
    let mut context: Option<SessionContext> = None;
    let mut mode = (true, false, true);
    let mut response_sequence = 0u64;
    loop {
        let deadline = if context.is_none() { 10 } else { 30 };
        let packet = tokio::time::timeout(Duration::from_secs(deadline), socket.recv()).await;
        let Ok(Some(Ok(packet))) = packet else {
            break;
        };
        if !active(auth.clone(), device_id).await {
            break;
        }
        let Message::Text(text) = packet else {
            break;
        };
        let Ok(envelope) = decode_control(text.as_bytes()) else {
            break;
        };
        let Ok(now) = now_ms().and_then(|v| u64::try_from(v).map_err(|_| "Clock invalid".into()))
        else {
            break;
        };
        if context.is_none() {
            let baseline = SessionContext {
                device_id,
                session_id,
                last_sequence: 0,
                capture_epoch: envelope.capture_epoch,
                action_epoch: envelope.action_epoch,
            };
            if !matches!(envelope.message, ControlMessage::Hello { .. })
                || envelope.validate(&baseline, now).is_err()
            {
                break;
            }
            context = Some(SessionContext {
                device_id,
                session_id,
                last_sequence: envelope.sequence,
                capture_epoch: envelope.capture_epoch,
                action_epoch: envelope.action_epoch,
            });
        } else {
            let Some(current) = context.as_mut() else {
                break;
            };
            // A local control may advance epochs; any backwards move remains stale.
            if matches!(envelope.message, ControlMessage::Mode { .. })
                && envelope.capture_epoch >= current.capture_epoch
                && envelope.action_epoch >= current.action_epoch
            {
                let updated = SessionContext {
                    device_id,
                    session_id,
                    last_sequence: current.last_sequence,
                    capture_epoch: envelope.capture_epoch,
                    action_epoch: envelope.action_epoch,
                };
                if envelope.validate(&updated, now).is_err() {
                    break;
                }
                current.capture_epoch = updated.capture_epoch;
                current.action_epoch = updated.action_epoch;
            } else if envelope.validate(current, now).is_err() {
                break;
            }
            current.last_sequence = envelope.sequence;
            if matches!(envelope.message, ControlMessage::Hello { .. }) {
                break;
            }
        }
        if let ControlMessage::Mode {
            muted,
            deafened,
            paused,
        } = envelope.message
        {
            mode = (muted, deafened, paused);
        }
        let Some(next) = response_sequence.checked_add(1) else {
            break;
        };
        response_sequence = next;
        let reply = ServerStatus {
            version: 1,
            device_id,
            session_id,
            sequence: response_sequence,
            request_id: envelope.request_id,
            request_sequence: envelope.sequence,
            capture_epoch: envelope.capture_epoch,
            action_epoch: envelope.action_epoch,
            status: if matches!(envelope.message, ControlMessage::Action(_)) {
                ConnectionStatus::RejectedOwnerSetupRequired
            } else {
                ConnectionStatus::ConnectedOwnerSetupRequired
            },
            muted: mode.0,
            deafened: mode.1,
            paused: mode.2,
        };
        let Ok(reply) = serde_json::to_string(&reply) else {
            break;
        };
        if !send(&mut socket, Message::Text(reply.into())).await {
            break;
        }
    }
    let _ = send(&mut socket, Message::Close(None)).await;
}
