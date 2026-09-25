//! Durable external browser ownership. Metadata never reconstructs a read handle.
use crate::store::Store;
use avesra_contracts::{Action, ActionPayload, ErrorCode, browser::reading::Context};
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

pub(crate) const SCHEMA: &str = "CREATE TABLE browser_read_owner(id INTEGER PRIMARY KEY CHECK(id=1),revision TEXT UNIQUE NOT NULL CHECK(length(revision)=36),dispatch TEXT UNIQUE NOT NULL REFERENCES dispatch_bindings(dispatch_id),action_revision TEXT UNIQUE NOT NULL REFERENCES action_revisions(revision),context TEXT NOT NULL CHECK(length(CAST(context AS BLOB)) BETWEEN 1 AND 4096),state TEXT NOT NULL CHECK(state IN ('pending','uncertain')))";
const MAX_CONTEXT: usize = 4096;

fn sql<T>(value: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    value.map_err(|_| ErrorCode::Storage)
}
fn id(value: &str) -> Result<Uuid, ErrorCode> {
    let parsed = Uuid::parse_str(value).map_err(|_| ErrorCode::Malformed)?;
    if parsed.is_nil() || parsed.to_string() != value {
        return Err(ErrorCode::Malformed);
    }
    Ok(parsed)
}

pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    // LIMIT bounds user-defined objects too, before reading their SQL/text.
    let mut query = sql(db.prepare("SELECT substr(CAST(type AS BLOB),1,17),substr(CAST(name AS BLOB),1,129),substr(CAST(sql AS BLOB),1,2049) FROM sqlite_master WHERE name='browser_read_owner' OR tbl_name='browser_read_owner' LIMIT 5"))?;
    let mut rows = sql(query.query([]))?;
    let mut found = [false; 4];
    while let Some(row) = sql(rows.next())? {
        if version < 8 {
            return Err(ErrorCode::Malformed);
        }
        let kind: Vec<u8> = sql(row.get(0))?;
        let name: Vec<u8> = sql(row.get(1))?;
        let body: Option<Vec<u8>> = sql(row.get(2))?;
        let slot = match (kind.as_slice(), name.as_slice(), body.as_deref()) {
            (b"table", b"browser_read_owner", Some(body)) if body == SCHEMA.as_bytes() => 0,
            (b"index", b"sqlite_autoindex_browser_read_owner_1", None) => 1,
            (b"index", b"sqlite_autoindex_browser_read_owner_2", None) => 2,
            (b"index", b"sqlite_autoindex_browser_read_owner_3", None) => 3,
            _ => return Err(ErrorCode::Malformed),
        };
        if found[slot] {
            return Err(ErrorCode::Malformed);
        }
        found[slot] = true;
    }
    if version < 8 {
        return Ok(());
    }
    if found != [true; 4] {
        return Err(ErrorCode::Malformed);
    }
    let mut query = sql(db.prepare("PRAGMA index_list('browser_read_owner')"))?;
    let mut rows = sql(query.query([]))?;
    let mut found = [false; 3];
    while let Some(row) = sql(rows.next())? {
        let name: String = sql(row.get(1))?;
        let slot = match name.as_str() {
            "sqlite_autoindex_browser_read_owner_1" => 0,
            "sqlite_autoindex_browser_read_owner_2" => 1,
            "sqlite_autoindex_browser_read_owner_3" => 2,
            _ => return Err(ErrorCode::Malformed),
        };
        if found[slot]
            || sql::<i64>(row.get(2))? != 1
            || sql::<String>(row.get(3))? != "u"
            || sql::<i64>(row.get(4))? != 0
        {
            return Err(ErrorCode::Malformed);
        }
        found[slot] = true;
    }
    if found != [true; 3] {
        return Err(ErrorCode::Malformed);
    }
    for (query, column, name) in [
        (
            "PRAGMA index_info('sqlite_autoindex_browser_read_owner_1')",
            1,
            "revision",
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_browser_read_owner_2')",
            2,
            "dispatch",
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_browser_read_owner_3')",
            3,
            "action_revision",
        ),
    ] {
        let mut statement = sql(db.prepare(query))?;
        let mut rows = sql(statement.query([]))?;
        let row = sql(rows.next())?.ok_or(ErrorCode::Malformed)?;
        if sql::<i64>(row.get(0))? != 0
            || sql::<i64>(row.get(1))? != column
            || sql::<String>(row.get(2))? != name
            || sql(rows.next())?.is_some()
        {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    Uncertain,
}
/// Read-only provenance. No Deserialize, publication, retirement or reset API.
#[derive(Serialize)]
pub struct Ownership {
    pub revision: Uuid,
    pub context: Context,
    pub state: State,
}

pub(crate) fn current(db: &Connection) -> Result<Option<Ownership>, ErrorCode> {
    let mut query = sql(db.prepare("SELECT id,substr(CAST(revision AS BLOB),1,37),substr(CAST(dispatch AS BLOB),1,37),substr(CAST(action_revision AS BLOB),1,37),substr(CAST(context AS BLOB),1,4097),substr(CAST(state AS BLOB),1,10) FROM browser_read_owner LIMIT 2"))?;
    let mut rows = sql(query.query([]))?;
    let Some(row) = sql(rows.next())? else {
        return Ok(None);
    };
    if sql::<i64>(row.get(0))? != 1 {
        return Err(ErrorCode::Malformed);
    }
    let text = |index| -> Result<String, ErrorCode> {
        String::from_utf8(sql::<Vec<u8>>(row.get(index))?).map_err(|_| ErrorCode::Malformed)
    };
    let revision = id(&text(1)?)?;
    let dispatch = id(&text(2)?)?;
    let action_revision = id(&text(3)?)?;
    let body: Vec<u8> = sql(row.get(4))?;
    if body.is_empty() || body.len() > MAX_CONTEXT {
        return Err(ErrorCode::Malformed);
    }
    let context: Context = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    context.validate()?;
    let state = match text(5)?.as_str() {
        "pending" => State::Pending,
        "uncertain" => State::Uncertain,
        _ => return Err(ErrorCode::Malformed),
    };
    if sql(rows.next())?.is_some()
        || dispatch != context.dispatch.uuid()
        || action_revision != context.action_revision.uuid()
    {
        return Err(ErrorCode::Malformed);
    }
    // Check immutable action and source rows, irrespective of later outcome or
    // reconciliation. Expiration withdraws authority, not external ownership.
    let mut query = sql(db.prepare(
        "SELECT substr(CAST(a.body AS BLOB),1,32769),substr(CAST(a.step_id AS BLOB),1,37),substr(CAST(d.actor_id AS BLOB),1,37),substr(CAST(d.device_id AS BLOB),1,37),substr(CAST(d.session_id AS BLOB),1,37),substr(CAST(d.capture_epoch AS BLOB),1,21),substr(CAST(d.action_epoch AS BLOB),1,21) FROM action_revisions a JOIN dispatch_bindings d ON d.dispatch_id=a.dispatch_id WHERE a.revision=?1 AND a.dispatch_id=?2 LIMIT 2"
    ))?;
    let mut rows = sql(query.query([action_revision.to_string(), dispatch.to_string()]))?;
    let row = sql(rows.next())?.ok_or(ErrorCode::Malformed)?;
    let body: Vec<u8> = sql(row.get(0))?;
    let text = |index| -> Result<String, ErrorCode> {
        String::from_utf8(sql::<Vec<u8>>(row.get(index))?).map_err(|_| ErrorCode::Malformed)
    };
    let (step, actor, device, session, capture, action_epoch) =
        (text(1)?, text(2)?, text(3)?, text(4)?, text(5)?, text(6)?);
    if sql(rows.next())?.is_some() {
        return Err(ErrorCode::Malformed);
    }
    if body.is_empty() || body.len() > 32768 {
        return Err(ErrorCode::Malformed);
    }
    let action: Action = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    action.validate(action.issued_at_ms)?;
    if !matches!(action.payload, ActionPayload::ReadPage { .. })
        || action.revision != action_revision
        || action.step_id != context.step.uuid()
        || action.task_id != context.task.uuid()
        || action.actor_id != context.actor.uuid()
        || action.intent_revision != context.intent_revision.uuid()
        || action.grant_id != context.grant.uuid()
        || action.target_id != context.target.id.uuid()
        || step != context.step.uuid().to_string()
        || actor != context.actor.uuid().to_string()
        || device != context.source.device.uuid().to_string()
        || session != context.source.session.uuid().to_string()
        || capture != context.source.capture_epoch.to_string()
        || action_epoch != context.source.action_epoch.to_string()
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(Ownership {
        revision,
        context,
        state,
    }))
}

impl Store {
    /// Same-worker metadata only; neither absence nor this value grants effects.
    pub fn browser_read_ownership(&self) -> Result<Option<Ownership>, ErrorCode> {
        current(&self.connection)
    }
}
