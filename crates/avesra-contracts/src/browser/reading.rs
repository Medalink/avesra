//! Bounded untrusted read observations. Valid parsing grants no page authority.
use super::{
    Id, MAX_MESSAGE, MAX_SAFE_COUNTER, Origin, PairingRef, ScopeRef, documents::Candidate,
};
use crate::{Action, ActionPayload, ErrorCode};
use serde::{Deserialize, Serialize};

pub const LIFETIME_MS: u64 = 10_000;
pub const MAX_BLOCKS: usize = 16;
pub const MAX_BLOCK_BYTES: usize = 512;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_TITLE_BYTES: usize = 256;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub device: Id,
    pub session: Id,
    /// Frozen accepted provenance, independent of the current microphone.
    pub capture_epoch: u64,
    pub action_epoch: u64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub request: Id,
    pub dispatch: Id,
    pub task: Id,
    pub step: Id,
    pub actor: Id,
    pub action_revision: Id,
    pub intent_revision: Id,
    pub grant: Id,
    pub target: ScopeRef,
    pub scope: ScopeRef,
    pub pairing: PairingRef,
    pub selection: Id,
    pub browser_app: ScopeRef,
    pub browser_session: Id,
    pub browser_generation: u64,
    pub observation_revision: u64,
    pub source: Source,
}
impl Context {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [
            self.browser_generation,
            self.observation_revision,
            self.source.capture_epoch,
            self.source.action_epoch,
        ]
        .iter()
        .any(|v| *v == 0 || *v > MAX_SAFE_COUNTER)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub context: Context,
    pub origin: Origin,
    pub document: Candidate,
    pub message_limit: u16,
    /// Reduced from the original owner deadline; repeated status never renews it.
    pub remaining_ms: u64,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.context.validate()?;
        self.document.validate(&self.origin)?;
        if !(1..=100).contains(&self.message_limit) {
            return Err(ErrorCode::Malformed);
        }
        if self.remaining_ms == 0 || self.remaining_ms > LIFETIME_MS {
            return Err(ErrorCode::Expired);
        }
        encoded_bound(self)
    }
    /// Correlation only. The ledger must independently claim/revalidate authority.
    pub fn matches_action(&self, action: &Action) -> Result<(), ErrorCode> {
        self.validate()?;
        let ActionPayload::ReadPage {
            origin,
            message_limit,
        } = &action.payload
        else {
            return Err(ErrorCode::Denied);
        };
        if action.task_id != self.context.task.uuid()
            || action.step_id != self.context.step.uuid()
            || action.actor_id != self.context.actor.uuid()
            || action.target_id != self.context.target.id.uuid()
            || action.grant_id != self.context.grant.uuid()
            || action.revision != self.context.action_revision.uuid()
            || action.intent_revision != self.context.intent_revision.uuid()
            || Origin::parse(origin)? != self.origin
            || message_limit != &self.message_limit
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
}
/// The generic extractor cannot attest mailbox ordering or complete coverage.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Partial,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excerpt {
    pub coverage: Coverage,
    pub document: Candidate,
    pub dom_revision: u64,
    pub title: String,
    pub blocks: Vec<String>,
    pub truncated: bool,
    pub excluded_content: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Excerpt { excerpt: Excerpt },
    Empty,
    Changed,
    Expired,
    Unavailable,
    Unsupported,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub context: Context,
    pub outcome: Outcome,
}
impl Reply {
    pub fn validate(&self, request: &Request) -> Result<(), ErrorCode> {
        request.validate()?;
        if self.context != request.context {
            return Err(ErrorCode::Stale);
        }
        if let Outcome::Excerpt { excerpt } = &self.outcome {
            excerpt.document.validate(&request.origin)?;
            if excerpt.document != request.document
                || excerpt.dom_revision == 0
                || excerpt.dom_revision > MAX_SAFE_COUNTER
            {
                return Err(ErrorCode::Stale);
            }
            if excerpt.blocks.is_empty()
                || excerpt.blocks.len() > MAX_BLOCKS.min(usize::from(request.message_limit))
                || excerpt.title.len() > MAX_TITLE_BYTES
                || !text(&excerpt.title)
            {
                return Err(ErrorCode::Malformed);
            }
            let mut bytes = 0usize;
            for block in &excerpt.blocks {
                if block.trim().is_empty() || block.len() > MAX_BLOCK_BYTES || !text(block) {
                    return Err(ErrorCode::Malformed);
                }
                bytes = bytes.checked_add(block.len()).ok_or(ErrorCode::TooLarge)?;
                if bytes > MAX_TEXT_BYTES {
                    return Err(ErrorCode::TooLarge);
                }
            }
        }
        encoded_bound(self)
    }
}
fn text(value: &str) -> bool {
    value
        .chars()
        .all(|c| !c.is_control() || matches!(c, '\n' | '\t'))
}
fn encoded_bound(value: &impl Serialize) -> Result<(), ErrorCode> {
    let bytes = serde_json::to_vec(value).map_err(|_| ErrorCode::Malformed)?;
    // Reserve space for the authenticated control envelope inside MAX_MESSAGE.
    if bytes.len() > MAX_MESSAGE - 2048 {
        return Err(ErrorCode::TooLarge);
    }
    Ok(())
}

/// V6 wire claim only. Parsing does not prove authenticated settlement.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementKind {
    ActualJobSettled,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub context: Context,
    pub kind: SettlementKind,
}
impl Settlement {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.context.validate()?;
        encoded_bound(self)
    }
    pub fn matches_context(&self, context: &Context) -> Result<(), ErrorCode> {
        self.validate()?;
        if &self.context != context {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
}
