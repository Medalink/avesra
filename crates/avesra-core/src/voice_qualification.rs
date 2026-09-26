//! Native held-out evaluation; a report or a Boolean cannot construct authority.
use super::{Context, Decision, Observation, QualifiedProfile, TurnGate, utterance};
use crate::enrollment::Candidate;
use avesra_contracts::ErrorCode;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Bounded explicit collection; every observation still revalidates its context.
pub const COLLECTION_LIFETIME: Duration = Duration::from_secs(8 * 3600);

/// Only native measured calibration supplies this operating point.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatingPoint {
    pub actor: Uuid,
    pub grant_revision: Uuid,
    pub asr_revision: String,
    pub threshold: f32,
    pub held_out_margin: f32,
    pub signal_revision: String,
    pub directed_revision: String,
    pub overlap_revision: String,
    pub echo_revision: String,
    pub endpoint_policy: utterance::Policy,
    pub minimum_voiced_samples: u32,
    pub maximum_clipped_fraction: f32,
}
#[derive(Clone, Copy)]
pub enum Condition {
    OwnerDirected,
    OwnerDirectedWithoutName,
    OtherSpeaker,
    Recorded,
    AssistantPlayback,
    Overlap,
    OwnerConversation,
    SpeakerSwitch,
    AmbiguousApproval,
}
impl Condition {
    fn index(self) -> usize {
        self as usize
    }
    fn positive(self) -> bool {
        matches!(self, Self::OwnerDirected | Self::OwnerDirectedWithoutName)
    }
}
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub admitted: usize,
    pub measured: usize,
    pub accepted: usize,
    pub missing: usize,
}
#[derive(serde::Serialize)]
pub struct Summary {
    pub counts: [Count; 9],
    pub pending: bool,
    pub reviewable: bool,
}
pub struct Qualification {
    profile: QualifiedProfile,
    context: Context,
    gate: TurnGate,
    counts: [Count; 9],
    used: HashSet<Uuid>,
    pending: Option<(Uuid, Condition)>,
    frozen: Instant,
}
/// Evidence only. Native protected storage and fresh identity/model revalidation
/// are mandatory; this serialized value is never passed to live TurnGate.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    version: u16,
    candidate: Uuid,
    candidate_revision: Uuid,
    qualification: Uuid,
    point: OperatingPoint,
    counts: [Count; 9],
    requests: Vec<Uuid>,
    #[serde(default)]
    development: Option<DevelopmentMeasurement>,
}

