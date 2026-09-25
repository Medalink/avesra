//! Native grant metadata only. A record is neither a Chrome permission nor an
//! accepted action; callers must enforce the current selected native session.
use avesra_contracts::{
    ErrorCode,
    browser::{Id, MAX_SAFE_COUNTER, Origin, PairingRef, ScopeOperation, validate_operations},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

const MAX_RECORDS: usize = 32;
const MAX_BODY: usize = 4096;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub id: Id,
    pub revision: Id,
    pub actor: Id,
    pub selection: Id,
    pub pairing: PairingRef,
    pub browser_app: Id,
    pub browser_revision: Id,
    pub origin: Origin,
    pub operations: Vec<ScopeOperation>,
    /// Display metadata only; never a freshness or authorization clock.
    pub created_at_ms: u64,
}
impl Grant {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        validate_operations(&self.operations)?;
        if self.created_at_ms == 0 || self.created_at_ms > MAX_SAFE_COUNTER {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
pub struct Store(Connection);
const VERSION_SCHEMA: &str = "CREATE TABLE schema_version(version INTEGER NOT NULL)";
const GRANTS_SCHEMA: &str = "CREATE TABLE scopes(id TEXT PRIMARY KEY CHECK(length(id)=36),revision TEXT UNIQUE NOT NULL CHECK(length(revision)=36),actor TEXT NOT NULL CHECK(length(actor)=36),selection TEXT NOT NULL CHECK(length(selection)=36),origin TEXT NOT NULL CHECK(length(origin)<=512),body TEXT NOT NULL CHECK(length(body)<=4096),UNIQUE(actor,selection,origin))";
fn check_schema(db: &Connection) -> Result<bool, ErrorCode> {
    let objects: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Unavailable)?;
    if objects == 0 {
        return Ok(false);
    }
    let marker: (i64, i64) = db
        .query_row(
            "SELECT count(*),min(version) FROM schema_version",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ErrorCode::Malformed)?;
    if marker != (1, 1) {
        return Err(ErrorCode::Unsupported);
    }
    let supported: i64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('schema_version','scopes')", [], |r|r.get(0)).map_err(|_|ErrorCode::Malformed)?;
    if objects != 2 || supported != 2 {
        return Err(ErrorCode::Malformed);
    }
    for (name, expected) in [
        ("schema_version", VERSION_SCHEMA),
        ("scopes", GRANTS_SCHEMA),
    ] {
        let sql: String = db
            .query_row(
                "SELECT substr(sql,1,2049) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Malformed)?;
        if sql != expected {
            return Err(ErrorCode::Malformed);
        }
    }
    // Exact SQL binds affinities/checks; inspect actual constraint indexes too.
    let mut indexes = db
        .prepare("PRAGMA index_list('scopes')")
        .map_err(|_| ErrorCode::Malformed)?;
    let rows = indexes
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|_| ErrorCode::Malformed)?;
    let mut found = [false; 3];
    for row in rows {
        let (name, unique, origin, partial) = row.map_err(|_| ErrorCode::Malformed)?;
        let index = match (name.as_str(), origin.as_str()) {
            ("sqlite_autoindex_scopes_1", "pk") => 0,
            ("sqlite_autoindex_scopes_2", "u") => 1,
            ("sqlite_autoindex_scopes_3", "u") => 2,
            _ => return Err(ErrorCode::Malformed),
        };
        if found[index] || unique != 1 || partial != 0 {
            return Err(ErrorCode::Malformed);
        }
        found[index] = true;
    }
    if found != [true; 3] {
        return Err(ErrorCode::Malformed);
    }
    for (query, expected) in [
        (
            "PRAGMA index_info('sqlite_autoindex_scopes_1')",
            vec![(0, "id")],
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_scopes_2')",
            vec![(1, "revision")],
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_scopes_3')",
            vec![(2, "actor"), (3, "selection"), (4, "origin")],
        ),
    ] {
        let mut statement = db.prepare(query).map_err(|_| ErrorCode::Malformed)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
        for (position, (column, name)) in expected.into_iter().enumerate() {
            let row = rows
                .next()
                .map_err(|_| ErrorCode::Malformed)?
                .ok_or(ErrorCode::Malformed)?;
            if row.get::<_, i64>(0).map_err(|_| ErrorCode::Malformed)? != position as i64
                || row.get::<_, i64>(1).map_err(|_| ErrorCode::Malformed)? != column
                || row.get::<_, String>(2).map_err(|_| ErrorCode::Malformed)? != name
            {
                return Err(ErrorCode::Malformed);
            }
        }
        if rows.next().map_err(|_| ErrorCode::Malformed)?.is_some() {
            return Err(ErrorCode::Malformed);
        }
    }
    db.prepare("SELECT id,revision,actor,selection,origin,body FROM scopes LIMIT 0")
        .map_err(|_| ErrorCode::Malformed)?;
    Ok(true)
}
// SQL bounds allocation before deserialization even for an externally damaged DB.
const COLUMNS: &str = "substr(id,1,37),substr(revision,1,37),substr(actor,1,37),substr(selection,1,37),substr(origin,1,513),substr(body,1,4097),length(body)";
type Row = (String, String, String, String, String, String, i64);
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
    ))
}
fn decode(value: Row) -> Result<Grant, ErrorCode> {
    let (id, revision, actor, selection, origin, body, length) = value;
    if body.len() > MAX_BODY || length <= 0 || length > MAX_BODY as i64 {
        return Err(ErrorCode::TooLarge);
    }
    let grant: Grant = serde_json::from_str(&body).map_err(|_| ErrorCode::Malformed)?;
    grant.validate()?;
    if grant.id.uuid().to_string() != id
        || grant.revision.uuid().to_string() != revision
        || grant.actor.uuid().to_string() != actor
        || grant.selection.uuid().to_string() != selection
        || grant.origin.as_str() != origin
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(grant)
}
fn count(db: &Connection) -> Result<usize, ErrorCode> {
    let count: i64 = db
        .query_row("SELECT count(*) FROM scopes", [], |r| r.get(0))
        .map_err(|_| ErrorCode::Malformed)?;
    let count = usize::try_from(count).map_err(|_| ErrorCode::Malformed)?;
    if count > MAX_RECORDS {
        return Err(ErrorCode::TooLarge);
    }
    Ok(count)
}
fn get(db: &Connection, actor: Id, id: Id, revision: Id) -> Result<Grant, ErrorCode> {
    let value = db
        .query_row(
            &format!("SELECT {COLUMNS} FROM scopes WHERE actor=?1 AND id=?2 AND revision=?3"),
            params![
                actor.uuid().to_string(),
                id.uuid().to_string(),
                revision.uuid().to_string()
            ],
            row,
        )
        .optional()
        .map_err(|_| ErrorCode::Malformed)?
        .ok_or(ErrorCode::Denied)?;
    decode(value)
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, ErrorCode> {
        let mut db = Connection::open(path).map_err(|_| ErrorCode::Unavailable)?;
        db.busy_timeout(Duration::from_secs(2))
            .map_err(|_| ErrorCode::Unavailable)?;
        // Schema check and initialization share admission with concurrent writers.
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Unavailable)?;
        if !check_schema(&tx)? {
            tx.execute_batch(VERSION_SCHEMA)
                .map_err(|_| ErrorCode::Unavailable)?;
            tx.execute("INSERT INTO schema_version VALUES(1)", [])
                .map_err(|_| ErrorCode::Unavailable)?;
            tx.execute_batch(GRANTS_SCHEMA)
                .map_err(|_| ErrorCode::Unavailable)?;
        }
        count(&tx)?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self(db))
    }
    pub fn list(&self, actor: Id) -> Result<Vec<Grant>, ErrorCode> {
        count(&self.0)?;
        let mut query = self
            .0
            .prepare(&format!(
                "SELECT {COLUMNS} FROM scopes WHERE actor=?1 ORDER BY id LIMIT 33"
            ))
            .map_err(|_| ErrorCode::Malformed)?;
        let rows = query
            .query_map([actor.uuid().to_string()], row)
            .map_err(|_| ErrorCode::Malformed)?;
        let mut values = Vec::new();
        for value in rows {
            if values.len() >= MAX_RECORDS {
                return Err(ErrorCode::TooLarge);
            }
            values.push(decode(value.map_err(|_| ErrorCode::Malformed)?)?);
        }
        Ok(values)
    }
    pub fn get(&self, actor: Id, id: Id, revision: Id) -> Result<Grant, ErrorCode> {
        get(&self.0, actor, id, revision)
    }
    /// Actual worker retains this transaction through commit even if an async
    /// waiter disappears. Callback authorizes commit initiation, not rollback.
    pub fn publish(
        &mut self,
        grant: &Grant,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        grant.validate()?;
        let body = serde_json::to_string(grant).map_err(|_| ErrorCode::Malformed)?;
        if body.len() > MAX_BODY {
            return Err(ErrorCode::TooLarge);
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Unavailable)?;
        if count(&tx)? >= MAX_RECORDS {
            return Err(ErrorCode::TooLarge);
        }
        tx.execute(
            "INSERT INTO scopes(id,revision,actor,selection,origin,body) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                grant.id.uuid().to_string(),
                grant.revision.uuid().to_string(),
                grant.actor.uuid().to_string(),
                grant.selection.uuid().to_string(),
                grant.origin.as_str(),
                body
            ],
        )
        .map_err(|_| ErrorCode::Denied)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)
    }
    pub fn revoke(
        &mut self,
        actor: Id,
        id: Id,
        revision: Id,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Unavailable)?;
        get(&tx, actor, id, revision)?;
        if tx
            .execute(
                "DELETE FROM scopes WHERE actor=?1 AND id=?2 AND revision=?3",
                params![
                    actor.uuid().to_string(),
                    id.uuid().to_string(),
                    revision.uuid().to_string()
                ],
            )
            .map_err(|_| ErrorCode::Unavailable)?
            != 1
        {
            return Err(ErrorCode::Stale);
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)
    }
}
