//! Typed private generated-voice operations with retained cancellation ownership.
use super::{AudioClient, ErrorCode, STANDARD};
use avesra_contracts::voice::VoiceIdentity;
use base64::Engine;
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

use avesra_contracts::voices::valid_selection_revision as revision;
pub use avesra_contracts::voices::{Candidate, VoiceStatus};
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
            || text.chars().count() > 512
            || description.trim().is_empty()
            || description.chars().count() > 1024
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
