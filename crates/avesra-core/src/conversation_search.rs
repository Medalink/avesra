//! Rebuildable private search copies. They never substitute for source validation.
use super::{deletion, history, planner, tasks};
use crate::action_permissions::TaskTarget;
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Instant,
};
use uuid::Uuid;
#[path = "conversation_search_schema.rs"]
mod schema;
pub(crate) use schema::{check_schema, create};
const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone)]
pub struct Cursor {
    query: Query,
    high: i64,
    before: i64,
    coverage: Coverage,
    revision: i64,
}
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Text { text: String },
    Turn { id: Uuid },
    Task { id: Uuid },
    App { id: Uuid },
}
#[derive(Clone, Serialize)]
pub struct Coverage {
    pub indexed_through: i64,
    pub highwater: i64,
    pub complete: bool,
    pub processed: usize,
    pub budget_exhausted: bool,
    #[serde(skip)]
    revision: i64,
}
pub struct ResultView {
    pub page: history::Page,
    pub next: Option<Cursor>,
    pub coverage: Coverage,
}
struct Budget<'a>(&'a Connection, Arc<AtomicBool>);
impl<'a> Budget<'a> {
    fn new(db: &'a Connection, deadline: Instant) -> Result<Self, ErrorCode> {
        if Instant::now() >= deadline {
            return Err(ErrorCode::Expired);
        }
        Self::metered(db, deadline, Arc::new(AtomicU32::new(0)), 200_000)
    }
    fn metered(
        db: &'a Connection,
        deadline: Instant,
        instructions: Arc<AtomicU32>,
        limit: u32,
    ) -> Result<Self, ErrorCode> {
        if Instant::now() >= deadline {
            return Err(ErrorCode::Expired);
        }
        let active = Arc::new(AtomicBool::new(true));
        let observed = active.clone();
        db.progress_handler(
            1000,
            Some(move || {
                observed.load(Ordering::Relaxed)
                    && (instructions
                        .fetch_add(1000, Ordering::Relaxed)
                        .saturating_add(1000)
                        >= limit
                        || Instant::now() >= deadline)
            }),
        )
        .map_err(|_| ErrorCode::Storage)?;
        Ok(Self(db, active))
    }
}
impl Drop for Budget<'_> {
    fn drop(&mut self) {
        // Disarm before cleanup even if rusqlite refuses removal. The owned
        // Store connection normally guarantees check_owned succeeds here.
        self.1.store(false, Ordering::Relaxed);
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}
fn scope(actor: Uuid, device: Uuid) -> String {
    format!("a{}d{}", actor.simple(), device.simple())
}
fn expression(text: &str, actor: Uuid, device: Uuid) -> Result<String, ErrorCode> {
    if text.len() > 256 {
        return Err(ErrorCode::TooLarge);
    }
    let terms = text
        .split(|v: char| !v.is_alphanumeric())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() || terms.len() > 8 {
        return Err(ErrorCode::Malformed);
    }
    Ok(format!(
        "owner_scope : \"{}\" AND ({{original response}} : ({}))",
        scope(actor, device),
        terms
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(" AND ")
    ))
}
struct Document {
    rowid: i64,
    turn: Uuid,
    revision: Uuid,
    actor: Uuid,
    device: Uuid,
    task: Option<Uuid>,
    app: Option<Uuid>,
    original: String,
    response: String,
}
fn document(db: &Connection, turn: Uuid) -> Result<Option<Document>, ErrorCode> {
    if deletion::read(db, turn)?.is_some_and(|v| v.selected) {
        return Ok(None);
    }
    let (record, _) = tasks::history_record(db, turn)?;
    let plan = planner::history(db, &record)?;
    let response = plan
        .and_then(|v| v.reply)
        .map(|r| r.response.text().to_owned())
        .unwrap_or_default();
    let link = tasks::linked(db, &record)?;
    let task = link.as_ref().map(|v| v.task);
    let app = link.as_ref().and_then(|v| match &v.target {
        TaskTarget::Application { app, .. } => Some(*app),
        TaskTarget::Prompt { binding } => Some(binding.app),
        _ => None,
    });
    let rowid = db
        .query_row(
            "SELECT rowid FROM accepted_conversations WHERE id=?1",
            [turn.to_string()],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Storage)?;
    Ok(Some(Document {
        rowid,
        turn,
        revision: record.revision,
        actor: record.actor,
        device: record.source.device,
        task,
        app,
        original: record.text,
        response,
    }))
}
pub(crate) fn refresh(db: &Connection, turn: Uuid) -> Result<usize, ErrorCode> {
    if db
        .pragma_query_value(None, "secure_delete", |r| r.get::<_, i64>(0))
        .map_err(|_| ErrorCode::Storage)?
        != 1
        || db
            .query_row(
                "SELECT v FROM conversation_fts_config WHERE k='secure-delete'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|_| ErrorCode::Storage)?
            != 1
    {
        return Err(ErrorCode::Malformed);
    }
    let(rowid,actor,device):(i64,String,String)=db.query_row("SELECT rowid,substr(actor,1,37),substr(device,1,37) FROM accepted_conversations WHERE id=?1",[turn.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|ErrorCode::Storage)?;
    db.execute("INSERT INTO conversation_search_progress(actor,device,through,revision) VALUES(?1,?2,0,1) ON CONFLICT(actor,device) DO UPDATE SET revision=revision+1 WHERE revision<9007199254740991",params![actor,device]).map_err(|_|ErrorCode::Storage)?;
    let revision: i64 = db
        .query_row(
            "SELECT revision FROM conversation_search_progress WHERE actor=?1 AND device=?2",
            params![actor, device],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Storage)?;
    if revision >= 9007199254740991 {
        return Err(ErrorCode::TooLarge);
    }
    db.execute("DELETE FROM conversation_fts WHERE rowid=?1", [rowid])
        .map_err(|_| ErrorCode::Storage)?;
    db.execute("DELETE FROM conversation_search_docs WHERE id=?1", [rowid])
        .map_err(|_| ErrorCode::Storage)?;
    let Some(d) = document(db, turn)? else {
        return Ok(0);
    };
    let bytes = d.original.len() + d.response.len();
    if bytes > 16384 || d.rowid != rowid {
        return Err(ErrorCode::Malformed);
    }
    db.execute("INSERT INTO conversation_search_docs(id,turn,revision,actor,device,task,app) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![rowid,d.turn.to_string(),d.revision.to_string(),d.actor.to_string(),d.device.to_string(),d.task.map(|v|v.to_string()),d.app.map(|v|v.to_string())]).map_err(|_|ErrorCode::Storage)?;
    db.execute(
        "INSERT INTO conversation_fts(rowid,owner_scope,original,response) VALUES(?1,?2,?3,?4)",
        params![rowid, scope(d.actor, d.device), d.original, d.response],
    )
    .map_err(|_| ErrorCode::Storage)?;
    Ok(bytes)
}
fn coverage(db: &Connection, actor: Uuid, device: Uuid) -> Result<Coverage, ErrorCode> {
    let(indexed,revision):(i64,i64)=db.query_row("SELECT through,revision FROM conversation_search_progress WHERE actor=?1 AND device=?2",params![actor.to_string(),device.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|ErrorCode::Storage)?.unwrap_or((0,0));
    let high:i64=db.query_row("SELECT COALESCE(MAX(rowid),0) FROM accepted_conversations INDEXED BY conversation_search_owner WHERE actor=?1 AND device=?2",params![actor.to_string(),device.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
    if indexed < 0 || indexed > high {
        return Err(ErrorCode::Malformed);
    }
    Ok(Coverage {
        indexed_through: indexed,
        highwater: high,
        complete: indexed == high,
        processed: 0,
        budget_exhausted: false,
        revision,
    })
}
pub(crate) fn backfill(
    db: &Connection,
    actor: Uuid,
    device: Uuid,
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Coverage, ErrorCode> {
    authorize()?;
    let meter = Arc::new(AtomicU32::new(0));
    let (mut result, rows) = {
        let _budget = Budget::metered(db, deadline, meter.clone(), 2_000_000)?;
        let result = coverage(db, actor, device)?;
        let mut statement=db.prepare("SELECT rowid,substr(id,1,37) FROM accepted_conversations INDEXED BY conversation_search_owner WHERE actor=?1 AND device=?2 AND rowid>?3 AND rowid<=?4 ORDER BY rowid LIMIT 200").map_err(|_|ErrorCode::Storage)?;
        let rows = statement
            .query_map(
                params![
                    actor.to_string(),
                    device.to_string(),
                    result.indexed_through,
                    result.highwater
                ],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(|_| ErrorCode::Storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ErrorCode::Storage)?;
        (result, rows)
    };
    let mut bytes = 0usize;
    for (rowid, id) in rows {
        authorize()?;
        if meter.load(Ordering::Relaxed) >= 1_800_000 {
            result.budget_exhausted = true;
            break;
        }
        let id = Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?;
        let tx = db.unchecked_transaction().map_err(|_| ErrorCode::Storage)?;
        let step = (|| {
            let _budget = Budget::metered(&tx, deadline, meter.clone(), 2_000_000)?;
            let body_bytes:i64=tx.query_row("SELECT length(CAST(a.body AS BLOB))+COALESCE(length(CAST(p.body AS BLOB)),0)+COALESCE(length(CAST(r.body AS BLOB)),0)+COALESCE(length(CAST(t.body AS BLOB)),0) FROM accepted_conversations a LEFT JOIN conversation_plans p ON p.turn=a.id LEFT JOIN conversation_replies r ON r.turn=a.id LEFT JOIN conversation_tasks t ON t.turn=a.id WHERE a.id=?1",[id.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
            let next = bytes
                .checked_add(usize::try_from(body_bytes).map_err(|_| ErrorCode::Malformed)?)
                .ok_or(ErrorCode::TooLarge)?;
            if next > MAX_BYTES {
                return Ok(None);
            }
            refresh(&tx, id)?;
            tx.execute(
                "UPDATE conversation_search_progress SET through=?3 WHERE actor=?1 AND device=?2",
                params![actor.to_string(), device.to_string(), rowid],
            )
            .map_err(|_| ErrorCode::Storage)?;
            authorize()?;
            Ok(Some(next))
        })();
        match step {
            Ok(Some(next)) => {
                authorize()?;
                tx.commit().map_err(|_| ErrorCode::Storage)?;
                bytes = next;
                result.indexed_through = rowid;
                result.processed += 1;
            }
            Ok(None) => {
                drop(tx);
                break;
            }
            Err(error) => {
                drop(tx);
                if meter.load(Ordering::Relaxed) >= 2_000_000 {
                    result.budget_exhausted = true;
                    break;
                }
                return Err(error);
            }
        }
    }
    authorize()?;
    result.complete = result.indexed_through == result.highwater;
    Ok(result)
}
pub(crate) fn run(
    db: &Connection,
    actor: Uuid,
    device: Uuid,
    query: Option<Query>,
    cursor: Option<Cursor>,
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<ResultView, ErrorCode> {
    authorize()?;
    let _budget = Budget::new(db, deadline)?;
    let coverage = coverage(db, actor, device)?;
    let c = match (cursor, query) {
        (Some(c), None) => c,
        (None, Some(query)) => Cursor {
            query,
            high: coverage.highwater,
            before: coverage
                .highwater
                .checked_add(1)
                .ok_or(ErrorCode::Malformed)?,
            revision: coverage.revision,
            coverage: coverage.clone(),
        },
        _ => return Err(ErrorCode::Malformed),
    };
    if c.revision != coverage.revision {
        return Err(ErrorCode::Stale);
    }
    let coverage = c.coverage.clone();
    let ids: Vec<(i64, Uuid)> = match &c.query {
        Query::Text { text } => {
            let expression = expression(text, actor, device)?;
            let mut statement=db.prepare("SELECT rowid FROM conversation_fts WHERE conversation_fts MATCH ?1 AND rowid<=?2 AND rowid<?3 ORDER BY rowid DESC LIMIT 200").map_err(|_|ErrorCode::Storage)?;
            let positions = statement
                .query_map(params![expression, c.high, c.before], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(|_| ErrorCode::Storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ErrorCode::Storage)?;
            let mut ids = Vec::new();
            for position in positions {
                let (id,revision,owner,endpoint):(String,String,String,String)=db.query_row("SELECT substr(turn,1,37),substr(revision,1,37),substr(actor,1,37),substr(device,1,37) FROM conversation_search_docs WHERE id=?1",[position],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_|ErrorCode::Malformed)?;
                if owner != actor.to_string() || endpoint != device.to_string() {
                    return Err(ErrorCode::Denied);
                }
                let id = Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?;
                if Uuid::parse_str(&revision)
                    .map_err(|_| ErrorCode::Malformed)?
                    .is_nil()
                {
                    return Err(ErrorCode::Malformed);
                }
                ids.push((position, id));
            }
            ids
        }
        Query::Turn { id } | Query::Task { id } => {
            if id.is_nil() {
                return Err(ErrorCode::Malformed);
            }
            let sql = if matches!(&c.query, Query::Turn { .. }) {
                "SELECT rowid,id FROM accepted_conversations WHERE id=?1 AND actor=?2 AND device=?3 AND rowid<=?4 AND rowid<?5"
            } else {
                "SELECT a.rowid,a.id FROM conversation_tasks t JOIN accepted_conversations a ON a.id=t.turn WHERE t.task=?1 AND a.actor=?2 AND a.device=?3 AND a.rowid<=?4 AND a.rowid<?5"
            };
            db.query_row(
                sql,
                params![
                    id.to_string(),
                    actor.to_string(),
                    device.to_string(),
                    c.high,
                    c.before
                ],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|_| ErrorCode::Storage)?
            .map(|(n, id)| {
                Uuid::parse_str(&id)
                    .map(|id| vec![(n, id)])
                    .map_err(|_| ErrorCode::Malformed)
            })
            .transpose()?
            .unwrap_or_default()
        }
        Query::App { id } => {
            if id.is_nil() {
                return Err(ErrorCode::Malformed);
            }
            let mut statement=db.prepare("SELECT id,substr(turn,1,37) FROM conversation_search_docs WHERE actor=?1 AND device=?2 AND app=?3 AND id<=?4 AND id<?5 ORDER BY id DESC LIMIT 200").map_err(|_|ErrorCode::Storage)?;
            let rows = statement
                .query_map(
                    params![
                        actor.to_string(),
                        device.to_string(),
                        id.to_string(),
                        c.high,
                        c.before
                    ],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                )
                .map_err(|_| ErrorCode::Storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ErrorCode::Storage)?;
            rows.into_iter()
                .map(|(n, v)| {
                    Uuid::parse_str(&v)
                        .map(|v| (n, v))
                        .map_err(|_| ErrorCode::Malformed)
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    let mut rows = Vec::new();
    let mut last = None;
    let mut examined = 0;
    for (position, id) in &ids {
        authorize()?;
        last = Some(*position);
        examined += 1;
        if deletion::read(db, *id)?.is_some_and(|v| v.selected) {
            continue;
        }
        if matches!(&c.query, Query::Text { .. }) {
            let d = document(db, *id)?.ok_or(ErrorCode::Stale)?;
            let values:(String,String,String)=db.query_row("SELECT substr(owner_scope,1,80),substr(original,1,8193),substr(response,1,8193) FROM conversation_fts WHERE rowid=?1",[position],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|ErrorCode::Malformed)?;
            let revision: String = db
                .query_row(
                    "SELECT revision FROM conversation_search_docs WHERE id=?1",
                    [position],
                    |r| r.get(0),
                )
                .map_err(|_| ErrorCode::Malformed)?;
            if d.rowid != *position
                || d.revision.to_string() != revision
                || values != (scope(actor, device), d.original, d.response)
            {
                return Err(ErrorCode::Stale);
            }
        }
        let value = history::row(db, actor, device, *id, None)?;
        if let Query::App { id: app } = &c.query {
            let d = document(db, *id)?.ok_or(ErrorCode::Stale)?;
            if d.app != Some(*app) {
                return Err(ErrorCode::Stale);
            }
        }
        rows.push(value);
        if rows.len() == 20 {
            break;
        }
    }
    let complete = examined == ids.len() && ids.len() < 200;
    let next = if complete {
        None
    } else {
        Some(Cursor {
            before: last.ok_or(ErrorCode::Malformed)?,
            ..c
        })
    };
    authorize()?;
    Ok(ResultView {
        page: history::Page {
            rows,
            scanned_window_complete: complete,
            has_older: next.is_some(),
        },
        next,
        coverage,
    })
}
