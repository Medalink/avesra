//! Transient pre-acceptance observations, never planner or action authority.
use crate::{ErrorCode, browser::MAX_SAFE_COUNTER, planner};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: u16 = 2;
pub const POLICY: &str = "avesra-text-directedness-1";
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub device: Uuid,
    pub session: Uuid,
    pub capture_epoch: u64,
    pub action_epoch: u64,
    pub microphone: String,
    pub actor: Option<Uuid>,
    pub grant_revision: Option<Uuid>,
}
impl Context {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.device.is_nil()
            || self.session.is_nil()
            || [self.capture_epoch, self.action_epoch]
                .iter()
                .any(|v| *v == 0 || *v > MAX_SAFE_COUNTER)
            || self.microphone.is_empty()
            || self.microphone.len() > 1024
            || self.microphone.chars().any(char::is_control)
            || self.actor.is_some_and(|v| v.is_nil())
            || self.grant_revision.is_some_and(|v| v.is_nil())
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Inspect,
    Classify { utterance: Uuid, transcript: String },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request: Uuid,
    pub context: Context,
    pub remaining_ms: u64,
    pub operation: Operation,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.context.validate()?;
        if self.version != VERSION
            || self.request.is_nil()
            || self.remaining_ms == 0
            || self.remaining_ms > 30_000
        {
            return Err(ErrorCode::Malformed);
        }
        if let Operation::Classify {
            utterance,
            transcript,
        } = &self.operation
            && (utterance.is_nil() || !planner::valid_text(transcript))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn utterance(&self) -> Option<Uuid> {
        match self.operation {
            Operation::Inspect => None,
            Operation::Classify { utterance, .. } => Some(utterance),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Request,
    FollowUp,
    Rejected,
    Unknown,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    pub category: Category,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u16,
    pub request: Uuid,
    pub context: Context,
    pub utterance: Option<Uuid>,
    pub adapter_revision: String,
    pub artifact_revision: String,
    pub engine_incarnation: String,
    pub quality_fingerprint: Option<String>,
    pub category: Option<Category>,
}
impl Reply {
    pub fn validate(&self, request: &Request) -> Result<(), ErrorCode> {
        request.validate()?;
        if self.version != VERSION
            || self.request != request.request
            || self.context != request.context
            || self.utterance != request.utterance()
            || self.category.is_some() != self.utterance.is_some()
            || [&self.adapter_revision, &self.engine_incarnation]
                .iter()
                .any(|v| !hexadecimal(v, 64))
            || !hexadecimal(&self.artifact_revision, 40)
            || self
                .quality_fingerprint
                .as_ref()
                .is_some_and(|v| !hexadecimal(v, 64))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
fn hexadecimal(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
