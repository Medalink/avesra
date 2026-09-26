//! Bounded untrusted mailbox evidence. Callers must retain the actual browser
//! job; these data structures never grant authority or prove provider identity.
use avesra_contracts::ErrorCode;
use serde::Deserialize;
use std::{collections::HashSet, time::Instant};
use uuid::Uuid;

const MAX_BODY: usize = 16_384;
const MAX_BODIES: usize = 262_144;

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: String,
    pub thread: String,
    pub timestamp_ms: u64,
    pub sender: String,
    pub subject: String,
    pub reference: String,
    pub body: String,
    pub body_complete: bool,
    pub thread_expanded: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub binding_revision: Uuid,
    pub account: String,
    pub scope: Scope,
    pub document: String,
    pub dom_revision: u64,
    pub ordinal: u16,
    pub cursor: Option<String>,
    pub next: Option<String>,
    pub end_of_inbox: bool,
    pub order: Order,
    pub messages: Vec<Message>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Inbox,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    IndividualMessagesNewestFirst,
}

/// Intentionally neither Clone nor Serialize: message bodies stay within the
/// original native consumer. An incomplete accumulator cannot expose a report.
pub struct Mailbox {
    binding: Uuid,
    account: String,
    requested: usize,
    deadline: Instant,
    batches: u16,
    cursor: Option<String>,
    cursors: HashSet<String>,
    messages: Vec<Message>,
    bytes: usize,
    complete: bool,
    failed: bool,
}

fn text(value: &str, maximum: usize, multiline: bool) -> bool {
    value.len() <= maximum
        && value
            .chars()
            .all(|c| !c.is_control() || (multiline && matches!(c, '\n' | '\t')))
}
fn identity(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && value.bytes().all(|b| (33..=126).contains(&b))
}
impl Message {
    fn validate(&self) -> Result<(), ErrorCode> {
        if !identity(&self.id, 128)
            || !identity(&self.thread, 128)
            || !(1..=8_640_000_000_000_000).contains(&self.timestamp_ms)
            || self.sender.trim().is_empty()
            || !text(&self.sender, 320, false)
            || !text(&self.subject, 512, false)
            || !identity(&self.reference, 2048)
            || !text(&self.body, MAX_BODY, true)
            || !self.body_complete
            || !self.thread_expanded
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}

impl Mailbox {
    pub fn new(
        binding: Uuid,
        account: String,
        requested: u16,
        original_deadline: Instant,
    ) -> Result<Self, ErrorCode> {
        if binding.is_nil() || !identity(&account, 320) || !(1..=100).contains(&requested) {
            return Err(ErrorCode::Malformed);
        }
        if Instant::now() >= original_deadline {
            return Err(ErrorCode::Expired);
        }
        Ok(Self {
            binding,
            account,
            requested: requested.into(),
            deadline: original_deadline,
            batches: 0,
            cursor: None,
            cursors: HashSet::new(),
            messages: Vec::new(),
            bytes: 0,
            complete: false,
            failed: false,
        })
    }

