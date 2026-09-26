//! Scoped observed transitions. Candidate metadata is never permission or success.
use crate::{
    apps::{AppCatalog, AppRecord, alias_phrase},
    store::Store,
};
use avesra_contracts::{ActionPayload, ErrorCode};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub(crate) const TABLES: [(&str, &str); 3] = [
    (
        "observation_scopes",
        "CREATE TABLE observation_scopes(id TEXT PRIMARY KEY NOT NULL,actor TEXT NOT NULL,body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=2048))",
    ),
    (
        "demonstration_candidates",
        "CREATE TABLE demonstration_candidates(id TEXT PRIMARY KEY NOT NULL,actor TEXT NOT NULL,revision TEXT NOT NULL,body TEXT CHECK(body IS NULL OR length(CAST(body AS BLOB))<=8192))",
    ),
    (
        "demonstration_invocations",
        "CREATE TABLE demonstration_invocations(task TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id),candidate TEXT NOT NULL REFERENCES demonstration_candidates(id),revision TEXT NOT NULL)",
    ),
];
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    for (name, schema) in TABLES {
        let mut q = db
            .prepare(
                "SELECT type,name,sql FROM sqlite_master WHERE tbl_name=?1 ORDER BY type LIMIT 3",
            )
            .map_err(|_| ErrorCode::Storage)?;
        let rows = q
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
        if version < 25 {
            if !rows.is_empty() {
                return Err(ErrorCode::Malformed);
            }
        } else if rows
            != [
                ("index".into(), format!("sqlite_autoindex_{name}_1"), None),
                ("table".into(), name.into(), Some(schema.into())),
            ]
        {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub id: Uuid,
    pub revision: Uuid,
    pub actor: Uuid,
    pub app: Uuid,
    pub app_revision: Uuid,
    pub alias: Uuid,
    pub alias_revision: Uuid,
    pub name: String,
    pub excluded: bool,
}
impl Scope {
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
            || alias_phrase(&self.name)? != self.name
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    ObservedTransition,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub source: SourceKind,
    pub demonstration: Uuid,
    pub baseline_ms: u64,
    pub process_created: u64,
    pub window_class: String,
    pub id: Uuid,
    pub revision: Uuid,
    pub actor: Uuid,
    pub device: Uuid,
    pub scope: Scope,
    pub name: String,
    pub observed_ms: u64,
    pub elapsed_ms: u64,
    pub transitions: u16,
    pub disabled: bool,
}
impl Candidate {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.scope.validate()?;
        if [
            self.id,
            self.revision,
            self.actor,
            self.device,
            self.demonstration,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.actor != self.scope.actor
            || self.scope.excluded
            || alias_phrase(&self.name)? != self.name
            || self.baseline_ms == 0
            || self.observed_ms < self.baseline_ms
            || self.observed_ms - self.baseline_ms > 300_000
            || self.process_created == 0
            || self.window_class.is_empty()
            || self.window_class.len() > 256
            || self.window_class.chars().any(char::is_control)
            || self.elapsed_ms > 300_000
            || !(1..=64).contains(&self.transitions)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn target(&self) -> crate::action_permissions::TaskTarget {
        crate::action_permissions::TaskTarget::Application {
            app: self.scope.app,
            app_revision: self.scope.app_revision,
            alias: self.scope.alias,
            alias_revision: self.scope.alias_revision,
        }
    }
    pub fn payload(&self) -> ActionPayload {
        ActionPayload::LaunchApp {
            app_id: self.scope.app,
        }
    }
}
#[derive(Serialize)]
pub struct View {
    pub candidate: Candidate,
    pub validated: bool,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub scopes: Vec<Scope>,
    pub candidates: Vec<View>,
}
pub struct Setup {
    pub scope: Scope,
    pub record: AppRecord,
}
fn json(value: &impl Serialize) -> Result<String, ErrorCode> {
    serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)
}
fn scopes(db: &Connection, actor: Uuid) -> Result<Vec<Scope>, ErrorCode> {
    let mut q=db.prepare("SELECT id,substr(CAST(body AS BLOB),1,2049) FROM observation_scopes WHERE actor=?1 LIMIT 65").map_err(|_|ErrorCode::Storage)?;
    let rows = q
        .query_map([actor.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|_| ErrorCode::Storage)?;
    let mut values = Vec::new();
    for row in rows {
        let (id, body) = row.map_err(|_| ErrorCode::Storage)?;
        let value: Scope = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
        value.validate()?;
        if body.len() > 2048 || value.id.to_string() != id || value.actor != actor {
            return Err(ErrorCode::Malformed);
        }
        if values.iter().any(|old: &Scope| old.app == value.app) {
            return Err(ErrorCode::Malformed);
        }
        values.push(value);
    }
    if values.len() > 64 {
        return Err(ErrorCode::TooLarge);
    }
    Ok(values)
}
pub(crate) fn candidates(db: &Connection, actor: Uuid) -> Result<Vec<Candidate>, ErrorCode> {
    let mut q=db.prepare("SELECT id,revision,substr(CAST(body AS BLOB),1,8193) FROM demonstration_candidates WHERE actor=?1 AND body IS NOT NULL LIMIT 257").map_err(|_|ErrorCode::Storage)?;
    let rows = q
        .query_map([actor.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(|_| ErrorCode::Storage)?;
    let mut values = Vec::new();
    for row in rows {
        let (id, revision, body) = row.map_err(|_| ErrorCode::Storage)?;
        let value: Candidate = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
        value.validate()?;
        if body.len() > 8192
            || value.id.to_string() != id
            || value.revision.to_string() != revision
            || value.actor != actor
        {
            return Err(ErrorCode::Malformed);
        }
        values.push(value);
    }
    if values.len() > 256 {
        return Err(ErrorCode::TooLarge);
    }
    Ok(values)
}
pub(crate) fn routine(
    db: &Connection,
    actor: Uuid,
    name: &str,
) -> Result<Option<Candidate>, ErrorCode> {
    let name = alias_phrase(name)?;
    let mut matches = candidates(db, actor)?
        .into_iter()
        .filter(|c| c.name == name && !c.disabled);
    let result = matches.next();
    if matches.next().is_some() {
        return Err(ErrorCode::Malformed);
    }
    Ok(result)
}
pub(crate) fn current_invocation(
    db: &Connection,
    actor: Uuid,
    task: Uuid,
) -> Result<(), ErrorCode> {
    let saved: Option<(String, String)> = db
        .query_row(
            "SELECT candidate,revision FROM demonstration_invocations WHERE task=?1",
            [task.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    if let Some((id, revision)) = saved {
        let current = candidates(db, actor)?
            .into_iter()
            .find(|c| c.id.to_string() == id && c.revision.to_string() == revision && !c.disabled)
            .ok_or(ErrorCode::Stale)?;
        let scope = scopes(db, actor)?
            .into_iter()
            .find(|s| s.id == current.scope.id)
            .ok_or(ErrorCode::Stale)?;
        if scope.excluded || scope.revision != current.scope.revision {
            return Err(ErrorCode::Stale);
        }
    }
    Ok(())
}
impl Store {
    pub fn teaching_snapshot(&self, actor: Uuid) -> Result<Snapshot, ErrorCode> {
        if actor.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        let candidates=candidates(&self.connection,actor)?.into_iter().map(|candidate| {
            let task:Option<String>=self.connection.query_row("SELECT i.task FROM demonstration_invocations i JOIN steps s ON s.task_id=i.task JOIN native_finalizations f ON f.action_revision=(SELECT revision FROM action_heads WHERE step_id=s.id) WHERE i.candidate=?1 AND i.revision=?2 AND f.actor_id=?3 AND f.outcome='\"success\"' ORDER BY i.rowid DESC LIMIT 1",params![candidate.id.to_string(),candidate.revision.to_string(),actor.to_string()],|r|r.get(0)).optional().map_err(|_|ErrorCode::Storage)?;
            let validated=match task {Some(task)=>crate::conversations::verified_task_source(&self.connection,actor,Uuid::parse_str(&task).map_err(|_|ErrorCode::Malformed)?).is_ok_and(|source|source.target==candidate.target()&&source.payload==candidate.payload()),None=>false};
            Ok(View{candidate,validated})
        }).collect::<Result<Vec<_>,ErrorCode>>()?;
        Ok(Snapshot {
            scopes: scopes(&self.connection, actor)?,
            candidates,
        })
    }
    pub fn teaching_scope(
        &mut self,
        actor: Uuid,
        alias: Uuid,
        revision: Uuid,
        excluded: bool,
        apps: &AppCatalog,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        authorize()?;
        let selected = apps
            .aliases()?
            .into_iter()
            .find(|v| {
                v.id == alias && v.revision == revision && v.selected_by == actor && v.available
            })
            .ok_or(ErrorCode::Stale)?;
        let resolved = apps.resolve(actor, &selected.phrase)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let old = scopes(&tx, actor)?;
        let previous = old.iter().find(|v| v.app == resolved.record.id);
        if previous.is_none() && old.len() >= 64 {
            return Err(ErrorCode::TooLarge);
        }
        if previous.is_some_and(|s| {
            s.excluded == excluded
                && s.app_revision == resolved.record.revision
                && s.alias == alias
                && s.alias_revision == revision
        }) {
            authorize()?;
            return Ok(());
        }
        let value = Scope {
            id: previous.map_or_else(Uuid::new_v4, |s| s.id),
            revision: Uuid::new_v4(),
            actor,
            app: resolved.record.id,
            app_revision: resolved.record.revision,
            alias,
            alias_revision: revision,
            name: selected.phrase,
            excluded,
        };
        value.validate()?;
        tx.execute("INSERT INTO observation_scopes VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET body=excluded.body",params![value.id.to_string(),actor.to_string(),json(&value)?]).map_err(|_|ErrorCode::Storage)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
    pub fn exclude_teaching_scope(
        &mut self,
        actor: Uuid,
        id: Uuid,
        revision: Uuid,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        authorize()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let mut value = scopes(&tx, actor)?
            .into_iter()
            .find(|s| s.id == id && s.revision == revision)
            .ok_or(ErrorCode::Stale)?;
        if !value.excluded {
            value.excluded = true;
            value.revision = Uuid::new_v4();
            tx.execute(
                "UPDATE observation_scopes SET body=?1 WHERE id=?2 AND actor=?3",
                params![json(&value)?, id.to_string(), actor.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
    pub fn teaching_setup(
        &self,
        actor: Uuid,
        id: Uuid,
        revision: Uuid,
        apps: &AppCatalog,
    ) -> Result<Setup, ErrorCode> {
        let scope = scopes(&self.connection, actor)?
            .into_iter()
            .find(|s| s.id == id && s.revision == revision && !s.excluded)
            .ok_or(ErrorCode::Denied)?;
        let resolved = apps.resolve(actor, &scope.name)?;
        if resolved.record.window_class.is_none()
            || resolved.record.id != scope.app
            || resolved.record.revision != scope.app_revision
            || resolved.alias_id != scope.alias
            || resolved.alias_revision != scope.alias_revision
        {
            return Err(ErrorCode::Stale);
        }
        Ok(Setup {
            scope,
            record: resolved.record,
        })
    }
    pub fn save_demonstration(
        &mut self,
        candidate: Candidate,
        apps: &AppCatalog,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        candidate.validate()?;
        authorize()?;
        let setup = self.teaching_setup(
            candidate.actor,
            candidate.scope.id,
            candidate.scope.revision,
            apps,
        )?;
        if setup.record.window_class.as_ref() != Some(&candidate.window_class) {
            return Err(ErrorCode::Stale);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let existing = candidates(&tx, candidate.actor)?;
        if existing.iter().any(|v| v.name == candidate.name) {
            return Err(ErrorCode::Denied);
        }
        if existing.len() >= 256
            || crate::memory::routine(&tx, candidate.actor, &candidate.name)?.is_some()
        {
            return Err(ErrorCode::TooLarge);
        }
        tx.execute(
            "INSERT INTO demonstration_candidates VALUES(?1,?2,?3,?4)",
            params![
                candidate.id.to_string(),
                candidate.actor.to_string(),
                candidate.revision.to_string(),
                json(&candidate)?
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        crate::notifications::demonstration_committed(&tx, &candidate)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
    pub fn change_demonstration(
        &mut self,
        actor: Uuid,
        id: Uuid,
        revision: Uuid,
        name: Option<String>,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        authorize()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let mut value = candidates(&tx, actor)?
            .into_iter()
            .find(|v| v.id == id && v.revision == revision)
            .ok_or(ErrorCode::Stale)?;
        crate::notifications::redact_memory(&tx, actor, id)?;
        if let Some(name) = name {
            let name = alias_phrase(&name)?;
            if name == value.name {
                authorize()?;
                return Ok(());
            }
            if candidates(&tx, actor)?
                .iter()
                .any(|v| v.id != id && v.name == name)
                || crate::memory::routine(&tx, actor, &name)?.is_some()
            {
                return Err(ErrorCode::Denied);
            }
            value.name = name;
            value.revision = Uuid::new_v4();
            tx.execute(
                "UPDATE demonstration_candidates SET revision=?1,body=?2 WHERE id=?3",
                params![value.revision.to_string(), json(&value)?, id.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
        } else {
            tx.execute(
                "UPDATE demonstration_candidates SET body=NULL WHERE id=?1",
                [id.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
}
