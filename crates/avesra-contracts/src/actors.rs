//! Native owner configuration assertions, never speaker qualification or grants.
use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: u16 = 2;
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Register {
        actor: Uuid,
        owner_revision: Uuid,
    },
    Status,
    Revoke {
        actor: Uuid,
        registration_revision: Uuid,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request: Uuid,
    pub attempt: Uuid,
    pub session: Uuid,
    pub action_epoch: u64,
    pub command: Command,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        if self.attempt.is_nil()
            || self.request.is_nil()
            || self.session.is_nil()
            || self.action_epoch == 0
            || self.action_epoch > crate::browser::MAX_SAFE_COUNTER
        {
            return Err(ErrorCode::Malformed);
        }
        let ids = match self.command {
            Command::Register {
                actor,
                owner_revision,
            } => Some([actor, owner_revision]),
            Command::Revoke {
                actor,
                registration_revision,
            } => Some([actor, registration_revision]),
            Command::Status => None,
        };
        if ids.is_some_and(|ids| ids.iter().any(Uuid::is_nil)) {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub device: Uuid,
    pub actor: Uuid,
    pub owner_revision: Uuid,
    pub registration_revision: Uuid,
    pub registered_by: Uuid,
    pub revoked: bool,
}
impl Binding {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [
            self.device,
            self.actor,
            self.owner_revision,
            self.registration_revision,
            self.registered_by,
        ]
        .iter()
        .any(Uuid::is_nil)
        {
            Err(ErrorCode::Malformed)
        } else {
            Ok(())
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u16,
    pub request: Uuid,
    pub attempt: Uuid,
    pub session: Uuid,
    pub action_epoch: u64,
    #[serde(deserialize_with = "required_binding")]
    pub binding: Option<Binding>,
}
fn required_binding<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Binding>, D::Error> {
    Option::<Binding>::deserialize(deserializer)
}
impl Reply {
    pub fn validate(&self, request: &Request, device: Uuid) -> Result<(), ErrorCode> {
        request.validate()?;
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        if device.is_nil()
            || self.attempt != request.attempt
            || self.request != request.request
            || self.session != request.session
            || self.action_epoch != request.action_epoch
        {
            return Err(ErrorCode::Stale);
        }
        if let Some(binding) = &self.binding {
            binding.validate()?;
            if binding.device != device {
                return Err(ErrorCode::Stale);
            }
        }
        match request.command {
            Command::Register {
                actor,
                owner_revision,
            } => {
                if !self.binding.as_ref().is_some_and(|b| {
                    b.actor == actor
                        && b.owner_revision == owner_revision
                        && b.registered_by == request.request
                        && !b.revoked
                }) {
                    return Err(ErrorCode::Stale);
                }
            }
            Command::Revoke {
                actor,
                registration_revision,
            } => {
                if !self.binding.as_ref().is_some_and(|b| {
                    b.actor == actor
                        && b.registration_revision == registration_revision
                        && b.revoked
                }) {
                    return Err(ErrorCode::Stale);
                }
            }
            Command::Status => {}
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancel {
    pub version: u16,
    pub attempt: Uuid,
    pub session: Uuid,
    pub action_epoch: u64,
}
impl Cancel {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        if self.attempt.is_nil()
            || self.session.is_nil()
            || self.action_epoch == 0
            || self.action_epoch > crate::browser::MAX_SAFE_COUNTER
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
