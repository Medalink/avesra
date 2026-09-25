//! Actual local pipe ownership. Wire status is not an authentication capability.
use crate::{
    browser_pairing::{Admission, Pending},
    browser_pipe::Connection,
};
use avesra_contracts::{
    ErrorCode,
    browser::{self, Client, Id, ScopeRef},
};
use std::time::Instant;
use tokio::net::windows::named_pipe::NamedPipeServer;
use uuid::Uuid;

/// Can only be produced by reading Authenticate from this actual pipe owner.
pub struct Authentication {
    owner: Uuid,
    reply: browser::Authenticate,
}
impl Authentication {
    pub(crate) fn consume(self, owner: Uuid) -> Result<browser::Authenticate, ErrorCode> {
        if self.owner != owner {
            return Err(ErrorCode::Unauthenticated);
        }
        Ok(self.reply)
    }
}
/// Constructed only by native credential verification. No wire constructor.
pub struct Authenticated {
    owner: Uuid,
    challenge: Id,
    response: browser::Authenticated,
    expires: Instant,
    actor: Id,
    app: ScopeRef,
}
impl Authenticated {
    pub(crate) fn verified(
        owner: Uuid,
        challenge: Id,
        response: browser::Authenticated,
        expires: Instant,
        actor: Id,
        app: ScopeRef,
    ) -> Self {
        Self {
            owner,
            challenge,
            response,
            expires,
            actor,
            app,
        }
    }
}
/// Actual authenticated transport assertion, not proof of content success.
/// Only this receive owner constructs it; the ledger worker must still match
/// the entire retained marker and commit retirement before acknowledging it.
pub struct Settlement {
    context: browser::reading::Context,
}
impl Settlement {
    pub fn context(&self) -> &browser::reading::Context {
        &self.context
    }
}
/// Received only on the actual authenticated current connection. No raw-body
/// getter, Deserialize implementation or caller-supplied constructor exists.
pub struct Content {
    reply: browser::reading::Reply,
}
impl Content {
    pub fn context(&self) -> &browser::reading::Context {
        &self.reply.context
    }
    /// Both owners stay borrowed through one-use consumption. This prerequisite
    /// cannot transfer evidence beyond the live worker/resource reservation.
    pub fn finalize<'a, 'store>(
        self,
        execution: &'a mut avesra_core::browser_execution::ReadExecution<'store>,
        current: &'a mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<ReadReply<'a, 'store>, ErrorCode> {
        let result = (|| {
            current()?;
            execution.finalize_content(&self.reply)?;
            current()?;
            execution.content_current()
        })();
        if let Err(error) = result {
            execution.withdraw_content();
            return Err(error);
        }
        Ok(ReadReply {
            reply: Some(self.reply),
            execution,
            current,
        })
    }
}
/// Borrowed untrusted evidence, not a transferable effect/publication permit.
pub struct ReadReply<'a, 'store> {
    reply: Option<browser::reading::Reply>,
    execution: &'a mut avesra_core::browser_execution::ReadExecution<'store>,
    current: &'a mut dyn FnMut() -> Result<(), ErrorCode>,
}
impl ReadReply<'_, '_> {
    pub fn consume(mut self) -> Result<browser::reading::Reply, ErrorCode> {
        self.execution.content_current()?;
        (self.current)()?;
        self.execution.content_current()?;
        self.reply.take().ok_or(ErrorCode::Stale)
    }
}
impl Drop for ReadReply<'_, '_> {
    fn drop(&mut self) {
        if self.reply.is_some() {
            self.execution.withdraw_content();
        }
    }
}
pub enum Incoming {
    Message(Client),
    Authentication(Authentication),
    Content {
        session: Id,
        sequence: u64,
        observation_revision: u64,
        content: Content,
    },
    Settlement {
        session: Id,
        sequence: u64,
        observation_revision: u64,
        proof: Settlement,
    },
}
/// Cannot be reconstructed from a session ID, caller bytes or a status frame.
pub struct ReceiveOwner {
    pipe: Connection<NamedPipeServer>,
    owner: Uuid,
    session: Id,
    challenge: Id,
    generation: u64,
    sequence: u64,
    observation_revision: u64,
    authenticated: Option<browser::Authenticated>,
    authenticated_binding: Option<(Id, ScopeRef)>,
    authentication_pending: bool,
    failed: bool,
}
impl ReceiveOwner {
    pub async fn open(
        mut pipe: Connection<NamedPipeServer>,
        admission: Admission,
        configured_extension: &str,
        generation: u64,
    ) -> Result<(Pending, Self), ErrorCode> {
        admission.current(generation)?;
        let Client::Hello(hello) = browser::decode(&pipe.receive().await?)? else {
            return Err(ErrorCode::Malformed);
        };
        admission.current(generation)?;
        let owner = Uuid::new_v4();
        let pending =
            admission.challenge_from_pipe(hello, configured_extension, generation, owner)?;
        let challenge = pending.challenge();
        let receive = Self {
            pipe,
            owner,
            session: challenge.session,
            challenge: challenge.challenge,
            generation,
            sequence: 0,
            observation_revision: 0,
            authenticated: None,
            authenticated_binding: None,
            authentication_pending: false,
            failed: false,
        };
        Ok((pending, receive))
    }
    pub fn install(&mut self, seal: Authenticated) -> Result<browser::Authenticated, ErrorCode> {
        if self.failed
            || self.authenticated.is_some()
            || !self.authentication_pending
            || seal.owner != self.owner
            || seal.challenge != self.challenge
            || seal.response.session != self.session
            || seal.response.generation != self.generation
            || Instant::now() >= seal.expires
        {
            self.failed = true;
            return Err(ErrorCode::Unauthenticated);
        }
        self.authenticated = Some(browser::Authenticated {
            version: seal.response.version,
            session: seal.response.session,
            installation: seal.response.installation,
            connection: seal.response.connection,
            pairing: seal.response.pairing,
            generation: seal.response.generation,
        });
        self.authenticated_binding = Some((seal.actor, seal.app));
        self.authentication_pending = false;
        Ok(seal.response)
    }
    /// Read-only verified pairing metadata, never a settlement/reset capability.
    pub fn authenticated_binding(&self) -> Option<(Id, ScopeRef)> {
        if self.failed {
            None
        } else {
            self.authenticated_binding
        }
    }
    pub async fn receive(&mut self) -> Result<Incoming, ErrorCode> {
        if self.failed || self.authentication_pending {
            self.failed = true;
            return Err(ErrorCode::Stale);
        }
        // Remain poisoned if this future is dropped mid-frame or validation fails.
        self.failed = true;
        let message: Client = browser::decode(&self.pipe.receive().await?)?;
        let revision = match &message {
            Client::Poll {
                session,
                sequence,
                observation_revision,
            }
            | Client::ScopeResult {
                session,
                sequence,
                observation_revision,
                ..
            }
            | Client::ReadResult {
                session,
                sequence,
                observation_revision,
                ..
            }
            | Client::ReadSettlement {
                session,
                sequence,
                observation_revision,
                ..
            } => Some((*session, *sequence, *observation_revision)),
            Client::DocumentResult {
                session,
                sequence,
                reply,
            } => Some((*session, *sequence, reply.observation_revision)),
            _ => None,
        };
        if let Some((session, sequence, revision)) = revision {
            if session != self.session
                || self.sequence.checked_add(1) != Some(sequence)
                || sequence > browser::MAX_SAFE_COUNTER
                || revision == 0
                || revision > browser::MAX_SAFE_COUNTER
                || revision < self.observation_revision
                || (self.authenticated.is_none() && !matches!(message, Client::Poll { .. }))
            {
                return Err(ErrorCode::Malformed);
            }
            self.sequence = sequence;
            self.observation_revision = revision;
        }
        let incoming = match message {
            Client::Authenticate(reply)
                if self.authenticated.is_none()
                    && reply.session == self.session
                    && reply.challenge == self.challenge
                    && reply.version == browser::VERSION =>
            {
                self.authentication_pending = true;
                Incoming::Authentication(Authentication {
                    owner: self.owner,
                    reply,
                })
            }
            Client::ReadSettlement {
                session,
                sequence,
                observation_revision,
                settlement,
            } => {
                settlement.validate()?;
                let authenticated = self
                    .authenticated
                    .as_ref()
                    .ok_or(ErrorCode::Unauthenticated)?;
                let (actor, app) = self
                    .authenticated_binding
                    .ok_or(ErrorCode::Unauthenticated)?;
                if settlement.context.pairing != authenticated.pairing
                    || settlement.context.actor != actor
                    || settlement.context.browser_app != app
                {
                    return Err(ErrorCode::Unauthenticated);
                }
                Incoming::Settlement {
                    session,
                    sequence,
                    observation_revision,
                    proof: Settlement {
                        context: settlement.context,
                    },
                }
            }
            Client::ReadResult {
                session,
                sequence,
                observation_revision,
                reply,
            } => {
                reply.context.validate()?;
                let authenticated = self
                    .authenticated
                    .as_ref()
                    .ok_or(ErrorCode::Unauthenticated)?;
                let (actor, app) = self
                    .authenticated_binding
                    .ok_or(ErrorCode::Unauthenticated)?;
                if reply.context.pairing != authenticated.pairing
                    || reply.context.actor != actor
                    || reply.context.browser_app != app
                {
                    return Err(ErrorCode::Unauthenticated);
                }
                if reply.context.browser_session != self.session
                    || reply.context.browser_generation != self.generation
                {
                    // Same authenticated pairing, withdrawn old connection:
                    // consume its contiguous envelope without minting content.
                    Incoming::Message(Client::Poll {
                        session,
                        sequence,
                        observation_revision,
                    })
                } else {
                    Incoming::Content {
                        session,
                        sequence,
                        observation_revision,
                        content: Content { reply },
                    }
                }
            }
            Client::Poll { .. } | Client::ScopeResult { .. } | Client::DocumentResult { .. } => {
                Incoming::Message(message)
            }
            Client::Disconnect { session } if session == self.session => {
                // Terminal message: no subsequent send/receive may reuse the owner.
                return Ok(Incoming::Message(message));
            }
            _ => return Err(ErrorCode::Malformed),
        };
        self.failed = false;
        Ok(incoming)
    }
    pub async fn send(&mut self, bytes: &[u8]) -> Result<(), ErrorCode> {
        if self.failed {
            return Err(ErrorCode::Stale);
        }
        self.failed = true;
        self.pipe.send(bytes).await?;
        self.failed = false;
        Ok(())
    }
}
