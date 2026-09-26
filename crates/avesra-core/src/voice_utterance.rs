//! Shared measured endpoint decisions and bounded native PCM ownership.
use super::Context;
use avesra_contracts::{ErrorCode, activity::FRAME_SAMPLES};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_SAMPLES: u64 = 160_000;
const PRE_ROLL: u64 = 3_200;
const MAX_RETAINED: usize = (MAX_SAMPLES + PRE_ROLL + 32_000) as usize;

/// Native measured operating point. Never accepted from a serialized request.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    revision: Uuid,
    speech: f64,
    overlap: f64,
    quiet: u32,
}
impl Policy {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        Self::measured(self.revision, self.speech, self.overlap, self.quiet).map(|_| ())
    }
    pub fn measured(
        revision: Uuid,
        speech: f64,
        overlap: f64,
        quiet: u32,
    ) -> Result<Self, ErrorCode> {
        if revision.is_nil()
            || !speech.is_finite()
            || !overlap.is_finite()
            || !(0.0..=1.0).contains(&speech)
            || !(0.0..=1.0).contains(&overlap)
            || !(1..=125).contains(&quiet)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self {
            revision,
            speech,
            overlap,
            quiet,
        })
    }
}
#[derive(Clone, Copy)]
pub enum Boundary {
    Start {
        frame: u64,
    },
    End {
        start: u64,
        end: u64,
        voiced: u32,
        overlap: u32,
    },
    Cancel {
        frame: u64,
    },
}
struct Active {
    start: u64,
    voiced: u32,
    overlap: u32,
}
pub struct Segmenter {
    policy: Policy,
    next: u64,
    quiet: u32,
    active: Option<Active>,
    suppressed: bool,
    cancelled: bool,
    ended: bool,
    speech: u64,
    overlap: u64,
}
impl Segmenter {
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            next: 0,
            quiet: 0,
            active: None,
            suppressed: true,
            cancelled: false,
            ended: false,
            speech: 0,
            overlap: 0,
        }
    }
    pub fn processed(&self) -> u64 {
        self.next
    }
    pub fn speech_frames(&self) -> u64 {
        self.speech
    }
    pub fn overlap_frames(&self) -> u64 {
        self.overlap
    }
    pub fn unfinished(&self) -> bool {
        self.active.is_some() || self.cancelled
    }
    pub fn ready_for_speech(&self) -> bool {
        !self.ended && !self.suppressed && self.active.is_none()
    }
    fn quiet_boundary(&self) -> bool {
        !self.suppressed
            && !self.cancelled
            && self.active.is_none()
            && self.quiet >= self.policy.quiet
    }
    pub fn final_observation(&self) -> bool {
        self.ended
    }
    pub fn push(
        &mut self,
        offset: u64,
        frames: &[[f32; 4]],
        end: bool,
    ) -> Result<Vec<Boundary>, ErrorCode> {
        if self.ended
            || offset != self.next
            || frames.len() > 125
            || self.next.checked_add(frames.len() as u64).is_none()
            || frames
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(ErrorCode::Malformed);
        }
        let mut events = Vec::new();
        for frame in frames {
            let mut rank = frame.map(f64::from);
            rank.sort_by(|a, b| b.total_cmp(a));
            let speech = rank[0] >= self.policy.speech;
            let overlap = rank[1] >= self.policy.overlap;
            self.speech += u64::from(speech);
            self.overlap += u64::from(overlap);
            self.quiet = if speech {
                0
            } else {
                self.quiet.saturating_add(1)
            };
            if self.suppressed {
                if self.quiet >= self.policy.quiet {
                    self.suppressed = false;
                    self.cancelled = false;
                }
            } else {
                if speech && self.active.is_none() {
                    self.active = Some(Active {
                        start: self.next,
                        voiced: 0,
                        overlap: 0,
                    });
                    events.push(Boundary::Start { frame: self.next });
                }
                if let Some(active) = &mut self.active {
                    active.voiced += u32::from(speech);
                    active.overlap += u32::from(overlap);
                    // The original hard bound wins over an end decision at EOF.
                    if (self.next + 1 - active.start) * u64::from(FRAME_SAMPLES) >= MAX_SAMPLES {
                        self.active = None;
                        self.suppressed = true;
                        self.cancelled = true;
                        events.push(Boundary::Cancel {
                            frame: self.next + 1,
                        });
                    } else if self.quiet >= self.policy.quiet {
                        let active = self.active.take().ok_or(ErrorCode::InvalidTransition)?;
                        events.push(Boundary::End {
                            start: active.start,
                            end: self.next + 1,
                            voiced: active.voiced,
                            overlap: active.overlap,
                        });
                    }
                }
            }
            self.next += 1;
        }
        self.ended = end;
        if end && self.active.take().is_some() {
            self.suppressed = true;
            self.cancelled = true;
            events.push(Boundary::Cancel { frame: self.next });
        }
        Ok(events)
    }
}

