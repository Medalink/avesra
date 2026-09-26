//! Native conversation evidence boundary; no serialized value grants authority.
#[path = "voice_personal.rs"]
pub mod personal;
#[path = "voice_qualification.rs"]
pub mod qualification;
#[path = "voice_utterance.rs"]
pub mod utterance;
use crate::enrollment::Candidate;
use avesra_contracts::ErrorCode;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct Context {
    pub device: Uuid,
    pub session: Uuid,
    pub capture_epoch: u64,
    pub action_epoch: u64,
    pub microphone: String,
    pub grant_revision: Option<Uuid>,
    pub actor: Option<Uuid>,
}
impl Context {
    fn valid(&self) -> bool {
        !self.device.is_nil()
            && !self.session.is_nil()
            && self.capture_epoch > 0
            && self.action_epoch > 0
            && crate::state::valid_audio_device_id(&self.microphone)
            && self.grant_revision.is_none_or(|id| !id.is_nil())
            && self.actor.is_none_or(|id| !id.is_nil())
    }
}
pub enum AudioCondition {
    Unknown,
    PersonalReference {
        utterance: Uuid,
        context: Context,
        known: bool,
        output_overlap: bool,
        near_end: bool,
    },
    Measured {
        adapter_revision: String,
        utterance: Uuid,
        context: Context,
        detected: bool,
    },
}
pub enum SignalEvidence {
    Unknown,
    Measured {
        adapter_revision: String,
        voiced_samples: u32,
        total_samples: u32,
        clipped_samples: u32,
    },
}
pub enum DirectedIntent {
    Personal {
        utterance: Uuid,
        context: Context,
    },
    Unknown,
    Rejected {
        adapter_revision: String,
        utterance: Uuid,
        context: Context,
    },
    Directed {
        adapter_revision: String,
        utterance: Uuid,
        context: Context,
        kind: DirectedKind,
    },
}
/// A qualified adapter distinguishes complete requests from contextual replies.
/// This classification is evidence, never speaker or action authority.
pub enum DirectedKind {
    Request,
    FollowUp,
}
pub struct Observation {
    pub utterance: Uuid,
    pub context: Context,
    pub asr_revision: String,
    pub speaker_revision: String,
    pub started: Instant,
    pub completed: Instant,
    pub transcript: String,
    pub embedding: Option<Vec<f32>>,
    pub overlap: AudioCondition,
    pub echo: AudioCondition,
    pub signal: SignalEvidence,
    pub directed: DirectedIntent,
}
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionKind {
    Personal,
    Development,
    ReleaseQualified,
}
/// Created only by native measured admission review. Never deserialized.
pub struct QualifiedProfile {
    kind: AdmissionKind,
    candidate: Option<Candidate>,
    personal: Option<personal::Voice>,
    actor: Uuid,
    qualification_revision: Uuid,
    grant_revision: Uuid,
    asr_revision: String,
    threshold: f32,
    held_out_margin: f32,
    signal_adapter_revision: String,
    directed_adapter_revision: String,
    overlap_adapter_revision: String,
    echo_adapter_revision: String,
    endpoint_policy: utterance::Policy,
    minimum_voiced_samples: u32,
    maximum_clipped_fraction: f32,
    valid_until: Instant,
}
impl QualifiedProfile {
    pub fn kind(&self) -> AdmissionKind {
        self.kind
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn grant_revision(&self) -> Uuid {
        self.grant_revision
    }
    pub fn candidate_revision(&self) -> Uuid {
        self.candidate.as_ref().map_or_else(
            || self.personal.as_ref().map_or(Uuid::nil(), |v| v.id),
            |v| v.revision,
        )
    }
    pub fn qualification_revision(&self) -> Uuid {
        self.qualification_revision
    }
    pub fn endpoint_policy(&self) -> utterance::Policy {
        self.endpoint_policy.clone()
    }
    pub fn valid(&self) -> bool {
        Instant::now() < self.valid_until
    }
    fn microphone(&self) -> &str {
        self.candidate.as_ref().map_or_else(
            || self.personal.as_ref().map_or("", |v| v.microphone.as_str()),
            |v| v.microphone.as_str(),
        )
    }
    fn speaker_revision(&self) -> &str {
        self.candidate.as_ref().map_or_else(
            || {
                self.personal
                    .as_ref()
                    .map_or("", |v| v.model_revision.as_str())
            },
            |v| v.model_revision.as_str(),
        )
    }
    fn speaker_valid(&self) -> bool {
        match (&self.candidate, &self.personal) {
            (Some(v), None) => {
                self.kind != AdmissionKind::Personal
                    && v.validate().is_ok()
                    && v.held_out_similarities
                        .iter()
                        .all(|s| *s >= self.threshold + self.held_out_margin)
            }
            (None, Some(v)) => self.kind == AdmissionKind::Personal && v.validate().is_ok(),
            _ => false,
        }
    }
    pub fn personal_voice(&self) -> Option<&personal::Voice> {
        self.personal.as_ref()
    }
    pub fn learn_personal(
        &mut self,
        utterance: Uuid,
        samples: u32,
        embedding: &[f32],
    ) -> Result<bool, ErrorCode> {
        match self.personal.as_mut() {
            Some(value) => value.observe(utterance, samples, embedding),
            None => Ok(false),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Abstention {
    Stale,
    Malformed,
    Replay,
    Capacity,
    Unqualified,
    UnknownSpeaker,
    OverlapUnknown,
    Overlap,
    EchoUnknown,
    Echo,
    NotAddressed,
    SignalUnknown,
    InsufficientSignal,
    DirectednessUnknown,
}
pub enum Decision {
    Abstain(Abstention),
    Accepted(Box<AcceptedConversation>),
}
pub struct AcceptedConversation {
    context: Context,
    utterance: Uuid,
    profile_revision: Uuid,
    qualification_revision: Uuid,
    accepted: Instant,
    text: String,
    follow_up_issued: bool,
}
/// Consuming this text does not create an AcceptedIntent or approve an action.
pub struct Conversation {
    utterance: Uuid,
    text: String,
    context: Context,
    profile_revision: Uuid,
    qualification_revision: Uuid,
    accepted: Instant,
}
impl Conversation {
    /// Original gate time, not a renewed budget at consume/queue/commit.
    pub fn precommit_current(&self) -> bool {
        self.accepted.elapsed() < Duration::from_secs(5)
    }
    pub fn utterance(&self) -> Uuid {
        self.utterance
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn profile_revision(&self) -> Uuid {
        self.profile_revision
    }
    pub fn qualification_revision(&self) -> Uuid {
        self.qualification_revision
    }
}
pub struct FollowUp {
    context: Context,
    profile_revision: Uuid,
    qualification_revision: Uuid,
    created: Instant,
    choices: Vec<String>,
}
impl AcceptedConversation {
    fn current(&self, context: &Context, profile: &QualifiedProfile) -> bool {
        self.context == *context
            && self.profile_revision == profile.candidate_revision()
            && self.qualification_revision == profile.qualification_revision
            && context.grant_revision == Some(profile.grant_revision)
            && context.actor == Some(profile.actor)
            && Instant::now() < profile.valid_until
            && self.accepted.elapsed() < Duration::from_secs(5)
    }
    pub fn invite_follow_up(
        &mut self,
        context: &Context,
        profile: &QualifiedProfile,
        choices: Vec<String>,
    ) -> Result<FollowUp, ErrorCode> {
        if self.follow_up_issued
            || !self.current(context, profile)
            || choices.is_empty()
            || choices.len() > 8
            || choices.iter().any(|v| v.trim().is_empty() || v.len() > 64)
        {
            return Err(ErrorCode::Denied);
        }
        let choices: Vec<_> = choices
            .into_iter()
            .map(|value| value.trim().to_lowercase())
            .collect();
        if choices
            .iter()
            .enumerate()
            .any(|(index, value)| choices[..index].contains(value))
        {
            return Err(ErrorCode::Malformed);
        }
        self.follow_up_issued = true;
        Ok(FollowUp {
            context: context.clone(),
            profile_revision: self.profile_revision,
            qualification_revision: self.qualification_revision,
            created: Instant::now(),
            choices,
        })
    }
    pub fn consume(
        self,
        context: &Context,
        profile: &QualifiedProfile,
    ) -> Result<Conversation, ErrorCode> {
        if !self.current(context, profile) {
            return Err(ErrorCode::Stale);
        }
        Ok(Conversation {
            utterance: self.utterance,
            text: self.text,
            context: self.context,
            profile_revision: self.profile_revision,
            qualification_revision: self.qualification_revision,
            accepted: self.accepted,
        })
    }
}
#[derive(Default)]
pub struct TurnGate {
    recent: VecDeque<(Uuid, Instant)>,
}
impl TurnGate {
    pub fn analyze(
        &mut self,
        current: &Context,
        profile: Option<&QualifiedProfile>,
        observation: Observation,
        follow_up: Option<FollowUp>,
    ) -> Decision {
        let reject = Decision::Abstain;
        let now = Instant::now();
        if !current.valid()
            || !observation.context.valid()
            || observation.utterance.is_nil()
            || observation.transcript.len() > 8192
            || observation.transcript.trim().is_empty()
            || observation
                .embedding
                .as_ref()
                .is_some_and(|v| v.len() != 192 || v.iter().any(|n| !n.is_finite()))
        {
            return reject(Abstention::Malformed);
        }
        if observation.context != *current
            || observation.started > observation.completed
            || observation.completed > now
            || now.duration_since(observation.started) > Duration::from_secs(25)
            || now.duration_since(observation.completed) > Duration::from_secs(2)
        {
            return reject(Abstention::Stale);
        }
        while self
            .recent
            .front()
            .is_some_and(|(_, time)| time.elapsed() > Duration::from_secs(31))
        {
            self.recent.pop_front();
        }
        if self
            .recent
            .iter()
            .any(|(id, _)| *id == observation.utterance)
        {
            return reject(Abstention::Replay);
        }
        if self.recent.len() >= 128 {
            return reject(Abstention::Capacity);
        }
        self.recent.push_back((observation.utterance, now));
        let Some(profile) = profile else {
            return reject(Abstention::Unqualified);
        };
        if !profile.speaker_valid()
            || profile.actor.is_nil()
            || current.actor != Some(profile.actor)
            || profile.qualification_revision.is_nil()
            || profile.grant_revision.is_nil()
            || current.grant_revision != Some(profile.grant_revision)
            || profile.microphone() != current.microphone
            || profile.speaker_revision() != observation.speaker_revision
            || profile.asr_revision != observation.asr_revision
            || now >= profile.valid_until
            || !profile.threshold.is_finite()
            || !(-1.0..=1.0).contains(&profile.threshold)
            || !profile.held_out_margin.is_finite()
            || !(0.0..=1.0).contains(&profile.held_out_margin)
            || !(320..=160_000).contains(&profile.minimum_voiced_samples)
            || !profile.maximum_clipped_fraction.is_finite()
            || !(0.0..=1.0).contains(&profile.maximum_clipped_fraction)
            || profile.signal_adapter_revision.is_empty()
            || profile.directed_adapter_revision.is_empty()
            || profile.overlap_adapter_revision.is_empty()
            || profile.echo_adapter_revision.is_empty()
        {
            return reject(Abstention::Unqualified);
        }
        match observation.signal {
            SignalEvidence::Unknown => return reject(Abstention::SignalUnknown),
            SignalEvidence::Measured {
                adapter_revision,
                voiced_samples,
                total_samples,
                clipped_samples,
            } => {
                if adapter_revision != profile.signal_adapter_revision
                    || total_samples == 0
                    || total_samples > 160_000
                    || voiced_samples > total_samples
                    || clipped_samples > total_samples
                    || voiced_samples < profile.minimum_voiced_samples
                    || clipped_samples as f32 / total_samples as f32
                        > profile.maximum_clipped_fraction
                {
                    return reject(Abstention::InsufficientSignal);
                }
            }
        }
        let directed_kind = match observation.directed {
            DirectedIntent::Personal { utterance, context } => {
                if profile.kind != AdmissionKind::Personal
                    || utterance != observation.utterance
                    || context != *current
                {
                    return reject(Abstention::DirectednessUnknown);
                }
                DirectedKind::Request
            }
            DirectedIntent::Unknown => return reject(Abstention::DirectednessUnknown),
            DirectedIntent::Rejected {
                adapter_revision,
                utterance,
                context,
            } => {
                return reject(
                    if adapter_revision == profile.directed_adapter_revision
                        && utterance == observation.utterance
                        && context == *current
                    {
                        Abstention::NotAddressed
                    } else {
                        Abstention::DirectednessUnknown
                    },
                );
            }
            DirectedIntent::Directed {
                adapter_revision,
                utterance,
                context,
                kind,
            } => {
                if adapter_revision != profile.directed_adapter_revision
                    || utterance != observation.utterance
                    || context != *current
                {
                    return reject(Abstention::DirectednessUnknown);
                }
                kind
            }
        };
        match observation.overlap {
            AudioCondition::PersonalReference { .. } => return reject(Abstention::OverlapUnknown),
            AudioCondition::Unknown => return reject(Abstention::OverlapUnknown),
            AudioCondition::Measured {
                adapter_revision,
                utterance,
                context,
                detected,
            } => {
                if adapter_revision != profile.overlap_adapter_revision
                    || utterance != observation.utterance
                    || context != *current
                {
                    return reject(Abstention::OverlapUnknown);
                }
                if detected {
                    return reject(Abstention::Overlap);
                }
            }
        }
        match observation.echo {
            AudioCondition::PersonalReference {
                utterance,
                context,
                known,
                output_overlap,
                near_end,
            } => {
                if profile.kind != AdmissionKind::Personal
                    || utterance != observation.utterance
                    || context != *current
                    || !known
                {
                    return reject(Abstention::EchoUnknown);
                }
                if output_overlap
                    && (!near_end
                        || profile
                            .personal
                            .as_ref()
                            .and_then(|v| v.representation())
                            .is_none())
                {
                    return reject(Abstention::Echo);
                }
            }
            AudioCondition::Unknown => return reject(Abstention::EchoUnknown),
            AudioCondition::Measured {
                adapter_revision,
                utterance,
                context,
                detected,
            } => {
                if adapter_revision != profile.echo_adapter_revision
                    || utterance != observation.utterance
                    || context != *current
                {
                    return reject(Abstention::EchoUnknown);
                }
                if detected {
                    return reject(Abstention::Echo);
                }
            }
        }
        let Some(vector) = observation.embedding else {
            return reject(Abstention::UnknownSpeaker);
        };
        let norm = vector
            .iter()
            .map(|v| f64::from(*v).powi(2))
            .sum::<f64>()
            .sqrt();
        if !norm.is_finite() || norm < 1e-12 {
            return reject(Abstention::UnknownSpeaker);
        }
        let representation = profile
            .candidate
            .as_ref()
            .map(|v| v.representation.clone())
            .or_else(|| profile.personal.as_ref().and_then(|v| v.representation()));
        if representation.as_ref().is_some_and(|representation| {
            vector
                .iter()
                .zip(representation)
                .map(|(a, b)| f64::from(*a) * f64::from(*b) / norm)
                .sum::<f64>()
                < f64::from(profile.threshold)
        }) {
            return reject(Abstention::UnknownSpeaker);
        }
        let text = observation.transcript.trim();
        let follow = follow_up.is_some_and(|invitation| {
            invitation.context == *current
                && invitation.profile_revision == profile.candidate_revision()
                && invitation.qualification_revision == profile.qualification_revision
                && invitation.created.elapsed() < Duration::from_secs(5)
                && invitation
                    .choices
                    .iter()
                    .any(|choice| *choice == text.to_lowercase())
        });
        // Name prefixes never prove directedness. A complete request relies on
        // the qualified adapter above; contextual replies need its invitation.
        if matches!(directed_kind, DirectedKind::FollowUp) && !follow {
            return reject(Abstention::NotAddressed);
        }
        Decision::Accepted(Box::new(AcceptedConversation {
            context: current.clone(),
            utterance: observation.utterance,
            profile_revision: profile.candidate_revision(),
            qualification_revision: profile.qualification_revision,
            accepted: now,
            text: text.into(),
            follow_up_issued: false,
        }))
    }
}
