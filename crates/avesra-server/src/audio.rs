//! Same-user local audio driver client. This is not an owner authority.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use std::{
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    sync::Semaphore,
};
use uuid::Uuid;
const MAX_PACKET: usize = 2_000_000;
#[derive(Serialize)]
#[serde(untagged)]
pub enum AudioInput {
    Pcm { pcm_s16le: String },
    Speech { text: String },
    Design { text: String, description: String },
}
impl AudioInput {
    fn validate(&self) -> Result<(), ErrorCode> {
        let valid = match self {
            Self::Pcm { pcm_s16le } => {
                !pcm_s16le.is_empty()
                    && pcm_s16le.len() <= 1_280_000
                    && pcm_s16le
                        .bytes()
                        .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'+' | b'/' | b'='))
            }
            Self::Speech { text } => !text.trim().is_empty() && text.len() <= 512,
            Self::Design { text, description } => {
                !text.trim().is_empty()
                    && text.len() <= 512
                    && !description.trim().is_empty()
                    && description.len() <= 1024
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        }
    }
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum AudioOutput {
    Embedding {
        outcome: String,
        embedding: Vec<f32>,
    },
    Insufficient {
        outcome: String,
    },
    Transcript {
        text: String,
        r#final: bool,
    },
    Speech {
        pcm_s16le: String,
        sample_rate: u32,
    },
}
impl AudioOutput {
    fn validate(&self) -> Result<(), ErrorCode> {
        let valid = match self {
            Self::Embedding { outcome, embedding } => {
                outcome == "embedding"
                    && embedding.len() == 192
                    && embedding.iter().all(|v| v.is_finite())
            }
            Self::Insufficient { outcome } => outcome == "insufficient_speech",
            Self::Transcript { text, r#final } => text.len() <= 8192 && *r#final,
            Self::Speech {
                pcm_s16le,
                sample_rate,
            } => {
                !pcm_s16le.is_empty()
                    && pcm_s16le.len() <= 1_920_000
                    && (8000..=48000).contains(sample_rate)
                    && pcm_s16le
                        .bytes()
                        .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'+' | b'/' | b'='))
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        }
    }
}
/// Carries controller correlation across the async worker boundary; never logs content.
pub struct AudioResult {
    pub request_id: Uuid,
    pub utterance_id: Uuid,
    pub session_id: Uuid,
    pub capture_epoch: u64,
    pub output: AudioOutput,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioHealth {
    pub version: u16,
    pub lane: String,
    pub model_revision: String,
    pub state: String,
    pub streaming: bool,
    pub cancellation: String,
    pub permission_authority: bool,
    pub busy: bool,
    pub successful_inferences: u64,
    pub last_inference_ms: Option<f64>,
}
impl AudioHealth {
    fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != 1
            || !matches!(
                self.lane.as_str(),
                "asr" | "speaker" | "tts" | "voice-design"
            )
            || self.model_revision.len() != 40
            || !self.model_revision.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(
                self.state.as_str(),
                "unavailable" | "loading" | "loaded_unqualified" | "termination_pending"
            )
            || self.permission_authority
            || self.streaming
            || self.cancellation != "terminate_process"
            || self
                .last_inference_ms
                .is_some_and(|v| !v.is_finite() || !(0.0..=30_000.0).contains(&v))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
pub struct AudioClient {
    socket: PathBuf,
    uid: u32,
    session_id: Uuid,
    sequence: AtomicU64,
    epoch: AtomicU64,
    admission: Arc<Semaphore>,
    deployment: Option<(String, String)>,
}
impl AudioClient {
    pub fn new(socket: &Path) -> Result<Self, ErrorCode> {
        let metadata = std::fs::symlink_metadata(socket).map_err(|_| ErrorCode::Unavailable)?;
        let own_uid = std::fs::metadata("/proc/self")
            .map_err(|_| ErrorCode::Unavailable)?
            .uid();
        let parent = std::fs::symlink_metadata(socket.parent().ok_or(ErrorCode::Malformed)?)
            .map_err(|_| ErrorCode::Unavailable)?;
        if !socket.is_absolute()
            || !metadata.file_type().is_socket()
            || metadata.uid() != own_uid
            || metadata.mode() & 0o077 != 0
            || parent.uid() != own_uid
            || parent.mode() & 0o077 != 0
            || !parent.is_dir()
        {
            return Err(ErrorCode::Denied);
        }
        Ok(Self {
            socket: socket.into(),
            uid: own_uid,
            session_id: Uuid::new_v4(),
            sequence: AtomicU64::new(0),
            epoch: AtomicU64::new(1),
            admission: Arc::new(Semaphore::new(1)),
            deployment: None,
        })
    }
    pub fn for_deployment(socket: &Path, lane: &str, revision: &str) -> Result<Self, ErrorCode> {
        if !matches!(lane, "asr" | "speaker" | "tts" | "voice-design")
            || revision.len() != 40
            || !revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ErrorCode::Malformed);
        }
        let mut value = Self::new(socket)?;
        value.deployment = Some((lane.into(), revision.into()));
        Ok(value)
    }
    pub fn invalidate(&self, epoch: u64) -> Result<(), ErrorCode> {
        self.epoch
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                (epoch > old).then_some(epoch)
            })
            .map_err(|_| ErrorCode::Stale)?;
        Ok(())
    }
    async fn exchange(
        &self,
        request: serde_json::Value,
        deadline: Duration,
    ) -> Result<serde_json::Value, ErrorCode> {
        let encoded = serde_json::to_vec(&request).map_err(|_| ErrorCode::Malformed)?;
        if encoded.len() > MAX_PACKET {
            return Err(ErrorCode::TooLarge);
        }
        tokio::time::timeout(deadline, async {
            let mut connection = UnixStream::connect(&self.socket)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            if connection.peer_cred().map_err(|_| ErrorCode::Denied)?.uid() != self.uid {
                return Err(ErrorCode::Denied);
            }
            connection
                .write_u32(encoded.len() as u32)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            connection
                .write_all(&encoded)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            let length = connection
                .read_u32()
                .await
                .map_err(|_| ErrorCode::Unavailable)? as usize;
            if length == 0 || length > MAX_PACKET {
                return Err(ErrorCode::TooLarge);
            }
            let mut body = vec![0; length];
            connection
                .read_exact(&mut body)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            let value: serde_json::Value =
                serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
            if value.get("error").is_some() {
                return Err(ErrorCode::Unavailable);
            }
            Ok(value)
        })
        .await
        .map_err(|_| ErrorCode::Expired)?
    }
    fn request(
        &self,
        operation: &str,
        request_id: Uuid,
        epoch: u64,
        lifetime: u64,
    ) -> Result<serde_json::Value, ErrorCode> {
        let sequence = self
            .sequence
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| old.checked_add(1))
            .map_err(|_| ErrorCode::Stale)?
            + 1;
        let issued = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| ErrorCode::Unavailable)?
                .as_millis(),
        )
        .map_err(|_| ErrorCode::Unavailable)?;
        let expires = issued.checked_add(lifetime).ok_or(ErrorCode::Expired)?;
        Ok(
            serde_json::json!({"version":1,"operation":operation,"request_id":request_id,"session_id":self.session_id,"capture_epoch":epoch,"sequence":sequence,"issued_at_ms":issued,"expires_at_ms":expires}),
        )
    }
    pub async fn health(&self) -> Result<AudioHealth, ErrorCode> {
        let value = self
            .exchange(
                serde_json::json!({"version":1,"operation":"health"}),
                Duration::from_secs(3),
            )
            .await?;
        let health: AudioHealth =
            serde_json::from_value(value).map_err(|_| ErrorCode::Malformed)?;
        health.validate()?;
        if self.deployment.as_ref().is_some_and(|(lane, revision)| {
            health.lane != *lane || health.model_revision != *revision
        }) {
            return Err(ErrorCode::Unsupported);
        }
        Ok(health)
    }
    pub async fn load(&self) -> Result<(), ErrorCode> {
        let _permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::Unavailable)?;
        let epoch = self.epoch.load(Ordering::SeqCst);
        let result = self
            .exchange(
                self.request("load", Uuid::new_v4(), epoch, 120_000)?,
                Duration::from_secs(120),
            )
            .await?;
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return Err(ErrorCode::Stale);
        }
        if result != serde_json::json!({"state":"loaded_unqualified"}) {
            return Err(ErrorCode::Unavailable);
        }
        Ok(())
    }
    /// Payload remains transient and may only be supplied by the bounded media
    /// controller. The result is not accepted identity or authorization.
    pub async fn infer(
        &self,
        request_id: Uuid,
        epoch: u64,
        utterance_id: Uuid,
        payload: AudioInput,
    ) -> Result<AudioResult, ErrorCode> {
        if request_id.is_nil()
            || utterance_id.is_nil()
            || epoch != self.epoch.load(Ordering::SeqCst)
        {
            return Err(ErrorCode::Stale);
        }
        let (lane, revision) = self.deployment.as_ref().ok_or(ErrorCode::Unavailable)?;
        let _permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::Unavailable)?;
        let mut request = self.request("infer", request_id, epoch, 30_000)?;
        payload.validate()?;
        request["payload"] = serde_json::to_value(payload).map_err(|_| ErrorCode::Malformed)?;
        let result = self.exchange(request, Duration::from_secs(30)).await?;
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return Err(ErrorCode::Stale);
        }
        let object = result.as_object().ok_or(ErrorCode::Malformed)?;
        if object.len() != 3
            || !object.contains_key("result")
            || object.get("lane").and_then(|v| v.as_str()) != Some(lane.as_str())
            || object.get("model_revision").and_then(|v| v.as_str()) != Some(revision.as_str())
        {
            return Err(ErrorCode::Malformed);
        }
        let output: AudioOutput =
            serde_json::from_value(object["result"].clone()).map_err(|_| ErrorCode::Malformed)?;
        output.validate()?;
        if !matches!(
            (lane.as_str(), &output),
            ("asr", AudioOutput::Transcript { .. })
                | (
                    "speaker",
                    AudioOutput::Embedding { .. } | AudioOutput::Insufficient { .. }
                )
                | ("tts" | "voice-design", AudioOutput::Speech { .. })
        ) {
            return Err(ErrorCode::Malformed);
        }
        Ok(AudioResult {
            request_id,
            utterance_id,
            session_id: self.session_id,
            capture_epoch: epoch,
            output,
        })
    }
    pub async fn cancel(&self, request_id: Uuid) -> Result<(), ErrorCode> {
        if request_id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let epoch = self.epoch.load(Ordering::SeqCst);
        let value = self
            .exchange(
                self.request("cancel", request_id, epoch, 3_000)?,
                Duration::from_secs(3),
            )
            .await?;
        if value != serde_json::json!({"outcome":"cancelled"}) {
            return Err(ErrorCode::Unavailable);
        }
        Ok(())
    }
}
