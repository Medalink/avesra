//! Bounded private history projections. No live capability can be reconstructed here.
use super::{Record, Source, planner, tasks};
use crate::{memory, store::Store};
use avesra_contracts::{ErrorCode, TaskState, planner::Response, speech::Provenance};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone)]
pub struct Cursor {
    high: i64,
    before: i64,
}
pub enum Query {
    Page(Option<Cursor>),
    MemorySource { memory: Uuid, revision: Uuid },
}
#[derive(Serialize)]
pub struct Reply {
    pub revision: Uuid,
    pub response: Response,
    pub provenance: Provenance,
}
#[derive(Serialize)]
pub struct Planner {
    pub request: Uuid,
    pub state: String,
    pub owner_revision: Uuid,
    pub registration_revision: Uuid,
    pub reply: Option<Reply>,
    pub server_fingerprint: Option<String>,
}
#[derive(Serialize)]
pub struct Linked {
    pub task: Uuid,
    pub state: TaskState,
}
#[derive(Serialize)]
pub struct Row {
    pub id: Uuid,
    pub revision: Uuid,
    pub created_ms: u64,
    pub state: String,
    pub text: String,
    pub source: Source,
    pub planner: Option<Planner>,
    pub linked_task: Option<Linked>,
    pub playback_outcome: Option<String>,
}
#[derive(Serialize)]
pub struct Page {
    pub rows: Vec<Row>,
    pub scanned_window_complete: bool,
    pub has_older: bool,
}
#[derive(Serialize)]
pub struct SourceView {
    pub memory: Uuid,
    pub revision: Uuid,
    pub key: String,
    pub current_value: String,
    pub corrected: bool,
    pub original: Row,
    pub correction: Option<Row>,
}
pub struct ResultView {
    pub page: Option<Page>,
    pub source: Option<SourceView>,
    pub next: Option<Cursor>,
    pub anchor: Option<Cursor>,
}
fn row(
    db: &rusqlite::Connection,
    actor: Uuid,
    device: Uuid,
    id: Uuid,
    revision: Option<Uuid>,
) -> Result<Row, ErrorCode> {
    let (record, state) = tasks::read_record(db, id)?;
    if record.actor != actor
        || record.source.device != device
        || revision.is_some_and(|v| v != record.revision)
    {
        return Err(ErrorCode::Denied);
    }
    project(db, record, state)
}
fn project(db: &rusqlite::Connection, record: Record, state: String) -> Result<Row, ErrorCode> {
    if !matches!(
        state.as_str(),
        "accepted" | "planning" | "waiting_input" | "answered" | "cancelled" | "suspended"
    ) {
        return Err(ErrorCode::Malformed);
    }
    let planner = planner::history(db, &record)?;
    let linked_task = tasks::linked(db, &record)?.map(|v| Linked {
        task: v.task,
        state: v.state,
    });
    Ok(Row {
        id: record.id,
        revision: record.revision,
        created_ms: record.created_ms,
        state,
        text: record.text,
        source: record.source,
        planner,
        linked_task,
        playback_outcome: None,
    })
}
impl Store {
    pub fn conversation_history(
        &self,
        actor: Uuid,
        device: Uuid,
        query: Query,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<ResultView, ErrorCode> {
        if actor.is_nil() || device.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        authorize()?;
        let result = match query {
            Query::MemorySource {
                memory: id,
                revision,
            } => {
                if id.is_nil() || revision.is_nil() {
                    return Err(ErrorCode::Malformed);
                }
                let entry = memory::read(&self.connection, actor, id)?;
                if entry.revision != revision {
                    return Err(ErrorCode::Stale);
                }
                let memory::Content::NamedFact { key, value } = entry.content else {
                    return Err(ErrorCode::Unsupported);
                };
                let source = entry.accepted_source.ok_or(ErrorCode::Malformed)?;
                let original = row(
                    &self.connection,
                    actor,
                    device,
                    source.turn,
                    Some(source.revision),
                )?;
                let correction = entry
                    .changed_by
                    .map(|s| row(&self.connection, actor, device, s.turn, Some(s.revision)))
                    .transpose()?;
                ResultView {
                    page: None,
                    next: None,
                    anchor: None,
                    source: Some(SourceView {
                        memory: id,
                        revision,
                        key,
                        current_value: value,
                        corrected: entry.corrected,
                        original,
                        correction,
                    }),
                }
            }
            Query::Page(cursor) => {
                let cursor = match cursor {
                    Some(value) => value,
                    None => {
                        let high: i64 = self
                            .connection
                            .query_row(
                                "SELECT COALESCE(MAX(rowid),0) FROM accepted_conversations",
                                [],
                                |r| r.get(0),
                            )
                            .map_err(|_| ErrorCode::Storage)?;
                        Cursor { high, before: high }
                    }
                };
                if cursor.high < 0 || cursor.before < 0 || cursor.before > cursor.high {
                    return Err(ErrorCode::Malformed);
                }
                // Bound examined headers independently of actor sparsity; the rowid
                // range uses the existing SQLite primary rowid btree, no full scan.
                let mut statement=self.connection.prepare("SELECT rowid,substr(id,1,37),substr(actor,1,37),substr(device,1,37) FROM accepted_conversations WHERE rowid>0 AND rowid<=?1 ORDER BY rowid DESC LIMIT 200").map_err(|_|ErrorCode::Storage)?;
                let candidates = statement
                    .query_map([cursor.before], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    })
                    .map_err(|_| ErrorCode::Storage)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ErrorCode::Storage)?;
                let mut rows = Vec::new();
                let mut last = None;
                let mut examined = 0;
                let actor_key = actor.to_string();
                let device_key = device.to_string();
                for (position, id, owner, endpoint) in &candidates {
                    authorize()?;
                    last = Some(*position);
                    examined += 1;
                    if owner == &actor_key && endpoint == &device_key {
                        rows.push(row(
                            &self.connection,
                            actor,
                            device,
                            Uuid::parse_str(id).map_err(|_| ErrorCode::Malformed)?,
                            None,
                        )?);
                        if rows.len() == 20 {
                            break;
                        }
                    }
                }
                let exhausted = (examined == candidates.len() && candidates.len() < 200)
                    || last == Some(1)
                    || candidates.is_empty();
                let next = if exhausted {
                    None
                } else {
                    Some(Cursor {
                        high: cursor.high,
                        before: last
                            .ok_or(ErrorCode::Malformed)?
                            .checked_sub(1)
                            .ok_or(ErrorCode::Malformed)?,
                    })
                };
                ResultView {
                    anchor: Some(Cursor {
                        high: cursor.high,
                        before: cursor.high,
                    }),
                    page: Some(Page {
                        rows,
                        scanned_window_complete: exhausted,
                        has_older: next.is_some(),
                    }),
                    source: None,
                    next,
                }
            }
        };
        let bytes = serde_json::to_vec(&(&result.page, &result.source))
            .map_err(|_| ErrorCode::Malformed)?;
        if bytes.len() > 512 * 1024 {
            return Err(ErrorCode::TooLarge);
        }
        authorize()?;
        Ok(result)
    }
}