    /// The caller obtains Batch only from a validated owned provider operation.
    /// Any failed append poisons this accumulator; retry cannot hide a gap.
    pub fn append(&mut self, batch: Batch) -> Result<(), ErrorCode> {
        let result = self.append_inner(batch);
        if result.is_err() {
            self.failed = true;
            self.messages.clear();
        }
        result
    }
    fn append_inner(&mut self, batch: Batch) -> Result<(), ErrorCode> {
        if self.failed || self.complete || Instant::now() >= self.deadline {
            return Err(ErrorCode::Stale);
        }
        if self.batches >= 32
            || batch.ordinal != self.batches
            || batch.binding_revision != self.binding
            || batch.account != self.account
            || !identity(&batch.document, 128)
            || batch.dom_revision == 0
            || batch.dom_revision > 9_007_199_254_740_991
            || batch.cursor != self.cursor
            || batch.messages.len() > self.requested
            || batch.next.as_ref().is_some_and(|v| !identity(v, 256))
            || batch.end_of_inbox == batch.next.is_some()
            || (!batch.end_of_inbox && batch.messages.is_empty())
        {
            return Err(ErrorCode::Malformed);
        }
        if let Some(next) = &batch.next
            && !self.cursors.insert(next.clone())
        {
            return Err(ErrorCode::Stale);
        }
        let mut local_ids = HashSet::new();
        let mut previous = u64::MAX;
        for message in batch.messages {
            message.validate()?;
            if !local_ids.insert(message.id.clone()) || message.timestamp_ms > previous {
                return Err(ErrorCode::Malformed);
            }
            previous = message.timestamp_ms;
            if let Some(existing) = self.messages.iter().find(|v| v.id == message.id) {
                if existing != &message {
                    return Err(ErrorCode::Stale);
                }
                continue;
            }
            if self.messages.len() >= self.requested
                || self
                    .messages
                    .last()
                    .is_some_and(|v| v.timestamp_ms < message.timestamp_ms)
            {
                return Err(ErrorCode::Malformed);
            }
            self.bytes = self
                .bytes
                .checked_add(message.body.len())
                .ok_or(ErrorCode::TooLarge)?;
            if self.bytes > MAX_BODIES {
                return Err(ErrorCode::TooLarge);
            }
            self.messages.push(message);
        }
        if Instant::now() >= self.deadline {
            return Err(ErrorCode::Expired);
        }
        self.batches += 1;
        self.cursor = batch.next;
        self.complete = self.messages.len() == self.requested || batch.end_of_inbox;
        Ok(())
    }

    /// Borrowed complete messages are untrusted source material, never policy.
    /// The original browser owner must recheck authority before using a report.
    pub fn messages(&self) -> Result<&[Message], ErrorCode> {
        if self.failed || !self.complete || Instant::now() >= self.deadline {
            return Err(ErrorCode::Stale);
        }
        Ok(&self.messages)
    }
    pub fn actual_count(&self) -> Result<usize, ErrorCode> {
        self.messages().map(<[Message]>::len)
    }
}

/// Literal candidates only. A caller must preserve each message source and
/// ambiguity; these labels never attest physical delivery or package identity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeliveryClaim {
    Delivered,
    OutForDelivery,
    Shipped,
    Ambiguous,
}
pub struct Candidate<'a> {
    pub message: &'a Message,
    pub claim: DeliveryClaim,
    pub phrase: &'static str,
}
pub fn delivery_candidates(message: &Message) -> Result<Vec<Candidate<'_>>, ErrorCode> {
    message.validate()?;
    let combined = format!("{}\n{}", message.subject, message.body).to_ascii_lowercase();
    let mut candidates = Vec::new();
    for (phrase, claim) in [
        ("out for delivery", DeliveryClaim::OutForDelivery),
        ("has been delivered", DeliveryClaim::Delivered),
        ("was delivered", DeliveryClaim::Delivered),
        ("has shipped", DeliveryClaim::Shipped),
        ("has been shipped", DeliveryClaim::Shipped),
    ] {
        for (offset, _) in combined.match_indices(phrase) {
            let start = combined[..offset]
                .rfind(['.', '!', '?', '\n'])
                .map_or(0, |v| v + 1);
            let end = combined[offset..]
                .find(['.', '!', '?', '\n'])
                .map_or(combined.len(), |v| offset + v);
            let sentence = &combined[start..end];
            let uncertain = sentence.split(|c: char| !c.is_ascii_alphabetic()).any(|v| {
                matches!(
                    v,
                    "not"
                        | "never"
                        | "if"
                        | "estimated"
                        | "expected"
                        | "may"
                        | "might"
                        | "when"
                        | "unless"
                        | "isn"
                        | "hasn"
                        | "wasn"
                )
            });
            candidates.push(Candidate {
                message,
                claim: if uncertain {
                    DeliveryClaim::Ambiguous
                } else {
                    claim
                },
                phrase,
            });
            // A body is bounded, but do not expose an unbounded repetitive report.
            if candidates.len() == 8 {
                return Ok(candidates);
            }
        }
    }
    Ok(candidates)
}
