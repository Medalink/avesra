//! Public LAN discovery metadata. An advertisement is not authenticated identity.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PORT: u16 = 9476;
pub const MAX_PACKET: usize = 8192;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub product: String,
    pub version: u16,
    pub nonce: Uuid,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advertisement {
    pub product: String,
    pub version: u16,
    pub protocol: u16,
    pub nonce: Uuid,
    pub name: String,
    pub certificate: String,
    pub pairing_open: bool,
}

pub fn local_address(ip: std::net::Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local() || ip.is_loopback()
}

impl Advertisement {
    pub fn valid(&self, nonce: Uuid) -> bool {
        self.product == "Avesra"
            && self.version == 1
            && self.protocol == crate::PROTOCOL_VERSION
            && self.nonce == nonce
            && !nonce.is_nil()
            && !self.name.is_empty()
            && self.name.len() <= 64
            && !self.name.chars().any(char::is_control)
            && !self.certificate.is_empty()
            && self.certificate.len() <= 4096
    }
}
