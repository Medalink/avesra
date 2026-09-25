use crate::auth::{AuthStore, now_ms};
use avesra_contracts::{
    ConnectionStatus, ControlMessage, MAX_CONTROL_BYTES, PROTOCOL_VERSION, ServerStatus,
    SessionContext, decode_control,
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
#[cfg(unix)]
#[path = "voice_preview.rs"]
mod voice_preview;
#[cfg(unix)]
#[path = "voice_setup.rs"]
mod voice_setup;
#[cfg(unix)]
#[path = "voice_stream.rs"]
mod voice_stream;
struct ServerState {
    auth: Mutex<AuthStore>,
    admission: Arc<Semaphore>,
    connections: Arc<Semaphore>,
    #[cfg(unix)]
    speaker: Option<Arc<avesra_server::audio::AudioClient>>,
    #[cfg(unix)]
    asr: Option<Arc<avesra_server::audio::AudioClient>>,
    #[cfg(unix)]
    tts: Option<Arc<avesra_server::audio::AudioClient>>,
    #[cfg(unix)]
    voice_design: Option<Arc<avesra_server::audio::AudioClient>>,
    #[cfg(unix)]
    voice_admission: Arc<Semaphore>,
    #[cfg(unix)]
    speaker_admission: Arc<Semaphore>,
    #[cfg(unix)]
    speaker_health_admission: Arc<Semaphore>,
    #[cfg(unix)]
    sessions: Mutex<std::collections::HashMap<Uuid, LiveSession>>,
}
#[cfg(unix)]
struct LiveSession {
    device: Uuid,
    epoch: u64,
    output_epoch: u64,
    enabled: bool,
    output_enabled: bool,
    output_permission: tokio::sync::watch::Sender<(u64, bool)>,
    permission: tokio::sync::watch::Sender<(u64, bool)>,
    updated: std::time::Instant,
    seen: std::collections::VecDeque<(Uuid, std::time::Instant)>,
}
#[cfg(unix)]
fn session_current(auth: &Shared, device: Uuid, session: Uuid, epoch: u64) -> bool {
    auth.sessions.lock().is_ok_and(|sessions| {
        sessions.get(&session).is_some_and(|live| {
            live.device == device
                && live.epoch == epoch
                && live.enabled
                && live.updated.elapsed() < Duration::from_secs(30)
        })
    })
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
pub fn router(auth: AuthStore, directory: &std::path::Path) -> Result<Router, String> {
    #[cfg(unix)]
    let speaker = audio_client(directory, "speaker")?;
    #[cfg(unix)]
    let asr = audio_client(directory, "asr")?;
    #[cfg(unix)]
    let tts = audio_client(directory, "tts")?;
    #[cfg(unix)]
    let voice_design = audio_client(directory, "voice-design")?;
    #[cfg(not(unix))]
    let _ = directory;
    let router = Router::new()
        .route("/pair", post(pair))
        .route("/control", get(control))
        .route("/speaker", get(speaker_health).post(speaker_infer))
        .route("/voice-analysis", post(voice_analysis))
        .route("/voice-stream", get(voice_stream_upgrade))
        .route("/voice-preview", get(voice_preview_upgrade))
        .route(
            "/voices",
            post(voice_operations).layer(DefaultBodyLimit::max(4096)),
        )
        .layer(DefaultBodyLimit::max(1024))
        .with_state(Arc::new(ServerState {
            auth: Mutex::new(auth),
            admission: Arc::new(Semaphore::new(8)),
            connections: Arc::new(Semaphore::new(4)),
            #[cfg(unix)]
            speaker,
            #[cfg(unix)]
            asr,
            #[cfg(unix)]
            tts,
            #[cfg(unix)]
            voice_design,
            #[cfg(unix)]
            voice_admission: Arc::new(Semaphore::new(1)),
            #[cfg(unix)]
            speaker_admission: Arc::new(Semaphore::new(1)),
            #[cfg(unix)]
            speaker_health_admission: Arc::new(Semaphore::new(2)),
            #[cfg(unix)]
            sessions: Mutex::new(std::collections::HashMap::new()),
        }));
    Ok(router)
}
async fn voice_operations(
    State(auth): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    #[cfg(unix)]
    {
        voice_setup::operation(auth, headers, body).await.map(Json)
    }
    #[cfg(not(unix))]
    {
        let _ = (auth, headers, body);
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}
async fn voice_preview_upgrade(
    State(auth): State<Shared>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    #[cfg(unix)]
    {
        voice_preview::upgrade(auth, headers, ws).await
    }
    #[cfg(not(unix))]
    {
        let _ = (auth, headers, ws);
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}
async fn voice_stream_upgrade(
    State(auth): State<Shared>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    #[cfg(unix)]
    {
        voice_stream::upgrade(auth, headers, ws).await
    }
    #[cfg(not(unix))]
    {
        let _ = (auth, headers, ws);
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

#[cfg(unix)]
fn audio_client(
    directory: &std::path::Path,
    lane: &str,
) -> Result<Option<Arc<avesra_server::audio::AudioClient>>, String> {
    use std::io::Read;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Configuration {
        socket: std::path::PathBuf,
        model_revision: String,
    }
    let file = match std::fs::File::open(directory.join(format!("{lane}-deployment.json"))) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Speaker deployment cannot be read".into()),
    };
    let mut bytes = vec![];
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| "Speaker deployment cannot be read")?;
    if bytes.len() > 4096 {
        return Err("Speaker deployment exceeds limit".into());
    }
    let config: Configuration =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid speaker deployment")?;
    let client = avesra_server::audio::AudioClient::for_deployment(
        &config.socket,
        lane,
        &config.model_revision,
    )
    .map_err(|_| "Speaker deployment is unavailable")?;
    Ok(Some(Arc::new(client)))
}
async fn authenticate_headers(auth: Shared, headers: &HeaderMap) -> Result<Uuid, StatusCode> {
    let permit = auth
        .admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_owned();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        auth.auth
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .authenticate(&token)
            .map_err(|_| StatusCode::UNAUTHORIZED)
    })
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
}
async fn speaker_health(
    State(auth): State<Shared>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authenticate_headers(auth.clone(), &headers).await?;
    #[cfg(unix)]
    {
        let _permit = auth
            .speaker_health_admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
        let client = auth
            .speaker
            .as_ref()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        let health = client
            .health()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        Ok(Json(
            serde_json::to_value(health).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        ))
    }
    #[cfg(not(unix))]
    Err(StatusCode::SERVICE_UNAVAILABLE)
}
async fn speaker_infer(
    State(auth): State<Shared>,
    request: axum::extract::Request,
) -> Result<Json<serde_json::Value>, StatusCode> {
    analyze(auth, request, false).await
}
async fn voice_analysis(
    State(auth): State<Shared>,
    request: axum::extract::Request,
) -> Result<Json<serde_json::Value>, StatusCode> {
    analyze(auth, request, true).await
}
async fn analyze(
    auth: Shared,
    request: axum::extract::Request,
    transcribe: bool,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let device = authenticate_headers(auth.clone(), request.headers()).await?;
    #[cfg(unix)]
    {
        use avesra_server::audio::{AudioInput, AudioOutput};
        let client = auth
            .speaker
            .as_ref()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
            .clone();
        let asr = if transcribe {
            Some(
                auth.asr
                    .as_ref()
                    .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
                    .clone(),
            )
        } else {
            None
        };
        let _speaker_permit = auth
            .speaker_admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
        // Keep admission for the complete request, including body retention.
        let _permit = auth
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            version: u16,
            request_id: Uuid,
            session_id: Uuid,
            capture_epoch: u64,
            pcm_s16le: String,
        }
        let bytes = tokio::time::timeout(
            Duration::from_secs(3),
            axum::body::to_bytes(request.into_body(), 460_000),
        )
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
        let input: Input = serde_json::from_slice(&bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
        drop(bytes);
        if input.version != 1
            || input.request_id.is_nil()
            || input.session_id.is_nil()
            || input.capture_epoch == 0
            || input.pcm_s16le.len() > 426_668
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        let mut permission = {
            let mut sessions = auth
                .sessions
                .lock()
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            let live = sessions
                .get_mut(&input.session_id)
                .ok_or(StatusCode::UNAUTHORIZED)?;
            if live.device != device
                || live.epoch != input.capture_epoch
                || !live.enabled
                || live.updated.elapsed() >= Duration::from_secs(30)
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
            if live.seen.len() >= 128 || live.seen.iter().any(|(id, _)| *id == input.request_id) {
                return Err(StatusCode::CONFLICT);
            }
            live.seen
                .push_back((input.request_id, std::time::Instant::now()));
            live.permission.subscribe()
        };
        {
            use base64::Engine;
            let raw = base64::engine::general_purpose::STANDARD
                .decode(&input.pcm_s16le)
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            if !(32_000..=320_000).contains(&raw.len()) || !raw.len().is_multiple_of(2) {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
        struct Cancel {
            client: Arc<avesra_server::audio::AudioClient>,
            id: Uuid,
            complete: bool,
        }
        impl Drop for Cancel {
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
        let worker_id = Uuid::new_v4();
        let mut cancel = Cancel {
            client: client.clone(),
            id: worker_id,
            complete: false,
        };
        let asr_worker_id = Uuid::new_v4();
        let mut cancel_asr = asr.as_ref().map(|client| Cancel {
            client: client.clone(),
            id: asr_worker_id,
            complete: false,
        });
        let asr_pcm = asr.as_ref().map(|_| input.pcm_s16le.clone());
        let inference = client.infer_with_budget(
            worker_id,
            1,
            input.request_id,
            AudioInput::Pcm {
                pcm_s16le: input.pcm_s16le,
            },
            Duration::from_secs(15),
        );
        let transcription = async {
            match (asr.as_ref(), asr_pcm) {
                (Some(client), Some(pcm_s16le)) => client
                    .infer_with_budget(
                        asr_worker_id,
                        1,
                        input.request_id,
                        AudioInput::Pcm { pcm_s16le },
                        Duration::from_secs(15),
                    )
                    .await
                    .map(Some),
                _ => Ok(None),
            }
        };
        let (result, transcript) = tokio::select! {
            biased;
            _=permission.changed()=>return Err(StatusCode::UNAUTHORIZED),
            result=async {tokio::try_join!(inference, transcription)}=>result.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        };
        cancel.complete = true;
        if let Some(cancel) = cancel_asr.as_mut() {
            cancel.complete = true;
        }
        if !session_current(&auth, device, input.session_id, input.capture_epoch)
            || !active(auth.clone(), device).await
            || !session_current(&auth, device, input.session_id, input.capture_epoch)
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        if transcribe {
            let Some(transcript) = transcript else {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            };
            // Final lane data is still transient speculation. A valid transcript
            // and embedding alone create no accepted turn, identity or tool rights.
            let AudioOutput::Transcript {
                text,
                r#final: true,
            } = transcript.output
            else {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            };
            let (embedding, reason) = match result.output {
                AudioOutput::Embedding { embedding, .. } => (
                    Some(embedding),
                    "owner_overlap_directness_qualification_required",
                ),
                AudioOutput::Insufficient { .. } => (None, "insufficient_speech"),
                _ => return Err(StatusCode::UNPROCESSABLE_ENTITY),
            };
            return Ok(Json(
                serde_json::json!({"version":1,"request_id":input.request_id,"session_id":input.session_id,
                "capture_epoch":input.capture_epoch,"speaker_revision":client.configured_revision(),
                "asr_revision":asr.as_ref().and_then(|client| client.configured_revision()),
                "transcript":text,"embedding":embedding,"outcome":"abstain","reason":reason,
                "accepted_turn":false}),
            ));
        }
        match result.output {
            AudioOutput::Embedding { embedding, .. } => Ok(Json(
                serde_json::json!({"version":1,"request_id":input.request_id,"session_id":input.session_id,"capture_epoch":input.capture_epoch,"model_revision":client.configured_revision(),"embedding":embedding}),
            )),
            _ => Err(StatusCode::UNPROCESSABLE_ENTITY),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (device, transcribe);
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
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
    #[cfg(unix)]
    struct SessionLease {
        auth: Shared,
        id: Uuid,
    }
    #[cfg(unix)]
    impl Drop for SessionLease {
        fn drop(&mut self) {
            if let Ok(mut sessions) = self.auth.sessions.lock() {
                sessions.remove(&self.id);
            }
        }
    }
    #[cfg(unix)]
    let _lease = SessionLease {
        auth: auth.clone(),
        id: session_id,
    };
    let reply = SessionReply {
        version: PROTOCOL_VERSION,
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
                playback_epoch: envelope.playback_epoch,
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
                playback_epoch: envelope.playback_epoch,
                action_epoch: envelope.action_epoch,
            });
        } else {
            let Some(current) = context.as_mut() else {
                break;
            };
            // A local control may advance epochs; any backwards move remains stale.
            if matches!(envelope.message, ControlMessage::Mode { .. })
                && envelope.capture_epoch >= current.capture_epoch
                && envelope.playback_epoch >= current.playback_epoch
                && envelope.action_epoch >= current.action_epoch
            {
                let updated = SessionContext {
                    device_id,
                    session_id,
                    last_sequence: current.last_sequence,
                    capture_epoch: envelope.capture_epoch,
                    playback_epoch: envelope.playback_epoch,
                    action_epoch: envelope.action_epoch,
                };
                if envelope.validate(&updated, now).is_err() {
                    break;
                }
                current.capture_epoch = updated.capture_epoch;
                current.playback_epoch = updated.playback_epoch;
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
        #[cfg(unix)]
        {
            let Ok(mut sessions) = auth.sessions.lock() else {
                break;
            };
            let live = sessions.entry(session_id).or_insert_with(|| LiveSession {
                device: device_id,
                epoch: envelope.capture_epoch,
                output_epoch: envelope.playback_epoch,
                enabled: !mode.0 && !mode.1 && !mode.2,
                output_enabled: !mode.1 && !mode.2,
                output_permission: tokio::sync::watch::channel((
                    envelope.playback_epoch,
                    !mode.1 && !mode.2,
                ))
                .0,
                permission: tokio::sync::watch::channel((
                    envelope.capture_epoch,
                    !mode.0 && !mode.1 && !mode.2,
                ))
                .0,
                updated: std::time::Instant::now(),
                seen: std::collections::VecDeque::new(),
            });
            live.epoch = envelope.capture_epoch;
            live.output_epoch = envelope.playback_epoch;
            live.enabled = !mode.0 && !mode.1 && !mode.2;
            live.output_enabled = !mode.1 && !mode.2;
            live.output_permission.send_if_modified(|permission| {
                let next = (live.output_epoch, live.output_enabled);
                if *permission == next {
                    false
                } else {
                    *permission = next;
                    true
                }
            });
            live.permission.send_if_modified(|permission| {
                let next = (live.epoch, live.enabled);
                if *permission == next {
                    false
                } else {
                    *permission = next;
                    true
                }
            });
            live.updated = std::time::Instant::now();
        }
        let Some(next) = response_sequence.checked_add(1) else {
            break;
        };
        response_sequence = next;
        let reply = ServerStatus {
            version: PROTOCOL_VERSION,
            device_id,
            session_id,
            sequence: response_sequence,
            request_id: envelope.request_id,
            request_sequence: envelope.sequence,
            capture_epoch: envelope.capture_epoch,
            playback_epoch: envelope.playback_epoch,
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
