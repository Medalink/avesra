//! Fixed provider metadata vocabulary. Validation establishes neither browser
//! ownership, authenticated account identity nor permission to invoke a control.
use super::{Id, MAX_MESSAGE, MAX_SAFE_COUNTER, Origin, documents::Candidate};
use crate::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_CHOICES: usize = 64;
pub const MAX_ATTRIBUTES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Gmail,
    X,
}
impl Provider {
    pub fn origin(self) -> &'static str {
        match self {
            Self::Gmail => "https://mail.google.com",
            Self::X => "https://x.com",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Inspect,
    GmailInboxPage,
    GmailOpenThread,
    GmailExpandThread,
    GmailNextPage,
    XReady,
}
impl Operation {
    pub fn supports(self, provider: Provider) -> bool {
        matches!(
            (self, provider),
            (Self::Inspect, _)
                | (
                    Self::GmailInboxPage
                        | Self::GmailOpenThread
                        | Self::GmailExpandThread
                        | Self::GmailNextPage,
                    Provider::Gmail
                )
                | (Self::XReady, Provider::X)
        )
    }
    pub fn may_mutate(self) -> bool {
        matches!(
            self,
            Self::GmailOpenThread | Self::GmailExpandThread | Self::GmailNextPage
        )
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Button,
    Link,
    Article,
    List,
    ListItem,
    Heading,
    Group,
    Banner,
    Main,
    Navigation,
    Text,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: Id,
    /// Closest included semantic ancestor, never a caller-supplied CSS path.
    pub parent: Option<Id>,
    pub role: Role,
    pub label: String,
    pub attributes: Vec<Attribute>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub provider: Provider,
    pub scope: ProbeScope,
    pub document: Candidate,
    pub dom_revision: u64,
    pub complete: bool,
    pub choices: Vec<Choice>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeScope {
    ProviderHeader,
}
fn text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.chars().any(char::is_control)
}
fn attribute_name(value: &str) -> bool {
    value.len() <= 48
        && !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && (matches!(value, "id" | "role" | "datetime")
            || matches!(
                value,
                "aria-label"
                    | "aria-labelledby"
                    | "aria-controls"
                    | "aria-expanded"
                    | "aria-selected"
            )
            || matches!(
                value,
                "data-testid"
                    | "data-message-id"
                    | "data-legacy-message-id"
                    | "data-thread-id"
                    | "data-legacy-thread-id"
            ))
}
impl Probe {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.document
            .validate(&Origin::parse(self.provider.origin())?)?;
        if self.dom_revision == 0
            || self.dom_revision > MAX_SAFE_COUNTER
            || self.choices.len() > MAX_CHOICES
        {
            return Err(ErrorCode::Malformed);
        }
        let mut ids = HashSet::new();
        for choice in &self.choices {
            if !ids.insert(choice.id.uuid())
                || !text(&choice.label, 128)
                || choice.attributes.len() > MAX_ATTRIBUTES
                || choice.parent == Some(choice.id)
            {
                return Err(ErrorCode::Malformed);
            }
            let mut attributes = HashSet::new();
            for attr in &choice.attributes {
                if !attribute_name(&attr.name)
                    || !text(&attr.value, 96)
                    || !attributes.insert(&attr.name)
                {
                    return Err(ErrorCode::Malformed);
                }
            }
        }
        for choice in &self.choices {
            let mut ancestor = choice.parent;
            let mut depth = 0;
            while let Some(id) = ancestor {
                depth += 1;
                if depth > 16 || id == choice.id {
                    return Err(ErrorCode::Malformed);
                }
                ancestor = self
                    .choices
                    .iter()
                    .find(|v| v.id == id)
                    .ok_or(ErrorCode::Malformed)?
                    .parent;
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| ErrorCode::Malformed)?
            .len()
            > MAX_MESSAGE - 2048
        {
            return Err(ErrorCode::TooLarge);
        }
        Ok(())
    }
    /// Only complete observations can supply protected binding choices. This
    /// still does not attest any provider-specific field or operation meaning.
    pub fn choice(&self, id: Id) -> Result<&Choice, ErrorCode> {
        self.validate()?;
        if !self.complete {
            return Err(ErrorCode::Unsupported);
        }
        self.choices
            .iter()
            .find(|v| v.id == id)
            .ok_or(ErrorCode::Stale)
    }
}
