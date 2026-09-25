//! Typed generated voice metadata, shared by private controller and native Settings.
use crate::{ErrorCode, voice::VoiceIdentity};
use serde::{Deserialize, Serialize};
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
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.identity.validate()?;
        if self.version != 1
            || self.base_revision != "5d83992436eae1d760afd27aff78a71d676296fc"
            || self.design_revision != "5ecdb67327fd37bb2e042aab12ff7391903235d3"
            || self.kind != "generated_voice_candidate"
            || self.text.trim().is_empty()
            || self.text.len() > 2048
            || self.text.chars().count() > 512
            || self.description.trim().is_empty()
            || self.description.len() > 4096
            || self.description.chars().count() > 1024
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
    pub text: Option<String>,
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
pub fn valid_selection_revision(value: Option<&str>) -> bool {
    value.is_none_or(|v| {
        v.len() == 64
            && v.bytes()
                .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
    })
}
impl VoiceStatus {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if !valid_selection_revision(self.selection_revision.as_deref())
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
                        || candidate.description.as_ref().is_none_or(|v| {
                            v.trim().is_empty() || v.len() > 4096 || v.chars().count() > 1024
                        })
                        || candidate.text.as_ref().is_none_or(|v| {
                            v.trim().is_empty() || v.len() > 2048 || v.chars().count() > 512
                        })
                        || candidate.created_at_ms.is_none_or(|v| v == 0)
                    {
                        return Err(ErrorCode::Malformed);
                    }
                }
                "unavailable" => {
                    if candidate.identity.is_some()
                        || candidate.description.is_some()
                        || candidate.text.is_some()
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum VoiceCommand {
    Status,
    Generate {
        text: String,
        description: String,
    },
    Select {
        voice: VoiceIdentity,
        expected_selection: Option<String>,
    },
    Clear {
        expected_selection: Option<String>,
    },
    Discard {
        id: Uuid,
        revision: Uuid,
    },
}
impl VoiceCommand {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Generate { text, description }
                if text.trim().is_empty()
                    || text.chars().count() > 512
                    || description.trim().is_empty()
                    || description.chars().count() > 1024 =>
            {
                Err(ErrorCode::Malformed)
            }
            Self::Select {
                voice,
                expected_selection,
            } => {
                voice.validate()?;
                if !valid_selection_revision(expected_selection.as_deref()) {
                    return Err(ErrorCode::Malformed);
                }
                Ok(())
            }
            Self::Clear { expected_selection }
                if !valid_selection_revision(expected_selection.as_deref()) =>
            {
                Err(ErrorCode::Malformed)
            }
            Self::Discard { id, revision } if id.is_nil() || revision.is_nil() => {
                Err(ErrorCode::Malformed)
            }
            _ => Ok(()),
        }
    }
}
