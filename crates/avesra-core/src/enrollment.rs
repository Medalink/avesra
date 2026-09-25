//! Sensitive derived enrollment data. No raw audio or permission grants live here.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub const SEGMENTS: usize = 6;
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Prompted,
    Natural,
    HeldOut,
}
const ORDER: [SegmentKind; SEGMENTS] = [
    SegmentKind::Prompted,
    SegmentKind::Prompted,
    SegmentKind::Prompted,
    SegmentKind::Natural,
    SegmentKind::HeldOut,
    SegmentKind::HeldOut,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version: u16,
    pub id: Uuid,
    pub revision: Uuid,
    pub model_revision: String,
    pub microphone: String,
    pub representation: Vec<f32>,
    pub held_out_similarities: Vec<f32>,
    pub collected_samples: u64,
    pub segments: usize,
    // Deliberately no accepted/owner/grants field: a candidate cannot grant rights.
}
impl Candidate {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != 1
            || self.id.is_nil()
            || self.revision.is_nil()
            || !valid_revision(&self.model_revision)
            || self.microphone.is_empty()
            || self.microphone.len() > 512
            || self.segments != SEGMENTS
            || !(96_000..=960_000).contains(&self.collected_samples)
            || self.held_out_similarities.len() != 2
            || self
                .held_out_similarities
                .iter()
                .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
        {
            return Err(ErrorCode::Malformed);
        }
        let vector = normalized(&self.representation)?;
        if vector
            .iter()
            .zip(&self.representation)
            .any(|(a, b)| (*a - *b).abs() > 0.001)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
fn valid_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|v| v.is_ascii_hexdigit())
}
fn normalized(value: &[f32]) -> Result<Vec<f32>, ErrorCode> {
    if value.len() != 192 || value.iter().any(|v| !v.is_finite()) {
        return Err(ErrorCode::Malformed);
    }
    let norm = value
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm < 1e-12 {
        return Err(ErrorCode::Malformed);
    }
    Ok(value
        .iter()
        .map(|v| (f64::from(*v) / norm) as f32)
        .collect())
}
pub struct Enrollment {
    pub id: Uuid,
    epoch: u64,
    revision: String,
    microphone: String,
    created: Instant,
    next: usize,
    samples: u64,
    used: HashSet<Uuid>,
    active: Option<(Uuid, Instant)>,
    vectors: Vec<Vec<f32>>,
}
impl Enrollment {
    pub fn new(epoch: u64, revision: String, microphone: String) -> Result<Self, ErrorCode> {
        if epoch == 0
            || !valid_revision(&revision)
            || microphone.is_empty()
            || microphone.len() > 512
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self {
            id: Uuid::new_v4(),
            epoch,
            revision,
            microphone,
            created: Instant::now(),
            next: 0,
            samples: 0,
            used: HashSet::new(),
            active: None,
            vectors: Vec::with_capacity(SEGMENTS),
        })
    }
    pub fn next_kind(&self) -> Option<SegmentKind> {
        ORDER.get(self.next).copied()
    }
    pub fn completed(&self) -> usize {
        self.next
    }
    pub fn current(&self, epoch: u64) -> bool {
        self.epoch == epoch && self.created.elapsed() < Duration::from_secs(300)
    }
    pub fn begin_segment(&mut self, epoch: u64) -> Result<Uuid, ErrorCode> {
        if !self.current(epoch)
            || self.next >= SEGMENTS
            || self.active.is_some()
            || self.used.len() >= 32
        {
            return Err(ErrorCode::InvalidTransition);
        }
        let id = Uuid::new_v4();
        self.used.insert(id);
        self.active = Some((id, Instant::now()));
        Ok(id)
    }
    pub fn discard_segment(&mut self, id: Uuid) {
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == id)
        {
            self.active = None;
        }
    }
    /// Only a configured inference adapter supplies this result, never the UI.
    pub fn add_embedding(
        &mut self,
        epoch: u64,
        id: Uuid,
        revision: &str,
        samples: u32,
        embedding: &[f32],
    ) -> Result<(), ErrorCode> {
        if !self.current(epoch)
            || revision != self.revision
            || !self.active.as_ref().is_some_and(|(active, started)| {
                *active == id && started.elapsed() < Duration::from_secs(30)
            })
        {
            return Err(ErrorCode::Stale);
        }
        if !(16_000..=160_000).contains(&samples) {
            return Err(ErrorCode::Malformed);
        }
        let vector = normalized(embedding)?;
        self.vectors.push(vector);
        self.samples += u64::from(samples);
        self.next += 1;
        self.active = None;
        Ok(())
    }
    pub fn candidate(self, epoch: u64) -> Result<Candidate, ErrorCode> {
        if !self.current(epoch) || self.next != SEGMENTS || self.active.is_some() {
            return Err(ErrorCode::InvalidTransition);
        }
        let mut mean = vec![0.0; 192];
        for vector in &self.vectors[..4] {
            for (sum, value) in mean.iter_mut().zip(vector) {
                *sum += value / 4.0;
            }
        }
        let representation = normalized(&mean)?;
        let held_out_similarities = self.vectors[4..]
            .iter()
            .map(|vector| {
                vector
                    .iter()
                    .zip(&representation)
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
                    .clamp(-1.0, 1.0)
            })
            .collect();
        let value = Candidate {
            version: 1,
            id: self.id,
            revision: Uuid::new_v4(),
            model_revision: self.revision,
            microphone: self.microphone,
            representation,
            held_out_similarities,
            collected_samples: self.samples,
            segments: self.next,
        };
        value.validate()?;
        Ok(value)
    }
}
