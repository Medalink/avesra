//! Strict accepted-conversation assertions; parsing alone confers no authority.
use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: u16 = 3;
pub const MAX_BUDGET_MS: u64 = 30_000;
pub const MAX_REQUEST_BYTES: usize = 32_768;
pub const MAX_DIALOGUE_PAIRS: usize = 3;
pub const MAX_DIALOGUE_BYTES: usize = 4096;
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
    /// Bounded native-selected content, never an acceptance or action capability.
    #[serde(default)]
    pub dialogue: Vec<DialoguePair>,
    pub remaining_ms: u64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialoguePair {
    pub user: String,
    pub assistant: Response,
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
        if self.dialogue.len() > MAX_DIALOGUE_PAIRS {
            return Err(ErrorCode::TooLarge);
        }
        let mut bytes = 0usize;
        for pair in &self.dialogue {
            if !valid_text(&pair.user) || matches!(pair.assistant, Response::Proposal { .. }) {
                return Err(ErrorCode::Malformed);
            }
            pair.assistant.validate()?;
            bytes += pair.user.len() + pair.assistant.text().len();
        }
        if bytes > MAX_DIALOGUE_BYTES {
            return Err(ErrorCode::TooLarge);
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancel {
    pub version: u16,
    pub context: Context,
}
impl Cancel {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        self.context.validate()
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Answer { text: String },
    NeedsInput { text: String },
    Proposal { action: Proposal },
}
/// Logical arguments only; resolution and grants belong to the native owner.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Proposal {
    LaunchApp { alias: String },
    SetVolume { percent: u8 },
}
impl Proposal {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let valid = match self {
            Self::LaunchApp { alias } => {
                !alias.is_empty()
                    && alias.trim() == alias
                    && alias.len() <= 256
                    && alias.chars().count() <= 64
                    && alias
                        .chars()
                        .all(|v| v.is_alphanumeric() || matches!(v, ' ' | '-' | '\''))
            }
            Self::SetVolume { percent } => *percent <= 100,
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        }
    }
}
impl Response {
    /// A proposal has no speakable response. Call validate for all reply shapes.
    pub fn text(&self) -> &str {
        match self {
            Self::Answer { text } | Self::NeedsInput { text } => text,
            Self::Proposal { .. } => "",
        }
    }
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Proposal { action } => action.validate(),
            _ if valid_text(self.text()) => Ok(()),
            _ => Err(ErrorCode::Malformed),
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
        self.response.validate()
    }
}
