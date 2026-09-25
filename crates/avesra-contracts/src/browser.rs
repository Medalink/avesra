//! Browser handshake data. Parsing never establishes identity or action rights.
use crate::ErrorCode;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use uuid::Uuid;
pub mod documents;
pub mod reading;

pub const VERSION: u16 = 5;
pub const MAX_MESSAGE: usize = 65536;
pub const HANDSHAKE_SECONDS: u64 = 45;

/// Exact native origin, not a Chrome match pattern or arbitrary navigation URL.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Origin(String);
impl Origin {
    pub fn parse(value: &str) -> Result<Self, ErrorCode> {
        if value.is_empty()
            || value.len() > 512
            || !value.is_ascii()
            || value
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
        {
            return Err(ErrorCode::Malformed);
        }
        let parsed = url::Url::parse(value).map_err(|_| ErrorCode::Malformed)?;
        let Some(url::Host::Domain(domain)) = parsed.host() else {
            return Err(ErrorCode::Unsupported);
        };
        if domain.len() > 253
            || domain.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
        {
            return Err(ErrorCode::Malformed);
        }
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
            || domain.ends_with('.')
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || value != parsed.origin().ascii_serialization()
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn chrome_pattern(&self) -> String {
        format!("{}/*", self.0)
    }
}
impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?)
            .map_err(|_| D::Error::custom("invalid exact browser origin"))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeOperation {
    Read,
    Navigate,
}
pub fn validate_operations(values: &[ScopeOperation]) -> Result<(), ErrorCode> {
    if values.is_empty() || values.len() > 2 || values.windows(2).any(|v| v[0] >= v[1]) {
        return Err(ErrorCode::Malformed);
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Id(Uuid);
impl Id {
    pub fn new(value: Uuid) -> Result<Self, ErrorCode> {
        if value.is_nil() {
            Err(ErrorCode::Malformed)
        } else {
            Ok(Self(value))
        }
    }
    pub fn uuid(self) -> Uuid {
        self.0
    }
}
impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let id = Uuid::parse_str(&value).map_err(D::Error::custom)?;
        if id.is_nil() || value != id.to_string() {
            return Err(D::Error::custom("noncanonical browser identity"));
        }
        Ok(Self(id))
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Hex32([u8; 32]);
impl Hex32 {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn text(&self) -> String {
        self.0.iter().map(|v| format!("{v:02x}")).collect()
    }
}
impl Serialize for Hex32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text())
    }
}
impl<'de> Deserialize<'de> for Hex32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(D::Error::custom("invalid browser nonce/proof"));
        }
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                .map_err(D::Error::custom)?;
        }
        Ok(Self(bytes))
    }
}
pub fn extension_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|v| (b'a'..=b'p').contains(&v))
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingRef {
    pub id: Id,
    pub revision: Id,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeRef {
    pub id: Id,
    pub revision: Id,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScopeStatus {
    Pending {
        reference: ScopeRef,
        origin: Origin,
        operations: Vec<ScopeOperation>,
        remaining_ms: u64,
    },
    Saving {
        reference: ScopeRef,
    },
    Saved {
        reference: ScopeRef,
    },
    Declined {
        reference: ScopeRef,
    },
    Unavailable {
        reference: ScopeRef,
    },
}
impl ScopeStatus {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if let Self::Pending {
            operations,
            remaining_ms,
            ..
        } = self
        {
            validate_operations(operations)?;
            if *remaining_ms == 0 || *remaining_ms > 45_000 {
                return Err(ErrorCode::Expired);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub version: u16,
    pub installation: Id,
    pub connection: Id,
    pub extension: String,
    pub nonce: Hex32,
    pub pairing: Option<PairingRef>,
}
impl Hello {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Unsupported);
        }
        if !extension_id(&self.extension) {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    pub version: u16,
    pub installation: Id,
    pub connection: Id,
    pub session: Id,
    pub challenge: Id,
    pub nonce: Hex32,
    pub pairing: Option<PairingRef>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authenticate {
    pub version: u16,
    pub session: Id,
    pub challenge: Id,
    pub pairing: PairingRef,
    pub proof: Hex32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authenticated {
    pub version: u16,
    pub session: Id,
    pub installation: Id,
    pub connection: Id,
    pub pairing: PairingRef,
    /// Native ownership generation, never a frontend-granted permission epoch.
    pub generation: u64,
}
pub const MAX_SAFE_COUNTER: u64 = 9_007_199_254_740_991;
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    pub selection: Id,
    pub action_epoch: u64,
    /// Mode permission alone never grants a browser operation.
    pub mode_allowed: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Pending,
    AuthenticatedNoScopes,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusReply {
    pub version: u16,
    pub session: Id,
    pub generation: u64,
    pub sequence: u64,
    pub state: Phase,
    #[serde(deserialize_with = "required_nullable")]
    pub authority: Option<Authority>,
    #[serde(deserialize_with = "required_nullable")]
    pub scope: Option<ScopeStatus>,
    #[serde(deserialize_with = "required_nullable")]
    pub document: Option<documents::Request>,
}
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
impl StatusReply {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Unsupported);
        }
        if self.generation == 0
            || self.generation > MAX_SAFE_COUNTER
            || self.sequence == 0
            || self.sequence > MAX_SAFE_COUNTER
            || (self.state == Phase::Pending && self.authority.is_some())
            || self
                .authority
                .is_some_and(|v| v.action_epoch == 0 || v.action_epoch > MAX_SAFE_COUNTER)
        {
            return Err(ErrorCode::Malformed);
        }
        if let Some(scope) = &self.scope {
            if self.authority.is_none() {
                return Err(ErrorCode::Malformed);
            }
            scope.validate()?;
        }
        if let Some(document) = &self.document {
            if self.authority.is_none() {
                return Err(ErrorCode::Malformed);
            }
            document.validate()?;
        }
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(
    tag = "type",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Client {
    Hello(Hello),
    Authenticate(Authenticate),
    Poll {
        session: Id,
        sequence: u64,
        observation_revision: u64,
    },
    ScopeResult {
        session: Id,
        sequence: u64,
        observation_revision: u64,
        reference: ScopeRef,
        action_epoch: u64,
        permitted: bool,
    },
    DocumentResult {
        session: Id,
        sequence: u64,
        reply: documents::Reply,
    },
    Disconnect {
        session: Id,
    },
}
/// Canonical comparison input, independent of JSON field order/whitespace.
/// Pairing is excluded because the native-generated pairing is issued only after
/// the user compares this original pending challenge. The MAC binds it separately.
pub fn comparison_transcript(challenge: &Challenge) -> Result<Vec<u8>, ErrorCode> {
    if challenge.version != VERSION {
        return Err(ErrorCode::Unsupported);
    }
    Ok(format!(
        "AVESRA-BROWSER-COMPARE-5\n{}\n{}\n{}\n{}\n{}\n",
        challenge.installation.uuid(),
        challenge.connection.uuid(),
        challenge.session.uuid(),
        challenge.challenge.uuid(),
        challenge.nonce.text()
    )
    .into_bytes())
}

/// Credential-independent canonical transcript. Caller must validate the
/// challenge against its actual pending native session before using it.
pub fn transcript(
    hello: &Hello,
    challenge: &Challenge,
    pairing: PairingRef,
) -> Result<Vec<u8>, ErrorCode> {
    hello.validate()?;
    if challenge.version != VERSION
        || challenge.installation != hello.installation
        || challenge.connection != hello.connection
        || challenge.pairing != Some(pairing)
        || hello.pairing != Some(pairing)
    {
        return Err(ErrorCode::Stale);
    }
    Ok(format!(
        "AVESRA-BROWSER-AUTH-5\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        pairing.id.uuid(),
        pairing.revision.uuid(),
        hello.installation.uuid(),
        hello.connection.uuid(),
        challenge.session.uuid(),
        hello.nonce.text(),
        challenge.nonce.text(),
        hello.extension
    )
    .into_bytes())
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ErrorCode> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE {
        return Err(ErrorCode::TooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_| ErrorCode::Malformed)
}
