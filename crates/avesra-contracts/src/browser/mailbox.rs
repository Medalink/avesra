//! Untrusted mailbox transport. These values cannot create a browser owner.
use super::{Origin, documents::Candidate, reading::Context};
use crate::ErrorCode;
use serde::{Deserialize, Serialize};

pub const LIFETIME_MS: u64 = 120_000;
pub const CHUNK_BYTES: usize = 8192;
pub const BODY_BYTES: usize = 65_536;
pub const TOTAL_BODY_BYTES: usize = 1_048_576;
// JSON escaping plus bounded headers/provenance, separately from body limits.
pub const STREAM_BYTES: usize = 8_388_608;
pub const MAX_CHUNKS: u16 = 1024;
pub const EMPTY_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub fn account(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && value.len() <= 254
        && value.is_ascii()
        && !domain.contains('@')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}
pub fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    pub context: Context,
    pub ordinal: u16,
    pub previous: String,
    pub data: String,
}
impl Chunk {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.context.validate()?;
        if self.ordinal >= MAX_CHUNKS
            || !digest(&self.previous)
            || self.data.is_empty()
            || self.data.len() > CHUNK_BYTES
        {
            return Err(ErrorCode::Malformed);
        }
        super::reading::encoded_bound(self)
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ack {
    pub context: Context,
    pub ordinal: u16,
    pub digest: String,
}
impl Ack {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.context.validate()?;
        if self.ordinal >= MAX_CHUNKS || !digest(&self.digest) {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Incomplete {
    AccountUnverified,
    InboxMembershipUnverified,
    IndividualOrderUnverified,
    DateUnverified,
    BodyIncomplete,
    PaginationUnverified,
    ProviderUnsupported,
    LimitReached,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    pub document: Candidate,
    pub dom_revision: u64,
    pub account: String,
    pub chunks: u16,
    pub bytes: u32,
    pub digest: String,
    pub incomplete: Option<Incomplete>,
}
impl Terminal {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.document
            .validate(&Origin::parse("https://mail.google.com")?)?;
        if !account(&self.account)
            || self.dom_revision == 0
            || self.dom_revision > super::MAX_SAFE_COUNTER
            || self.chunks > MAX_CHUNKS
            || self.bytes as usize > STREAM_BYTES
            || !digest(&self.digest)
            || (self.chunks == 0) != (self.bytes == 0)
            || (self.chunks == 0 && (self.digest != EMPTY_DIGEST || self.incomplete.is_none()))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
