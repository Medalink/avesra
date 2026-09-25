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

pub(crate) const RETIREMENT_SCHEMA: &str = "CREATE TABLE browser_read_retirements(dispatch TEXT PRIMARY KEY NOT NULL REFERENCES dispatch_bindings(dispatch_id),request TEXT UNIQUE NOT NULL CHECK(length(request)=36),revision TEXT UNIQUE NOT NULL CHECK(length(revision)=36),action_revision TEXT NOT NULL REFERENCES action_revisions(revision),context TEXT NOT NULL CHECK(length(CAST(context AS BLOB)) BETWEEN 1 AND 4096))";

pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    check_table(
        db,
        version,
        8,
        "browser_read_owner",
        SCHEMA,
        &[(1, "revision"), (2, "dispatch"), (3, "action_revision")],
    )?;
    check_table(
        db,
        version,
        9,
        "browser_read_retirements",
        RETIREMENT_SCHEMA,
        &[(0, "dispatch"), (1, "request"), (2, "revision")],
    )
}
fn check_table(
    db: &Connection,
    version: u64,
    introduced: u64,
    table: &str,
    expected_sql: &str,
    columns: &[(i64, &str); 3],
) -> Result<(), ErrorCode> {
    // All names are native constants. Bound attached objects before reading SQL.
    let mut query = sql(db.prepare("SELECT substr(CAST(type AS BLOB),1,17),substr(CAST(name AS BLOB),1,129),substr(CAST(sql AS BLOB),1,2049) FROM sqlite_master WHERE name=?1 OR tbl_name=?1 LIMIT 5"))?;
    let mut rows = sql(query.query([table]))?;
    let indexes = [
        format!("sqlite_autoindex_{table}_1"),
        format!("sqlite_autoindex_{table}_2"),
        format!("sqlite_autoindex_{table}_3"),
    ];
    let mut found = [false; 4];
    while let Some(row) = sql(rows.next())? {
        if version < introduced {
            return Err(ErrorCode::Malformed);
        }
        let kind: Vec<u8> = sql(row.get(0))?;
        let name: Vec<u8> = sql(row.get(1))?;
        let body: Option<Vec<u8>> = sql(row.get(2))?;
        let slot = if kind == b"table"
            && name == table.as_bytes()
            && body.as_deref() == Some(expected_sql.as_bytes())
        {
            0
        } else if kind == b"index" && body.is_none() {
            indexes
                .iter()
                .position(|v| v.as_bytes() == name)
                .map(|v| v + 1)
                .ok_or(ErrorCode::Malformed)?
        } else {
            return Err(ErrorCode::Malformed);
        };
        if found[slot] {
            return Err(ErrorCode::Malformed);
        }
        found[slot] = true;
    }
    if version < introduced {
        return Ok(());
    }
    if found != [true; 4] {
        return Err(ErrorCode::Malformed);
    }
    let mut query = sql(
        db.prepare(r#"SELECT name,"unique",origin,partial FROM pragma_index_list(?1) LIMIT 4"#)
    )?;
    let mut rows = sql(query.query([table]))?;
    let mut found = [false; 3];
    while let Some(row) = sql(rows.next())? {
        let name: String = sql(row.get(0))?;
        let slot = indexes
            .iter()
            .position(|v| *v == name)
            .ok_or(ErrorCode::Malformed)?;
        let origin = if table == "browser_read_retirements" && slot == 0 {
            "pk"
        } else {
            "u"
        };
        if found[slot]
            || sql::<i64>(row.get(1))? != 1
            || sql::<String>(row.get(2))? != origin
            || sql::<i64>(row.get(3))? != 0
        {
            return Err(ErrorCode::Malformed);
        }
        found[slot] = true;
    }
    if found != [true; 3] {
        return Err(ErrorCode::Malformed);
    }
    for (index, (column, name)) in indexes.iter().zip(columns) {
        let mut query =
            sql(db.prepare("SELECT seqno,cid,name FROM pragma_index_info(?1) LIMIT 2"))?;
        let mut rows = sql(query.query([index]))?;
        let row = sql(rows.next())?.ok_or(ErrorCode::Malformed)?;
        if sql::<i64>(row.get(0))? != 0
            || sql::<i64>(row.get(1))? != *column
            || sql::<String>(row.get(2))? != *name
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
    validate_context(db, &context)?;
    Ok(Some(Ownership {
        revision,
        context,
        state,
    }))
}

pub(crate) fn validate_context(db: &Connection, context: &Context) -> Result<(), ErrorCode> {
    context.validate()?;
    let action_revision = context.action_revision.uuid();
    let dispatch = context.dispatch.uuid();
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
    Ok(())
}

/// Historical resource receipt only. It never establishes content success.
#[derive(Serialize)]
pub struct Retirement {
    pub revision: Uuid,
    pub context: Context,
}

pub(crate) fn retired(
    db: &Connection,
    expected: &Context,
) -> Result<Option<Retirement>, ErrorCode> {
    expected.validate()?;
    let mut query = sql(db.prepare("SELECT substr(CAST(dispatch AS BLOB),1,37),substr(CAST(request AS BLOB),1,37),substr(CAST(revision AS BLOB),1,37),substr(CAST(action_revision AS BLOB),1,37),substr(CAST(context AS BLOB),1,4097) FROM browser_read_retirements WHERE dispatch=?1 LIMIT 2"))?;
    let mut rows = sql(query.query([expected.dispatch.uuid().to_string()]))?;
    let Some(row) = sql(rows.next())? else {
        return Ok(None);
    };
    let text = |index| -> Result<String, ErrorCode> {
        String::from_utf8(sql::<Vec<u8>>(row.get(index))?).map_err(|_| ErrorCode::Malformed)
    };
    let dispatch = id(&text(0)?)?;
    let request = id(&text(1)?)?;
    let revision = id(&text(2)?)?;
    let action_revision = id(&text(3)?)?;
    let body: Vec<u8> = sql(row.get(4))?;
    if body.is_empty() || body.len() > MAX_CONTEXT || sql(rows.next())?.is_some() {
        return Err(ErrorCode::Malformed);
    }
    let context: Context = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    validate_context(db, &context)?;
    if context.dispatch.uuid() != dispatch
        || context.request.uuid() != request
        || context.action_revision.uuid() != action_revision
    {
        return Err(ErrorCode::Malformed);
    }
    if &context != expected {
        return Err(ErrorCode::Stale);
    }
    Ok(Some(Retirement { revision, context }))
}

/// Withdrawal-only borrowed ownership read from the actual uncertain slot.
/// No caller-context constructor, publication permit or content authority.
pub struct Recovery<'a> {
    store: &'a mut Store,
    owned: Ownership,
}
impl Recovery<'_> {
    pub fn context(&self) -> &Context {
        &self.owned.context
    }
    pub fn revision(&self) -> Uuid {
        self.owned.revision
    }
    /// The native worker must first match its received opaque Settlement.
    pub fn retire(self) -> Result<Retirement, ErrorCode> {
        retire(
            &mut self.store.connection,
            self.owned.revision,
            &self.owned.context,
            true,
        )
    }
}

