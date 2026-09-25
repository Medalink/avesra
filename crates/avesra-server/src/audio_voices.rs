//! Typed private generated-voice operations with retained cancellation ownership.
use super::{AudioClient, ErrorCode, STANDARD};
use avesra_contracts::voice::VoiceIdentity;
use base64::Engine;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version: u16,
    pub identity: VoiceIdentity,
    pub base_revision: String,
    pub design_revision: String,
    pub kind: String,
    pub text: String,
    pub description: String,
    pub created_at_ms: u64,
    pub sample_rate: u32,
    pub samples: usize,
}
impl Candidate {
    fn validate(&self) -> Result<(), ErrorCode> {
        self.identity.validate()?;
        if self.version != 1
            || self.base_revision != "5d83992436eae1d760afd27aff78a71d676296fc"
            || self.design_revision != "5ecdb67327fd37bb2e042aab12ff7391903235d3"
            || self.kind != "generated_voice_candidate"
            || self.text.trim().is_empty()
            || self.text.len() > 2048
            || self.description.trim().is_empty()
            || self.description.len() > 4096
            || self.created_at_ms == 0
            || self.sample_rate != 24000
            || !(24000..=720000).contains(&self.samples)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSummary {
    pub id: Uuid,
    pub revision: Uuid,
    pub state: String,
    pub identity: Option<VoiceIdentity>,
    pub description: Option<String>,
    pub created_at_ms: Option<u64>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceStatus {
    pub selected: Option<VoiceIdentity>,
    pub selection_revision: Option<String>,
    pub selection_state: String,
    pub candidates: Vec<CandidateSummary>,
    pub active_voice: Option<VoiceIdentity>,
    pub active_state: String,
}
fn revision(value: Option<&str>) -> bool {
    value.is_none_or(|v| {
        v.len() == 64
            && v.bytes()
                .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
    })
}
impl VoiceStatus {
    fn validate(&self) -> Result<(), ErrorCode> {
        if !revision(self.selection_revision.as_deref())
            || self.candidates.len() > 32
            || !matches!(
                self.selection_state.as_str(),
                "none" | "unreadable" | "unavailable" | "available"
            )
            || !matches!(self.active_state.as_str(), "unavailable" | "available")
        {
            return Err(ErrorCode::Malformed);
        }
        if let Some(value) = &self.selected {
            value.validate()?;
        }
        if let Some(value) = &self.active_voice {
            value.validate()?;
        }
        let selection_consistent = match self.selection_state.as_str() {
            "none" => self.selected.is_none() && self.selection_revision.is_none(),
            "unreadable" => self.selected.is_none() && self.selection_revision.is_some(),
            "unavailable" | "available" => {
                self.selected.is_some() && self.selection_revision.is_some()
            }
            _ => false,
        };
        if !selection_consistent {
            return Err(ErrorCode::Malformed);
        }
        let mut ids = std::collections::HashSet::new();
        for candidate in &self.candidates {
            if candidate.id.is_nil()
                || candidate.revision.is_nil()
                || !ids.insert((candidate.id, candidate.revision))
            {
                return Err(ErrorCode::Malformed);
            }
            match candidate.state.as_str() {
                "available" => {
                    let identity = candidate.identity.as_ref().ok_or(ErrorCode::Malformed)?;
                    identity.validate()?;
                    if identity.id != candidate.id
                        || identity.revision != candidate.revision
                        || candidate
                            .description
                            .as_ref()
                            .is_none_or(|v| v.trim().is_empty() || v.len() > 4096)
                        || candidate.created_at_ms.is_none_or(|v| v == 0)
                    {
                        return Err(ErrorCode::Malformed);
                    }
                }
                "unavailable" => {
                    if candidate.identity.is_some()
                        || candidate.description.is_some()
                        || candidate.created_at_ms.is_some()
                    {
                        return Err(ErrorCode::Malformed);
                    }
                }
                _ => return Err(ErrorCode::Malformed),
            }
        }
        let available = self.selected.as_ref().is_some_and(|selected| {
            self.candidates.iter().any(|candidate| {
                candidate.state == "available" && candidate.identity.as_ref() == Some(selected)
            })
        });
        if (self.selection_state == "none") != (self.selection_revision.is_none())
            || (self.selection_state == "available") != available
            || (self.selection_state == "unreadable" && self.selected.is_some())
            || (self.selection_state == "unavailable" && self.selected.is_none())
            || (self.active_state == "available"
                && (!available || self.active_voice != self.selected))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply<T> {
    request_id: Uuid,
    session_id: Uuid,
    capture_epoch: u64,
    lane: String,
    model_revision: String,
    result: T,
}
struct CommandLease {
    client: Arc<AudioClient>,
    permit: Option<OwnedSemaphorePermit>,
    id: Uuid,
    started: Instant,
    sent: bool,
}
impl Drop for CommandLease {
    fn drop(&mut self) {
        let Some(permit) = self.permit.take() else {
            return;
        };
        if !self.sent {
            return;
        }
        let client = self.client.clone();
        let id = self.id;
        let remaining = Duration::from_secs(31).saturating_sub(self.started.elapsed());
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _permit = permit;
                if client.cancel(id).await.is_err() {
                    tokio::time::sleep(remaining).await;
                }
            });
        } else {
            self.client.admission.close();
        }
    }
}
impl AudioClient {
    async fn voice_command<T: DeserializeOwned>(
        self: &Arc<Self>,
        epoch: u64,
        lane: &str,
        operation: &str,
        payload: serde_json::Value,
    ) -> Result<T, ErrorCode> {
        if epoch != self.epoch.load(Ordering::SeqCst)
            || self
                .deployment
                .as_ref()
                .is_none_or(|(value, _)| value != lane)
        {
            return Err(ErrorCode::Stale);
        }
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::Unavailable)?;
        let id = Uuid::new_v4();
        let mut request = self.request(operation, id, epoch, 30_000)?;
        request["payload"] = payload;
        let mut lease = CommandLease {
            client: self.clone(),
            permit: Some(permit),
            id,
            started: Instant::now(),
            sent: true,
        };
        let value = self.exchange(request, Duration::from_secs(30)).await?;
        let reply: Reply<T> = serde_json::from_value(value).map_err(|_| ErrorCode::Malformed)?;
        if epoch != self.epoch.load(Ordering::SeqCst)
            || reply.request_id != id
            || reply.session_id != self.session_id
            || reply.capture_epoch != epoch
            || reply.lane != lane
            || Some(reply.model_revision.as_str()) != self.configured_revision()
        {
            return Err(ErrorCode::Stale);
        }
        lease.sent = false;
        Ok(reply.result)
    }
    pub async fn voice_status(self: &Arc<Self>, epoch: u64) -> Result<VoiceStatus, ErrorCode> {
        let value: VoiceStatus = self
            .voice_command(epoch, "tts", "voice_status", serde_json::json!({}))
            .await?;
        value.validate()?;
        Ok(value)
    }
    pub async fn create_voice(
        self: &Arc<Self>,
        epoch: u64,
        text: &str,
        description: &str,
    ) -> Result<Candidate, ErrorCode> {
        if text.trim().is_empty()
            || text.len() > 512
            || description.trim().is_empty()
            || description.len() > 1024
        {
            return Err(ErrorCode::Malformed);
        }
        let value: Candidate = self
            .voice_command(
                epoch,
                "voice-design",
                "create_voice",
                serde_json::json!({"text":text,"description":description}),
            )
            .await?;
        value.validate()?;
        if value.text != text || value.description != description {
            return Err(ErrorCode::Stale);
        }
        Ok(value)
    }
    pub async fn select_voice(
        self: &Arc<Self>,
        epoch: u64,
        voice: &VoiceIdentity,
        expected_revision: Option<&str>,
    ) -> Result<(), ErrorCode> {
        voice.validate()?;
        if !revision(expected_revision) {
            return Err(ErrorCode::Malformed);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Selected {
            selected: VoiceIdentity,
        }
        let value: Selected = self
            .voice_command(
                epoch,
                "tts",
                "select_voice",
                serde_json::json!({"identity":voice,"selection_revision":expected_revision}),
            )
            .await?;
        if value.selected != *voice {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    pub async fn clear_voice(
        self: &Arc<Self>,
        epoch: u64,
        expected_revision: Option<&str>,
    ) -> Result<(), ErrorCode> {
        if !revision(expected_revision) {
            return Err(ErrorCode::Malformed);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Cleared {
            selected: Option<VoiceIdentity>,
        }
        let value: Cleared = self
            .voice_command(
                epoch,
                "tts",
                "clear_voice",
                serde_json::json!({"selection_revision":expected_revision}),
            )
            .await?;
        if value.selected.is_some() {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub async fn discard_voice(
        self: &Arc<Self>,
        epoch: u64,
        id: Uuid,
        revision: Uuid,
    ) -> Result<(), ErrorCode> {
        if id.is_nil() || revision.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Discarded {
            outcome: String,
        }
        let value: Discarded = self
            .voice_command(
                epoch,
                "tts",
                "discard_voice",
                serde_json::json!({"id":id,"revision":revision}),
            )
            .await?;
        if value.outcome != "discarded" {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    /// Generated-reference preview does not select a voice or synthesize new speech.
    pub async fn preview_voice(
        self: &Arc<Self>,
        epoch: u64,
        voice: &VoiceIdentity,
    ) -> Result<Vec<i16>, ErrorCode> {
        voice.validate()?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Preview {
            voice: VoiceIdentity,
            pcm_s16le: String,
            sample_rate: u32,
        }
        let value: Preview = self
            .voice_command(
                epoch,
                "tts",
                "preview_voice",
                serde_json::json!({"identity":voice}),
            )
            .await?;
        if value.voice != *voice
            || value.sample_rate != 24000
            || !super::valid_pcm(&value.pcm_s16le, 48000, 1_440_000)
        {
            return Err(ErrorCode::Malformed);
        }
        let bytes = STANDARD
            .decode(value.pcm_s16le)
            .map_err(|_| ErrorCode::Malformed)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect())
    }
}
