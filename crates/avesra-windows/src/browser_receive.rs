//! Actual local pipe ownership. Wire status is not an authentication capability.
use crate::{
    browser_pairing::{Admission, Pending},
    browser_pipe::Connection,
};
use avesra_contracts::{
    ErrorCode,
    browser::{self, Client, Id},
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
}
impl Authenticated {
    pub(crate) fn verified(
        owner: Uuid,
        challenge: Id,
        response: browser::Authenticated,
        expires: Instant,
    ) -> Self {
        Self {
            owner,
            challenge,
            response,
            expires,
        }
    }
}
pub enum Incoming {
    Message(Client),
    Authentication(Authentication),
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
        self.authentication_pending = false;
        Ok(seal.response)
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
