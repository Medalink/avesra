//! Completed accepted reply output. No text-only, history or preview admission.
use super::{Shared, authenticate_headers, planner_ingress};
use avesra_contracts::{ErrorCode, speech};
use avesra_server::audio::{
    AudioClient,
    synthesis::{Completion, SpeechEvent},
};
use axum::{
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use std::{
    sync::{Arc, atomic::AtomicUsize},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use uuid::Uuid;

pub(super) async fn upgrade(
    auth: Shared,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let device = authenticate_headers(auth.clone(), &headers).await?;
    // Startup has no reasoning qualifier and therefore cannot issue the required
    // completed source. Do not activate a TTS-only arbitrary-text route.
    if auth.reasoning.is_none() || auth.tts.is_none() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let connection = auth
        .connections
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let admission = Arc::new(
        auth.voice_admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?,
    );
    Ok(ws
        .max_message_size(avesra_contracts::planner::MAX_REQUEST_BYTES)
        .max_frame_size(avesra_contracts::planner::MAX_REQUEST_BYTES)
        .write_buffer_size(1024)
        .max_write_buffer_size(65536)
        .on_upgrade(move |socket| session(socket, auth, device, connection, admission)))
}
async fn session(
    mut socket: WebSocket,
    auth: Shared,
    device: Uuid,
    _connection: OwnedSemaphorePermit,
    admission: Arc<OwnedSemaphorePermit>,
) {
    let deadline = Instant::now() + Duration::from_secs(70);
    let _ = tokio::time::timeout_at(
        deadline.into(),
        start(&mut socket, auth, device, admission, deadline),
    )
    .await;
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Close(None)),
    )
    .await;
}
async fn read(socket: &mut WebSocket, budget: Duration) -> Result<String, ErrorCode> {
    match tokio::time::timeout(budget, socket.recv())
        .await
        .map_err(|_| ErrorCode::Expired)?
        .ok_or(ErrorCode::Unavailable)?
        .map_err(|_| ErrorCode::Malformed)?
    {
        Message::Text(value) => Ok(value.to_string()),
        _ => Err(ErrorCode::Stale),
    }
}
async fn send(
    socket: &mut WebSocket,
    context: &speech::Context,
    message: speech::Message,
) -> Result<(), ErrorCode> {
    let event = speech::Event {
        version: speech::VERSION,
        context: context.clone(),
        message,
    };
    event.validate(context)?;
    let value = serde_json::to_string(&event).map_err(|_| ErrorCode::Malformed)?;
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
struct Inspector {
    admission: Arc<OwnedSemaphorePermit>,
    gate: Arc<Semaphore>,
    private: Arc<AtomicUsize>,
}
async fn inspect(
    auth: &Shared,
    context: &speech::Context,
    inspector: &Inspector,
    deadline: Instant,
) -> Result<(), ErrorCode> {
    let auth = auth.clone();
    let context = context.clone();
    let admission = inspector.admission.clone();
    let gate = tokio::time::timeout_at(deadline.into(), inspector.gate.clone().acquire_owned())
        .await
        .map_err(|_| ErrorCode::Expired)?
        .map_err(|_| ErrorCode::Unavailable)?;
    tokio::time::timeout_at(
        deadline.into(),
        tokio::task::spawn_blocking(move || {
            let _admission = admission;
            let _gate = gate;
            planner_ingress::speech_authority(&auth, &context, deadline)
        }),
    )
    .await
    .map_err(|_| ErrorCode::Expired)?
    .map_err(|_| ErrorCode::Unavailable)?
}
async fn start(
    socket: &mut WebSocket,
    auth: Shared,
    device: Uuid,
    admission: Arc<OwnedSemaphorePermit>,
    deadline: Instant,
) -> Result<(), ErrorCode> {
    let request: speech::Request =
        serde_json::from_str(&read(socket, Duration::from_secs(3)).await?)
            .map_err(|_| ErrorCode::Malformed)?;
    request.validate()?;
    let reservation = planner_ingress::reserve_speech(&auth, device, &request, &admission)?;
    let context = request.stream_context();
    let (mut action, mut output) = {
        let sessions = auth.sessions.lock().map_err(|_| ErrorCode::Unavailable)?;
        let session = sessions
            .get(&context.planner.session)
            .ok_or(ErrorCode::Stale)?;
        (
            session.action_permission.subscribe(),
            session.output_permission.subscribe(),
        )
    };
    let inspector = Inspector {
        admission,
        gate: Arc::new(Semaphore::new(1)),
        private: reservation.private.clone(),
    };
    let monitor = async {
        loop {
            if !planner_ingress::speech_current(&auth, &context) {
                return Err::<(), ErrorCode>(ErrorCode::Stale);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let authority = async {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if let Err(error) = inspect(&auth, &context, &inspector, deadline).await {
                break Err::<(), ErrorCode>(error);
            }
        }
    };
    tokio::select! { biased;
        _=action.changed()=>Err(ErrorCode::Stale),
        _=output.changed()=>Err(ErrorCode::Stale),
        result=monitor=>result,
        result=authority=>result,
        result=stream(socket,&auth,&request,&context,&inspector,deadline)=>result,
    }
}
enum Piece {
    Audio(Vec<i16>),
    End { samples: usize, outcome: Completion },
}
async fn produce<F, Fut>(
    lane: Arc<AudioClient>,
    request: &speech::Request,
    tx: mpsc::Sender<Piece>,
    private: Arc<AtomicUsize>,
    mut authorize: F,
) -> Result<(), ErrorCode>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), ErrorCode>>,
{
    let segments = speech::text_segments(request.source.response.text())?;
    let mut total = 0usize;
    for (index, text) in segments.iter().enumerate() {
        // Fresh private identity per actual job; the single public source and
        // output reservation remain owned by start/stream across every segment.
        let mut source = lane
            .synthesize(
                1,
                Uuid::new_v4(),
                &request.voice,
                text,
                Some(private.clone()),
                &mut authorize,
            )
            .await?;
        let mut segment_samples = 0usize;
        loop {
            match source.next().await? {
                SpeechEvent::Audio { samples, .. } => {
                    total += samples.len();
                    segment_samples += samples.len();
                    if total > speech::MAX_SAMPLES as usize {
                        return Err(ErrorCode::TooLarge);
                    }
                    tx.send(Piece::Audio(samples))
                        .await
                        .map_err(|_| ErrorCode::Stale)?;
                }
                SpeechEvent::End {
                    samples, outcome, ..
                } => {
                    if samples != segment_samples {
                        return Err(ErrorCode::Malformed);
                    }
                    if outcome == Completion::Truncated || index + 1 == segments.len() {
                        tx.send(Piece::End {
                            samples: total,
                            outcome,
                        })
                        .await
                        .map_err(|_| ErrorCode::Stale)?;
                        return Ok(());
                    }
                    // Only actual Complete permits a successor. Intermediate
                    // terminals never finalize the public utterance/renderer.
                    break;
                }
            }
        }
    }
    Err(ErrorCode::Malformed)
}
async fn stream(
    socket: &mut WebSocket,
    auth: &Shared,
    request: &speech::Request,
    context: &speech::Context,
    inspector: &Inspector,
    deadline: Instant,
) -> Result<(), ErrorCode> {
    inspect(auth, context, inspector, deadline).await?;
    let lane = auth.tts.as_ref().ok_or(ErrorCode::Unavailable)?.clone();
    let status = tokio::select! { biased;
        _=socket.recv()=>return Err(ErrorCode::Stale),
        value=tokio::time::timeout(Duration::from_secs(3),lane.voice_status(1))=>value.map_err(|_|ErrorCode::Expired)??,
    };
    if status.selection_state != "available"
        || status.active_state != "available"
        || status.selected.as_ref() != Some(&request.voice)
        || status.active_voice.as_ref() != Some(&request.voice)
    {
        return Err(ErrorCode::Unavailable);
    }
    inspect(auth, context, inspector, deadline).await?;
    // At most 64 codec chunks (5.12s/245760 PCM bytes), not a whole-wave wait.
    // Queue pressure pauses private reads, never renews the 30s source budget.
    let (tx, rx) = mpsc::channel(64);
    let synth_deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let producer = async {
        tokio::time::timeout_at(
            synth_deadline,
            produce(lane, request, tx, inspector.private.clone(), || {
                inspect(auth, context, inspector, deadline)
            }),
        )
        .await
        .map_err(|_| ErrorCode::Expired)?
    };
    let consumer = consume(socket, context, rx);
    tokio::try_join!(producer, consumer)?;
    inspect(auth, context, inspector, deadline).await?;
    Ok(())
}
async fn next(socket: &mut WebSocket, rx: &mut mpsc::Receiver<Piece>) -> Result<Piece, ErrorCode> {
    tokio::select! { biased; _=socket.recv()=>Err(ErrorCode::Stale), piece=rx.recv()=>piece.ok_or(ErrorCode::Unavailable) }
}
async fn packet(
    socket: &mut WebSocket,
    context: &speech::Context,
    origin: tokio::time::Instant,
    sequence: u64,
    samples: Vec<i16>,
    last_sent: &mut Option<tokio::time::Instant>,
) -> Result<(), ErrorCode> {
    let due = origin + Duration::from_millis((sequence - 1) * 20);
    let send_at = last_sent.map_or(due, |last| due.max(last + Duration::from_millis(10)));
    tokio::select! { biased; _=socket.recv()=>return Err(ErrorCode::Stale), _=tokio::time::sleep_until(send_at)=>{} }
    // Fixed timeline, at most 100ms drift and at least 10ms between packets.
    // Bounded catch-up cannot become an immediate queue-draining burst.
    if tokio::time::Instant::now().saturating_duration_since(due) > Duration::from_millis(100) {
        return Err(ErrorCode::Expired);
    }
    *last_sent = Some(tokio::time::Instant::now());
    send(
        socket,
        context,
        speech::Message::Audio {
            sequence,
            sample_offset: (sequence - 1) * speech::FRAME_SAMPLES,
            samples,
        },
    )
    .await
}
async fn consume(
    socket: &mut WebSocket,
    context: &speech::Context,
    mut rx: mpsc::Receiver<Piece>,
) -> Result<(), ErrorCode> {
    let Piece::Audio(first) = next(socket, &mut rx).await? else {
        return Err(ErrorCode::Malformed);
    };
    send(
        socket,
        context,
        speech::Message::Ready {
            sample_rate: 24000,
            frame_samples: speech::FRAME_SAMPLES,
            max_samples: speech::MAX_SAMPLES,
        },
    )
    .await?;
    let play: speech::Control = serde_json::from_str(&read(socket, Duration::from_secs(3)).await?)
        .map_err(|_| ErrorCode::Malformed)?;
    play.validate(context.request)?;
    if !matches!(play, speech::Control::Play { .. }) {
        return Err(ErrorCode::Stale);
    }
    let origin = tokio::time::Instant::now();
    let deadline = origin + Duration::from_secs(32);
    let output = async {
        let mut incoming = Some(first);
        let mut held = None;
        let mut received = 0usize;
        let mut chunks = 0u64;
        let mut last_sent = None;
        loop {
            if let Some(samples) = incoming.take() {
                if samples.len() != 1920 {
                    return Err(ErrorCode::Malformed);
                }
                received += samples.len();
                if received > speech::MAX_SAMPLES as usize {
                    return Err(ErrorCode::TooLarge);
                }
                for frame in samples.chunks_exact(speech::FRAME_SAMPLES as usize) {
                    if let Some(previous) = held.replace(frame.to_vec()) {
                        chunks += 1;
                        packet(socket, context, origin, chunks, previous, &mut last_sent).await?;
                    }
                }
            }
            match next(socket, &mut rx).await? {
                Piece::Audio(samples) => incoming = Some(samples),
                Piece::End { samples, outcome } => {
                    if samples != received {
                        return Err(ErrorCode::Malformed);
                    }
                    let outcome = match outcome {
                        Completion::Complete => {
                            let last = held.take().ok_or(ErrorCode::Malformed)?;
                            chunks += 1;
                            packet(socket, context, origin, chunks, last, &mut last_sent).await?;
                            speech::Completion::Complete
                        }
                        Completion::Truncated => speech::Completion::Truncated,
                    };
                    // For truncation, counts describe only wire-delivered audio;
                    // the withheld final frame is discarded, never finalized.
                    let count = chunks * speech::FRAME_SAMPLES;
                    send(
                        socket,
                        context,
                        speech::Message::End {
                            chunks,
                            samples: count,
                            outcome,
                        },
                    )
                    .await?;
                    if outcome == speech::Completion::Truncated {
                        return Err(ErrorCode::Unavailable);
                    }
                    let receipt: speech::Control =
                        serde_json::from_str(&read(socket, Duration::from_secs(2)).await?)
                            .map_err(|_| ErrorCode::Malformed)?;
                    receipt.validate(context.request)?;
                    if !matches!(receipt,speech::Control::Submitted{samples,..} if samples==count) {
                        return Err(ErrorCode::Stale);
                    }
                    return Ok(());
                }
            }
        }
    };
    tokio::time::timeout_at(deadline, output)
        .await
        .map_err(|_| ErrorCode::Expired)?
}
