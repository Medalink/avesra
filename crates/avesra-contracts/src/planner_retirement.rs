//! Read-only paired metadata. No reply here constructs a claim or deletion right.
use super::Context;
use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub const VERSION: u16 = 1;
pub const MAX_CONTEXTS: usize = 128;
pub const MAX_BYTES: usize = 131_072;
pub const MAX_BUDGET_MS: u64 = 5_000;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Current {
    pub device: Uuid,
    pub actor: Uuid,
    pub registration_revision: Uuid,
    pub session: Uuid,
    pub action_epoch: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request: Uuid,
    pub current: Current,
    pub contexts: Vec<Context>,
    pub remaining_ms: u64,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        if self.contexts.len() > MAX_CONTEXTS {
            return Err(ErrorCode::TooLarge);
        }
        if self.request.is_nil()
            || self.contexts.is_empty()
            || self.remaining_ms == 0
            || self.remaining_ms > MAX_BUDGET_MS
            || self.current.action_epoch == 0
            || self.current.action_epoch > crate::browser::MAX_SAFE_COUNTER
        {
            return Err(ErrorCode::Malformed);
        }
        let mut requests = HashSet::new();
        let mut turns = HashSet::new();
        let mut revisions = HashSet::new();
        let mut utterances = HashSet::new();
        let mut ordinals = HashSet::new();
        for context in &self.contexts {
            context.validate()?;
            if context.device != self.current.device
                || context.actor != self.current.actor
                || context.registration_revision != self.current.registration_revision
                || context.session != self.current.session
                || context.action_epoch > self.current.action_epoch
                || !requests.insert(context.request)
                || !turns.insert(context.turn)
                || !revisions.insert(context.turn_revision)
                || !utterances.insert(context.utterance)
                || !ordinals.insert(context.ordinal)
            {
                return Err(ErrorCode::Malformed);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Retired,
    ClosedAndCompacted,
    Busy,
    PrivateRetirementUnconfirmed,
    Unknown,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub context: Context,
    pub status: Status,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u16,
    pub request: Uuid,
    pub current: Current,
    pub observations: Vec<Observation>,
}
impl Reply {
    pub fn validate(&self, expected: &Request) -> Result<(), ErrorCode> {
        expected.validate()?;
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        if self.request != expected.request
            || self.current != expected.current
            || self.observations.len() != expected.contexts.len()
        {
            return Err(ErrorCode::Stale);
        }
        for (observation, context) in self.observations.iter().zip(&expected.contexts) {
            if observation.context != *context {
                return Err(ErrorCode::Stale);
            }
        }
        Ok(())
    }
}
