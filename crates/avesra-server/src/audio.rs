//! Same-user local audio driver client. This is not an owner authority.
use avesra_contracts::ErrorCode;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
#[path = "audio_activity.rs"]
pub mod activity;
#[path = "audio_stream.rs"]
pub mod streaming;
#[path = "audio_tts.rs"]
pub mod synthesis;
#[path = "audio_voices.rs"]
pub mod voices;
fn valid_pcm(encoded: &str, min_bytes: usize, max_bytes: usize) -> bool {
    if encoded.len() > max_bytes.div_ceil(3) * 4 {
        return false;
    }
    STANDARD.decode(encoded).is_ok_and(|bytes| {
        bytes.len() >= min_bytes && bytes.len() <= max_bytes && bytes.len().is_multiple_of(2)
    })
}
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
            Self::Pcm { pcm_s16le } => valid_pcm(pcm_s16le, 320, 960_000),
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
    Activity {
        activity: avesra_contracts::activity::Activity,
    },
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
            Self::Activity { activity } => activity.validate(activity.samples).is_ok(),
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
                (8000..=48000).contains(sample_rate)
                    && valid_pcm(
                        pcm_s16le,
                        2,
                        ((*sample_rate as usize * 30).min(720_000)) * 2,
                    )
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
                "asr" | "speaker" | "tts" | "voice-design" | "activity"
            )
            || self.model_revision.len() != 40
            || !self.model_revision.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(
                self.state.as_str(),
                "unavailable" | "loading" | "loaded_unqualified" | "termination_pending"
            )
            || self.permission_authority
            || (self.streaming && !matches!(self.lane.as_str(), "asr" | "tts" | "activity"))
            || !matches!(
                self.cancellation.as_str(),
                "terminate_process" | "cooperative_reset_or_terminate"
            )
            || (self.cancellation == "cooperative_reset_or_terminate"
                && !matches!(self.lane.as_str(), "activity" | "tts"))
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
    load_attempted: AtomicBool,
    recent_streams: std::sync::Mutex<std::collections::HashMap<Uuid, std::time::Instant>>,
}
struct LoadRetirement {
    client: Arc<AudioClient>,
    uncertain: bool,
}
impl Drop for LoadRetirement {
    fn drop(&mut self) {
        if self.uncertain {
            self.client.admission.close();
        }
    }
}
impl AudioClient {
    fn observed_admission(
        &self,
        operation: avesra_core::engine_observer::Operation,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, ErrorCode> {
        let result = self.admission.clone().try_acquire_owned();
        avesra_core::engine_observer::admission(
            avesra_core::engine_observer::Lane::audio(
                self.deployment.as_ref().map(|(lane, _)| lane.as_str()),
            ),
            operation,
            matches!(&result, Err(tokio::sync::TryAcquireError::NoPermits)),
            matches!(&result, Err(tokio::sync::TryAcquireError::Closed)),
        );
        result.map_err(|_| ErrorCode::Unavailable)
    }
    pub fn new(socket: &Path) -> Result<Self, ErrorCode> {
        let value = Self::configured_socket(socket)?;
        value.socket_identity()?;
        Ok(value)
    }
    fn configured_socket(socket: &Path) -> Result<Self, ErrorCode> {
        use std::os::unix::ffi::OsStrExt;
        let bytes = socket.as_os_str().as_bytes();
        if !socket.is_absolute()
            || bytes.len() >= 108
            || bytes.contains(&0)
            || socket.file_name().is_none()
            || socket
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || socket
                .components()
                .collect::<PathBuf>()
                .as_os_str()
                .as_bytes()
                != bytes
        {
            return Err(ErrorCode::Malformed);
        }
        let own_uid = std::fs::metadata("/proc/self")
            .map_err(|_| ErrorCode::Unavailable)?
            .uid();
        let value = Self {
            socket: socket.into(),
            uid: own_uid,
            session_id: Uuid::new_v4(),
            sequence: AtomicU64::new(0),
            epoch: AtomicU64::new(1),
            admission: Arc::new(Semaphore::new(1)),
            deployment: None,
            load_attempted: AtomicBool::new(false),
            recent_streams: std::sync::Mutex::new(std::collections::HashMap::new()),
        };
        // Services may not yet have created /run/avesra. Existing unsafe
        // objects still fail construction; absence never creates directories.
        value.check_path(true)?;
        Ok(value)
    }
    fn check_path(&self, allow_missing: bool) -> Result<Option<(u64, u64)>, ErrorCode> {
        let parent_path = self.socket.parent().ok_or(ErrorCode::Malformed)?;
        match std::fs::symlink_metadata(parent_path) {
            Ok(parent) => {
                if !parent.is_dir()
                    || parent.uid() != self.uid
                    || parent.mode() & 0o077 != 0
                    || std::fs::canonicalize(parent_path).map_err(|_| ErrorCode::Denied)?
                        != parent_path
                {
                    return Err(ErrorCode::Denied);
                }
            }
            Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(_) => return Err(ErrorCode::Unavailable),
        }
        match std::fs::symlink_metadata(&self.socket) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket()
                    || metadata.uid() != self.uid
                    || metadata.mode() & 0o077 != 0
                {
                    return Err(ErrorCode::Denied);
                }
                Ok(Some((metadata.dev(), metadata.ino())))
            }
            Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(ErrorCode::Unavailable),
        }
    }
    fn socket_identity(&self) -> Result<(u64, u64), ErrorCode> {
        self.check_path(false)?.ok_or(ErrorCode::Unavailable)
    }
    async fn connect_checked(&self) -> Result<UnixStream, ErrorCode> {
        self.connect_checked_to(None).await
    }
    async fn connect_checked_to(
        &self,
        expected: Option<(u64, u64)>,
    ) -> Result<UnixStream, ErrorCode> {
        let identity = self.socket_identity()?;
        if expected.is_some_and(|expected| expected != identity) {
            return Err(ErrorCode::Stale);
        }
        let connection = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| ErrorCode::Unavailable)?;
        if connection.peer_cred().map_err(|_| ErrorCode::Denied)?.uid() != self.uid
            || self.socket_identity()? != identity
        {
            return Err(ErrorCode::Denied);
        }
        Ok(connection)
    }
    pub fn for_deployment(socket: &Path, lane: &str, revision: &str) -> Result<Self, ErrorCode> {
        if !matches!(
            lane,
            "asr" | "speaker" | "tts" | "voice-design" | "activity"
        ) || revision.len() != 40
            || !revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ErrorCode::Malformed);
        }
        let mut value = Self::configured_socket(socket)?;
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
    pub fn configured_revision(&self) -> Option<&str> {
        self.deployment
            .as_ref()
            .map(|(_, revision)| revision.as_str())
    }
    async fn exchange(
        &self,
        request: serde_json::Value,
        deadline: Duration,
    ) -> Result<serde_json::Value, ErrorCode> {
        self.exchange_until(request, tokio::time::Instant::now() + deadline, None)
            .await
    }
    async fn exchange_until(
        &self,
        request: serde_json::Value,
        deadline: tokio::time::Instant,
        identity: Option<(u64, u64)>,
    ) -> Result<serde_json::Value, ErrorCode> {
        if tokio::time::Instant::now() >= deadline {
            return Err(ErrorCode::Expired);
        }
        let encoded = serde_json::to_vec(&request).map_err(|_| ErrorCode::Malformed)?;
        if encoded.len() > MAX_PACKET {
            return Err(ErrorCode::TooLarge);
        }
        tokio::time::timeout_at(deadline, async {
            let mut connection = self.connect_checked_to(identity).await?;
            if tokio::time::Instant::now() >= deadline {
                return Err(ErrorCode::Expired);
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
    pub fn load_attempted(&self) -> bool {
        self.load_attempted.load(Ordering::SeqCst)
    }
    /// Local uncertainty is distinct from a service-reported health state.
    pub fn load_quarantined(&self) -> bool {
        self.admission.is_closed()
    }
    pub async fn health(&self) -> Result<AudioHealth, ErrorCode> {
        if self.load_quarantined() {
            return Err(ErrorCode::Unavailable);
        }
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
        if self.load_quarantined() {
            return Err(ErrorCode::Unavailable);
        }
        Ok(health)
    }
    pub async fn startup_health(&self) -> Result<AudioHealth, ErrorCode> {
        let health = self.health().await?;
        if self.deployment.as_ref().is_some_and(|(lane, _)| {
            matches!(lane.as_str(), "activity" | "tts") && !health.streaming
        }) {
            return Err(ErrorCode::Unsupported);
        }
        Ok(health)
    }
    pub async fn load(self: &Arc<Self>) -> Result<(), ErrorCode> {
        let permit = self.observed_admission(avesra_core::engine_observer::Operation::Load)?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        let epoch = self.epoch.load(Ordering::SeqCst);
        let request_id = Uuid::new_v4();
        let request = self.request("load", request_id, epoch, 120_000)?;
        let client = self.clone();
        // Dropping the caller detaches, never aborts this actual owner. All
        // preparation and worker scheduling consume the original deadline.
        tokio::spawn(async move {
            let _permit = permit;
            let mut retirement = LoadRetirement {
                client: client.clone(),
                uncertain: false,
            };
            let identity = client.socket_identity()?;
            let health = tokio::time::timeout_at(deadline, client.startup_health())
                .await
                .map_err(|_| ErrorCode::Expired)??;
            if health.state == "loaded_unqualified" {
                return Ok(());
            }
            if health.busy || health.state != "unavailable" {
                return Err(ErrorCode::Unavailable);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ErrorCode::Expired);
            }
            if client.socket_identity()? != identity {
                return Err(ErrorCode::Stale);
            }
            if client.load_attempted.swap(true, Ordering::SeqCst) {
                return Err(ErrorCode::InvalidTransition);
            }
            retirement.uncertain = true;
            let result = client
                .exchange_until(request, deadline, Some(identity))
                .await
                .and_then(|value| {
                    if value != serde_json::json!({"state":"loaded_unqualified"}) {
                        Err(ErrorCode::Malformed)
                    } else {
                        Ok(())
                    }
                });
            if result.is_ok() {
                // The exact terminal settles loading. Subsequent health failure
                // must never cancel/unload that successfully retired operation.
                retirement.uncertain = false;
                if tokio::time::Instant::now() >= deadline {
                    return Err(ErrorCode::Expired);
                }
                if client.epoch.load(Ordering::SeqCst) != epoch {
                    return Err(ErrorCode::Stale);
                }
                if client.socket_identity()? != identity {
                    return Err(ErrorCode::Stale);
                }
                let health = tokio::time::timeout_at(deadline, client.startup_health())
                    .await
                    .map_err(|_| ErrorCode::Expired)??;
                if tokio::time::Instant::now() >= deadline {
                    return Err(ErrorCode::Expired);
                }
                if client.socket_identity()? != identity {
                    return Err(ErrorCode::Stale);
                }
                if health.state != "loaded_unqualified" {
                    return Err(ErrorCode::Unavailable);
                }
            } else {
                let cancel_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
                let cancel = client.request("cancel", request_id, epoch, 3_000)?;
                if client
                    .exchange_until(cancel, cancel_deadline, Some(identity))
                    .await
                    .is_ok_and(|value| value == serde_json::json!({"outcome":"cancelled"}))
                {
                    retirement.uncertain = false;
                }
                // Unknown request, changed socket and terminal/timeout races
                // leave the Drop guard armed; no next admission can enter.
            }
            result
        })
        .await
        .map_err(|_| {
            // An unexpected worker failure is not proof that remote work stopped.
            self.admission.close();
            ErrorCode::Unavailable
        })?
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
        self.infer_with_budget(
            request_id,
            epoch,
            utterance_id,
            payload,
            Duration::from_secs(30),
        )
        .await
    }
    pub async fn infer_with_budget(
        &self,
        request_id: Uuid,
        epoch: u64,
        utterance_id: Uuid,
        payload: AudioInput,
        budget: Duration,
    ) -> Result<AudioResult, ErrorCode> {
        if budget.is_zero() || budget > Duration::from_secs(30) {
            return Err(ErrorCode::Expired);
        }
        if request_id.is_nil()
            || utterance_id.is_nil()
            || epoch != self.epoch.load(Ordering::SeqCst)
        {
            return Err(ErrorCode::Stale);
        }
        let (lane, revision) = self.deployment.as_ref().ok_or(ErrorCode::Unavailable)?;
        let _permit = self.observed_admission(avesra_core::engine_observer::Operation::Infer)?;
        let mut request = self.request("infer", request_id, epoch, budget.as_millis() as u64)?;
        payload.validate()?;
        request["payload"] = serde_json::to_value(payload).map_err(|_| ErrorCode::Malformed)?;
        let result = self.exchange(request, budget).await?;
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
                | ("activity", AudioOutput::Activity { .. })
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
