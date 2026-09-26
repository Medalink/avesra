//! Explicit generated-reference or text preview; no microphone or intent authority.
use crate::{ErrorCode, voice::VoiceIdentity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const VERSION: u16 = 2;
pub const FRAME_SAMPLES: u64 = 480;
pub const MAX_SAMPLES: u64 = 720_000;

/// Fixed product greeting, never an arbitrary speech-text ingress.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Greeting {
    pub owner_name: Option<String>,
}
impl Greeting {
    pub fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.trim() == name
            && name.len() <= 320
            && name.chars().count() <= 80
            && name.chars().any(char::is_alphabetic)
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\'' | '’' | '.'))
    }
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self
            .owner_name
            .as_deref()
            .is_some_and(|name| !Self::valid_name(name))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn text(&self) -> Result<String, ErrorCode> {
        self.validate()?;
        Ok(match &self.owner_name {
            Some(name) => format!("Hello, {name}."),
            None => "Hello.".into(),
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    pub version: u16,
    pub session_id: Uuid,
    pub playback_epoch: u64,
    pub request_id: Uuid,
    pub voice: VoiceIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greeting: Option<Greeting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_text: Option<String>,
}
impl PreviewRequest {
    pub fn valid_test_text(text: &str) -> bool {
        !text.trim().is_empty()
            && text.len() <= 512
            && text
                .chars()
                .all(|c| !c.is_control() || matches!(c, '\n' | '\t'))
    }

    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION
            || self.session_id.is_nil()
            || self.request_id.is_nil()
            || self.playback_epoch == 0
            || (self.greeting.is_some() && self.test_text.is_some())
            || self
                .test_text
                .as_deref()
                .is_some_and(|text| !Self::valid_test_text(text))
        {
            return Err(ErrorCode::Malformed);
        }
        self.voice.validate()?;
        if let Some(greeting) = &self.greeting {
            greeting.validate()?;
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreviewMessage {
    StreamingReady {
        sample_rate: u32,
        frame_samples: u64,
        max_samples: u64,
    },
    Ready {
        sample_rate: u32,
        samples: u64,
    },
    Audio {
        sequence: u64,
        sample_offset: u64,
        samples: Vec<i16>,
    },
    End {
        chunks: u64,
        samples: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewEvent {
    pub context: PreviewRequest,
    pub message: PreviewMessage,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreviewControl {
    Play { request_id: Uuid },
    Submitted { request_id: Uuid, samples: u64 },
    Cancel { request_id: Uuid },
}
