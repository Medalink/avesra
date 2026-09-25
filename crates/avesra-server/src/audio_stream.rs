//! Controller-owned incremental ASR lease. Interim text is never owner authority.
use super::{AudioClient, ErrorCode, STANDARD};
use base64::Engine;
use serde::Deserialize;
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transcript {
    text: String,
    r#final: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    request_id: Uuid,
    session_id: Uuid,
    capture_epoch: u64,
    chunk_sequence: u64,
    lane: String,
    model_revision: String,
    result: Transcript,
}
pub struct TranscriptChunk {
    pub worker_id: Uuid,
    pub utterance_id: Uuid,
    pub session_id: Uuid,
    pub capture_epoch: u64,
    pub sequence: u64,
    pub text: String,
    pub final_chunk: bool,
}
pub struct AsrStream {
    client: Arc<AudioClient>,
    permit: Option<OwnedSemaphorePermit>,
    worker_id: Uuid,
    utterance_id: Uuid,
    epoch: u64,
    next: u64,
    samples: usize,
    started: Instant,
    issued: u64,
    expires: u64,
    complete: bool,
    failed: bool,
    sent: bool,
}
impl AudioClient {
    pub async fn begin_stream(
        self: &Arc<Self>,
        epoch: u64,
        utterance_id: Uuid,
    ) -> Result<AsrStream, ErrorCode> {
        if epoch != self.epoch.load(Ordering::SeqCst)
            || utterance_id.is_nil()
            || self
                .deployment
                .as_ref()
                .is_none_or(|(lane, _)| lane != "asr")
        {
            return Err(ErrorCode::Stale);
        }
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ErrorCode::Unavailable)?;
        let health = self.health().await?;
        if !health.streaming
            || health.state != "loaded_unqualified"
            || health.busy
            || epoch != self.epoch.load(Ordering::SeqCst)
        {
            return Err(ErrorCode::Unavailable);
        }
        {
            let mut recent = self
                .recent_streams
                .lock()
                .map_err(|_| ErrorCode::Unavailable)?;
            recent.retain(|_, until| *until > Instant::now());
            if recent.contains_key(&utterance_id) || recent.len() >= 128 {
                return Err(ErrorCode::Stale);
            }
            recent.insert(utterance_id, Instant::now() + Duration::from_secs(31));
        }
        let worker_id = Uuid::new_v4();
        let template = self.request("stream", worker_id, epoch, 30_000)?;
        let issued = template["issued_at_ms"]
            .as_u64()
            .ok_or(ErrorCode::Malformed)?;
        let expires = template["expires_at_ms"]
            .as_u64()
            .ok_or(ErrorCode::Malformed)?;
        Ok(AsrStream {
            client: self.clone(),
            permit: Some(permit),
            worker_id,
            utterance_id,
            epoch,
            next: 1,
            samples: 0,
            started: Instant::now(),
            issued,
            expires,
            complete: false,
            failed: false,
            sent: false,
        })
    }
}
impl AsrStream {
    /// Captured time comes from validated native media/transport age, not a
    /// timestamp invented after receipt. Chunk sequencing cannot renumber loss.
    pub async fn push(
        &mut self,
        samples: &[i16],
        source_sequence: u64,
        captured: Instant,
        final_chunk: bool,
    ) -> Result<TranscriptChunk, ErrorCode> {
        if self.failed
            || self.complete
            || self.epoch != self.client.epoch.load(Ordering::SeqCst)
            || self.started.elapsed() >= Duration::from_secs(30)
        {
            self.failed = true;
            return Err(ErrorCode::Stale);
        }
        if source_sequence != self.next
            || samples.len() > 3200
            || (!final_chunk && samples.is_empty())
            || self.samples.saturating_add(samples.len()) > 480_000
            || captured > Instant::now()
            || captured.elapsed() > Duration::from_millis(500)
        {
            self.failed = true;
            return Err(ErrorCode::Malformed);
        }
        self.failed = true; // Any cancelled/incomplete await forbids retry of a chunk.
        let mut request = self
            .client
            .request("stream", self.worker_id, self.epoch, 30_000)?;
        let now = request["issued_at_ms"]
            .as_u64()
            .ok_or(ErrorCode::Malformed)?;
        let expected = self
            .issued
            .saturating_add(self.started.elapsed().as_millis() as u64);
        if now >= self.expires || now.abs_diff(expected) > 1000 {
            return Err(ErrorCode::Expired);
        }
        let mut raw = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            raw.extend_from_slice(&sample.to_le_bytes());
        }
        request["expires_at_ms"] = self.expires.into();
        request["chunk_sequence"] = self.next.into();
        request["final"] = final_chunk.into();
        request["payload"] = serde_json::json!({"pcm_s16le":STANDARD.encode(&raw)});
        drop(raw);
        self.sent = true;
        let remaining = Duration::from_secs(30).saturating_sub(self.started.elapsed());
        let value = self
            .client
            .exchange(request, remaining.min(Duration::from_secs(2)))
            .await?;
        let reply: Reply = serde_json::from_value(value).map_err(|_| ErrorCode::Malformed)?;
        let revision = self
            .client
            .configured_revision()
            .ok_or(ErrorCode::Unavailable)?;
        if self.epoch != self.client.epoch.load(Ordering::SeqCst)
            || self.started.elapsed() >= Duration::from_secs(30)
            || reply.request_id != self.worker_id
            || reply.session_id != self.client.session_id
            || reply.capture_epoch != self.epoch
            || reply.chunk_sequence != self.next
            || reply.lane != "asr"
            || reply.model_revision != revision
            || reply.result.r#final != final_chunk
            || reply.result.text.len() > 8192
        {
            return Err(ErrorCode::Stale);
        }
        let sequence = self.next;
        self.next = self.next.checked_add(1).ok_or(ErrorCode::Stale)?;
        self.samples += samples.len();
        self.failed = false;
        self.complete = final_chunk;
        if final_chunk {
            self.permit.take();
        }
        Ok(TranscriptChunk {
            worker_id: self.worker_id,
            utterance_id: self.utterance_id,
            session_id: reply.session_id,
            capture_epoch: self.epoch,
            sequence,
            text: reply.result.text,
            final_chunk,
        })
    }
}
impl Drop for AsrStream {
    fn drop(&mut self) {
        let Some(permit) = self.permit.take() else {
            return;
        };
        if !self.sent {
            return;
        }
        let client = self.client.clone();
        let request = self.worker_id;
        let remaining = Duration::from_secs(31).saturating_sub(self.started.elapsed());
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _permit = permit;
                // Preserve ownership until cancellation acknowledgement or the
                // complete fixed remote lifetime. Dropping a HTTP/socket caller
                // is not evidence that the worker stopped processing.
                if client.cancel(request).await.is_err() {
                    tokio::time::sleep(remaining).await;
                }
            });
        } else {
            self.client.admission.close();
        }
    }
}
