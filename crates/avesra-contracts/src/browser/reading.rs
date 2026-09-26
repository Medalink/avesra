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
    pub document: Option<Candidate>,
    pub message_limit: u16,
    pub mode: Mode,
    /// Reduced from the original owner deadline; repeated status never renews it.
    pub remaining_ms: u64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mode {
    Excerpt,
    ProviderInspection { provider: super::provider::Provider },
    XReady { account: String },
    Inbox { account: String },
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.context.validate()?;
        if let Some(document) = &self.document {
            document.validate(&self.origin)?;
        } else if !matches!(self.mode, Mode::XReady { .. }) {
            return Err(ErrorCode::Malformed);
        }
        if let Mode::Inbox { account } = &self.mode
            && (!super::mailbox::account(account)
                || self.origin.as_str() != "https://mail.google.com")
        {
            return Err(ErrorCode::Malformed);
        }
        if let Mode::XReady { account } = &self.mode
            && (!super::provider::x_account(account)
                || self.origin.as_str() != super::provider::Provider::X.origin()
                || self.message_limit != 1)
        {
            return Err(ErrorCode::Malformed);
        }
        if let Mode::ProviderInspection { provider } = self.mode
            && (self.origin.as_str() != provider.origin() || self.message_limit != 1)
        {
            return Err(ErrorCode::Malformed);
        }
        if !(1..=100).contains(&self.message_limit) {
            return Err(ErrorCode::Malformed);
        }
        if self.remaining_ms == 0
            || self.remaining_ms
                > if matches!(self.mode, Mode::Inbox { .. }) {
                    super::mailbox::LIFETIME_MS
                } else {
                    LIFETIME_MS
                }
        {
            return Err(ErrorCode::Expired);
        }
        encoded_bound(self)
    }
    /// Correlation only. The ledger must independently claim/revalidate authority.
    pub fn matches_action(&self, action: &Action) -> Result<(), ErrorCode> {
        self.validate()?;
        let (origin, message_limit) = match (&action.payload, &self.mode) {
            (
                ActionPayload::ReadPage {
                    origin,
                    message_limit,
                },
                Mode::Excerpt,
            ) => (origin.as_str(), *message_limit),
            (
                ActionPayload::InspectBrowserProvider { provider },
                Mode::ProviderInspection { provider: expected },
            ) if provider == expected => (provider.origin(), 1),
            (ActionPayload::OpenX { account }, Mode::XReady { account: expected })
                if account == expected =>
            {
                (super::provider::Provider::X.origin(), 1)
            }
            (ActionPayload::ReadInbox { account, count }, Mode::Inbox { account: expected })
                if account == expected =>
            {
                ("https://mail.google.com", *count)
            }
            _ => return Err(ErrorCode::Denied),
        };
        if action.task_id != self.context.task.uuid()
            || action.step_id != self.context.step.uuid()
            || action.actor_id != self.context.actor.uuid()
            || action.target_id != self.context.target.id.uuid()
            || action.grant_id != self.context.grant.uuid()
            || action.revision != self.context.action_revision.uuid()
            || action.intent_revision != self.context.intent_revision.uuid()
            || Origin::parse(origin)? != self.origin
            || message_limit != self.message_limit
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
    Inbox {
        terminal: super::mailbox::Terminal,
    },
    Excerpt {
        excerpt: Excerpt,
    },
    ProviderInspection {
        probe: super::provider::Probe,
    },
    XReady {
        ready: super::provider::XReady,
    },
    XNeedsInput {
        evidence: super::provider::XNeedsInput,
    },
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
        match (&request.mode, &self.outcome) {
            (Mode::Inbox { account }, Outcome::Inbox { terminal }) => {
                terminal.validate()?;
                if terminal.account != *account
                    || Some(&terminal.document) != request.document.as_ref()
                {
                    return Err(ErrorCode::Stale);
                }
            }
            (
                Mode::Inbox { .. },
                Outcome::Excerpt { .. }
                | Outcome::ProviderInspection { .. }
                | Outcome::XReady { .. }
                | Outcome::XNeedsInput { .. }
                | Outcome::Empty,
            ) => return Err(ErrorCode::Malformed),
            (_, Outcome::Inbox { .. }) => return Err(ErrorCode::Malformed),
            (Mode::XReady { account }, Outcome::XNeedsInput { evidence }) => {
                evidence.validate()?;
                if evidence.account != *account
                    || evidence.created != request.document.is_none()
                    || request
                        .document
                        .as_ref()
                        .is_some_and(|document| evidence.document != *document)
                {
                    return Err(ErrorCode::Stale);
                }
            }
            (Mode::XReady { account }, Outcome::XReady { ready }) => {
                ready.validate()?;
                if ready.account != *account
                    || ready.created != request.document.is_none()
                    || request
                        .document
                        .as_ref()
                        .is_some_and(|document| ready.document != *document)
                {
                    return Err(ErrorCode::Stale);
                }
            }
            (
                Mode::XReady { .. },
                Outcome::Excerpt { .. } | Outcome::ProviderInspection { .. } | Outcome::Empty,
            )
            | (
                Mode::Excerpt | Mode::ProviderInspection { .. },
                Outcome::XReady { .. } | Outcome::XNeedsInput { .. },
            ) => return Err(ErrorCode::Malformed),
            (Mode::Excerpt, Outcome::ProviderInspection { .. })
            | (Mode::ProviderInspection { .. }, Outcome::Excerpt { .. } | Outcome::Empty) => {
                return Err(ErrorCode::Malformed);
            }
            (Mode::ProviderInspection { provider }, Outcome::ProviderInspection { probe }) => {
                probe.validate()?;
                if probe.provider != *provider || Some(&probe.document) != request.document.as_ref()
                {
                    return Err(ErrorCode::Stale);
                }
            }
            _ => {}
        }
        if let Outcome::Excerpt { excerpt } = &self.outcome {
            excerpt.document.validate(&request.origin)?;
            if Some(&excerpt.document) != request.document.as_ref()
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
pub(super) fn encoded_bound(value: &impl Serialize) -> Result<(), ErrorCode> {
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