/// Single-consumption native segment. No Clone/Deserialize and no public fields.
pub struct Completed {
    id: Uuid,
    context: Context,
    policy: Uuid,
    started: Instant,
    completed: Instant,
    pcm: Vec<u8>,
    voiced: u32,
    overlap: u32,
    clipped: u32,
}
impl Completed {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn policy(&self) -> Uuid {
        self.policy
    }
    pub fn started(&self) -> Instant {
        self.started
    }
    pub fn completed(&self) -> Instant {
        self.completed
    }
    pub fn samples(&self) -> u32 {
        (self.pcm.len() / 2) as u32
    }
    pub fn voiced_samples(&self) -> u32 {
        self.voiced
    }
    pub fn overlapping_samples(&self) -> u32 {
        self.overlap
    }
    pub fn clipped_samples(&self) -> u32 {
        self.clipped
    }
    pub fn into_pcm(self) -> Vec<u8> {
        self.pcm
    }
}
/// Capture and model callbacks share this owner; neither can append out of order.
pub struct Owner {
    context: Context,
    segmenter: Segmenter,
    pcm: VecDeque<i16>,
    base: u64,
    samples: u64,
    sequence: u64,
    origin: Option<Instant>,
    active: Option<(Uuid, u64)>,
    failed: bool,
}
impl Owner {
    /// Remains inspectable after a real final tail; EOF alone never proves quiet.
    pub fn quiet_boundary(&self) -> bool {
        !self.failed && self.segmenter.quiet_boundary()
    }
    pub fn ready_for_speech(&self) -> bool {
        !self.failed && self.segmenter.ready_for_speech()
    }
    pub fn unfinished(&self) -> bool {
        self.segmenter.unfinished()
    }
    pub fn new(context: Context, policy: Policy) -> Result<Self, ErrorCode> {
        if !context.valid() {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self {
            context,
            segmenter: Segmenter::new(policy),
            pcm: VecDeque::new(),
            base: 0,
            samples: 0,
            sequence: 0,
            origin: None,
            active: None,
            failed: false,
        })
    }
    pub fn capture(
        &mut self,
        sequence: u64,
        captured: Instant,
        samples: &[i16],
    ) -> Result<(), ErrorCode> {
        let now = Instant::now();
        if self.failed
            || self.segmenter.ended
            || self.sequence.checked_add(1) != Some(sequence)
            || samples.len() != 320
            || captured > now
            || now.duration_since(captured) > Duration::from_millis(500)
            || self.pcm.len() + samples.len() > MAX_RETAINED
        {
            self.failed = true;
            self.pcm.clear();
            return Err(ErrorCode::Stale);
        }
        let origin = *self.origin.get_or_insert(captured);
        let expected = origin
            .checked_add(Duration::from_micros(
                self.samples
                    .checked_mul(1_000_000)
                    .ok_or(ErrorCode::TooLarge)?
                    / 16_000,
            ))
            .ok_or(ErrorCode::TooLarge)?;
        if if captured >= expected {
            captured.duration_since(expected)
        } else {
            expected.duration_since(captured)
        } > Duration::from_millis(1)
        {
            self.failed = true;
            self.pcm.clear();
            return Err(ErrorCode::Stale);
        }
        self.pcm.extend(samples);
        self.samples = self
            .samples
            .checked_add(samples.len() as u64)
            .ok_or(ErrorCode::TooLarge)?;
        self.sequence = sequence;
        Ok(())
    }
    pub fn observe(
        &mut self,
        offset: u64,
        frames: &[[f32; 4]],
        end: bool,
    ) -> Result<Vec<Completed>, ErrorCode> {
        let result = self.observe_inner(offset, frames, end);
        if result.is_err() {
            self.failed = true;
            self.pcm.clear();
            self.active = None;
        }
        result
    }
    fn observe_inner(
        &mut self,
        offset: u64,
        frames: &[[f32; 4]],
        end: bool,
    ) -> Result<Vec<Completed>, ErrorCode> {
        let available = offset
            .checked_add(frames.len() as u64)
            .ok_or(ErrorCode::Malformed)?;
        let scored_samples = available
            .checked_mul(u64::from(FRAME_SAMPLES))
            .ok_or(ErrorCode::TooLarge)?;
        if self.failed || scored_samples > self.samples || (end && scored_samples != self.samples) {
            return Err(ErrorCode::Stale);
        }
        let origin = self.origin.ok_or(ErrorCode::InvalidTransition)?;
        let mut completed = Vec::new();
        for event in self.segmenter.push(offset, frames, end)? {
            match event {
                Boundary::Start { frame } => {
                    self.active = Some((
                        Uuid::new_v4(),
                        (frame * u64::from(FRAME_SAMPLES))
                            .saturating_sub(PRE_ROLL)
                            .max(self.base),
                    ));
                }
                Boundary::Cancel { .. } => {
                    self.active = None;
                }
                Boundary::End {
                    end,
                    voiced,
                    overlap,
                    ..
                } => {
                    let (id, start) = self.active.take().ok_or(ErrorCode::InvalidTransition)?;
                    let last = end * u64::from(FRAME_SAMPLES);
                    if start < self.base || last > self.samples || last - start > MAX_SAMPLES {
                        return Err(ErrorCode::Stale);
                    }
                    let started = origin + Duration::from_micros(start * 1_000_000 / 16_000);
                    let ended = origin + Duration::from_micros(last * 1_000_000 / 16_000);
                    if started.elapsed() >= Duration::from_secs(10) {
                        return Err(ErrorCode::Expired);
                    }
                    let samples: Vec<_> = self
                        .pcm
                        .iter()
                        .skip((start - self.base) as usize)
                        .take((last - start) as usize)
                        .copied()
                        .collect();
                    let clipped =
                        samples.iter().filter(|v| v.unsigned_abs() >= 32760).count() as u32;
                    completed.push(Completed {
                        id,
                        context: self.context.clone(),
                        policy: self.segmenter.policy.revision,
                        started,
                        completed: ended,
                        pcm: samples.iter().flat_map(|v| v.to_le_bytes()).collect(),
                        voiced: voiced * FRAME_SAMPLES,
                        overlap: overlap * FRAME_SAMPLES,
                        clipped,
                    });
                }
            }
        }
        let keep = self.active.map_or_else(
            || (available * u64::from(FRAME_SAMPLES)).saturating_sub(PRE_ROLL),
            |(_, start)| start,
        );
        let discard = keep.saturating_sub(self.base).min(self.pcm.len() as u64);
        self.pcm.drain(..discard as usize);
        self.base += discard;
        if end {
            self.pcm.clear();
            self.active = None;
        }
        Ok(completed)
    }
}
