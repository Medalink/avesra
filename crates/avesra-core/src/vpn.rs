//! Explicit selected Cisco target and observations; none creates action authority.
use crate::diagnostics::{Reading, VpnState};
use avesra_contracts::{Action, ActionPayload, ErrorCode, Outcome};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: Uuid,
    pub actor: Uuid,
    pub revision: Uuid,
    pub name: String,
    pub address: IpAddr,
    pub source_sha256: [u8; 32],
}
impl Profile {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [self.id, self.actor, self.revision]
            .iter()
            .any(Uuid::is_nil)
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
            || self.address.is_unspecified()
            || self.address.is_loopback()
            || self.address.is_multicast()
            || self.source_sha256 == [0; 32]
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn action(&self, action: &Action) -> Result<(), ErrorCode> {
        self.validate()?;
        if action.actor_id != self.actor
            || action.target_id != self.id
            || action.payload
                != (ActionPayload::ConnectVpn {
                    profile_id: self.id,
                })
        {
            return Err(ErrorCode::Denied);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Connected,
    AlreadyConnected,
    OtherConnection,
    NeedsOwner,
    Uncertain,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerStep {
    Authentication,
    BannerOrCertificate,
    ReconcileConnection,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub profile: Uuid,
    pub revision: Uuid,
    pub state: State,
    pub command_issued: bool,
    pub observed: Reading<VpnState>,
    pub target_matched: bool,
    pub owner_step: Option<OwnerStep>,
}
impl Report {
    pub fn validate(&self, action: &Action, outcome: Outcome) -> Result<(), ErrorCode> {
        if self.profile.is_nil()
            || self.revision.is_nil()
            || action.target_id != self.profile
            || action.payload
                != (ActionPayload::ConnectVpn {
                    profile_id: self.profile,
                })
            || (self.target_matched
                && !matches!(
                    self.observed,
                    Reading::Available {
                        value: VpnState::Connected
                    }
                ))
            || (matches!(self.state, State::Connected | State::AlreadyConnected)
                && !self.target_matched)
            || (self.state == State::Connected && !self.command_issued)
            || (matches!(self.state, State::AlreadyConnected | State::OtherConnection)
                && self.command_issued)
            || (outcome == Outcome::Success && self.state != State::Connected)
            || (outcome == Outcome::AlreadySatisfied
                && (self.state != State::AlreadyConnected
                    || self.command_issued
                    || self.owner_step.is_some()))
            || (self.command_issued
                && !matches!(
                    outcome,
                    Outcome::Success | Outcome::UnknownEffect | Outcome::NeedsInput
                ))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
