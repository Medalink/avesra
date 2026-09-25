//! Generated prompt identity; never an owner/permission identity.
use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceIdentity {
    pub id: Uuid,
    pub revision: Uuid,
    pub audio_sha256: String,
    pub metadata_sha256: String,
}
impl VoiceIdentity {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.id.is_nil()
            || self.revision.is_nil()
            || [&self.audio_sha256, &self.metadata_sha256]
                .iter()
                .any(|value| {
                    value.len() != 64
                        || !value
                            .bytes()
                            .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
                })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
