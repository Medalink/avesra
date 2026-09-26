//! Bounded observed semantic identities, never permission or dispatch authority.
#[path = "mailbox.rs"]
pub mod mailbox;
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlIdentity {
    pub control_type: i32,
    pub name: String,
    pub automation_id: String,
    pub class: String,
    pub framework: String,
}
impl ControlIdentity {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if !(50000..=50040).contains(&self.control_type)
            || [
                &self.name,
                &self.automation_id,
                &self.class,
                &self.framework,
            ]
            .iter()
            .any(|v| v.len() > 256 || v.chars().any(char::is_control))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
pub struct ControlChoice {
    pub id: Uuid,
    pub identity: ControlIdentity,
    pub invoke: bool,
    pub writable_value: bool,
    pub readable_text: bool,
    pub depth: u8,
    pub claude_route: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct PromptDiscovery {
    pub app: Uuid,
    pub revision: Uuid,
    pub controls: Vec<ControlChoice>,
    pub complete: bool,
}

/// Persisted configuration is not authority. Constructed from actual native
/// choices only; the grant remains separate and every use reobserves the UI.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptBinding {
    pub id: Uuid,
    pub revision: Uuid,
    pub actor: Uuid,
    pub app: Uuid,
    pub app_revision: Uuid,
    pub alias: Uuid,
    pub alias_revision: Uuid,
    pub app_name: String,
    pub project: String,
    pub project_route: String,
    pub project_button: ControlIdentity,
    pub project_indicator: ControlIdentity,
    pub new_chat: ControlIdentity,
    pub prompt: ControlIdentity,
    pub route: ControlIdentity,
}
impl PromptBinding {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [
            self.id,
            self.revision,
            self.actor,
            self.app,
            self.app_revision,
            self.alias,
            self.alias_revision,
        ]
        .iter()
        .any(Uuid::is_nil)
            || crate::apps::alias_phrase(&self.project)? != self.project
            || crate::apps::alias_phrase(&self.app_name)? != self.app_name
            || !project_route(&self.project_route)
            || self.new_chat.name != "New Chat"
            || crate::apps::alias_phrase(&self.project_button.name)? != self.project
            || crate::apps::alias_phrase(&self.project_indicator.name)? != self.project
            || !matches!(self.prompt.control_type, 50004 | 50030)
            || self.project_indicator.control_type != 50020
            || self.route.control_type != 50030
        {
            return Err(ErrorCode::Malformed);
        }
        for control in [
            &self.project_button,
            &self.project_indicator,
            &self.new_chat,
            &self.prompt,
            &self.route,
        ] {
            control.validate()?;
        }
        if serde_json::to_vec(self)
            .map_err(|_| ErrorCode::Malformed)?
            .len()
            > 6144
        {
            return Err(ErrorCode::TooLarge);
        }
        Ok(())
    }
}
pub fn project_route(value: &str) -> bool {
    value
        .strip_prefix("https://claude.ai/project/")
        .is_some_and(|id| Uuid::parse_str(id).is_ok_and(|id| !id.is_nil()))
}
pub fn fresh_route(value: &str) -> bool {
    value.len() <= 1024
        && !value.chars().any(char::is_control)
        && value.split('?').next() == Some("https://claude.ai/new")
        && !value.contains('#')
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingChoices {
    pub project: String,
    pub project_button: Uuid,
    pub project_indicator: Uuid,
    pub new_chat: Uuid,
    pub prompt: Uuid,
    pub route: Uuid,
}

/// Closed whole-request grammar. Returns lookup text, never application/project
/// identities, a grant or an executable step. Exact requested draft is preserved.
pub struct DraftRequest<'a> {
    pub app: &'a str,
    pub project: &'a str,
    pub text: &'a str,
}
pub fn draft_request(text: &str) -> Option<DraftRequest<'_>> {
    if text.len() > 4096 || text.chars().any(char::is_control) {
        return None;
    }
    let text = text.trim_matches(' ');
    let lower = text.to_ascii_lowercase();
    let prefix = if lower.starts_with("avesra, open ") {
        13
    } else if lower.starts_with("avesra open ") {
        12
    } else if lower.starts_with("open ") {
        5
    } else {
        return None;
    };
    let project_at = lower[prefix..].find(" for the ")? + prefix;
    let tail = &lower[project_at + 9..];
    let precise = " project, create a new chat, and type exactly ";
    let alternate = " project and type ";
    let (type_at, start, end) = if let Some(at) = tail.find(precise) {
        let at = at + project_at + 9;
        (at, at.checked_add(precise.len())?, text.len())
    } else {
        let suffix = " into a new chat";
        let sentence_end = if lower.ends_with(" into a new chat.") {
            text.len() - 1
        } else {
            text.len()
        };
        if !lower[..sentence_end].ends_with(suffix) {
            return None;
        }
        let at = tail.find(alternate)? + project_at + 9;
        (
            at,
            at.checked_add(alternate.len())?,
            sentence_end.checked_sub(suffix.len())?,
        )
    };
    let app = &text[prefix..project_at];
    let project = &text[project_at + 9..type_at];
    if start >= end {
        return None;
    }
    let draft = &text[start..end];
    crate::apps::alias_phrase(app).ok()?;
    crate::apps::alias_phrase(project).ok()?;
    if draft.is_empty() {
        return None;
    }
    Some(DraftRequest {
        app,
        project,
        text: draft,
    })
}
