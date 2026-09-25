//! Explicit generated-reference preview; no microphone or intent authority.
use crate::{ErrorCode, voice::VoiceIdentity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    pub version: u16,
    pub session_id: Uuid,
    pub playback_epoch: u64,
    pub request_id: Uuid,
    pub voice: VoiceIdentity,
}
impl PreviewRequest {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != 1
            || self.session_id.is_nil()
            || self.request_id.is_nil()
            || self.playback_epoch == 0
        {
            return Err(ErrorCode::Malformed);
        }
        self.voice.validate()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreviewMessage {
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
