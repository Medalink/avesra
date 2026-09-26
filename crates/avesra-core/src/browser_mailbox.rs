//! Complete transient source, minted only by the settled read worker.
use super::*;
use avesra_contracts::speech::{MailboxSource, Provenance};

/// No Deserialize/Clone/public constructor. Serialized messages cannot mint it.
pub struct MailboxEvidence {
    context: Context,
    action: avesra_contracts::Action,
    observation: crate::browser_reading::Observation,
    mailbox: crate::workflows::mailbox::Mailbox,
    deadline: Instant,
}
impl MailboxEvidence {
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn mailbox(&self) -> Result<&crate::workflows::mailbox::Mailbox, ErrorCode> {
        if Instant::now() >= self.deadline {
            return Err(ErrorCode::Expired);
        }
        self.mailbox.actual_count()?;
        Ok(&self.mailbox)
    }
    pub(crate) fn finish(
        self,
        action: &avesra_contracts::Action,
        dispatch: Uuid,
        observed: &EffectObservation,
    ) -> Result<(String, Provenance, Instant), ErrorCode> {
        let EffectObservation::BrowserRead { observation } = observed else {
            return Err(ErrorCode::Malformed);
        };
        if action != &self.action
            || dispatch != self.context.dispatch.uuid()
            || serde_json::to_vec(observation).map_err(|_| ErrorCode::Malformed)?
                != serde_json::to_vec(&self.observation).map_err(|_| ErrorCode::Malformed)?
        {
            return Err(ErrorCode::Stale);
        }
        let ActionPayload::ReadInbox { account: _, count } = &action.payload else {
            return Err(ErrorCode::Denied);
        };
        let messages = self.mailbox()?.messages()?;
        let metadata = self
            .observation
            .inbox
            .as_ref()
            .filter(|v| v.incomplete.is_none())
            .ok_or(ErrorCode::Stale)?;
        let mut text = format!("I checked {} individual Inbox messages. ", messages.len());
        let mut sources = Vec::new();
        let mut total = 0usize;
        for message in messages {
            let candidates = crate::workflows::mailbox::delivery_candidates(message)?;
            if candidates.is_empty() {
                continue;
            }
            total += 1;
            if sources.len() == 4 {
                continue;
            }
            let index = sources.len() + 1;
            let ambiguous = candidates
                .iter()
                .any(|v| v.claim == crate::workflows::mailbox::DeliveryClaim::Ambiguous);
            let claim = if ambiguous {
                "has ambiguous delivery wording"
            } else if candidates
                .iter()
                .any(|v| v.claim == crate::workflows::mailbox::DeliveryClaim::Delivered)
            {
                "says a delivery was completed"
            } else if candidates
                .iter()
                .any(|v| v.claim == crate::workflows::mailbox::DeliveryClaim::OutForDelivery)
            {
                "says a delivery was out for delivery"
            } else {
                "says a delivery was shipped"
            };
            let sender: String = message.sender.chars().take(80).collect();
            text.push_str(&format!("Message {index}, from {sender}, {claim}. "));
            sources.push(MailboxSource {
                id: message.id.clone(),
                thread: message.thread.clone(),
                reference: message.reference.clone(),
                timestamp_ms: message.timestamp_ms,
            });
        }
        if sources.is_empty() {
            text.push_str("I did not find one of the supported delivery-confirmation phrases in those messages. This does not establish whether your package was delivered.");
        } else {
            if total > sources.len() {
                text.push_str(
                    "More messages contain delivery wording; only the first four are summarized. ",
                );
            }
            text.push_str("These are email claims. I have not verified physical delivery or established that they refer to your package.");
        }
        let provenance = Provenance::NativeMailbox {
            dispatch,
            action_revision: action.revision,
            grant: action.grant_id,
            scope: self.context.scope,
            account_sha256: metadata.account_sha256.clone(),
            digest: metadata.digest.clone(),
            requested: *count,
            count: messages.len() as u16,
            sources,
        };
        provenance.validate()?;
        if Instant::now() >= self.deadline {
            return Err(ErrorCode::Expired);
        }
        Ok((text, provenance, self.deadline))
    }
}
impl ReadExecution<'_> {
    pub fn take_mailbox_evidence(&mut self) -> Result<Option<MailboxEvidence>, ErrorCode> {
        self.content_current()?;
        if !self.finished {
            return Err(ErrorCode::InvalidTransition);
        }
        let Some(mailbox) = self.mailbox.take() else {
            return Ok(None);
        };
        let Some(EffectObservation::BrowserRead { observation }) = &self.observation else {
            return Err(ErrorCode::Malformed);
        };
        if observation
            .inbox
            .as_ref()
            .is_none_or(|v| v.incomplete.is_some())
        {
            return Err(ErrorCode::Stale);
        }
        mailbox.actual_count()?;
        self.mailbox_taken = true;
        Ok(Some(MailboxEvidence {
            context: observation.context.clone(),
            action: self.permit.action.clone(),
            observation: *observation.clone(),
            mailbox,
            deadline: self.deadline,
        }))
    }
}
