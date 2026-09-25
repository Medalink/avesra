//! Versioned data contracts. Validation is not peer authentication.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_CONTROL_BYTES: usize = 65_536;
pub const MAX_ACTION_AGE_MS: u64 = 30_000;
pub const MAX_AUDIO_FRAME_BYTES: usize = 640;
pub const MAX_AUDIO_QUEUE: usize = 64;
pub const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MEDIA_TTL_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    #[error("unsupported protocol version")]
    Version,
    #[error("malformed message")]
    Malformed,
    #[error("message exceeds its size limit")]
    TooLarge,
    #[error("stale session, sequence, or epoch")]
    Stale,
    #[error("action expired or deadline is invalid")]
    Expired,
    #[error("authentication or enrollment is required")]
    Unauthenticated,
    #[error("permission denied")]
    Denied,
    #[error("exact current approval is required")]
    ApprovalRequired,
    #[error("service is unavailable")]
    Unavailable,
    #[error("operation is unsupported")]
    Unsupported,
    #[error("state transition is invalid")]
    InvalidTransition,
    #[error("storage operation failed")]
    Storage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Asr,
    Speaker,
    Planning,
    Vision,
    Tts,
    Decision,
    Memory,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Configured,
    Starting,
    Loading,
    Ready,
    Degraded,
    Unavailable,
    Incompatible,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    Spark,
    Client,
    CloudJev,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    #[default]
    SingleSpark,
    Accelerated,
    Gaming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    pub id: Uuid,
    pub lane: Lane,
    pub driver: String,
    pub model: String,
    pub revision: String,
    pub locality: Locality,
    pub health: Health,
    pub streaming: bool,
    pub cancellable: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    LaunchApp,
    SetVolume,
    ReadPage,
    Navigate,
    FillPrompt,
    SubmitPrompt,
    Send,
    Publish,
    Delete,
    Spend,
    ChangePermission,
    ChangeConfiguration,
    Diagnostic,
    ConnectVpn,
}
impl Operation {
    pub fn needs_approval(self) -> bool {
        matches!(
            self,
            Self::Send
                | Self::Publish
                | Self::Delete
                | Self::Spend
                | Self::ChangePermission
                | Self::ChangeConfiguration
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub grant_id: Uuid,
    pub operation: Operation,
    pub approval_id: Option<Uuid>,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ControlMessage {
    Hello {
        capabilities: Vec<Lane>,
    },
    Ping,
    Cancel {
        task_id: Uuid,
    },
    Action(Action),
    Mode {
        muted: bool,
        deafened: bool,
        paused: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub version: u16,
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub request_id: Uuid,
    pub sequence: u64,
    pub capture_epoch: u64,
    pub action_epoch: u64,
    pub message: ControlMessage,
}

/// Construct only after authenticating a peer and completing the epoch handshake.
pub struct SessionContext {
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub last_sequence: u64,
    pub capture_epoch: u64,
    pub action_epoch: u64,
}

pub fn decode_control(bytes: &[u8]) -> Result<Envelope, ErrorCode> {
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(ErrorCode::TooLarge);
    }
    let value: Envelope = serde_json::from_slice(bytes).map_err(|_| ErrorCode::Malformed)?;
    if value.version != PROTOCOL_VERSION {
        return Err(ErrorCode::Version);
    }
    if value.device_id.is_nil() || value.session_id.is_nil() || value.request_id.is_nil() {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}

impl Envelope {
    pub fn validate(&self, session: &SessionContext, now_ms: u64) -> Result<(), ErrorCode> {
        if self.version != PROTOCOL_VERSION {
            return Err(ErrorCode::Version);
        }
        if self.device_id.is_nil() || self.session_id.is_nil() || self.request_id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        if self.device_id != session.device_id
            || self.session_id != session.session_id
            || self.sequence <= session.last_sequence
            || self.capture_epoch != session.capture_epoch
            || self.action_epoch != session.action_epoch
        {
            return Err(ErrorCode::Stale);
        }
        if let ControlMessage::Action(action) = &self.message {
            if [
                action.task_id,
                action.step_id,
                action.actor_id,
                action.target_id,
                action.grant_id,
            ]
            .iter()
            .any(Uuid::is_nil)
            {
                return Err(ErrorCode::Malformed);
            }
            if action.issued_at_ms > now_ms
                || now_ms >= action.expires_at_ms
                || action.expires_at_ms.saturating_sub(action.issued_at_ms) > MAX_ACTION_AGE_MS
            {
                return Err(ErrorCode::Expired);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Failed,
    NeedsInput,
    Cancelled,
    Unsupported,
    UnknownEffect,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Proposed,
    AwaitingApproval,
    Queued,
    Running,
    WaitingForUser,
    Suspended,
    Succeeded,
    Failed,
    Cancelled,
    UnknownEffect,
}
impl TaskState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use TaskState::*;
        matches!(
            (self, next),
            (Proposed, AwaitingApproval | Queued | Cancelled | Failed)
                | (AwaitingApproval, Queued | Cancelled | Failed)
                | (Queued, Running | Suspended | Cancelled | Failed)
                | (
                    Running,
                    WaitingForUser | Suspended | Succeeded | Failed | Cancelled | UnknownEffect
                )
                | (
                    WaitingForUser,
                    Queued | Suspended | Cancelled | Failed | UnknownEffect
                )
                | (Suspended, Queued | Cancelled | Failed | UnknownEffect)
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceStage {
    Capture,
    Transport,
    Asr,
    Identity,
    Intent,
    Planning,
    Policy,
    Dispatch,
    Execution,
    Verification,
    Tts,
    Playback,
    Memory,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageMetric {
    pub correlation_id: Uuid,
    pub stage: TraceStage,
    pub host_id: Uuid,
    pub deployment_id: Option<Uuid>,
    pub config_revision: Uuid,
    pub queue_micros: u64,
    pub active_micros: u64,
    pub outcome: Outcome,
    pub retry_count: u16,
    pub error: Option<ErrorCode>,
}
