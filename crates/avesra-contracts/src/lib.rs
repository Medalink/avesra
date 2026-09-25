//! Versioned data contracts. Validation is not peer authentication.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod media;
pub mod preview;
pub mod voice;
pub mod voices;

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_CONTROL_BYTES: usize = 65_536;
pub const MAX_ACTION_AGE_MS: u64 = 30_000;
pub const MAX_AUDIO_FRAME_BYTES: usize = 640;
pub const MAX_AUDIO_QUEUE: usize = 64;
pub const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MEDIA_TTL_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerStatus {
    pub version: u16,
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub sequence: u64,
    pub request_id: Uuid,
    pub request_sequence: u64,
    pub capture_epoch: u64,
    pub playback_epoch: u64,
    pub action_epoch: u64,
    pub status: ConnectionStatus,
    pub muted: bool,
    pub deafened: bool,
    pub paused: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    ConnectedOwnerSetupRequired,
    RejectedOwnerSetupRequired,
}

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
                | Self::ConnectVpn
        )
    }
}

/// Effect arguments are bounded data, never executable shell or JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionPayload {
    LaunchApp {
        app_id: Uuid,
    },
    SetVolume {
        percent: u8,
    },
    Navigate {
        url: String,
    },
    ReadPage {
        origin: String,
        message_limit: u16,
    },
    FillPrompt {
        app_id: Uuid,
        project_id: Uuid,
        text: String,
    },
    SubmitPrompt {
        app_id: Uuid,
        project_id: Uuid,
        expected_text: String,
    },
    Diagnostic {
        catalog_entry: Uuid,
    },
    ConnectVpn {
        profile_id: Uuid,
    },
    /// References an immutable, owner-visible proposal in the ledger. Its full
    /// concrete payload must match this digest before any executor may use it.
    ApprovedProposal {
        operation: Operation,
        proposal_id: Uuid,
        revision: Uuid,
        sha256: [u8; 32],
    },
}
impl ActionPayload {
    pub fn operation(&self) -> Operation {
        match self {
            Self::LaunchApp { .. } => Operation::LaunchApp,
            Self::SetVolume { .. } => Operation::SetVolume,
            Self::Navigate { .. } => Operation::Navigate,
            Self::ReadPage { .. } => Operation::ReadPage,
            Self::FillPrompt { .. } => Operation::FillPrompt,
            Self::SubmitPrompt { .. } => Operation::SubmitPrompt,
            Self::Diagnostic { .. } => Operation::Diagnostic,
            Self::ConnectVpn { .. } => Operation::ConnectVpn,
            Self::ApprovedProposal { operation, .. } => *operation,
        }
    }
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let valid = match self {
            Self::LaunchApp { app_id } => !app_id.is_nil(),
            Self::SetVolume { percent } => *percent <= 100,
            Self::Navigate { url } => canonical_https(url, false),
            Self::ReadPage {
                origin,
                message_limit,
            } => canonical_https(origin, true) && (1..=100).contains(message_limit),
            Self::FillPrompt {
                app_id,
                project_id,
                text,
            } => {
                !app_id.is_nil() && !project_id.is_nil() && !text.is_empty() && text.len() <= 16_384
            }
            Self::SubmitPrompt {
                app_id,
                project_id,
                expected_text,
            } => {
                !app_id.is_nil()
                    && !project_id.is_nil()
                    && !expected_text.is_empty()
                    && expected_text.len() <= 16_384
            }
            Self::Diagnostic { catalog_entry } => !catalog_entry.is_nil(),
            Self::ConnectVpn { profile_id } => !profile_id.is_nil(),
            Self::ApprovedProposal {
                operation,
                proposal_id,
                revision,
                sha256,
            } => {
                operation.needs_approval()
                    && !proposal_id.is_nil()
                    && !revision.is_nil()
                    && sha256.iter().any(|b| *b != 0)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        }
    }
}

/// Payloads must already be canonical so approval equality is unambiguous.
fn canonical_https(value: &str, origin_only: bool) -> bool {
    if value.len() > 2048 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Ok(parsed) = url::Url::parse(value) else {
        return false;
    };
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    if origin_only {
        value == parsed.origin().ascii_serialization()
    } else {
        value == parsed.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub grant_id: Uuid,
    pub revision: Uuid,
    pub intent_revision: Uuid,
    pub payload: ActionPayload,
    pub approval_id: Option<Uuid>,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

impl Action {
    pub fn validate(&self, now_ms: u64) -> Result<(), ErrorCode> {
        if [
            self.task_id,
            self.step_id,
            self.actor_id,
            self.target_id,
            self.grant_id,
            self.revision,
            self.intent_revision,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.approval_id.is_some_and(|id| id.is_nil())
        {
            return Err(ErrorCode::Malformed);
        }
        if self.issued_at_ms > now_ms
            || now_ms >= self.expires_at_ms
            || self.expires_at_ms.saturating_sub(self.issued_at_ms) > MAX_ACTION_AGE_MS
        {
            return Err(ErrorCode::Expired);
        }
        self.payload.validate()
    }
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
    pub playback_epoch: u64,
    pub action_epoch: u64,
    pub message: ControlMessage,
}

/// Construct only after authenticating a peer and completing the epoch handshake.
pub struct SessionContext {
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub last_sequence: u64,
    pub capture_epoch: u64,
    pub playback_epoch: u64,
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
    if value.device_id.is_nil()
        || value.session_id.is_nil()
        || value.request_id.is_nil()
        || value.capture_epoch == 0
        || value.action_epoch == 0
        || value.playback_epoch == 0
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}

impl Envelope {
    pub fn validate(&self, session: &SessionContext, now_ms: u64) -> Result<(), ErrorCode> {
        if self.version != PROTOCOL_VERSION {
            return Err(ErrorCode::Version);
        }
        if self.device_id.is_nil()
            || self.session_id.is_nil()
            || self.request_id.is_nil()
            || self.capture_epoch == 0
            || self.action_epoch == 0
            || self.playback_epoch == 0
        {
            return Err(ErrorCode::Malformed);
        }
        if self.device_id != session.device_id
            || self.session_id != session.session_id
            || self.sequence <= session.last_sequence
            || self.capture_epoch != session.capture_epoch
            || self.playback_epoch != session.playback_epoch
            || self.action_epoch != session.action_epoch
        {
            return Err(ErrorCode::Stale);
        }
        match &self.message {
            ControlMessage::Action(action) => action.validate(now_ms)?,
            ControlMessage::Cancel { task_id } if task_id.is_nil() => {
                return Err(ErrorCode::Malformed);
            }
            ControlMessage::Hello { capabilities }
                if capabilities.len() > 7
                    || capabilities
                        .iter()
                        .enumerate()
                        .any(|(i, lane)| capabilities[..i].contains(lane)) =>
            {
                return Err(ErrorCode::Malformed);
            }
            _ => {}
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
