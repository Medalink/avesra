//! Native transient display, constructed only inside the borrowed read consumer.
use avesra_contracts::{
    ErrorCode,
    browser::{
        mailbox::Incomplete,
        reading::{Outcome, Reply},
    },
};
use serde::Serialize;
#[derive(Clone, Serialize)]
pub(crate) struct Message {
    id: String,
    thread: String,
    timestamp_ms: u64,
    sender: String,
    subject: String,
    reference: String,
    body: String,
}
#[derive(Clone, Serialize)]
pub(crate) struct View {
    account: String,
    messages: Vec<Message>,
    incomplete: Option<Incomplete>,
}
pub(crate) fn consume(
    reply: &Reply,
    mailbox: Option<avesra_core::workflows::mailbox::Mailbox>,
) -> Result<Option<View>, ErrorCode> {
    let Outcome::Inbox { terminal } = &reply.outcome else {
        return if mailbox.is_none() {
            Ok(None)
        } else {
            Err(ErrorCode::Malformed)
        };
    };
    if terminal.incomplete.is_some() {
        if mailbox.is_some() {
            return Err(ErrorCode::Malformed);
        }
        return Ok(Some(View {
            account: terminal.account.clone(),
            messages: Vec::new(),
            incomplete: terminal.incomplete,
        }));
    }
    let mailbox = mailbox.ok_or(ErrorCode::Stale)?;
    let messages = mailbox
        .messages()?
        .iter()
        .map(|m| Message {
            id: m.id.clone(),
            thread: m.thread.clone(),
            timestamp_ms: m.timestamp_ms,
            sender: m.sender.clone(),
            subject: m.subject.clone(),
            reference: m.reference.clone(),
            body: m.body.clone(),
        })
        .collect();
    mailbox.actual_count()?;
    Ok(Some(View {
        account: terminal.account.clone(),
        messages,
        incomplete: None,
    }))
}
