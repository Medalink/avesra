//! Accepted reply output assertions; decoding is not native publication proof.
use crate::{ErrorCode, browser::MAX_SAFE_COUNTER, planner, voice::VoiceIdentity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: u16 = 6;
pub const MAX_TEXT_BYTES: usize = 8192;
pub const MAX_SEGMENT_BYTES: usize = 512;
pub const MAX_SEGMENTS: usize = 64;
pub const FRAME_SAMPLES: u64 = 480;
pub const MAX_SAMPLES: u64 = 720_000;
/// Exact borrowed partition of accepted text, never a summary or replay handle.
/// Validate the entire partition before starting any private synthesis job.
pub fn text_segments(mut text: &str) -> Result<Vec<&str>, ErrorCode> {
    if !planner::valid_text(text) || text.len() > MAX_TEXT_BYTES {
        return Err(ErrorCode::Malformed);
    }
    let mut segments = Vec::new();
    while !text.is_empty() {
        if segments.len() == MAX_SEGMENTS || text.trim().is_empty() {
            return Err(ErrorCode::Unsupported);
        }
        let end = if text.len() <= MAX_SEGMENT_BYTES {
            text.len()
        } else {
            let mut whitespace = None;
            let mut sentence = None;
            for (index, ch) in text
                .char_indices()
                .take_while(|(i, _)| *i <= MAX_SEGMENT_BYTES)
            {
                if !ch.is_whitespace() {
                    continue;
                }
                let after = index + ch.len_utf8();
                let end = if after <= MAX_SEGMENT_BYTES {
                    after
                } else {
                    index
                };
                whitespace = Some(end);
                if end >= MAX_SEGMENT_BYTES / 2
                    && text[..index].ends_with(['.', '!', '?', '\u{3002}', '\u{ff01}', '\u{ff1f}'])
                {
                    sentence = Some(end);
                }
            }
            sentence.or(whitespace).ok_or(ErrorCode::Unsupported)?
        };
        let (segment, remaining) = text.split_at(end);
        if segment.trim().is_empty() {
            return Err(ErrorCode::Unsupported);
        }
        segments.push(segment);
        text = remaining;
    }
    Ok(segments)
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Learning,
    Action,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provenance {
    #[default]
    Model,
    NativeMailbox {
        dispatch: Uuid,
        action_revision: Uuid,
        grant: Uuid,
        scope: crate::browser::ScopeRef,
        account_sha256: String,
        digest: String,
        requested: u16,
        count: u16,
        sources: Vec<MailboxSource>,
    },
    NativeMemory {
        memory: Option<Uuid>,
        revision: Option<Uuid>,
        value_bearing: bool,
    },
    NativeClock {
        clock_kind: crate::clock::Kind,
        reading: crate::clock::LocalReading,
    },
    NativeObservation {
        dispatch: Uuid,
        action_revision: Uuid,
    },
    NativeEvents {
        event_kind: EventKind,
        batch: Option<Uuid>,
        events: Vec<Uuid>,
    },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailboxSource {
    pub id: String,
    pub thread: String,
    pub reference: String,
    pub timestamp_ms: u64,
}
impl Provenance {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if let Self::NativeMailbox {
            dispatch,
            action_revision,
            grant,
            scope,
            account_sha256,
            digest,
            requested,
            count,
            sources,
        } = self
        {
            let _ = scope; // Non-nil typed IDs are validated by their constructors/decoder.
            if dispatch.is_nil()
                || action_revision.is_nil()
                || grant.is_nil()
                || !crate::browser::mailbox::digest(account_sha256)
                || !crate::browser::mailbox::digest(digest)
                || !(1..=100).contains(requested)
                || count > requested
                || sources.len() > 4
                || sources.len() > usize::from(*count)
                || sources.iter().enumerate().any(|(i, s)| {
                    s.id.is_empty()
                        || s.id.len() > 128
                        || s.thread.is_empty()
                        || s.thread.len() > 128
                        || s.reference.is_empty()
                        || s.reference.len() > 2048
                        || s.timestamp_ms == 0
                        || s.timestamp_ms > 8_640_000_000_000_000
                        || [&s.id, &s.thread, &s.reference]
                            .iter()
                            .any(|v| !v.bytes().all(|b| (33..=126).contains(&b)))
                        || sources[..i].iter().any(|prior| prior.id == s.id)
                })
            {
                return Err(ErrorCode::Malformed);
            }
        }
        if let Self::NativeMemory {
            memory,
            revision,
            value_bearing,
        } = self
            && ((*value_bearing && memory.is_none())
                || memory.is_none() != revision.is_none()
                || memory.is_some_and(|v| v.is_nil())
                || revision.is_some_and(|v| v.is_nil()))
        {
            return Err(ErrorCode::Malformed);
        }
        if let Self::NativeClock { reading, .. } = self {
            reading.validate()?;
        }
        if let Self::NativeObservation {
            dispatch,
            action_revision,
        } = self
            && (dispatch.is_nil() || action_revision.is_nil())
        {
            return Err(ErrorCode::Malformed);
        }
        if let Self::NativeEvents { batch, events, .. } = self
            && (batch.is_some_and(|id| id.is_nil())
                || events.len() > 32
                || batch.is_none() != events.is_empty()
                || events
                    .iter()
                    .enumerate()
                    .any(|(i, id)| id.is_nil() || events[..i].contains(id)))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub planner: planner::Context,
    pub reply_revision: Uuid,
    pub response: planner::Response,
    pub provenance: Provenance,
}
impl Source {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.planner.validate()?;
        self.provenance.validate()?;
        if let Provenance::NativeClock {
            clock_kind,
            reading,
        } = &self.provenance
            && self.response.text() != reading.answer(*clock_kind)?
        {
            return Err(ErrorCode::Malformed);
        }
        if self.reply_revision.is_nil()
            || matches!(self.response, planner::Response::Proposal { .. })
            || !planner::valid_text(self.response.text())
            || (!matches!(self.provenance, Provenance::Model)
                && !matches!(self.response, planner::Response::Answer { .. }))
        {
            return Err(ErrorCode::Malformed);
        }
        text_segments(self.response.text())?;
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub source: Source,
    pub request: Uuid,
    pub playback_epoch: u64,
    pub voice: VoiceIdentity,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        self.source.validate()?;
        self.stream_context().validate()
    }
    pub fn stream_context(&self) -> Context {
        Context {
            planner: self.source.planner.clone(),
            reply_revision: self.source.reply_revision,
            request: self.request,
            playback_epoch: self.playback_epoch,
            voice: self.voice.clone(),
            provenance: self.source.provenance.clone(),
        }
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub planner: planner::Context,
    pub reply_revision: Uuid,
    pub request: Uuid,
    pub playback_epoch: u64,
    pub voice: VoiceIdentity,
    pub provenance: Provenance,
}
impl Context {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.planner.validate()?;
        self.provenance.validate()?;
        self.voice.validate()?;
        if self.reply_revision.is_nil()
            || self.request.is_nil()
            || self.playback_epoch == 0
            || self.playback_epoch > MAX_SAFE_COUNTER
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    Complete,
    Truncated,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Ready {
        sample_rate: u32,
        frame_samples: u64,
        max_samples: u64,
    },
    Audio {
        sequence: u64,
        sample_offset: u64,
        samples: Vec<i16>,
    },
    End {
        chunks: u64,
        samples: u64,
        outcome: Completion,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub version: u16,
    pub context: Context,
    pub message: Message,
}
impl Event {
    pub fn validate(&self, expected: &Context) -> Result<(), ErrorCode> {
        if self.version != VERSION {
            return Err(ErrorCode::Version);
        }
        self.context.validate()?;
        expected.validate()?;
        if self.context != *expected {
            return Err(ErrorCode::Stale);
        }
        match &self.message {
            Message::Ready {
                sample_rate,
                frame_samples,
                max_samples,
            } if *sample_rate == 24000
                && *frame_samples == FRAME_SAMPLES
                && *max_samples == MAX_SAMPLES =>
            {
                Ok(())
            }
            Message::Audio {
                sequence,
                sample_offset,
                samples,
            } if (1..=MAX_SAMPLES / FRAME_SAMPLES).contains(sequence)
                && *sample_offset == (*sequence - 1) * FRAME_SAMPLES
                && samples.len() == FRAME_SAMPLES as usize =>
            {
                Ok(())
            }
            Message::End {
                chunks, samples, ..
            } if (1..=MAX_SAMPLES / FRAME_SAMPLES).contains(chunks)
                && *samples == *chunks * FRAME_SAMPLES =>
            {
                Ok(())
            }
            _ => Err(ErrorCode::Malformed),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    Play { request: Uuid },
    Submitted { request: Uuid, samples: u64 },
    Cancel { request: Uuid },
}
impl Control {
    pub fn validate(&self, expected: Uuid) -> Result<(), ErrorCode> {
        let request = match self {
            Self::Play { request } | Self::Submitted { request, .. } | Self::Cancel { request } => {
                *request
            }
        };
        if request.is_nil() || request != expected {
            return Err(ErrorCode::Stale);
        }
        if let Self::Submitted { samples, .. } = self
            && (*samples == 0 || *samples > MAX_SAMPLES || *samples % FRAME_SAMPLES != 0)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
