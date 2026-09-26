use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: u16 = 2;
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Asr,
    Speaker,
}
impl Lane {
    pub fn revision(self) -> &'static str {
        match self {
            Self::Asr => "ebe59e5a817142986528bbbee5dba8db7b38ed50",
            Self::Speaker => "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Host {
    Controller,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub worker: Uuid,
    pub host: Host,
    pub process: Uuid,
    pub lane: Lane,
    pub model_revision: String,
    pub at_ms: u64,
    pub start_us: u64,
    pub duration_us: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    pub version: u16,
    pub request: Uuid,
    pub receipts: Vec<Receipt>,
}
impl Analysis {
    pub fn validate(&self, request: Uuid) -> Result<(), ErrorCode> {
        if self.version != 1
            || request.is_nil()
            || self.request != request
            || self.receipts.len() != 2
            || self.receipts[0].lane == self.receipts[1].lane
            || self.receipts[0].worker == self.receipts[1].worker
            || self.receipts[0].process != self.receipts[1].process
            || self.receipts.iter().any(|r| {
                r.worker.is_nil()
                    || r.worker == request
                    || r.worker.as_bytes()[..8].iter().all(|v| *v == 0)
                    || r.process.is_nil()
                    || r.model_revision != r.lane.revision()
                    || r.at_ms == 0
                    || r.at_ms > crate::browser::MAX_SAFE_COUNTER
                    || r.start_us > crate::browser::MAX_SAFE_COUNTER
                    || r.duration_us > 30_000_000
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
