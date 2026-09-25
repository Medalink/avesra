//! Read-only deployment-owner liveness metadata, never loaded-artifact proof.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn local_id(value: &str) -> bool {
    bounded(value, 128)
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_' | b'.'))
}
/// Private configuration selects these values; model/native text never does.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub instance: String,
    pub recipe: String,
    pub model: String,
    pub port: u16,
}
impl Expected {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if !local_id(&self.instance)
            || !local_id(&self.recipe)
            || !local_id(&self.model)
            || self.port < 1024
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Reference {
    #[serde(rename_all = "camelCase")]
    Docker {
        container_id: String,
        daemon_id: String,
        executable_path: String,
        executable_token: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    name: String,
    node_id: String,
    engine: String,
    recipe_id: String,
    runtime: String,
    #[serde(rename = "ref")]
    reference: Reference,
    port: u16,
    devices: Vec<String>,
    nonce: String,
    started_at: String,
    ready_deadline_at: String,
}
impl Record {
    fn validate(&self, expected: &Expected) -> Result<(), ErrorCode> {
        let Reference::Docker {
            container_id,
            daemon_id,
            executable_path,
            executable_token,
        } = &self.reference;
        if self.name != expected.instance
            || self.recipe_id != expected.recipe
            || self.node_id != "self"
            || self.engine != "vllm"
            || self.runtime != "docker"
            || self.port != expected.port
            || container_id.len() != 64
            || !container_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !bounded(daemon_id, 1024)
            || !bounded(executable_path, 4096)
            || !executable_path.starts_with('/')
            || !bounded(executable_token, 1024)
            || !bounded(&self.nonce, 128)
            || !bounded(&self.started_at, 64)
            || !bounded(&self.ready_deadline_at, 64)
            || self.devices.is_empty()
            || self.devices.len() > 8
            || self.devices.iter().any(|v| !bounded(v, 128))
            || self
                .devices
                .iter()
                .enumerate()
                .any(|(i, v)| self.devices[..i].contains(v))
        {
            return Err(ErrorCode::Unsupported);
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Roster {
    instances: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct View {
    record: Record,
    state: String,
}
/// Validated owner metadata with original read-admission freshness. It grants
/// neither inference nor unknown-job recovery and deliberately is not persisted.
pub struct ReadyIncarnation {
    record: Record,
    model: String,
    started: Instant,
}
impl ReadyIncarnation {
    /// Admission starts before the HTTP metadata request, not after parsing.
    pub fn parse(bytes: &[u8], expected: &Expected, started: Instant) -> Result<Self, ErrorCode> {
        expected.validate()?;
        if started.elapsed() >= Duration::from_secs(5) {
            return Err(ErrorCode::Expired);
        }
        if bytes.len() > 262_144 {
            return Err(ErrorCode::TooLarge);
        }
        let roster: Roster = serde_json::from_slice(bytes).map_err(|_| ErrorCode::Malformed)?;
        if roster.instances.len() > 64 {
            return Err(ErrorCode::TooLarge);
        }
        let mut selected = None;
        for value in roster.instances {
            let header = value
                .get("record")
                .and_then(|v| v.as_object())
                .ok_or(ErrorCode::Malformed)?;
            let name = header
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|v| bounded(v, 128))
                .ok_or(ErrorCode::Malformed)?;
            let node = header
                .get("nodeId")
                .and_then(|v| v.as_str())
                .filter(|v| bounded(v, 128))
                .ok_or(ErrorCode::Malformed)?;
            if name != expected.instance || node != "self" {
                continue;
            }
            if selected.is_some() {
                return Err(ErrorCode::Malformed);
            }
            let view: View = serde_json::from_value(value).map_err(|_| ErrorCode::Unsupported)?;
            if view.state != "ready" {
                return Err(ErrorCode::Unavailable);
            }
            view.record.validate(expected)?;
            selected = Some(view.record);
        }
        let value = Self {
            record: selected.ok_or(ErrorCode::Unavailable)?,
            model: expected.model.clone(),
            started,
        };
        value.current()?;
        Ok(value)
    }
    pub fn current(&self) -> Result<(), ErrorCode> {
        if self.started.elapsed() < Duration::from_secs(5) {
            Ok(())
        } else {
            Err(ErrorCode::Expired)
        }
    }
    pub fn fingerprint(&self) -> Result<String, ErrorCode> {
        self.current()?;
        let bytes = serde_json::to_vec(&self.record).map_err(|_| ErrorCode::Malformed)?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn artifact_qualified(&self) -> bool {
        false
    }
}
