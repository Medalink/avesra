//! Explicit sourced owner records. Never constructs acceptance or permissions.
use crate::{action_permissions::TaskTarget, store::Store};
use avesra_contracts::{ActionPayload, ErrorCode};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const TABLES: [(&str, &str); 4] = [
    (
        "private_memories",
        "CREATE TABLE private_memories(id TEXT PRIMARY KEY NOT NULL,actor TEXT NOT NULL,revision TEXT NOT NULL,source_task TEXT NOT NULL REFERENCES tasks(id),body TEXT CHECK(body IS NULL OR length(CAST(body AS BLOB))<=8192))",
    ),
    (
        "memory_revisions",
        "CREATE TABLE memory_revisions(revision TEXT PRIMARY KEY NOT NULL,memory TEXT NOT NULL REFERENCES private_memories(id),body TEXT CHECK(body IS NULL OR length(CAST(body AS BLOB))<=8192))",
    ),
    (
        "memory_events",
        "CREATE TABLE memory_events(id TEXT PRIMARY KEY NOT NULL,actor TEXT NOT NULL,memory TEXT NOT NULL REFERENCES private_memories(id),revision TEXT NOT NULL REFERENCES memory_revisions(revision),at_ms INTEGER NOT NULL,body TEXT CHECK(body IS NULL OR length(CAST(body AS BLOB))<=2048))",
    ),
    (
        "routine_invocations",
        "CREATE TABLE routine_invocations(task TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id),memory TEXT NOT NULL REFERENCES private_memories(id),revision TEXT NOT NULL REFERENCES memory_revisions(revision))",
    ),
];
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    for (name, schema) in TABLES {
        let mut query = db
            .prepare(
                "SELECT type,name,sql FROM sqlite_master WHERE tbl_name=?1 ORDER BY type LIMIT 3",
            )
            .map_err(|_| ErrorCode::Storage)?;
        let rows = query
            .query_map([name], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| ErrorCode::Storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ErrorCode::Storage)?;
        if version < 13 {
            if !rows.is_empty() {
                return Err(ErrorCode::Malformed);
            }
        } else if rows.len() != 2
            || rows[0] != ("index".into(), format!("sqlite_autoindex_{name}_1"), None)
            || rows[1] != ("table".into(), name.into(), Some(schema.into()))
        {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub task: Uuid,
    pub turn: Uuid,
    pub action: Uuid,
    pub target: TaskTarget,
    pub payload: ActionPayload,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Content {
    Fact { value: String },
    Routine { name: String, disabled: bool },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: Uuid,
    pub actor: Uuid,
    pub revision: Uuid,
    pub source: Source,
    pub content: Content,
    pub corrected: bool,
    pub created_ms: u64,
}
#[derive(Serialize)]
pub struct View {
    pub entry: Entry,
    pub validated: bool,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    SaveFact {
        task: Uuid,
        value: String,
    },
    SaveRoutine {
        task: Uuid,
        name: String,
    },
    Correct {
        id: Uuid,
        revision: Uuid,
        text: String,
        disabled: bool,
    },
    Delete {
        id: Uuid,
        revision: Uuid,
    },
}
fn text(value: &str) -> Result<(), ErrorCode> {
    if value.trim().is_empty()
        || value.len() > 1024
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(())
}
fn encode(value: &impl Serialize) -> Result<String, ErrorCode> {
    serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)
}
fn compact(tx: &rusqlite::Transaction<'_>) -> Result<(), ErrorCode> {
    // Referenced invocation revisions are immutable source provenance, even
    // after deletion has cleared their bodies. Remove only unreferenced history.
    tx.execute("DELETE FROM memory_events WHERE rowid NOT IN (SELECT rowid FROM memory_events ORDER BY rowid DESC LIMIT 512)",[]).map_err(|_|ErrorCode::Storage)?;
    tx.execute("DELETE FROM memory_revisions WHERE revision NOT IN (SELECT revision FROM private_memories WHERE body IS NOT NULL) AND revision NOT IN (SELECT revision FROM routine_invocations) AND revision NOT IN (SELECT revision FROM memory_events)",[]).map_err(|_|ErrorCode::Storage)?;
    tx.execute("DELETE FROM private_memories WHERE body IS NULL AND id NOT IN (SELECT memory FROM memory_revisions) AND id NOT IN (SELECT memory FROM routine_invocations) AND id NOT IN (SELECT memory FROM memory_events)",[]).map_err(|_|ErrorCode::Storage)?;
    Ok(())
}
impl Entry {
    fn validate(&self) -> Result<(), ErrorCode> {
        if [
            self.id,
            self.actor,
            self.revision,
            self.source.task,
            self.source.turn,
            self.source.action,
        ]
        .iter()
        .any(Uuid::is_nil)
        {
            return Err(ErrorCode::Malformed);
        }
        self.source.target.validate()?;
        let same = match (&self.source.target, &self.source.payload) {
            (
                TaskTarget::Prompt { binding },
                ActionPayload::FillPrompt {
                    app_id, project_id, ..
                },
            ) => binding.app == *app_id && binding.id == *project_id,
            (TaskTarget::Application { app, .. }, ActionPayload::LaunchApp { app_id }) => {
                app == app_id
            }
            (TaskTarget::Volume { .. }, ActionPayload::SetVolume { percent }) => *percent <= 100,
            (TaskTarget::Diagnostic { catalog }, ActionPayload::Diagnostic { catalog_entry }) => {
                catalog.id() == *catalog_entry
            }
            _ => false,
        };
        if !same {
            return Err(ErrorCode::Malformed);
        }
        match &self.content {
            Content::Fact { value } => text(value)?,
            Content::Routine { name, .. } => {
                if crate::apps::alias_phrase(name)? != *name {
                    return Err(ErrorCode::Malformed);
                }
            }
        }
        Ok(())
    }
}
fn read(db: &Connection, actor: Uuid, id: Uuid) -> Result<Entry, ErrorCode> {
    let (revision,source,body):(String,String,Vec<u8>)=db.query_row("SELECT revision,source_task,substr(CAST(body AS BLOB),1,8193) FROM private_memories WHERE id=?1 AND actor=?2 AND body IS NOT NULL",params![id.to_string(),actor.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|ErrorCode::Stale)?;
    if body.len() > 8192 {
        return Err(ErrorCode::Malformed);
    }
    let entry: Entry = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    entry.validate()?;
    if entry.id != id
        || entry.actor != actor
        || entry.revision.to_string() != revision
        || entry.source.task.to_string() != source
    {
        return Err(ErrorCode::Malformed);
    }
    if crate::conversations::verified_task_source(db, actor, entry.source.task)? != entry.source {
        return Err(ErrorCode::Stale);
    }
    Ok(entry)
}
fn ids(db: &Connection, actor: Uuid) -> Result<Vec<Uuid>, ErrorCode> {
    let mut query=db.prepare("SELECT id FROM private_memories WHERE actor=?1 AND body IS NOT NULL ORDER BY rowid DESC LIMIT 257").map_err(|_|ErrorCode::Storage)?;
    let rows = query
        .query_map([actor.to_string()], |r| r.get::<_, String>(0))
        .map_err(|_| ErrorCode::Storage)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(
            Uuid::parse_str(&row.map_err(|_| ErrorCode::Storage)?)
                .map_err(|_| ErrorCode::Malformed)?,
        );
    }
    if result.len() > 256 {
        return Err(ErrorCode::TooLarge);
    }
    Ok(result)
}
pub(crate) fn routine(
    db: &Connection,
    actor: Uuid,
    name: &str,
) -> Result<Option<Entry>, ErrorCode> {
    let name = crate::apps::alias_phrase(name)?;
    let mut found = None;
    for id in ids(db, actor)? {
        let entry = read(db, actor, id)?;
        if matches!(&entry.content,Content::Routine{name: saved,disabled:false} if *saved==name) {
            if found.is_some() {
                return Err(ErrorCode::Malformed);
            }
            found = Some(entry);
        }
    }
    Ok(found)
}
pub(crate) fn current_invocation(
    db: &Connection,
    actor: Uuid,
    task: Uuid,
) -> Result<(), ErrorCode> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT memory,revision FROM routine_invocations WHERE task=?1",
            [task.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    if let Some((id, revision)) = row {
        let entry = read(
            db,
            actor,
            Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?,
        )?;
        if entry.revision.to_string() != revision
            || !matches!(
                entry.content,
                Content::Routine {
                    disabled: false,
                    ..
                }
            )
        {
            return Err(ErrorCode::Stale);
        }
    }
    Ok(())
}
impl Store {
    pub fn private_memories(&self, actor: Uuid) -> Result<Vec<View>, ErrorCode> {
        if actor.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        ids(&self.connection,actor)?.into_iter().map(|id| {
            let entry=read(&self.connection,actor,id)?;
            let successful:Option<String>=self.connection.query_row("SELECT i.task FROM routine_invocations i JOIN steps s ON s.task_id=i.task JOIN native_finalizations f ON f.action_revision=(SELECT revision FROM action_heads WHERE step_id=s.id) WHERE i.memory=?1 AND i.revision=?2 AND f.actor_id=?3 AND f.outcome='\"success\"' ORDER BY i.rowid DESC LIMIT 1",params![id.to_string(),entry.revision.to_string(),actor.to_string()],|r|r.get(0)).optional().map_err(|_|ErrorCode::Storage)?;
            let validated=match successful {
                Some(task)=>{let actual=crate::conversations::verified_task_source(&self.connection,actor,Uuid::parse_str(&task).map_err(|_|ErrorCode::Malformed)?)?; actual.target==entry.source.target && actual.payload==entry.source.payload},
                None=>false,
            };
            Ok(View{entry,validated})
        }).collect()
    }
    pub fn change_memory(
        &mut self,
        actor: Uuid,
        change: Change,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if actor.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        authorize()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        if let Change::Delete { id, revision } = &change {
            // Deletion remains available if the original source task can no
            // longer be resolved. It needs exact current ownership/revision,
            // not a successful reinterpretation of the content being removed.
            let actual: Option<String> = tx.query_row("SELECT revision FROM private_memories WHERE id=?1 AND actor=?2 AND body IS NOT NULL",params![id.to_string(),actor.to_string()],|r|r.get(0)).optional().map_err(|_|ErrorCode::Storage)?;
            if actual.as_deref() != Some(revision.to_string().as_str()) {
                return Err(ErrorCode::Stale);
            }
            tx.execute(
                "UPDATE private_memories SET body=NULL WHERE id=?1",
                [id.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
            tx.execute(
                "UPDATE memory_revisions SET body=NULL WHERE memory=?1",
                [id.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
            tx.execute(
                "UPDATE memory_events SET body=NULL WHERE memory=?1",
                [id.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
            crate::notifications::redact_memory(&tx, actor, *id)?;
            compact(&tx)?;
            authorize()?;
            return tx.commit().map_err(|_| ErrorCode::Storage);
        }
        let now = crate::execution::now_ms()?;
        let mut existing = false;
        let entry = match change {
            Change::SaveFact { task, value } => Entry {
                id: Uuid::new_v4(),
                actor,
                revision: Uuid::new_v4(),
                source: crate::conversations::verified_task_source(&tx, actor, task)?,
                content: Content::Fact { value },
                corrected: false,
                created_ms: now,
            },
            Change::SaveRoutine { task, name } => Entry {
                id: Uuid::new_v4(),
                actor,
                revision: Uuid::new_v4(),
                source: crate::conversations::verified_task_source(&tx, actor, task)?,
                content: Content::Routine {
                    name: crate::apps::alias_phrase(&name)?,
                    disabled: false,
                },
                corrected: false,
                created_ms: now,
            },
            Change::Correct {
                id,
                revision,
                text: value,
                disabled,
            } => {
                existing = true;
                let mut entry = read(&tx, actor, id)?;
                if entry.revision != revision {
                    return Err(ErrorCode::Stale);
                }
                let content = match &entry.content {
                    Content::Fact { .. } if !disabled => Content::Fact { value },
                    Content::Routine { .. } => Content::Routine {
                        name: crate::apps::alias_phrase(&value)?,
                        disabled,
                    },
                    _ => return Err(ErrorCode::Malformed),
                };
                if encode(&content)? == encode(&entry.content)? {
                    authorize()?;
                    return Ok(());
                }
                entry.content = content;
                entry.revision = Uuid::new_v4();
                entry.corrected = true;
                entry.created_ms = now;
                entry
            }
            Change::Delete { .. } => return Err(ErrorCode::Malformed),
        };
        entry.validate()?;
        let current = ids(&tx, actor)?;
        if !existing && current.len() >= 256 {
            return Err(ErrorCode::TooLarge);
        }
        for id in current {
            let other = read(&tx, actor, id)?;
            if other.id == entry.id {
                continue;
            }
            if !existing
                && encode(&other.content)? == encode(&entry.content)?
                && other.source.task == entry.source.task
            {
                authorize()?;
                return Ok(());
            }
            if matches!((&other.content,&entry.content),(Content::Routine{name:a,..},Content::Routine{name:b,..}) if a==b)
            {
                return Err(ErrorCode::Denied);
            }
        }
        let body = encode(&entry)?;
        if existing {
            tx.execute(
                "UPDATE memory_events SET body=NULL WHERE memory=?1",
                [entry.id.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
            crate::notifications::redact_memory(&tx, actor, entry.id)?;
        }
        tx.execute("INSERT INTO private_memories VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,body=excluded.body",params![entry.id.to_string(),actor.to_string(),entry.revision.to_string(),entry.source.task.to_string(),body]).map_err(|_|ErrorCode::Storage)?;
        tx.execute(
            "INSERT INTO memory_revisions VALUES(?1,?2,?3)",
            params![entry.revision.to_string(), entry.id.to_string(), body],
        )
        .map_err(|_| ErrorCode::Storage)?;
        let event = match &entry.content {
            Content::Fact { value } => format!("Saved explicit fact: {value}"),
            Content::Routine { name, .. } => format!("Saved routine candidate: {name}"),
        };
        tx.execute(
            "INSERT INTO memory_events VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                Uuid::new_v4().to_string(),
                actor.to_string(),
                entry.id.to_string(),
                entry.revision.to_string(),
                i64::try_from(now).map_err(|_| ErrorCode::Expired)?,
                event
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        crate::notifications::memory_committed(&tx, &entry)?;
        compact(&tx)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
}
