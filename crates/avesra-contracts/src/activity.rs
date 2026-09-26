//! Transient detector scores, not calibrated signal/overlap or identity authority.
use crate::ErrorCode;
use serde::{Deserialize, Serialize};

pub const REVISION: &str = "cd03eee90fbec18297ac31b8c21546e596b7f71c";
pub const FRAME_SAMPLES: u32 = 1280;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    pub model_revision: String,
    pub samples: u32,
    pub frame_offset: u32,
    pub frames: Vec<[f32; 4]>,
    pub r#final: bool,
}
impl Chunk {
    pub fn validate(&self, samples: u32, offset: u32, final_chunk: bool) -> Result<(), ErrorCode> {
        let end = self
            .frame_offset
            .checked_add(self.frames.len() as u32)
            .ok_or(ErrorCode::Malformed)?;
        if self.model_revision != REVISION
            || self.samples != samples
            || samples == 0
            || samples > 160_000
            || self.frame_offset != offset
            || self.r#final != final_chunk
            || self.frames.len() > 16
            || end > samples.div_ceil(FRAME_SAMPLES)
            || (final_chunk && end != samples.div_ceil(FRAME_SAMPLES))
            || self
                .frames
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub version: u16,
    pub session_id: uuid::Uuid,
    pub capture_epoch: u64,
    pub request_id: uuid::Uuid,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub sequence: u64,
    pub sample_offset: u32,
    pub pcm_s16le: String,
    pub r#final: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Acknowledgment {
    pub version: u16,
    pub session_id: uuid::Uuid,
    pub capture_epoch: u64,
    pub request_id: uuid::Uuid,
    pub model_revision: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StreamReply {
    pub version: u16,
    pub session_id: uuid::Uuid,
    pub capture_epoch: u64,
    pub request_id: uuid::Uuid,
    pub sequence: u64,
    pub observation: Chunk,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub model_revision: String,
    pub samples: u32,
    pub frames: Vec<[f32; 4]>,
}
impl Activity {
    pub fn validate(&self, expected_samples: u32) -> Result<(), ErrorCode> {
        if self.model_revision != REVISION
            || !(16_000..=160_000).contains(&self.samples)
            || self.samples != expected_samples
            || self.frames.len() != self.samples.div_ceil(FRAME_SAMPLES) as usize
            || self
                .frames
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
