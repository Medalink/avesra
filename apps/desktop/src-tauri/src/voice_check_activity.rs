//! One explicit check owns socket, bounded capture queue, and correlated scores.
use crate::performance::{Operation, Outcome, Stage};
use crate::{
    Runtime,
    connection::{MediaEndpoint, PairingRecord, SessionIdentity},
};
use avesra_contracts::activity::{
    Acknowledgment, Activity, Packet, REVISION, STREAM_VERSION, Start, StreamReply,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};
use uuid::Uuid;
pub struct Stream {
    socket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    session: SessionIdentity,
    request: Uuid,
    started: Instant,
    acknowledged: Instant,
    sequence: u64,
    samples: u32,
    frames: u32,
    final_received: bool,
}
pub struct Captured {
    pub pcm: Vec<u8>,
    pub captured: Instant,
    pub final_chunk: bool,
}
#[derive(Clone, serde::Serialize)]
struct Progress {
    request: Uuid,
    observation: avesra_contracts::activity::Chunk,
    diagnostic: Option<crate::qualification::activity::Diagnostic>,
}
impl Stream {
    pub async fn open(record: &PairingRecord, session: SessionIdentity) -> Result<Self, String> {
        let mut socket = crate::connection::voice_socket(record, MediaEndpoint::Activity).await?;
        let request = Uuid::new_v4();
        let started = Instant::now();
        let start = Start {
            version: STREAM_VERSION,
            session_id: session.id,
            capture_epoch: session.epoch,
            request_id: request,
        };
        let work = async {
            socket
                .send(Message::Text(
                    serde_json::to_string(&start)
                        .map_err(|_| "Invalid activity start")?
                        .into(),
                ))
                .await
                .map_err(|_| "Activity connection lost")?;
            let reply = socket
                .next()
                .await
                .ok_or("Activity disconnected")?
                .map_err(|_| "Activity connection lost")?;
            let acknowledged = Instant::now();
            let Message::Text(text) = reply else {
                return Err("Invalid activity acknowledgment");
            };
            let ack: Acknowledgment =
                serde_json::from_str(&text).map_err(|_| "Invalid activity acknowledgment")?;
            if ack.version != STREAM_VERSION
                || ack.session_id != session.id
                || ack.capture_epoch != session.epoch
                || ack.request_id != request
                || ack.model_revision != REVISION
            {
                return Err("Activity acknowledgment changed");
            }
            Ok(acknowledged)
        };
        let acknowledged = tokio::time::timeout(Duration::from_secs(3), work)
            .await
            .map_err(|_| "Activity start timed out")??;
        Ok(Self {
            socket,
            session,
            request,
            started,
            acknowledged,
            sequence: 1,
            samples: 0,
            frames: 0,
            final_received: false,
        })
    }
    pub async fn exchange(
        &mut self,
        app: &tauri::AppHandle,
        check: Uuid,
        frame: Captured,
        current: impl Fn() -> Result<(), String>,
    ) -> Result<avesra_contracts::activity::Chunk, String> {
        current()?;
        if self.final_received || self.started.elapsed() >= Duration::from_secs(20) {
            return Err("Activity stream expired".into());
        }
        let performance = app.state::<Runtime>().performance.clone();
        let age = frame.captured.elapsed();
        performance.record(
            Operation::Activity,
            Stage::CaptureAge,
            age,
            if age > Duration::from_millis(500) {
                Outcome::Failed
            } else {
                Outcome::Complete
            },
        );
        if frame.captured > Instant::now()
            || age > Duration::from_millis(500)
            || frame.pcm.is_empty()
            || frame.pcm.len() > 6400
            || !frame.pcm.len().is_multiple_of(2)
        {
            return Err("Activity capture is stale or malformed".into());
        }
        let samples = self
            .samples
            .checked_add((frame.pcm.len() / 2) as u32)
            .filter(|v| *v <= 160000)
            .ok_or("Activity sample limit exceeded")?;
        let mut packet = Packet {
            sequence: self.sequence,
            sample_offset: self.samples,
            captured_age_ms: 0,
            elapsed_since_ack_ms: 0,
            pcm_s16le: STANDARD.encode(frame.pcm),
            r#final: frame.final_chunk,
        };
        let span = performance.begin(Operation::Activity, Stage::ActivityExchange, check);
        let work = async {
            // Recheck after preparation/telemetry; neither a new socket nor queue
            // residence gives old microphone samples a fresh capture timestamp.
            let now = Instant::now();
            let age = now
                .checked_duration_since(frame.captured)
                .filter(|age| *age <= Duration::from_millis(500))
                .ok_or("Activity capture expired before send")?;
            packet.captured_age_ms = age.as_micros().div_ceil(1000) as u16;
            let elapsed = now.duration_since(self.acknowledged);
            if elapsed >= Duration::from_secs(20) {
                return Err("Activity stream expired before send");
            }
            packet.elapsed_since_ack_ms = elapsed.as_millis() as u32;
            self.socket
                .send(Message::Text(
                    serde_json::to_string(&packet)
                        .map_err(|_| "Invalid activity packet")?
                        .into(),
                ))
                .await
                .map_err(|_| "Activity connection lost")?;
            let response = self
                .socket
                .next()
                .await
                .ok_or("Activity disconnected")?
                .map_err(|_| "Activity connection lost")?;
            let Message::Text(text) = response else {
                return Err("Invalid activity response");
            };
            serde_json::from_str::<StreamReply>(&text).map_err(|_| "Invalid activity response")
        };
        let result = tokio::time::timeout(Duration::from_secs(2), work)
            .await
            .map_err(|_| "Activity response timed out")
            .and_then(|v| v);
        span.finish(if result.is_ok() {
            Outcome::Complete
        } else {
            Outcome::Failed
        });
        let reply = result?;
        current()?;
        if reply.version != STREAM_VERSION
            || reply.session_id != self.session.id
            || reply.capture_epoch != self.session.epoch
            || reply.request_id != self.request
            || reply.sequence != self.sequence
            || reply
                .observation
                .validate(samples, self.frames, frame.final_chunk)
                .is_err()
        {
            return Err("Activity stream correlation failed".into());
        }
        self.samples = samples;
        self.frames += reply.observation.frames.len() as u32;
        self.sequence += 1;
        self.final_received = frame.final_chunk;
        Ok(reply.observation)
    }
    pub async fn settle(&mut self) -> Result<(), String> {
        if !self.final_received {
            return Err("Activity stream has no terminal observation".into());
        }
        let remaining = Duration::from_secs(20)
            .saturating_sub(self.started.elapsed())
            .min(Duration::from_secs(2));
        let response = tokio::time::timeout(remaining, self.socket.next())
            .await
            .map_err(|_| "Activity stream retirement timed out")?;
        match response {
            Some(Ok(Message::Close(_))) | None => Ok(()),
            _ => Err("Activity stream did not retire after its terminal observation".into()),
        }
    }
    pub async fn run(
        mut self,
        app: &tauri::AppHandle,
        check: Uuid,
        calibration: Option<Uuid>,
        mut receiver: tokio::sync::mpsc::Receiver<Captured>,
    ) -> Result<Activity, String> {
        let performance = app.state::<Runtime>().performance.clone();
        let session_span = performance.begin(Operation::Activity, Stage::ActivitySession, check);
        let measured = async {
            let mut result = Activity {
                model_revision: REVISION.into(),
                samples: 0,
                frames: Vec::new(),
            };
            loop {
                app.state::<Runtime>().voice_check.current(app, check)?;
                if self.started.elapsed() >= Duration::from_secs(20) {
                    return Err("Activity check expired".into());
                }
                let frame = tokio::time::timeout(Duration::from_millis(750), receiver.recv())
                    .await
                    .map_err(|_| "Activity recording stalled")?
                    .ok_or("Activity recording stopped")?;
                let observation = self
                    .exchange(app, check, frame, || {
                        app.state::<Runtime>()
                            .voice_check
                            .current(app, check)
                            .map(|_| ())
                    })
                    .await?;
                let final_chunk = observation.r#final;
                result.samples = observation.samples;
                result.frames.extend_from_slice(&observation.frames);
                let diagnostic = if let Some(session) = calibration {
                    let state = app.state::<Runtime>();
                    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                    state.qualification.observe(&state, &local);
                    state
                        .qualification
                        .activity_chunk(session, check, &observation)?
                } else {
                    None
                };
                let _ = app.emit(
                    "voice-check-activity",
                    Progress {
                        request: check,
                        observation,
                        diagnostic,
                    },
                );
                if final_chunk {
                    result
                        .validate(128000)
                        .map_err(|_| "Activity stream incomplete")?;
                    self.settle().await?;
                    return Ok(result);
                }
            }
        }
        .await;
        session_span.finish(if measured.is_ok() {
            Outcome::Complete
        } else {
            Outcome::Failed
        });
        measured
    }
}
