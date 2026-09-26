//! One exact native resolver; no model payload can become accepted authority.
use super::{DurableTurn, Record, Store};
use crate::{
    action_permissions::TaskTarget,
    apps::AppCatalog,
    ledger::DispatchSession,
    policy::{Grant, PolicyContext},
};
use avesra_contracts::{Action, ActionPayload, ErrorCode, TaskState};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub(crate) const SCHEMA: &str = "CREATE TABLE conversation_tasks(turn TEXT PRIMARY KEY NOT NULL REFERENCES accepted_conversations(id),task TEXT UNIQUE NOT NULL REFERENCES tasks(id),actor TEXT NOT NULL,body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=16384))";

pub(super) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    let objects: i64=db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='conversation_tasks' OR tbl_name='conversation_tasks'",[],|r|r.get(0)).map_err(|_|ErrorCode::Malformed)?;
    if version < 6 {
        return if objects == 0 {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        };
    }
    if objects != 3 {
        return Err(ErrorCode::Malformed);
    }
    let sql:String=db.query_row("SELECT substr(sql,1,2049) FROM sqlite_master WHERE type='table' AND name='conversation_tasks'",[],|r|r.get(0)).map_err(|_|ErrorCode::Malformed)?;
    if sql != SCHEMA {
        return Err(ErrorCode::Malformed);
    }
    let mut statement = db
        .prepare("PRAGMA index_list('conversation_tasks')")
        .map_err(|_| ErrorCode::Malformed)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
    let mut seen = [false; 2];
    while let Some(row) = rows.next().map_err(|_| ErrorCode::Malformed)? {
        let name: String = row.get(1).map_err(|_| ErrorCode::Malformed)?;
        let unique: i64 = row.get(2).map_err(|_| ErrorCode::Malformed)?;
        let origin: String = row.get(3).map_err(|_| ErrorCode::Malformed)?;
        let partial: i64 = row.get(4).map_err(|_| ErrorCode::Malformed)?;
        let index = match (name.as_str(), origin.as_str()) {
            ("sqlite_autoindex_conversation_tasks_1", "pk") => 0,
            ("sqlite_autoindex_conversation_tasks_2", "u") => 1,
            _ => return Err(ErrorCode::Malformed),
        };
        if seen[index] || unique != 1 || partial != 0 {
            return Err(ErrorCode::Malformed);
        }
        seen[index] = true;
    }
    if seen != [true; 2] {
        return Err(ErrorCode::Malformed);
    }
    for (query, expected) in [
        (
            "PRAGMA index_info('sqlite_autoindex_conversation_tasks_1')",
            (0i64, 0i64, "turn"),
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_conversation_tasks_2')",
            (0, 1, "task"),
        ),
    ] {
        let mut statement = db.prepare(query).map_err(|_| ErrorCode::Malformed)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
        let row = rows
            .next()
            .map_err(|_| ErrorCode::Malformed)?
            .ok_or(ErrorCode::Malformed)?;
        let actual: (i64, i64, String) = (
            row.get(0).map_err(|_| ErrorCode::Malformed)?,
            row.get(1).map_err(|_| ErrorCode::Malformed)?,
            row.get(2).map_err(|_| ErrorCode::Malformed)?,
        );
        if actual != (expected.0, expected.1, expected.2.to_owned())
            || rows.next().map_err(|_| ErrorCode::Malformed)?.is_some()
        {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}

/// Opaque queue request carries an actual accepted handle and native session.
pub struct ExactTaskRequest {
    turn: DurableTurn,
    session: DispatchSession,
    started: Instant,
}
impl ExactTaskRequest {
    pub fn new(turn: DurableTurn, session: DispatchSession) -> Result<Self, ErrorCode> {
        if !session.active
            || session.actor_id != turn.actor
            || session.device_id != turn.source.device
            || session.session_id != turn.source.session
            || session.capture_epoch == 0
            || session.action_epoch == 0
        {
            return Err(ErrorCode::Stale);
        }
        Ok(Self {
            turn,
            session,
            started: Instant::now(),
        })
    }
    fn current(&self) -> Result<(), ErrorCode> {
        if self.started.elapsed() < Duration::from_secs(5) {
            Ok(())
        } else {
            Err(ErrorCode::Expired)
        }
    }
    pub fn target(&self) -> super::CancellationTarget {
        super::CancellationTarget {
            actor: self.turn.actor,
            source: self.turn.source,
            id: self.turn.id,
            revision: self.turn.revision,
        }
    }
    pub fn action_epoch(&self) -> u64 {
        self.session.action_epoch
    }
}
/// Read-only values for the real native current-action admission callback.
pub struct TaskAuthority<'a> {
    pub session: &'a DispatchSession,
    pub action: &'a Action,
    pub turn: Uuid,
    pub turn_revision: Uuid,
    pub target: &'a TaskTarget,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    turn: Uuid,
    turn_revision: Uuid,
    actor: Uuid,
    target: TaskTarget,
    action: Action,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyAppLink {
    turn: Uuid,
    turn_revision: Uuid,
    actor: Uuid,
    alias: Uuid,
    alias_revision: Uuid,
    app_revision: Uuid,
    action: Action,
}
/// Native-only result retains unresolved authority for an explicit planner or clarification.
pub enum TaskResolution {
    Linked(LinkedTask),
    NeedsInput(DurableTurn),
}
#[derive(Serialize)]
pub struct LinkedTask {
    pub turn: Uuid,
    pub task: Uuid,
    pub step: Uuid,
    pub action_revision: Uuid,
    pub target: TaskTarget,
    pub state: TaskState,
}

pub(super) fn wall_time() -> Result<u64, ErrorCode> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrorCode::Expired)?
            .as_millis(),
    )
    .map_err(|_| ErrorCode::Expired)
}
fn encode(value: &impl Serialize) -> Result<String, ErrorCode> {
    serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)
}
pub(super) fn read_record(db: &Connection, turn: Uuid) -> Result<(Record, String), ErrorCode> {
    let (body,state,revision,actor,device,session,utterance):(Vec<u8>,String,String,String,String,String,String)=db.query_row("SELECT substr(CAST(body AS BLOB),1,65537),substr(state,1,32),substr(revision,1,37),substr(actor,1,37),substr(device,1,37),substr(session,1,37),substr(utterance,1,37) FROM accepted_conversations WHERE id=?1",[turn.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(|_|ErrorCode::Stale)?;
    if body.len() > super::MAX_BODY {
        return Err(ErrorCode::Malformed);
    }
    let record: Record = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    record.validate()?;
    if record.id != turn
        || record.revision.to_string() != revision
        || record.actor.to_string() != actor
        || record.source.device.to_string() != device
        || record.source.session.to_string() != session
        || record.source.utterance.to_string() != utterance
    {
        return Err(ErrorCode::Malformed);
    }
    Ok((record, state))
}
fn read_link(db: &Connection, turn: Uuid) -> Result<Option<Link>, ErrorCode> {
    let row:Option<(String,String,Vec<u8>)>=db.query_row("SELECT substr(task,1,37),substr(actor,1,37),substr(CAST(body AS BLOB),1,16385) FROM conversation_tasks WHERE turn=?1",[turn.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_|ErrorCode::Storage)?;
    let Some((task, actor, body)) = row else {
        return Ok(None);
    };
    if body.len() > 16384 {
        return Err(ErrorCode::Malformed);
    }
    let link: Link = match serde_json::from_slice(&body) {
        Ok(link) => link,
        Err(_) => {
            let old: LegacyAppLink =
                serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
            if !matches!(old.action.payload, ActionPayload::LaunchApp { app_id } if app_id == old.action.target_id)
            {
                return Err(ErrorCode::Malformed);
            }
            Link {
                turn: old.turn,
                turn_revision: old.turn_revision,
                actor: old.actor,
                target: TaskTarget::Application {
                    app: old.action.target_id,
                    app_revision: old.app_revision,
                    alias: old.alias,
                    alias_revision: old.alias_revision,
                },
                action: old.action,
            }
        }
    };
    link.action.validate(link.action.issued_at_ms)?;
    link.target.validate()?;
    if link.turn != turn
        || [link.turn, link.turn_revision, link.actor]
            .iter()
            .any(Uuid::is_nil)
        || link.actor != link.action.actor_id
        || task != link.action.task_id.to_string()
        || actor != link.actor.to_string()
        || link.target.id() != link.action.target_id
        || link.target.operation() != link.action.payload.operation()
        || !match (&link.target, &link.action.payload) {
            (
                TaskTarget::BrowserRead { scope },
                ActionPayload::ReadPage {
                    origin,
                    message_limit,
                },
            ) => {
                scope.origin.as_str() == origin
                    && *message_limit == 16
                    && scope.actor.uuid() == link.actor
            }
            (
                TaskTarget::Prompt { binding },
                ActionPayload::FillPrompt {
                    app_id, project_id, ..
                },
            ) => binding.app == *app_id && binding.id == *project_id,
            (TaskTarget::Application { app, .. }, ActionPayload::LaunchApp { app_id }) => {
                app == app_id
            }
            (TaskTarget::Volume { .. }, ActionPayload::SetVolume { .. }) => true,
            (TaskTarget::Diagnostic { catalog }, ActionPayload::Diagnostic { catalog_entry }) => {
                catalog.id() == *catalog_entry
            }
            _ => false,
        }
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(link))
}
pub(super) fn linked(db: &Connection, record: &Record) -> Result<Option<LinkedTask>, ErrorCode> {
    let Some(link) = read_link(db, record.id)? else {
        return Ok(None);
    };
    if link.turn_revision != record.revision || link.actor != record.actor {
        return Err(ErrorCode::Malformed);
    }
    let (state, actor): (String, String) = db
        .query_row(
            "SELECT substr(state,1,64),substr(actor_id,1,37) FROM tasks WHERE id=?1",
            [link.action.task_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ErrorCode::Malformed)?;
    if actor != record.actor.to_string() {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(LinkedTask {
        turn: record.id,
        task: link.action.task_id,
        step: link.action.step_id,
        action_revision: link.action.revision,
        target: link.target,
        state: serde_json::from_str(&state).map_err(|_| ErrorCode::Malformed)?,
    }))
}
/// New linked tasks must retain original accepted provenance at every claim and
/// pre-effect recheck. Legacy native management remains a separate trusted API.
pub(crate) fn validate_dispatch(
    db: &Connection,
    action: &Action,
    session: &DispatchSession,
) -> Result<(), ErrorCode> {
    crate::memory::current_invocation(db, action.actor_id, action.task_id)?;
    let turn: Option<String> = db
        .query_row(
            "SELECT substr(turn,1,37) FROM conversation_tasks WHERE task=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    let Some(turn) = turn else {
        return Ok(());
    };
    let turn = Uuid::parse_str(&turn).map_err(|_| ErrorCode::Malformed)?;
    let (record, state) = read_record(db, turn)?;
    let link = read_link(db, turn)?.ok_or(ErrorCode::Malformed)?;
    if link.action != *action
        || link.turn_revision != record.revision
        || link.actor != record.actor
        || state != "planning"
        || !session.active
        || record.actor != session.actor_id
        || record.source.device != session.device_id
        || record.source.session != session.session_id
        || record.capture_epoch != session.capture_epoch
        || record.action_epoch != session.action_epoch
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
fn phrase(text: &str) -> Result<String, ErrorCode> {
    if text
        .chars()
        .any(|v| v.is_control() || (v.is_whitespace() && v != ' '))
    {
        return Err(ErrorCode::Unsupported);
    }
    let text = text.trim_matches(' ').to_lowercase();
    let text = text
        .strip_prefix("avesra, ")
        .or_else(|| text.strip_prefix("avesra "))
        .unwrap_or(&text);
    let remainder = text
        .strip_prefix("open ")
        .or_else(|| text.strip_prefix("launch "))
        .ok_or(ErrorCode::Unsupported)?;
    let phrase = crate::apps::alias_phrase(remainder).map_err(|_| ErrorCode::Unsupported)?;
    if phrase.split(' ').any(|v| {
        matches!(
            v,
            "not"
                | "no"
                | "never"
                | "don't"
                | "dont"
                | "and"
                | "or"
                | "then"
                | "but"
                | "unless"
                | "except"
                | "instead"
                | "please"
                | "avesra"
        )
    }) {
        return Err(ErrorCode::Unsupported);
    }
    Ok(phrase)
}
#[derive(Serialize)]
pub struct TaskView {
    #[serde(skip)]
    pub cancellation: super::CancellationTarget,
    pub task: Uuid,
    pub turn: Uuid,
    pub revision: Uuid,
    pub state: TaskState,
    pub target: TaskTarget,
    pub payload: ActionPayload,
    pub outcome: Option<avesra_contracts::Outcome>,
    pub diagnostic: Option<crate::diagnostics::Report>,
    pub created_ms: u64,
}
fn volume_percent(text: &str) -> Option<u8> {
    if text
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() && c != ' ')
    {
        return None;
    }
    let text = text.trim_matches(' ').to_ascii_lowercase();
    let text = text
        .strip_prefix("avesra, ")
        .or_else(|| text.strip_prefix("avesra "))
        .unwrap_or(&text);
    let number = text
        .strip_prefix("set speakers volume to ")?
        .strip_suffix(" percent")?;
    if number.is_empty() || number.len() > 3 || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    number.parse::<u8>().ok().filter(|v| *v <= 100)
}
fn diagnostic_request(text: &str) -> Option<crate::diagnostics::Catalog> {
    let text = text.trim_matches(' ').to_ascii_lowercase();
    let text = text
        .strip_prefix("avesra, ")
        .or_else(|| text.strip_prefix("avesra "))
        .unwrap_or(&text);
    match text {
        "check computer performance" => Some(crate::diagnostics::Catalog::HostResources),
        "check vpn status" => Some(crate::diagnostics::Catalog::CiscoVpnStatus),
        _ => None,
    }
}
/// The source is copied from the same writer's original native finalization,
/// never a frontend assertion or reconciled success label.
pub(crate) fn verified_task_source(
    db: &Connection,
    actor: Uuid,
    task: Uuid,
) -> Result<crate::memory::Source, ErrorCode> {
    let turn: String = db
        .query_row(
            "SELECT turn FROM conversation_tasks WHERE task=?1 AND actor=?2",
            params![task.to_string(), actor.to_string()],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Denied)?;
    let turn = Uuid::parse_str(&turn).map_err(|_| ErrorCode::Malformed)?;
    let (record, _) = read_record(db, turn)?;
    let link = read_link(db, turn)?.ok_or(ErrorCode::Stale)?;
    if record.actor != actor
        || link.actor != actor
        || link.action.task_id != task
        || record.revision != link.turn_revision
    {
        return Err(ErrorCode::Malformed);
    }
    let body:Vec<u8>=db.query_row("SELECT substr(CAST(o.body AS BLOB),1,32769) FROM native_observations o JOIN native_finalizations f ON f.dispatch_id=o.dispatch_id AND f.action_revision=o.action_revision AND f.target_id=o.target_id AND f.at_ms=o.at_ms JOIN action_revisions a ON a.revision=f.action_revision AND a.dispatch_id=f.dispatch_id WHERE f.actor_id=?1 AND f.action_revision=?2 AND f.outcome='\"success\"'",params![actor.to_string(),link.action.revision.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Denied)?;
    if body.len() > 32768 {
        return Err(ErrorCode::Malformed);
    }
    let observation: crate::execution::EffectObservation =
        serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    observation.validate(&link.action, avesra_contracts::Outcome::Success)?;
    Ok(crate::memory::Source {
        task,
        turn,
        action: link.action.revision,
        target: link.target,
        payload: link.action.payload,
    })
}
impl Store {
    pub fn recent_action_tasks(&self, actor: Uuid) -> Result<Vec<TaskView>, ErrorCode> {
        if actor.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT turn FROM conversation_tasks WHERE actor=?1 ORDER BY rowid DESC LIMIT 50",
            )
            .map_err(|_| ErrorCode::Storage)?;
        let mut rows = statement
            .query([actor.to_string()])
            .map_err(|_| ErrorCode::Storage)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(|_| ErrorCode::Storage)? {
            let id: String = row.get(0).map_err(|_| ErrorCode::Malformed)?;
            let (record, _) = read_record(
                &self.connection,
                Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?,
            )?;
            if record.actor != actor {
                return Err(ErrorCode::Malformed);
            }
            let link = read_link(&self.connection, record.id)?.ok_or(ErrorCode::Malformed)?;
            let task = linked(&self.connection, &record)?.ok_or(ErrorCode::Malformed)?;
            let outcome: Option<String> = self.connection.query_row("SELECT substr(outcome,1,64) FROM native_finalizations WHERE action_revision=?1 AND actor_id=?2",params![link.action.revision.to_string(),actor.to_string()],|r|r.get(0)).optional().map_err(|_| ErrorCode::Storage)?;
            let diagnostic = if matches!(link.action.payload, ActionPayload::Diagnostic { .. }) {
                let body: Option<Vec<u8>> = self.connection.query_row("SELECT substr(CAST(o.body AS BLOB),1,32769) FROM native_observations o JOIN native_finalizations f ON f.dispatch_id=o.dispatch_id AND f.action_revision=o.action_revision AND f.target_id=o.target_id AND f.at_ms=o.at_ms WHERE f.actor_id=?1 AND f.action_revision=?2 AND f.outcome='\"success\"'", params![actor.to_string(),link.action.revision.to_string()], |r|r.get(0)).optional().map_err(|_|ErrorCode::Storage)?;
                match body {
                    Some(body) if body.len() <= 32768 => {
                        let observation: crate::execution::EffectObservation =
                            serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
                        observation.validate(&link.action, avesra_contracts::Outcome::Success)?;
                        match observation {
                            crate::execution::EffectObservation::Diagnostic { report } => {
                                Some(*report)
                            }
                            _ => return Err(ErrorCode::Malformed),
                        }
                    }
                    Some(_) => return Err(ErrorCode::Malformed),
                    None => None,
                }
            } else {
                None
            };
            result.push(TaskView {
                cancellation: super::CancellationTarget {
                    actor,
                    source: record.source,
                    id: record.id,
                    revision: record.revision,
                },
                task: task.task,
                turn: record.id,
                revision: record.revision,
                state: task.state,
                target: task.target,
                payload: link.action.payload,
                outcome: outcome
                    .map(|value| serde_json::from_str(&value).map_err(|_| ErrorCode::Malformed))
                    .transpose()?,
                diagnostic,
                created_ms: record.created_ms,
            });
        }
        Ok(result)
    }
    pub fn conversation_for_step(
        &self,
        step: Uuid,
    ) -> Result<Option<super::CancellationTarget>, ErrorCode> {
        if step.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let turn:Option<String>=self.connection.query_row("SELECT substr(c.turn,1,37) FROM conversation_tasks c JOIN steps s ON s.task_id=c.task WHERE s.id=?1",[step.to_string()],|r|r.get(0)).optional().map_err(|_|ErrorCode::Storage)?;
        let Some(turn) = turn else {
            return Ok(None);
        };
        let turn = Uuid::parse_str(&turn).map_err(|_| ErrorCode::Malformed)?;
        let (record, _) = read_record(&self.connection, turn)?;
        let link = read_link(&self.connection, turn)?.ok_or(ErrorCode::Malformed)?;
        if link.action.step_id != step
            || link.turn_revision != record.revision
            || link.actor != record.actor
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Some(super::CancellationTarget {
            actor: record.actor,
            source: record.source,
            id: record.id,
            revision: record.revision,
        }))
    }
    pub fn accept_action_task(
        &mut self,
        request: ExactTaskRequest,
        apps: &AppCatalog,
        authorize: &mut dyn FnMut(&TaskAuthority<'_>) -> Result<(), ErrorCode>,
    ) -> Result<TaskResolution, ErrorCode> {
        request.current()?;
        let (record, state) = read_record(&self.connection, request.turn.id)?;
        if state != "accepted"
            || record.revision != request.turn.revision
            || record.actor != request.turn.actor
            || record.source != request.turn.source
            || record.capture_epoch != request.session.capture_epoch
            || record.action_epoch != request.session.action_epoch
        {
            return Err(ErrorCode::Stale);
        }
        let permissions = self.action_permissions(Some(record.actor))?;
        let routine_text = record.text.trim_matches(' ').to_lowercase();
        let routine_text = routine_text
            .strip_prefix("avesra ")
            .unwrap_or(&routine_text);
        let routine_name = routine_text.strip_prefix("run routine ");
        let routine = if let Some(name) = routine_name {
            match crate::memory::routine(&self.connection, record.actor, name)? {
                Some(entry) => Some(entry),
                None => return Ok(TaskResolution::NeedsInput(request.turn)),
            }
        } else {
            None
        };
        let read_origin = record
            .text
            .trim_matches(' ')
            .strip_prefix("Avesra, ")
            .or_else(|| record.text.trim_matches(' ').strip_prefix("Avesra "))
            .unwrap_or(record.text.trim_matches(' '));
        let read_origin = if read_origin
            .to_ascii_lowercase()
            .starts_with("read page at ")
        {
            Some(avesra_contracts::browser::Origin::parse(
                &read_origin[13..],
            )?)
        } else {
            None
        };
        let (target, payload, name) = if let Some(origin) = read_origin {
            let matches:Vec<_>=permissions.iter().filter(|p|!p.revoked && matches!(&p.permission.target,TaskTarget::BrowserRead{scope} if scope.origin==origin && scope.actor.uuid()==record.actor)).collect();
            if matches.len() != 1 {
                return Ok(TaskResolution::NeedsInput(request.turn));
            }
            (
                matches[0].permission.target.clone(),
                ActionPayload::ReadPage {
                    origin: origin.as_str().to_owned(),
                    message_limit: 16,
                },
                "browser page".to_owned(),
            )
        } else if let Some(draft) = crate::workflows::draft_request(&record.text) {
            let app_name = crate::apps::alias_phrase(draft.app)?;
            let project = crate::apps::alias_phrase(draft.project)?;
            let matches:Vec<_>=permissions.iter().filter(|p|!p.revoked && matches!(&p.permission.target,TaskTarget::Prompt{binding} if binding.app_name==app_name && binding.project==project && binding.actor==record.actor)).collect();
            if matches.len() != 1 {
                return Ok(TaskResolution::NeedsInput(request.turn));
            }
            let TaskTarget::Prompt { binding } = &matches[0].permission.target else {
                return Err(ErrorCode::Malformed);
            };
            (
                matches[0].permission.target.clone(),
                ActionPayload::FillPrompt {
                    app_id: binding.app,
                    project_id: binding.id,
                    text: draft.text.to_owned(),
                },
                project,
            )
        } else if let Some(entry) = &routine {
            let matches: Vec<_> = permissions
                .iter()
                .filter(|p| !p.revoked && p.permission.target == entry.source.target)
                .collect();
            if matches.len() != 1 {
                return Ok(TaskResolution::NeedsInput(request.turn));
            }
            (
                entry.source.target.clone(),
                entry.source.payload.clone(),
                matches[0].permission.name.clone(),
            )
        } else if let Some(catalog) = diagnostic_request(&record.text) {
            (
                TaskTarget::Diagnostic { catalog },
                ActionPayload::Diagnostic {
                    catalog_entry: catalog.id(),
                },
                catalog.name().to_owned(),
            )
        } else if let Some(percent) = volume_percent(&record.text) {
            let matches: Vec<_> = permissions
                .iter()
                .filter(|v| !v.revoked && matches!(v.permission.target, TaskTarget::Volume { .. }))
                .collect();
            if matches.len() != 1 {
                return Ok(TaskResolution::NeedsInput(request.turn));
            }
            (
                matches[0].permission.target.clone(),
                ActionPayload::SetVolume { percent },
                "speakers".to_owned(),
            )
        } else {
            let phrase = match phrase(&record.text) {
                Ok(value) => value,
                Err(ErrorCode::Unsupported) => return Ok(TaskResolution::NeedsInput(request.turn)),
                Err(error) => return Err(error),
            };
            let resolved = match apps.resolve(record.actor, &phrase) {
                Ok(value) => value,
                Err(ErrorCode::Denied) => return Ok(TaskResolution::NeedsInput(request.turn)),
                Err(error) => return Err(error),
            };
            (
                TaskTarget::Application {
                    app: resolved.record.id,
                    app_revision: resolved.record.revision,
                    alias: resolved.alias_id,
                    alias_revision: resolved.alias_revision,
                },
                ActionPayload::LaunchApp {
                    app_id: resolved.record.id,
                },
                phrase,
            )
        };
        let matches: Vec<_> = permissions
            .iter()
            .filter(|v| !v.revoked && v.permission.target == target && v.permission.name == name)
            .collect();
        if matches.len() != 1 {
            return Ok(TaskResolution::NeedsInput(request.turn));
        }
        let grant_id = matches[0].permission.id;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let resolved = ResolvedTarget {
            target,
            payload,
            grant_id,
            name,
            deadline: None,
        };
        let link = insert_link(&tx, &record, &request.session, resolved, apps, "accepted")?;
        if let Some(entry) = routine {
            tx.execute(
                "INSERT INTO routine_invocations VALUES(?1,?2,?3)",
                params![
                    link.action.task_id.to_string(),
                    entry.id.to_string(),
                    entry.revision.to_string()
                ],
            )
            .map_err(|_| ErrorCode::Storage)?;
        }
        let result = linked(&tx, &record)?.ok_or(ErrorCode::Malformed)?;
        request.current()?;
        authorize(&TaskAuthority {
            session: &request.session,
            action: &link.action,
            turn: record.id,
            turn_revision: record.revision,
            target: &link.target,
        })?;
        request.current()?;
        link.action.validate(wall_time()?)?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(TaskResolution::Linked(result))
    }
}

// The caller owns the existing IMMEDIATE transaction and final native check.
// This helper creates no dispatch permit and never commits independently.
struct ResolvedTarget {
    target: TaskTarget,
    payload: ActionPayload,
    grant_id: Uuid,
    name: String,
    deadline: Option<Instant>,
}
fn insert_link(
    tx: &Connection,
    record: &Record,
    session: &DispatchSession,
    resolved: ResolvedTarget,
    apps: &AppCatalog,
    previous_state: &str,
) -> Result<Link, ErrorCode> {
    let ResolvedTarget {
        target,
        payload,
        grant_id,
        name,
        deadline,
    } = resolved;
    let now = wall_time()?;
    let budget_ms = match deadline {
        Some(end) => u64::try_from(end.saturating_duration_since(Instant::now()).as_millis())
            .map_err(|_| ErrorCode::Expired)?,
        None => avesra_contracts::MAX_ACTION_AGE_MS,
    };
    if budget_ms == 0 {
        return Err(ErrorCode::Expired);
    }
    let sql_now = i64::try_from(now).map_err(|_| ErrorCode::Expired)?;
    let action = Action {
        task_id: Uuid::new_v4(),
        step_id: Uuid::new_v4(),
        actor_id: record.actor,
        target_id: target.id(),
        grant_id,
        revision: Uuid::new_v4(),
        intent_revision: Uuid::new_v4(),
        payload,
        approval_id: None,
        issued_at_ms: now,
        expires_at_ms: now
            .checked_add(budget_ms.min(avesra_contracts::MAX_ACTION_AGE_MS))
            .ok_or(ErrorCode::Expired)?,
    };
    let (body,revoked):(Vec<u8>,bool)=tx.query_row("SELECT substr(CAST(body AS BLOB),1,8193),revoked FROM ledger_grants WHERE id=?1 AND actor_id=?2",params![grant_id.to_string(),record.actor.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|ErrorCode::Denied)?;
    if body.len() > 8192 {
        return Err(ErrorCode::Malformed);
    }
    let mut grant: Grant = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    grant.revoked |= revoked;
    PolicyContext {
        actor_id: record.actor,
        accepted_task_id: action.task_id,
        intent_revision: action.intent_revision,
        permitted_payloads: std::slice::from_ref(&action.payload),
        now_ms: now,
        grant: &grant,
        approval: None,
        explicit_submit: false,
        session_active: session.active,
    }
    .authorize(&action)?;
    if tx.execute("UPDATE accepted_conversations SET state='planning' WHERE id=?1 AND revision=?2 AND actor=?3 AND state=?4",params![record.id.to_string(),record.revision.to_string(),record.actor.to_string(), previous_state]).map_err(|_|ErrorCode::Storage)?!=1{return Err(ErrorCode::Stale);}
    let queued = encode(&TaskState::Queued)?;
    tx.execute(
        "INSERT INTO tasks(id,actor_id,state,updated_ms) VALUES(?1,?2,?3,?4)",
        params![
            action.task_id.to_string(),
            record.actor.to_string(),
            queued,
            sql_now
        ],
    )
    .map_err(|_| ErrorCode::Storage)?;
    tx.execute("INSERT INTO accepted_intents(task_id,actor_id,revision,payloads,explicit_submit,sealed) VALUES(?1,?2,?3,?4,0,1)",params![action.task_id.to_string(),record.actor.to_string(),action.intent_revision.to_string(),encode(&vec![&action.payload])?]).map_err(|_|ErrorCode::Storage)?;
    tx.execute("INSERT INTO steps(id,task_id,state,target_id,operation,updated_ms) VALUES(?1,?2,?3,?4,?5,?6)",params![action.step_id.to_string(),action.task_id.to_string(),queued,action.target_id.to_string(),encode(&action.payload.operation())?,sql_now]).map_err(|_|ErrorCode::Storage)?;
    tx.execute(
        "INSERT INTO action_revisions(revision,step_id,body) VALUES(?1,?2,?3)",
        params![
            action.revision.to_string(),
            action.step_id.to_string(),
            encode(&action)?
        ],
    )
    .map_err(|_| ErrorCode::Storage)?;
    tx.execute(
        "INSERT INTO action_heads(step_id,revision) VALUES(?1,?2)",
        params![action.step_id.to_string(), action.revision.to_string()],
    )
    .map_err(|_| ErrorCode::Storage)?;
    tx.execute(
        "INSERT INTO ledger_events(task_id,step_id,kind,at_ms) VALUES(?1,?2,'accepted_action',?3)",
        params![
            action.task_id.to_string(),
            action.step_id.to_string(),
            sql_now
        ],
    )
    .map_err(|_| ErrorCode::Storage)?;
    let link = Link {
        turn: record.id,
        turn_revision: record.revision,
        actor: record.actor,
        target: target.clone(),
        action,
    };
    tx.execute(
        "INSERT INTO conversation_tasks(turn,task,actor,body) VALUES(?1,?2,?3,?4)",
        params![
            record.id.to_string(),
            link.action.task_id.to_string(),
            record.actor.to_string(),
            encode(&link)?
        ],
    )
    .map_err(|_| ErrorCode::Storage)?;
    linked(tx, record)?.ok_or(ErrorCode::Malformed)?;
    if let TaskTarget::Prompt { binding } = &target {
        let current = apps.resolve(record.actor, &binding.app_name)?;
        if current.alias_id != binding.alias
            || current.alias_revision != binding.alias_revision
            || current.record.id != binding.app
            || current.record.revision != binding.app_revision
        {
            return Err(ErrorCode::Stale);
        }
    }
    if let TaskTarget::Application {
        app,
        app_revision,
        alias,
        alias_revision,
    } = target
    {
        let current = apps.resolve(record.actor, &name)?;
        if current.alias_id != alias
            || current.alias_revision != alias_revision
            || current.record.id != app
            || current.record.revision != app_revision
        {
            return Err(ErrorCode::Stale);
        }
    }
    Ok(link)
}

/// Called only from the original planner completion transaction, never from
/// deserialized history. Model labels select existing native records only.
pub(super) fn link_proposal(
    tx: &Connection,
    record: &Record,
    proposal: &avesra_contracts::planner::Proposal,
    permissions: &[crate::action_permissions::PermissionView],
    apps: &AppCatalog,
    deadline: Instant,
) -> Result<Option<LinkedTask>, ErrorCode> {
    use avesra_contracts::planner::Proposal;
    if Instant::now() >= deadline || !proposal_grounded(&record.text, proposal) {
        return Ok(None);
    }
    let (target, payload, name) = match proposal {
        Proposal::LaunchApp { alias } => {
            let name = match crate::apps::alias_phrase(alias) {
                Ok(value) => value,
                Err(_) => return Ok(None),
            };
            let resolved = match apps.resolve(record.actor, &name) {
                Ok(value) => value,
                Err(ErrorCode::Denied | ErrorCode::Unsupported | ErrorCode::Stale) => {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            (
                TaskTarget::Application {
                    app: resolved.record.id,
                    app_revision: resolved.record.revision,
                    alias: resolved.alias_id,
                    alias_revision: resolved.alias_revision,
                },
                ActionPayload::LaunchApp {
                    app_id: resolved.record.id,
                },
                name,
            )
        }
        Proposal::SetVolume { percent } => {
            let matches: Vec<_> = permissions
                .iter()
                .filter(|v| {
                    !v.revoked
                        && v.permission.actor == record.actor
                        && matches!(v.permission.target, TaskTarget::Volume { .. })
                })
                .collect();
            if *percent > 100 || matches.len() != 1 {
                return Ok(None);
            }
            (
                matches[0].permission.target.clone(),
                ActionPayload::SetVolume { percent: *percent },
                "speakers".to_owned(),
            )
        }
    };
    let matches: Vec<_> = permissions
        .iter()
        .filter(|v| {
            !v.revoked
                && v.permission.actor == record.actor
                && v.permission.target == target
                && v.permission.name == name
        })
        .collect();
    if matches.len() != 1 {
        return Ok(None);
    }
    let session = DispatchSession {
        actor_id: record.actor,
        device_id: record.source.device,
        session_id: record.source.session,
        capture_epoch: record.capture_epoch,
        action_epoch: record.action_epoch,
        active: true,
    };
    let resolved = ResolvedTarget {
        target,
        payload,
        grant_id: matches[0].permission.id,
        name,
        deadline: Some(deadline),
    };
    let link = insert_link(tx, record, &session, resolved, apps, "planning")?;
    link.action.validate(wall_time()?)?;
    linked(tx, record)
}

// Independent, closed intent grammar over the actual accepted request. A model
// proposal is only a hint; matching a grant alone never supplies intent.
fn proposal_grounded(text: &str, proposal: &avesra_contracts::planner::Proposal) -> bool {
    use avesra_contracts::planner::Proposal;
    if text
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() && c != ' ')
    {
        return false;
    }
    let text = text.trim_matches(' ').to_lowercase();
    let text = text
        .strip_suffix('.')
        .or_else(|| text.strip_suffix('?'))
        .unwrap_or(&text);
    let text = text
        .strip_prefix("avesra, ")
        .or_else(|| text.strip_prefix("avesra "))
        .unwrap_or(text);
    let text = text
        .strip_prefix("can you ")
        .or_else(|| text.strip_prefix("could you "))
        .or_else(|| text.strip_prefix("would you "))
        .unwrap_or(text);
    let text = text.strip_prefix("please ").unwrap_or(text);
    let text = text.strip_suffix(" please").unwrap_or(text);
    match proposal {
        Proposal::LaunchApp { alias } => {
            let text = text
                .strip_prefix("start ")
                .map(|rest| format!("open {rest}"))
                .unwrap_or_else(|| text.to_owned());
            phrase(&text)
                .ok()
                .zip(crate::apps::alias_phrase(alias).ok())
                .is_some_and(|(accepted, proposed)| accepted == proposed)
        }
        Proposal::SetVolume { percent } => {
            let Some(rest) = text.strip_prefix("set ") else {
                return false;
            };
            let rest = rest.strip_prefix("the ").unwrap_or(rest);
            let rest = ["speakers ", "speaker ", "system ", "output "]
                .iter()
                .find_map(|prefix| rest.strip_prefix(*prefix))
                .unwrap_or(rest);
            let Some(number) = rest.strip_prefix("volume to ") else {
                return false;
            };
            let Some(number) = number
                .strip_suffix(" percent")
                .or_else(|| number.strip_suffix('%'))
            else {
                return false;
            };
            !number.is_empty()
                && number.len() <= 3
                && number.bytes().all(|c| c.is_ascii_digit())
                && number
                    .parse::<u8>()
                    .is_ok_and(|value| value <= 100 && value == *percent)
        }
    }
}
