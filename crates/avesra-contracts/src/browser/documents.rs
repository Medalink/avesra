//! Transient document metadata; none of these values authorize page effects.
use super::{Id, MAX_SAFE_COUNTER, Origin, ScopeRef};
use crate::ErrorCode;
use serde::{Deserialize, Serialize};

pub const MAX_CANDIDATES: usize = 16;
pub const LIFETIME_MS: u64 = 5_000;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub tab: u32,
    pub window: u32,
    pub frame: u32,
    pub document: Id,
    pub url: String,
}
impl Candidate {
    pub fn validate(&self, origin: &Origin) -> Result<(), ErrorCode> {
        if self.tab == 0
            || self.tab > i32::MAX as u32
            || self.window == 0
            || self.window > i32::MAX as u32
            || self.frame != 0
            || self.url.is_empty()
            || self.url.len() > 2048
            || !self.url.is_ascii()
            || self
                .url
                .bytes()
                .any(|v| v.is_ascii_control() || v.is_ascii_whitespace())
        {
            return Err(ErrorCode::Malformed);
        }
        let parsed = url::Url::parse(&self.url).map_err(|_| ErrorCode::Malformed)?;
        if parsed.as_str() != self.url
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || Origin::parse(&parsed.origin().ascii_serialization())? != *origin
        {
            return Err(ErrorCode::Denied);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mode {
    Discover,
    Revalidate { candidate: Candidate },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub request: Id,
    pub scope: ScopeRef,
    pub origin: Origin,
    pub remaining_ms: u64,
    pub mode: Mode,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.remaining_ms == 0 || self.remaining_ms > LIFETIME_MS {
            return Err(ErrorCode::Expired);
        }
        if let Mode::Revalidate { candidate } = &self.mode {
            candidate.validate(&self.origin)?;
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Available { candidates: Vec<Candidate> },
    Unavailable,
    Expired,
    TooMany,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub request: Id,
    pub scope: ScopeRef,
    pub observation_revision: u64,
    pub outcome: Outcome,
}
impl Reply {
    pub fn validate(&self, request: &Request) -> Result<(), ErrorCode> {
        request.validate()?;
        if self.request != request.request
            || self.scope != request.scope
            || self.observation_revision == 0
            || self.observation_revision > MAX_SAFE_COUNTER
        {
            return Err(ErrorCode::Stale);
        }
        if let Outcome::Available { candidates } = &self.outcome {
            if candidates.len() > MAX_CANDIDATES {
                return Err(ErrorCode::TooLarge);
            }
            for (index, candidate) in candidates.iter().enumerate() {
                candidate.validate(&request.origin)?;
                if candidates[..index]
                    .iter()
                    .any(|v| v.tab == candidate.tab || v.document == candidate.document)
                {
                    return Err(ErrorCode::Malformed);
                }
            }
            if let Mode::Revalidate { candidate } = &request.mode
                && candidates.as_slice() != std::slice::from_ref(candidate)
            {
                return Err(ErrorCode::Stale);
            }
        }
        Ok(())
    }
}
