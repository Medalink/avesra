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
    mailbox: Option<&avesra_core::browser_execution::MailboxEvidence>,
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
    let mailbox = mailbox.ok_or(ErrorCode::Stale)?.mailbox()?;
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

/// Registered actual-browser successor. Never constructed from frontend data.
pub(crate) struct Successor {
    app: tauri::AppHandle,
    context: avesra_contracts::browser::reading::Context,
    source: avesra_windows::browser_read_channel::ReadSuccessor,
    target_generation: u64,
    connection: u64,
}
impl Successor {
    pub(crate) fn new(
        app: &tauri::AppHandle,
        evidence: &avesra_core::browser_execution::MailboxEvidence,
    ) -> Result<Self, ErrorCode> {
        use tauri::Manager;
        evidence.mailbox()?;
        let state = app.state::<crate::Runtime>();
        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
        let inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        let context = evidence.context().clone();
        let attempt = inner.attempt.as_ref().ok_or(ErrorCode::Stale)?;
        let target_generation = attempt.target_generation;
        let connection = attempt.connection;
        let source = state
            .browser
            .reading
            .mailbox_successor(&state, &local, &inner, &context)?;
        Ok(Self {
            app: app.clone(),
            context,
            source,
            target_generation,
            connection,
        })
    }
    /// Called under normal output's existing local/session ownership; no local
    /// lock is reacquired and no disk/network work occurs on this path.
    pub(crate) fn current(&self) -> bool {
        use tauri::Manager;
        let state = self.app.state::<crate::Runtime>();
        let c = &self.context;
        self.source.current()
            && state
                .connection_generation
                .load(std::sync::atomic::Ordering::SeqCst)
                == self.connection
            && state.browser.inner.lock().is_ok_and(|inner| {
                inner.action_allowed
                    && inner.action_epoch == c.source.action_epoch
                    && inner.generation == c.browser_generation
                    && inner.attempt.as_ref().is_some_and(|a| {
                        a.target_generation == self.target_generation
                            && a.connection == self.connection
                            && a.current_time()
                            && a.actor == Some(c.actor.uuid())
                            && a.session == Some(c.browser_session)
                            && a.observation_revision == c.observation_revision
                            && a.selection == Some(c.selection)
                            && a.pairing == Some(c.pairing)
                            && a.state == "authenticated_no_scopes"
                            && a.selected.as_ref().is_some_and(|v| {
                                v.id == c.browser_app.id.uuid()
                                    && v.revision == c.browser_app.revision.uuid()
                                    && v.selected_by == c.actor.uuid()
                            })
                    })
            })
    }
}

impl Drop for Successor {
    fn drop(&mut self) {
        use tauri::Manager;
        self.app
            .state::<crate::Runtime>()
            .browser
            .reading
            .retire_mailbox_successor(self.context.request);
    }
}