// Only concrete borrowed live/recovery leases can call this mutation helper.
pub(crate) fn retire(
    db: &mut Connection,
    revision: Uuid,
    context: &Context,
    uncertain: bool,
) -> Result<Retirement, ErrorCode> {
    let tx = sql(db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate))?;
    let owned = current(&tx)?.ok_or(ErrorCode::Stale)?;
    if retired(&tx, context)?.is_some() {
        return Err(ErrorCode::Malformed);
    }
    if owned.revision != revision || owned.context != *context {
        return Err(ErrorCode::Stale);
    }
    if uncertain && !matches!(owned.state, State::Uncertain) {
        return Err(ErrorCode::InvalidTransition);
    }
    let body = serde_json::to_vec(context).map_err(|_| ErrorCode::Malformed)?;
    if body.is_empty() || body.len() > MAX_CONTEXT {
        return Err(ErrorCode::TooLarge);
    }
    let body = String::from_utf8(body).map_err(|_| ErrorCode::Malformed)?;
    sql(tx.execute("INSERT INTO browser_read_retirements(dispatch,request,revision,action_revision,context) VALUES(?1,?2,?3,?4,?5)", rusqlite::params![context.dispatch.uuid().to_string(),context.request.uuid().to_string(),revision.to_string(),context.action_revision.uuid().to_string(),body]))?;
    let receipt = retired(&tx, context)?.ok_or(ErrorCode::Malformed)?;
    if receipt.revision != revision {
        return Err(ErrorCode::Malformed);
    }
    if sql(tx.execute(
        "DELETE FROM browser_read_owner WHERE id=1 AND revision=?1 AND dispatch=?2",
        [revision.to_string(), context.dispatch.uuid().to_string()],
    ))? != 1
    {
        return Err(ErrorCode::Stale);
    }
    if current(&tx)?.is_some() {
        return Err(ErrorCode::Malformed);
    }
    sql(tx.commit())?;
    let receipt = retired(db, context)?.ok_or(ErrorCode::Malformed)?;
    if receipt.revision != revision
        || current(db)?.is_some_and(|v| v.context.dispatch == context.dispatch)
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(receipt)
}
impl Store {
    /// Authenticated native recovery must separately validate actual pairing.
    /// This bounded metadata lookup mints no lease or transport capability.
    pub fn browser_read_retirement(
        &self,
        expected: &Context,
    ) -> Result<Option<Retirement>, ErrorCode> {
        let receipt = retired(&self.connection, expected)?;
        if receipt.is_some()
            && current(&self.connection)?.is_some_and(|v| v.context.dispatch == expected.dispatch)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(receipt)
    }
    /// Reborrows this actual owner; never reconstructs a running read/permit.
    pub fn recover_browser_read(&mut self) -> Result<Option<Recovery<'_>>, ErrorCode> {
        let Some(owned) = current(&self.connection)? else {
            return Ok(None);
        };
        if retired(&self.connection, &owned.context)?.is_some() {
            return Err(ErrorCode::Malformed);
        }
        if !matches!(owned.state, State::Uncertain) {
            return Err(ErrorCode::InvalidTransition);
        }
        Ok(Some(Recovery { store: self, owned }))
    }
    /// Same-worker metadata only; neither absence nor this value grants effects.
    pub fn browser_read_ownership(&self) -> Result<Option<Ownership>, ErrorCode> {
        current(&self.connection)
    }
}
