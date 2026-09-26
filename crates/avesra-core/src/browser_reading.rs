//! Data correlation for the forthcoming native accepted-read adapter.
//! This does not establish current authority, claim a dispatch, or read a page.
use crate::ledger::DispatchPermit;
use avesra_contracts::{
    Action, ActionPayload, ErrorCode, Outcome,
    browser::{
        Id, Origin,
        reading::{Context, Reply, Request},
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Durable metadata only. It contains neither page text nor a content capability.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub context: Context,
    pub origin: Origin,
    pub document: Id,
    pub tab: u32,
    pub window: u32,
    pub url_sha256: String,
    pub reply_sha256: String,
    pub dom_revision: Option<u64>,
    pub blocks: u16,
    pub text_bytes: u16,
    pub title_bytes: u16,
    pub truncated: bool,
    pub excluded_content: bool,
    pub coverage: avesra_contracts::browser::reading::Coverage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderObservation>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderObservation {
    pub provider: avesra_contracts::browser::provider::Provider,
    pub choices: u16,
    pub complete: bool,
}
impl Observation {
    pub(crate) fn from_reply(request: &Request, reply: &Reply) -> Result<Self, ErrorCode> {
        reply.validate(request)?;
        let (dom_revision, blocks, text_bytes, title_bytes, truncated, excluded_content) =
            match &reply.outcome {
                avesra_contracts::browser::reading::Outcome::Excerpt { excerpt } => (
                    Some(excerpt.dom_revision),
                    excerpt.blocks.len(),
                    excerpt.blocks.iter().map(String::len).sum(),
                    excerpt.title.len(),
                    excerpt.truncated,
                    excerpt.excluded_content,
                ),
                avesra_contracts::browser::reading::Outcome::Empty => (None, 0, 0, 0, false, false),
                avesra_contracts::browser::reading::Outcome::ProviderInspection { probe } => {
                    (Some(probe.dom_revision), 0, 0, 0, !probe.complete, false)
                }
                _ => return Err(ErrorCode::Denied),
            };
        let body = serde_json::to_vec(reply).map_err(|_| ErrorCode::Malformed)?;
        Ok(Self {
            context: reply.context.clone(),
            origin: request.origin.clone(),
            document: request.document.document,
            tab: request.document.tab,
            window: request.document.window,
            url_sha256: format!("{:x}", Sha256::digest(request.document.url.as_bytes())),
            reply_sha256: format!("{:x}", Sha256::digest(body)),
            dom_revision,
            blocks: blocks as u16,
            text_bytes: text_bytes as u16,
            title_bytes: title_bytes as u16,
            truncated,
            excluded_content,
            coverage: avesra_contracts::browser::reading::Coverage::Partial,
            provider: match &reply.outcome {
                avesra_contracts::browser::reading::Outcome::ProviderInspection { probe } => {
                    Some(ProviderObservation {
                        provider: probe.provider,
                        choices: probe.choices.len() as u16,
                        complete: probe.complete,
                    })
                }
                _ => None,
            },
        })
    }
    pub fn validate(&self, action: &Action, outcome: Outcome) -> Result<(), ErrorCode> {
        self.context.validate()?;
        let (origin, message_limit) = match (&action.payload, &self.provider) {
            (
                ActionPayload::ReadPage {
                    origin,
                    message_limit,
                },
                None,
            ) => (origin.as_str(), *message_limit),
            (ActionPayload::InspectBrowserProvider { provider }, Some(observation))
                if *provider == observation.provider =>
            {
                if observation.choices > 64
                    || self.dom_revision.is_none()
                    || self.blocks != 0
                    || self.text_bytes != 0
                    || self.title_bytes != 0
                    || self.excluded_content
                    || self.truncated == observation.complete
                {
                    return Err(ErrorCode::Malformed);
                }
                (provider.origin(), 1)
            }
            _ => return Err(ErrorCode::Denied),
        };
        let context = &self.context;
        if outcome != Outcome::Success
            || context.task.uuid() != action.task_id
            || context.step.uuid() != action.step_id
            || context.actor.uuid() != action.actor_id
            || context.action_revision.uuid() != action.revision
            || context.intent_revision.uuid() != action.intent_revision
            || context.grant.uuid() != action.grant_id
            || context.target.id.uuid() != action.target_id
            || self.origin != Origin::parse(origin)?
            || self.tab == 0
            || self.tab > i32::MAX as u32
            || self.window == 0
            || self.window > i32::MAX as u32
            || self.blocks > 16u16.min(message_limit)
            || self.text_bytes > 4096
            || self.title_bytes > 256
            || self
                .dom_revision
                .is_some_and(|v| v == 0 || v > avesra_contracts::browser::MAX_SAFE_COUNTER)
            || (self.provider.is_none()
                && self.blocks == 0
                && (self.dom_revision.is_some()
                    || self.text_bytes != 0
                    || self.title_bytes != 0
                    || self.truncated
                    || self.excluded_content))
            || (self.blocks != 0
                && (self.dom_revision.is_none()
                    || self.text_bytes < self.blocks
                    || usize::from(self.text_bytes) > usize::from(self.blocks) * 512))
            || [&self.url_sha256, &self.reply_sha256].iter().any(|v| {
                v.len() != 64
                    || !v
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}

pub fn validate_dispatch(request: &Request, permit: &DispatchPermit) -> Result<(), ErrorCode> {
    request.matches_action(&permit.action)?;
    let context = &request.context;
    if context.dispatch.uuid() != permit.dispatch_id
        || context.source.device.uuid() != permit.device_id
        || context.source.session.uuid() != permit.session_id
        || context.source.capture_epoch != permit.capture_epoch
        || context.source.action_epoch != permit.action_epoch
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}

/// Only bounded untrusted evidence is validated here. Actual owner must still
/// recheck its original deadline, cancellation, live browser/scope/document and
/// Store::validate_dispatch before publication. No opaque authority is minted.
pub fn validate_reply(
    request: &Request,
    reply: &Reply,
    permit: &DispatchPermit,
) -> Result<(), ErrorCode> {
    validate_dispatch(request, permit)?;
    reply.validate(request)
}
