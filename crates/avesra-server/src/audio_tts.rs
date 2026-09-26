//! Bounded private early-audio receiver. Callers must separately authorize speech.
use super::{AudioClient, ErrorCode, STANDARD};
use avesra_contracts::voice::VoiceIdentity;
use base64::Engine;
use serde::Deserialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    sync::OwnedSemaphorePermit,
};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    pcm_s16le: String,
    sample_rate: u32,
    sequence: u64,
    samples: usize,
    voice: VoiceIdentity,
}
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    Complete,
    Truncated,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Terminal {
    outcome: Completion,
    chunks: u64,
    samples: usize,
    sample_rate: u32,
    voice: VoiceIdentity,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioReply {
    request_id: Uuid,
    session_id: Uuid,
    capture_epoch: u64,
    lane: String,
    model_revision: String,
    chunk: Chunk,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndReply {
    request_id: Uuid,
    session_id: Uuid,
    capture_epoch: u64,
    lane: String,
    model_revision: String,
    result: Terminal,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Reply {
    Audio(AudioReply),
    End(EndReply),
}
pub enum SpeechEvent {
    /// Always 24kHz mono PCM. Session/epoch here identify the private worker,
    /// not the paired PC; the controller must retain that independent binding.
    Audio {
        utterance_id: Uuid,
        session_id: Uuid,
        playback_epoch: u64,
        sequence: u64,
        samples: Vec<i16>,
        voice: VoiceIdentity,
    },
    End {
        utterance_id: Uuid,
        session_id: Uuid,
        playback_epoch: u64,
        outcome: Completion,
        samples: usize,
        voice: VoiceIdentity,
    },
}
pub struct TtsStream {
    client: Arc<AudioClient>,
    permit: Option<OwnedSemaphorePermit>,
    socket: Option<UnixStream>,
    request_id: Uuid,
    utterance_id: Uuid,
    epoch: u64,
    started: Instant,
    issued: u64,
    chunks: u64,
    samples: usize,
    sent: bool,
    failed: bool,
    complete: bool,
    voice: VoiceIdentity,
    retirement: Retirement,
}
// The source roster may compact only after actual private retirement. A failed
// cancellation intentionally leaves a nonzero uncertainty count, without leaking
// the stream/permit or treating its bounded cleanup wait as terminal evidence.
#[derive(Default)]
struct Retirement {
    counter: Option<Arc<AtomicUsize>>,
    retired: bool,
}
impl Retirement {
    fn new(counter: Option<Arc<AtomicUsize>>) -> Result<Self, ErrorCode> {
        if let Some(value) = &counter {
            value
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
                .map_err(|_| ErrorCode::TooLarge)?;
        }
        Ok(Self {
            counter,
            retired: true,
        })
    }
    fn confirm(&mut self) {
        self.retired = true;
    }
}
impl Drop for Retirement {
    fn drop(&mut self) {
        if self.retired
            && let Some(value) = self.counter.take()
        {
            value.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
impl AudioClient {
    pub async fn synthesize<F, Fut>(
        self: &Arc<Self>,
        epoch: u64,
        utterance_id: Uuid,
        voice: &VoiceIdentity,
        text: &str,
        retirement: Option<Arc<AtomicUsize>>,
        authorize: F,
    ) -> Result<TtsStream, ErrorCode>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), ErrorCode>>,
    {
        let retirement = Retirement::new(retirement)?;
        voice.validate()?;
        if utterance_id.is_nil() || epoch != self.epoch.load(Ordering::SeqCst) {
            return Err(ErrorCode::Stale);
        }
        if text.trim().is_empty()
            || text.len() > 512
            || self
                .deployment
                .as_ref()
                .is_none_or(|(lane, _)| lane != "tts")
        {
            return Err(ErrorCode::Malformed);
        }
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::Unavailable)?;
        let health = self.health().await?;
        if !health.streaming
            || health.state != "loaded_unqualified"
            || health.busy
            || epoch != self.epoch.load(Ordering::SeqCst)
        {
            return Err(ErrorCode::Unavailable);
        }
        {
            let mut recent = self
                .recent_streams
                .lock()
                .map_err(|_| ErrorCode::Unavailable)?;
            recent.retain(|_, until| *until > Instant::now());
            if recent.contains_key(&utterance_id) || recent.len() >= 128 {
                return Err(ErrorCode::Stale);
            }
            recent.insert(utterance_id, Instant::now() + Duration::from_secs(31));
        }
        let request_id = Uuid::new_v4();
        let mut request = self.request("tts_stream", request_id, epoch, 30_000)?;
        let issued = request["issued_at_ms"]
            .as_u64()
            .ok_or(ErrorCode::Malformed)?;
        request["payload"] = serde_json::json!({"text":text,"voice":voice});
        let encoded = serde_json::to_vec(&request).map_err(|_| ErrorCode::Malformed)?;
        drop(request);
        let mut stream = TtsStream {
            client: self.clone(),
            permit: Some(permit),
            socket: None,
            request_id,
            utterance_id,
            epoch,
            started: Instant::now(),
            issued,
            chunks: 0,
            samples: 0,
            sent: false,
            failed: false,
            complete: false,
            voice: voice.clone(),
            retirement,
        };
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut socket = UnixStream::connect(&self.socket)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            if socket.peer_cred().map_err(|_| ErrorCode::Denied)?.uid() != self.uid {
                return Err(ErrorCode::Denied);
            }
            // The caller's exact source/device/actor/output check follows all
            // preparation/peer awaits and immediately precedes request send.
            authorize().await?;
            stream.current()?;
            stream.sent = true;
            stream.retirement.retired = false;
            socket
                .write_u32(encoded.len() as u32)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            socket
                .write_all(&encoded)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            stream.socket = Some(socket);
            stream.current()
        })
        .await
        .map_err(|_| ErrorCode::Expired)??;
        Ok(stream)
    }
}
impl TtsStream {
    fn current(&self) -> Result<(), ErrorCode> {
        let elapsed = self.started.elapsed();
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrorCode::Expired)?
            .as_millis();
        if self.epoch != self.client.epoch.load(Ordering::SeqCst)
            || elapsed >= Duration::from_secs(30)
            || wall.abs_diff(self.issued as u128 + elapsed.as_millis()) > 1000
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    pub async fn next(&mut self) -> Result<SpeechEvent, ErrorCode> {
        if self.failed || self.complete {
            return Err(ErrorCode::Stale);
        }
        self.failed = true; // Cancellation mid-frame cannot retry on a partial socket.
        self.current()?;
        let remaining = Duration::from_secs(30).saturating_sub(self.started.elapsed());
        let budget = if self.chunks == 0 {
            remaining
        } else {
            remaining.min(Duration::from_secs(2))
        };
        let socket = self.socket.as_mut().ok_or(ErrorCode::Unavailable)?;
        let reply: Reply = tokio::time::timeout(budget, async {
            let size = socket
                .read_u32()
                .await
                .map_err(|_| ErrorCode::Unavailable)? as usize;
            if size == 0 || size > 16_384 {
                return Err(ErrorCode::TooLarge);
            }
            let mut bytes = vec![0; size];
            socket
                .read_exact(&mut bytes)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            serde_json::from_slice(&bytes).map_err(|_| ErrorCode::Malformed)
        })
        .await
        .map_err(|_| ErrorCode::Expired)??;
        self.current()?;
        let (request, session, epoch, lane, revision) = match &reply {
            Reply::Audio(value) => (
                value.request_id,
                value.session_id,
                value.capture_epoch,
                &value.lane,
                &value.model_revision,
            ),
            Reply::End(value) => (
                value.request_id,
                value.session_id,
                value.capture_epoch,
                &value.lane,
                &value.model_revision,
            ),
        };
        if request != self.request_id
            || session != self.client.session_id
            || epoch != self.epoch
            || lane != "tts"
            || Some(revision.as_str()) != self.client.configured_revision()
        {
            return Err(ErrorCode::Stale);
        }
        let event = match reply {
            Reply::Audio(value) => {
                let chunk = value.chunk;
                if chunk.sequence != self.chunks + 1
                    || chunk.voice != self.voice
                    || self.chunks >= 375
                    || chunk.sample_rate != 24_000
                    || chunk.samples != 1920
                    || chunk.pcm_s16le.len() != 5120
                {
                    return Err(ErrorCode::Malformed);
                }
                let bytes = STANDARD
                    .decode(chunk.pcm_s16le)
                    .map_err(|_| ErrorCode::Malformed)?;
                if bytes.len() != 3840 {
                    return Err(ErrorCode::Malformed);
                }
                let samples = bytes
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect();
                self.chunks += 1;
                self.samples += 1920;
                SpeechEvent::Audio {
                    utterance_id: self.utterance_id,
                    session_id: session,
                    playback_epoch: epoch,
                    sequence: self.chunks,
                    samples,
                    voice: self.voice.clone(),
                }
            }
            Reply::End(value) => {
                let terminal = value.result;
                if self.chunks == 0
                    || terminal.voice != self.voice
                    || terminal.chunks != self.chunks
                    || terminal.samples != self.samples
                    || terminal.sample_rate != 24_000
                {
                    return Err(ErrorCode::Malformed);
                }
                self.complete = true;
                self.socket.take();
                self.permit.take();
                self.retirement.confirm();
                self.retirement = Retirement::default();
                SpeechEvent::End {
                    utterance_id: self.utterance_id,
                    session_id: session,
                    playback_epoch: epoch,
                    outcome: terminal.outcome,
                    samples: self.samples,
                    voice: self.voice.clone(),
                }
            }
        };
        self.failed = false;
        Ok(event)
    }
}
impl Drop for TtsStream {
    fn drop(&mut self) {
        let Some(permit) = self.permit.take() else {
            return;
        };
        if !self.sent {
            return;
        }
        self.socket.take();
        let client = self.client.clone();
        let request = self.request_id;
        let remaining = Duration::from_secs(31).saturating_sub(self.started.elapsed());
        let mut retirement = std::mem::take(&mut self.retirement);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _permit = permit;
                // Success acknowledges actual child reset/CUDA retirement or
                // completed kill/reap; mere socket closure never releases this permit.
                if client.cancel(request).await.is_err() {
                    tokio::time::sleep(remaining).await;
                } else {
                    retirement.confirm();
                }
            });
        } else {
            self.client.admission.close();
        }
    }
}
