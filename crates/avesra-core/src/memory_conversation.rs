//! Exact accepted named-fact language; parsing alone is never acceptance.
use avesra_contracts::ErrorCode;

pub enum Command {
    Remember { key: String, value: String },
    Update { key: String, value: String },
    Recall { key: String },
    Forget { key: String },
    Clarify,
}
fn body(text: &str) -> &str {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("avesra, ") {
        &text[8..]
    } else if lower.starts_with("avesra ") {
        &text[7..]
    } else {
        text
    }
}
pub fn key(value: &str) -> Result<String, ErrorCode> {
    let value = value.trim().to_lowercase();
    if value.is_empty()
        || value.len() > 80
        || value
            .chars()
            .any(|c| !c.is_alphanumeric() && !matches!(c, ' ' | '-' | '\''))
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(value
        .split(' ')
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" "))
}
fn sensitive(value: &str) -> bool {
    let value = value.to_lowercase();
    value.split(|c: char| !c.is_alphanumeric()).any(|word| {
        matches!(
            word,
            "password"
                | "passwords"
                | "passcode"
                | "passcodes"
                | "pin"
                | "mfa"
                | "otp"
                | "totp"
                | "credential"
                | "credentials"
                | "secret"
                | "secrets"
        )
    }) || [
        "api key",
        "private key",
        "access token",
        "refresh token",
        "recovery phrase",
        "seed phrase",
        "recovery code",
        "security key",
    ]
    .iter()
    .any(|phrase| value.contains(phrase))
}
fn assignment(text: &str) -> Option<(&str, &str, bool)> {
    let lower = text.to_ascii_lowercase();
    let (offset, separator, update) = if lower.starts_with("remember that ") {
        (14, " is ", false)
    } else if lower.starts_with("update ") {
        (7, " to ", true)
    } else {
        return None;
    };
    let split = lower[offset..].find(separator)? + offset;
    Some((
        &text[offset..split],
        &text[split + separator.len()..],
        update,
    ))
}
/// Guard only known secret labels before accepted-history persistence.
pub fn before_accept(text: &str) -> Result<(), ErrorCode> {
    if assignment(body(text)).is_some_and(|(label, _, _)| sensitive(label)) {
        Err(ErrorCode::Denied)
    } else {
        Ok(())
    }
}
pub fn parse(text: &str) -> Option<Command> {
    let text = body(text);
    let lower = text.to_ascii_lowercase();
    if let Some((label, value, update)) = assignment(text) {
        let Ok(key) = key(label) else {
            return Some(Command::Clarify);
        };
        if sensitive(&key)
            || value.trim().is_empty()
            || value.len() > 512
            || value.chars().any(char::is_control)
        {
            return Some(Command::Clarify);
        }
        return Some(if update {
            Command::Update {
                key,
                value: value.to_owned(),
            }
        } else {
            Command::Remember {
                key,
                value: value.to_owned(),
            }
        });
    }
    for (prefix, forget) in [("what do you remember about ", false), ("forget ", true)] {
        if lower.starts_with(prefix) {
            let label = text[prefix.len()..]
                .trim_end_matches(['.', '!', '?'])
                .trim();
            return Some(match key(label) {
                Ok(key) if !sensitive(&key) => {
                    if forget {
                        Command::Forget { key }
                    } else {
                        Command::Recall { key }
                    }
                }
                _ => Command::Clarify,
            });
        }
    }
    if lower.starts_with("remember ")
        || lower.starts_with("update ")
        || lower.starts_with("forget ")
        || lower.starts_with("what do you remember")
    {
        Some(Command::Clarify)
    } else {
        None
    }
}

use super::{AcceptedSource, Content, Entry};
use rusqlite::Transaction;
use uuid::Uuid;