/// Actual native-held short check. This is evidence, never an IPC admission.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentMeasurement {
    pub request: Uuid,
    pub policy_revision: Uuid,
    pub similarity: f32,
    pub speech_start: u32,
    pub speech_end: u32,
    pub frames: Vec<[f32; 4]>,
    pub clipped_samples: u32,
    pub recognized_speech: bool,
    pub no_output: bool,
}
impl DevelopmentMeasurement {
    fn point(
        &self,
        candidate: &Candidate,
        context: &Context,
        adapters: [String; 5],
    ) -> Result<OperatingPoint, ErrorCode> {
        let threshold = candidate
            .held_out_similarities
            .iter()
            .copied()
            .reduce(f32::min)
            .ok_or(ErrorCode::Malformed)?;
        if self.request.is_nil()
            || self.policy_revision.is_nil()
            || !self.recognized_speech
            || !self.no_output
            || !self.similarity.is_finite()
            || self.similarity < threshold
            || self.similarity > 1.0
            || self.frames.len() != 100
            || self.speech_start == 0
            || self.speech_start >= self.speech_end
            || self.speech_end >= 100
            || self.clipped_samples > 128000
            || self
                .frames
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(ErrorCode::Denied);
        }
        let mut speech_peak = 0.0_f64;
        let mut quiet_max = 0.0_f64;
        let mut second_max = 0.0_f64;
        for (index, frame) in self.frames.iter().enumerate() {
            let mut scores = frame.map(f64::from);
            scores.sort_by(|a, b| b.total_cmp(a));
            second_max = second_max.max(scores[1]);
            if (self.speech_start as usize..self.speech_end as usize).contains(&index) {
                speech_peak = speech_peak.max(scores[0]);
            } else {
                quiet_max = quiet_max.max(scores[0]);
            }
        }
        if speech_peak <= quiet_max || speech_peak <= second_max {
            return Err(ErrorCode::Denied);
        }
        let policy = utterance::Policy::measured(
            self.policy_revision,
            quiet_max + (speech_peak - quiet_max) / 2.0,
            second_max + (speech_peak - second_max) / 2.0,
            self.speech_start.min(100 - self.speech_end),
        )?;
        // Refuse an interval whose actual scores cannot produce exactly one
        // complete utterance. EOF itself never supplies a speech boundary.
        let mut segmenter = utterance::Segmenter::new(policy.clone());
        let events = segmenter.push(0, &self.frames, true)?;
        if events
            .iter()
            .filter(|v| matches!(v, utterance::Boundary::End { .. }))
            .count()
            != 1
            || events
                .iter()
                .any(|v| matches!(v, utterance::Boundary::Cancel { .. }))
        {
            return Err(ErrorCode::Denied);
        }
        let [
            asr_revision,
            signal_revision,
            directed_revision,
            overlap_revision,
            echo_revision,
        ] = adapters;
        Ok(OperatingPoint {
            actor: context.actor.ok_or(ErrorCode::Denied)?,
            grant_revision: context.grant_revision.ok_or(ErrorCode::Denied)?,
            asr_revision,
            threshold,
            held_out_margin: 0.0,
            signal_revision,
            directed_revision,
            overlap_revision,
            echo_revision,
            endpoint_policy: policy,
            // Structural activity quantum, not a claimed calibrated quality bound.
            minimum_voiced_samples: 1280,
            maximum_clipped_fraction: self.clipped_samples as f32 / 128000.0,
        })
    }
}
impl Report {
    pub fn development(
        candidate: Candidate,
        context: Context,
        adapters: [String; 5],
        measurement: DevelopmentMeasurement,
    ) -> Result<(Self, QualifiedProfile), ErrorCode> {
        let point = measurement.point(&candidate, &context, adapters)?;
        let mut profile = Qualification::new(candidate, context, point.clone())?.profile;
        profile.kind = super::AdmissionKind::Development;
        profile.valid_until = Instant::now() + Duration::from_secs(12 * 3600);
        let report = Self {
            version: 2,
            candidate: profile.candidate.as_ref().ok_or(ErrorCode::Malformed)?.id,
            candidate_revision: profile.candidate_revision(),
            qualification: profile.qualification_revision,
            point,
            counts: [Count::default(); 9],
            requests: vec![measurement.request],
            development: Some(measurement),
        };
        Ok((report, profile))
    }
    pub fn candidate(&self) -> (Uuid, Uuid) {
        (self.candidate, self.candidate_revision)
    }
    pub fn point(&self) -> &OperatingPoint {
        &self.point
    }
    /// Native protected-report restore only, after a fresh controlled-load
    /// inspection. Equal complete deployment/policy fingerprints preserve the
    /// measured decision function; the new live revision still binds its lease.
    pub fn rebind_directed(
        &mut self,
        stored_quality: &str,
        current_quality: &str,
        revision: &str,
    ) -> Result<(), ErrorCode> {
        let digest = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if stored_quality != current_quality || !digest(stored_quality) || !digest(revision) {
            return Err(ErrorCode::Stale);
        }
        self.point.directed_revision = revision.into();
        Ok(())
    }
    pub fn restore(
        self,
        candidate: Candidate,
        context: Context,
    ) -> Result<QualifiedProfile, ErrorCode> {
        if let Some(measurement) = self.development {
            if self.version != 2
                || self.qualification.is_nil()
                || self.candidate != candidate.id
                || self.candidate_revision != candidate.revision
                || self.requests != [measurement.request]
                || self.counts.iter().any(|v| {
                    v.admitted != 0 || v.measured != 0 || v.accepted != 0 || v.missing != 0
                })
            {
                return Err(ErrorCode::Malformed);
            }
            let point = measurement.point(
                &candidate,
                &context,
                [
                    self.point.asr_revision.clone(),
                    self.point.signal_revision.clone(),
                    self.point.directed_revision.clone(),
                    self.point.overlap_revision.clone(),
                    self.point.echo_revision.clone(),
                ],
            )?;
            if point != self.point {
                return Err(ErrorCode::Malformed);
            }
            let mut profile = Qualification::new(candidate, context, point)?.profile;
            profile.kind = super::AdmissionKind::Development;
            profile.qualification_revision = self.qualification;
            profile.valid_until = Instant::now() + Duration::from_secs(12 * 3600);
            return Ok(profile);
        }
        if self.version != 1
            || self.qualification.is_nil()
            || candidate.id != self.candidate
            || candidate.revision != self.candidate_revision
            || self.requests.len() > 1024
            || self.requests.iter().any(Uuid::is_nil)
            || self.counts.iter().any(|v| {
                v.admitted > 1024
                    || v.measured > v.admitted
                    || v.accepted > v.measured
                    || v.missing > v.admitted
                    || v.measured + v.missing != v.admitted
            })
            || self.counts.iter().map(|v| v.admitted).sum::<usize>() != self.requests.len()
        {
            return Err(ErrorCode::Malformed);
        }
        let used: HashSet<_> = self.requests.iter().copied().collect();
        if used.len() != self.requests.len() {
            return Err(ErrorCode::Malformed);
        }
        let mut value = Qualification::new(candidate, context.clone(), self.point)?;
        value.counts = self.counts;
        value.used = used;
        value.profile.qualification_revision = self.qualification;
        value.review(&context)
    }
}
impl Qualification {
    pub fn new(
        candidate: Candidate,
        context: Context,
        point: OperatingPoint,
    ) -> Result<Self, ErrorCode> {
        candidate.validate()?;
        point.endpoint_policy.validate()?;
        if !context.valid()
            || context.actor != Some(point.actor)
            || context.grant_revision != Some(point.grant_revision)
            || candidate.microphone != context.microphone
            || !point.threshold.is_finite()
            || !(-1.0..=1.0).contains(&point.threshold)
            || !point.held_out_margin.is_finite()
            || !(0.0..=1.0).contains(&point.held_out_margin)
            || !(320..=160_000).contains(&point.minimum_voiced_samples)
            || !point.maximum_clipped_fraction.is_finite()
            || !(0.0..=1.0).contains(&point.maximum_clipped_fraction)
            || [
                &point.asr_revision,
                &point.signal_revision,
                &point.directed_revision,
                &point.overlap_revision,
                &point.echo_revision,
            ]
            .into_iter()
            .any(|v| v.is_empty() || v.len() > 256 || v.chars().any(char::is_control))
            || candidate
                .held_out_similarities
                .iter()
                .any(|v| *v < point.threshold + point.held_out_margin)
        {
            return Err(ErrorCode::Malformed);
        }
        let frozen = Instant::now();
        Ok(Self {
            profile: QualifiedProfile {
                kind: super::AdmissionKind::ReleaseQualified,
                candidate: Some(candidate),
                personal: None,
                actor: point.actor,
                grant_revision: point.grant_revision,
                qualification_revision: Uuid::new_v4(),
                asr_revision: point.asr_revision,
                threshold: point.threshold,
                held_out_margin: point.held_out_margin,
                signal_adapter_revision: point.signal_revision,
                directed_adapter_revision: point.directed_revision,
                overlap_adapter_revision: point.overlap_revision,
                echo_adapter_revision: point.echo_revision,
                endpoint_policy: point.endpoint_policy,
                minimum_voiced_samples: point.minimum_voiced_samples,
                maximum_clipped_fraction: point.maximum_clipped_fraction,
                valid_until: frozen + COLLECTION_LIFETIME,
            },
            context,
            gate: TurnGate::default(),
            counts: [Count::default(); 9],
            used: HashSet::new(),
            pending: None,
            frozen,
        })
    }
    fn same_authority(&self, current: &Context) -> bool {
        let mut expected = self.context.clone();
        expected.capture_epoch = current.capture_epoch;
        expected == *current && self.profile.valid() && current.valid()
    }
    pub fn begin(
        &mut self,
        current: &Context,
        id: Uuid,
        condition: Condition,
    ) -> Result<(), ErrorCode> {
        if !self.same_authority(current)
            || id.is_nil()
            || self.used.len() >= 1024
            || self.pending.is_some()
            || self.used.contains(&id)
        {
            return Err(ErrorCode::Stale);
        }
        self.used.insert(id);
        self.counts[condition.index()].admitted += 1;
        self.pending = Some((id, condition));
        Ok(())
    }
    pub fn missing(&mut self, id: Uuid) {
        if let Some((pending, condition)) = self.pending
            && pending == id
        {
            self.counts[condition.index()].missing += 1;
            self.pending = None;
        }
    }
    pub fn observe(
        &mut self,
        current: &Context,
        request: Uuid,
        value: Observation,
    ) -> Result<(), ErrorCode> {
        let Some((id, condition)) = self.pending else {
            return Err(ErrorCode::Stale);
        };
        if request != id || !self.same_authority(current) || value.started < self.frozen {
            return Err(ErrorCode::Stale);
        }
        let id_for_observation = value.utterance;
        let audio_known = |value: &super::AudioCondition, revision: &str| {
            matches!(value,
            super::AudioCondition::Measured { adapter_revision, utterance, context, .. }
            if adapter_revision == revision && *utterance == id_for_observation && context == current)
        };
        let directed_known = match &value.directed {
            super::DirectedIntent::Directed {
                adapter_revision,
                utterance,
                context,
                ..
            }
            | super::DirectedIntent::Rejected {
                adapter_revision,
                utterance,
                context,
            } => {
                adapter_revision == &self.profile.directed_adapter_revision
                    && *utterance == value.utterance
                    && context == current
            }
            super::DirectedIntent::Unknown | super::DirectedIntent::Personal { .. } => false,
        };
        let signal_known = matches!(&value.signal, super::SignalEvidence::Measured {
            adapter_revision, voiced_samples, total_samples, clipped_samples,
        } if adapter_revision == &self.profile.signal_adapter_revision && *total_samples > 0
            && *total_samples <= 160000 && voiced_samples <= total_samples && clipped_samples <= total_samples);
        let embedding_known = value.embedding.as_ref().is_some_and(|v| {
            v.len() == 192
                && v.iter().all(|n| n.is_finite())
                && v.iter().map(|n| f64::from(*n).powi(2)).sum::<f64>() > 1e-24
        });
        let complete = directed_known
            && signal_known
            && embedding_known
            && audio_known(&value.overlap, &self.profile.overlap_adapter_revision)
            && audio_known(&value.echo, &self.profile.echo_adapter_revision);
        let decision = self.gate.analyze(current, Some(&self.profile), value, None);
        // Even a positive held-out decision never escapes as live authority.
        let count = &mut self.counts[condition.index()];
        use super::Abstention;
        if !complete
            || matches!(
                decision,
                Decision::Abstain(
                    Abstention::Stale
                        | Abstention::Malformed
                        | Abstention::Replay
                        | Abstention::Capacity
                        | Abstention::Unqualified
                        | Abstention::SignalUnknown
                        | Abstention::DirectednessUnknown
                        | Abstention::OverlapUnknown
                        | Abstention::EchoUnknown
                )
            )
        {
            count.missing += 1;
        } else {
            count.measured += 1;
            count.accepted += usize::from(matches!(decision, Decision::Accepted(_)));
        }
        self.pending = None;
        Ok(())
    }
    fn reviewable(&self) -> bool {
        let positives = [
            Condition::OwnerDirected,
            Condition::OwnerDirectedWithoutName,
        ];
        let admitted: usize = positives
            .iter()
            .map(|v| self.counts[v.index()].admitted)
            .sum();
        let measured: usize = positives
            .iter()
            .map(|v| self.counts[v.index()].measured)
            .sum();
        let accepted: usize = positives
            .iter()
            .map(|v| self.counts[v.index()].accepted)
            .sum();
        let negatives = [
            Condition::OtherSpeaker,
            Condition::Recorded,
            Condition::AssistantPlayback,
            Condition::Overlap,
        ];
        let measured_negative: usize = negatives
            .iter()
            .map(|v| self.counts[v.index()].measured)
            .sum();
        let conversation = self.counts[Condition::OwnerConversation.index()];
        let extra = [Condition::SpeakerSwitch, Condition::AmbiguousApproval];
        self.pending.is_none()
            && self.profile.valid()
            && measured >= 100
            && accepted * 100 >= admitted * 95
            && self.counts[Condition::OwnerDirectedWithoutName.index()].accepted > 0
            && measured_negative >= 200
            && negatives
                .iter()
                .all(|v| self.counts[v.index()].measured > 0)
            && conversation.measured >= 200
            && extra.iter().all(|v| self.counts[v.index()].measured > 0)
            && self.counts.iter().enumerate().all(|(index, count)| {
                let positive = positives.iter().any(|v| v.index() == index && v.positive());
                positive || count.accepted == 0
            })
    }
    pub fn summary(&self) -> Summary {
        Summary {
            counts: self.counts,
            pending: self.pending.is_some(),
            reviewable: self.reviewable(),
        }
    }
    /// Caller must freshly revalidate native owner, consent and all incarnations.
    /// Evidence itself can never renew this original fixed lifetime.
    pub fn report(&self, current: &Context) -> Result<Report, ErrorCode> {
        if !self.same_authority(current) || !self.reviewable() {
            return Err(ErrorCode::Denied);
        }
        let profile = &self.profile;
        Ok(Report {
            version: 1,
            candidate: profile.candidate.as_ref().ok_or(ErrorCode::Malformed)?.id,
            candidate_revision: profile.candidate_revision(),
            qualification: profile.qualification_revision,
            point: OperatingPoint {
                actor: profile.actor,
                grant_revision: profile.grant_revision,
                asr_revision: profile.asr_revision.clone(),
                threshold: profile.threshold,
                held_out_margin: profile.held_out_margin,
                signal_revision: profile.signal_adapter_revision.clone(),
                directed_revision: profile.directed_adapter_revision.clone(),
                overlap_revision: profile.overlap_adapter_revision.clone(),
                echo_revision: profile.echo_adapter_revision.clone(),
                endpoint_policy: profile.endpoint_policy.clone(),
                minimum_voiced_samples: profile.minimum_voiced_samples,
                maximum_clipped_fraction: profile.maximum_clipped_fraction,
            },
            counts: self.counts,
            requests: self.used.iter().copied().collect(),
            development: None,
        })
    }
    pub fn review(mut self, current: &Context) -> Result<QualifiedProfile, ErrorCode> {
        if !self.same_authority(current) || !self.reviewable() {
            return Err(ErrorCode::Denied);
        }
        self.profile.valid_until = Instant::now() + Duration::from_secs(12 * 3600);
        Ok(self.profile)
    }
}
