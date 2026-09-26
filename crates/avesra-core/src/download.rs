//! Exact owner-supplied context and measured observations, never an approval.
use crate::diagnostics::{Reading, Report as HostReport};
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateUnit {
    BytesPerSecond,
    MegabytesPerSecond,
    MegabitsPerSecond,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub application: String,
    pub endpoint: String,
    pub port: u16,
    pub destination_drive: Option<char>,
    /// Integer thousandths of the explicitly selected unit, as reported by owner.
    pub reported_rate_milli: Option<u64>,
    pub reported_unit: RateUnit,
    pub reported_throttle: Option<bool>,
}
impl Input {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.application.len() > 128
            || self.application.chars().any(char::is_control)
            || !valid_endpoint(&self.endpoint)
            || !matches!(self.port, 80 | 443)
            || self
                .destination_drive
                .is_some_and(|drive| !drive.is_ascii_uppercase())
            || self
                .reported_rate_milli
                .is_some_and(|v| v > 1_000_000_000_000)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
pub fn valid_endpoint(value: &str) -> bool {
    if let Ok(ip) = value.parse::<IpAddr>() {
        return !ip.is_unspecified() && !ip.is_multicast() && !ip.is_loopback();
    }
    value.len() <= 253
        && value.contains('.')
        && value.is_ascii()
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub id: Uuid,
    pub revision: Uuid,
    pub actor: Uuid,
    pub input: Input,
}
impl Context {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [self.id, self.revision, self.actor]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(ErrorCode::Malformed);
        }
        self.input.validate()
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsState {
    Resolved,
    NameNotFound,
    NoRecords,
    ServerFailure,
    Refused,
    Unavailable,
    TimedOut,
    Numeric,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub address: IpAddr,
    pub route_interface: Reading<u32>,
    pub tcp_connect_micros: Reading<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub context: Uuid,
    pub revision: Uuid,
    pub dns_a: DnsState,
    pub dns_aaaa: DnsState,
    pub dns_micros: u64,
    pub endpoints: Vec<Endpoint>,
    pub destination_space: Reading<crate::diagnostics::DiskSpace>,
    pub resources: Box<HostReport>,
}
impl Report {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.resources.validate()?;
        if self.context.is_nil() || self.revision.is_nil() || self.dns_micros > 30_000_000
            || self.endpoints.len() > 4
            || !matches!(*self.resources, HostReport::HostResources { .. })
            || self.endpoints.iter().any(|v| v.address.is_unspecified() || v.address.is_multicast()
                || v.address.is_loopback()
                || matches!(v.route_interface, Reading::Available { value: 0 })
                || matches!(v.tcp_connect_micros, Reading::Available { value } if value > 1_000_000))
        { return Err(ErrorCode::Malformed); }
        if let Reading::Available { value } = &self.destination_space
            && (!value.drive.is_ascii_uppercase()
                || value.caller_available_bytes > value.caller_total_bytes)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    /// Suggestion eligibility only. This is not evidence that stale cache caused it.
    pub fn dns_failure(&self) -> bool {
        let failure = |s| {
            matches!(
                s,
                DnsState::NameNotFound
                    | DnsState::NoRecords
                    | DnsState::ServerFailure
                    | DnsState::Refused
            )
        };
        self.endpoints.is_empty() && failure(self.dns_a) && failure(self.dns_aaaa)
    }
}

/// Original successful observation only; no frontend report or model assertion.
pub(crate) fn evidence(
    db: &rusqlite::Connection,
    context: &Context,
    session: &crate::ledger::DispatchSession,
    now: u64,
) -> Result<Uuid, ErrorCode> {
    use rusqlite::{OptionalExtension, params};
    context.validate()?;
    if context.actor != session.actor_id || !session.active {
        return Err(ErrorCode::Denied);
    }
    let mut query=db.prepare("SELECT substr(CAST(p.body AS BLOB),1,8193),substr(CAST(g.body AS BLOB),1,8193) FROM action_permissions p JOIN ledger_grants g ON g.id=p.id WHERE p.actor=?1 AND g.revoked=0 LIMIT 129").map_err(|_|ErrorCode::Storage)?;
    let rows = query
        .query_map([context.actor.to_string()], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|_| ErrorCode::Storage)?;
    let mut found = false;
    let mut count = 0;
    for row in rows {
        count += 1;
        let (bytes, grant_bytes) = row.map_err(|_| ErrorCode::Storage)?;
        if bytes.len() > 8192 || grant_bytes.len() > 8192 || count > 128 {
            return Err(ErrorCode::Malformed);
        }
        let permission: crate::action_permissions::Permission =
            serde_json::from_slice(&bytes).map_err(|_| ErrorCode::Malformed)?;
        let grant: crate::policy::Grant =
            serde_json::from_slice(&grant_bytes).map_err(|_| ErrorCode::Malformed)?;
        found |= !grant.revoked
            && grant.id == permission.id
            && grant.actor_id == context.actor
            && permission.actor == context.actor
            && grant.target_id == context.id
            && grant.operations == [avesra_contracts::Operation::Diagnostic]
            && matches!(permission.target,crate::action_permissions::TaskTarget::Download{context:ref saved,configuration:false} if **saved==*context);
    }
    if !found {
        return Err(ErrorCode::Stale);
    }
    let row:Option<(String,Vec<u8>,Vec<u8>,i64)>=db.query_row(
        "SELECT f.dispatch_id,substr(CAST(o.body AS BLOB),1,65537),substr(CAST(a.body AS BLOB),1,32769),f.at_ms FROM native_finalizations f JOIN native_observations o ON o.dispatch_id=f.dispatch_id AND o.action_revision=f.action_revision AND o.target_id=f.target_id AND o.at_ms=f.at_ms JOIN action_revisions a ON a.revision=f.action_revision AND a.dispatch_id=f.dispatch_id JOIN dispatch_bindings b ON b.dispatch_id=f.dispatch_id WHERE f.actor_id=?1 AND f.target_id=?2 AND f.outcome='\"success\"' AND b.actor_id=f.actor_id AND b.device_id=?3 AND b.session_id=?4 AND b.action_epoch=?5 ORDER BY f.rowid DESC LIMIT 1",
        params![context.actor.to_string(),context.id.to_string(),session.device_id.to_string(),session.session_id.to_string(),session.action_epoch.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_|ErrorCode::Storage)?;
    let (dispatch, body, action, at) = row.ok_or(ErrorCode::Stale)?;
    if body.len() > 65536
        || action.len() > 32768
        || at < 0
        || now.saturating_sub(at as u64) > 120_000
        || at as u64 > now
    {
        return Err(ErrorCode::Stale);
    }
    let action: avesra_contracts::Action =
        serde_json::from_slice(&action).map_err(|_| ErrorCode::Malformed)?;
    let observation: crate::execution::EffectObservation =
        serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    observation.validate(&action, avesra_contracts::Outcome::Success)?;
    let crate::execution::EffectObservation::Download { report } = observation else {
        return Err(ErrorCode::Denied);
    };
    if report.context != context.id
        || report.revision != context.revision
        || !report.dns_failure()
        || action.actor_id != context.actor
    {
        return Err(ErrorCode::Denied);
    }
    Uuid::parse_str(&dispatch).map_err(|_| ErrorCode::Malformed)
}
