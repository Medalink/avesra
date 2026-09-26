//! Explicitly provisional personal speech affinity; never release qualification.
use super::{AdmissionKind, Context, QualifiedProfile, utterance};
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const AFFINITY: f32 = 0.65;
const MAX_SAMPLES: u64 = 180 * 16000;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub utterance: Uuid,
    pub voiced_samples: u32,
    pub embedding: Vec<f32>,
}
/// DPAPI-owned evidence. No PCM, transcript, synthetic enrollment or grants.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub version: u16,
    pub id: Uuid,
    pub actor: Uuid,
    pub owner_revision: Uuid,
    pub microphone: String,
    pub model_revision: String,
    pub source: Option<(Uuid, Uuid)>,
    pub seed: Option<Vec<f32>>,
    pub observations: Vec<Sample>,
}
fn normalized(input: &[f32]) -> Result<Vec<f32>, ErrorCode> {
    if input.len() != 192 || input.iter().any(|v| !v.is_finite()) {
        return Err(ErrorCode::Malformed);
    }
    let norm = input
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm < 1e-12 {
        return Err(ErrorCode::Malformed);
    }
    Ok(input
        .iter()
        .map(|v| (f64::from(*v) / norm) as f32)
        .collect())
}
impl Voice {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != 1
            || self.id.is_nil()
            || self.actor.is_nil()
            || self.owner_revision.is_nil()
            || !crate::state::valid_audio_device_id(&self.microphone)
            || self.model_revision.len() != 40
            || !self.model_revision.bytes().all(|v| v.is_ascii_hexdigit())
            || self.observations.len() > 32
            || self.source.is_some() != self.seed.is_some()
            || self
                .source
                .is_some_and(|(id, revision)| id.is_nil() || revision.is_nil())
            || (self.source.is_some() && !self.observations.is_empty())
            || self
                .observations
                .iter()
                .map(|v| u64::from(v.voiced_samples))
                .sum::<u64>()
                > MAX_SAMPLES
        {
            return Err(ErrorCode::Malformed);
        }
        if let Some(seed) = &self.seed {
            normalized(seed)?;
        }
        for (index, sample) in self.observations.iter().enumerate() {
            if sample.utterance.is_nil()
                || !(1280..=160000).contains(&sample.voiced_samples)
                || self.observations[..index]
                    .iter()
                    .any(|v| v.utterance == sample.utterance)
            {
                return Err(ErrorCode::Malformed);
            }
            normalized(&sample.embedding)?;
        }
        Ok(())
    }
    pub fn learning(&self) -> bool {
        self.source.is_none()
            && self.observations.len() < 32
            && self
                .observations
                .iter()
                .map(|v| u64::from(v.voiced_samples))
                .sum::<u64>()
                < MAX_SAMPLES
    }
    pub fn representation(&self) -> Option<Vec<f32>> {
        if let Some(seed) = &self.seed {
            return normalized(seed).ok();
        }
        if self.observations.is_empty() {
            return None;
        }
        let mut centroid = vec![0.0; 192];
        for sample in &self.observations {
            let value = normalized(&sample.embedding).ok()?;
            for (sum, value) in centroid.iter_mut().zip(value) {
                *sum += value;
            }
        }
        normalized(&centroid).ok()
    }
    pub fn observe(
        &mut self,
        utterance: Uuid,
        samples: u32,
        embedding: &[f32],
    ) -> Result<bool, ErrorCode> {
        self.validate()?;
        if !self.learning() {
            return Ok(false);
        }
        if utterance.is_nil()
            || !(1280..=160000).contains(&samples)
            || self.observations.iter().any(|v| v.utterance == utterance)
        {
            return Err(ErrorCode::Malformed);
        }
        let embedding = normalized(embedding)?;
        if self.representation().is_some_and(|anchor| {
            anchor
                .iter()
                .zip(&embedding)
                .map(|(a, b)| f64::from(*a) * f64::from(*b))
                .sum::<f64>()
                < f64::from(AFFINITY)
        }) {
            return Ok(false);
        }
        let used = self
            .observations
            .iter()
            .map(|v| u64::from(v.voiced_samples))
            .sum::<u64>();
        if used + u64::from(samples) > MAX_SAMPLES {
            return Ok(false);
        }
        self.observations.push(Sample {
            utterance,
            voiced_samples: samples,
            embedding,
        });
        Ok(true)
    }
    pub fn admit(
        self,
        context: &Context,
        adapters: [String; 5],
    ) -> Result<QualifiedProfile, ErrorCode> {
        self.validate()?;
        if !context.valid()
            || context.actor != Some(self.actor)
            || context.microphone != self.microphone
            || context.grant_revision.is_none()
            || adapters
                .iter()
                .any(|v| v.is_empty() || v.len() > 256 || v.chars().any(char::is_control))
        {
            return Err(ErrorCode::Denied);
        }
        let [asr, signal, directed, overlap, echo] = adapters;
        Ok(QualifiedProfile {
            kind: AdmissionKind::Personal,
            actor: self.actor,
            grant_revision: context.grant_revision.ok_or(ErrorCode::Denied)?,
            qualification_revision: Uuid::new_v4(),
            candidate: None,
            personal: Some(self),
            asr_revision: asr,
            threshold: AFFINITY,
            held_out_margin: 0.0,
            signal_adapter_revision: signal,
            directed_adapter_revision: directed,
            overlap_adapter_revision: overlap,
            echo_adapter_revision: echo,
            endpoint_policy: utterance::Policy::measured(Uuid::new_v4(), 0.5, 0.5, 10)?,
            minimum_voiced_samples: 1280,
            maximum_clipped_fraction: 0.01,
            valid_until: Instant::now() + Duration::from_secs(12 * 3600),
        })
    }
}