/// Opaque exact proposal; UI projections cannot deserialize it back into approval.
pub struct Deletion {
    actor: Uuid,
    source: AcceptedSource,
    id: Uuid,
    revision: Uuid,
    key: String,
    proposal: Uuid,
}
#[derive(Clone, serde::Serialize)]
pub struct DeleteView {
    pub proposal: Uuid,
    pub memory: Uuid,
    pub revision: Uuid,
    pub key: String,
}
impl Deletion {
    pub fn view(&self) -> DeleteView {
        DeleteView {
            proposal: self.proposal,
            memory: self.id,
            revision: self.revision,
            key: self.key.clone(),
        }
    }
}
pub(crate) fn deletion(
    db: &rusqlite::Connection,
    actor: Uuid,
    source: &AcceptedSource,
) -> Result<Option<Deletion>, ErrorCode> {
    let text = crate::conversations::memory_source_text(db, actor, source)?;
    let Some(Command::Forget { key }) = parse(&text) else {
        return Ok(None);
    };
    let Some(entry) = super::named(db, actor, &key)? else {
        return Ok(None);
    };
    Ok(Some(Deletion {
        actor,
        source: source.clone(),
        id: entry.id,
        revision: entry.revision,
        key,
        proposal: Uuid::new_v4(),
    }))
}
pub(crate) struct Reply {
    pub text: String,
    pub memory: Option<(Uuid, Uuid)>,
    pub value_bearing: bool,
}
/// Called only by the original accepted planner-claim transaction.
pub(crate) fn finish(
    tx: &Transaction<'_>,
    actor: Uuid,
    source: &AcceptedSource,
    approved: Option<&Deletion>,
) -> Result<Reply, ErrorCode> {
    let text = crate::conversations::memory_source_text(tx, actor, source)?;
    let command = parse(&text).ok_or(ErrorCode::Unsupported)?;
    if approved.is_some() && !matches!(command, Command::Forget { .. }) {
        return Err(ErrorCode::Denied);
    }
    let (key,value,update)=match command {
        Command::Remember{key,value}=>(key,value,false),
        Command::Update{key,value}=>(key,value,true),
        Command::Recall{key}=>{
            return Ok(match super::named(tx,actor,&key)? {
                Some(entry)=>{let Content::NamedFact{value,..}=&entry.content else{return Err(ErrorCode::Malformed)};Reply{value_bearing:true,text:format!("You told me {key} is {value}"),memory:Some((entry.id,entry.revision))}},
                None=>Reply{value_bearing:false,text:format!("I do not have a saved fact named {key}."),memory:None},
            });
        },
        Command::Forget{key}=>{
            let Some(entry)=super::named(tx,actor,&key)? else{return Ok(Reply{value_bearing:false,text:format!("I do not have a saved fact named {key}; nothing was deleted."),memory:None})};
            let approved=approved.ok_or(ErrorCode::ApprovalRequired)?;
            if approved.actor!=actor || approved.source!=*source || approved.id!=entry.id || approved.revision!=entry.revision || approved.key!=key {return Err(ErrorCode::Stale);}
            super::delete_exact(tx,actor,entry.id,entry.revision)?;
            return Ok(Reply{value_bearing:false,text:format!("I deleted the saved fact {key}. Your conversation history is unchanged."),memory:Some((entry.id,entry.revision))});
        },
        Command::Clarify=>return Ok(Reply{value_bearing:false,text:"Use remember that, followed by a short label, is, and its value. To read a fact, ask what do you remember about that label.".into(),memory:None}),
    };
    let existing = super::named(tx, actor, &key)?;
    let mut entry = match existing {
        Some(mut entry) => {
            if matches!(&entry.content,Content::NamedFact{value:saved,..} if saved==&value) {
                return Ok(Reply {
                    value_bearing: false,
                    text: format!("That value for {key} is already saved; nothing changed."),
                    memory: Some((entry.id, entry.revision)),
                });
            }
            if !update {
                return Ok(Reply {
                    value_bearing: false,
                    text: format!(
                        "A different value for {key} is already saved. Say update {key} to, followed by the new value, to replace it."
                    ),
                    memory: Some((entry.id, entry.revision)),
                });
            }
            entry.revision = Uuid::new_v4();
            entry.corrected = true;
            entry.changed_by = Some(source.clone());
            entry
        }
        None if update => {
            return Ok(Reply {
                value_bearing: false,
                text: format!(
                    "I do not have a saved fact named {key}. Say remember that {key} is, followed by its value."
                ),
                memory: None,
            });
        }
        None => Entry {
            id: Uuid::new_v4(),
            actor,
            revision: Uuid::new_v4(),
            source: None,
            accepted_source: Some(source.clone()),
            changed_by: None,
            content: Content::NamedFact {
                key: key.clone(),
                value: String::new(),
            },
            corrected: false,
            created_ms: 0,
        },
    };
    entry.content = Content::NamedFact {
        key: key.clone(),
        value,
    };
    entry.created_ms = crate::execution::now_ms()?;
    let reference = (entry.id, entry.revision);
    super::write_entry(tx, entry, update)?;
    Ok(Reply {
        value_bearing: false,
        text: format!(
            "{} the explicit fact {key}.",
            if update { "Updated" } else { "Saved" }
        ),
        memory: Some(reference),
    })
}
