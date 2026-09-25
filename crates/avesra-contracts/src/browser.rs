//! Browser handshake data. Parsing never establishes identity or action rights.
use crate::ErrorCode;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use uuid::Uuid;

pub const VERSION: u16 = 2;
pub const MAX_MESSAGE: usize = 65536;
pub const HANDSHAKE_SECONDS: u64 = 45;

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
    Poll { session: Id, sequence: u64 },
    Disconnect { session: Id },
}
/// Canonical comparison input, independent of JSON field order/whitespace.
/// Pairing is excluded because the native-generated pairing is issued only after
/// the user compares this original pending challenge. The MAC binds it separately.
pub fn comparison_transcript(challenge: &Challenge) -> Result<Vec<u8>, ErrorCode> {
    if challenge.version != VERSION {
        return Err(ErrorCode::Unsupported);
    }
    Ok(format!(
        "AVESRA-BROWSER-COMPARE-2\n{}\n{}\n{}\n{}\n{}\n",
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
        "AVESRA-BROWSER-AUTH-2\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
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
