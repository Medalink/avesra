//! Strict accepted-conversation assertions; parsing alone confers no authority.
use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: u16 = 1;
pub const MAX_BUDGET_MS: u64 = 30_000;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub request: Uuid,
    pub turn: Uuid,
    pub turn_revision: Uuid,
    pub device: Uuid,
    pub actor: Uuid,
    pub registration_revision: Uuid,
    pub session: Uuid,
    pub utterance: Uuid,
    pub capture_epoch: u64,
    pub action_epoch: u64,
}
impl Context {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [
            self.request,
            self.turn,
            self.turn_revision,
            self.device,
            self.actor,
            self.registration_revision,
            self.session,
            self.utterance,
        ]
        .iter()
        .any(Uuid::is_nil)
            || [self.capture_epoch, self.action_epoch]
                .iter()
                .any(|v| *v == 0 || *v > crate::browser::MAX_SAFE_COUNTER)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
pub fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 8192
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub context: Context,
    pub text: String,
    pub remaining_ms: u64,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        self.context.validate()?;
        if !valid_text(&self.text) || self.remaining_ms == 0 || self.remaining_ms > MAX_BUDGET_MS {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "text",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    Answer(String),
    NeedsInput(String),
}
impl Response {
    pub fn text(&self) -> &str {
        match self {
            Self::Answer(text) | Self::NeedsInput(text) => text,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terminal {
    Complete,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u16,
    pub context: Context,
    /// Controller assertion after actual configured-driver terminal validation.
    /// This field cannot substitute for that adapter's completion evidence.
    pub terminal: Terminal,
    pub response: Response,
}
impl Reply {
    pub fn validate(&self, expected: &Context) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        self.context.validate()?;
        expected.validate()?;
        if self.context != *expected {
            return Err(ErrorCode::Stale);
        }
        if !valid_text(self.response.text()) {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
