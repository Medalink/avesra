//! Immutable native registration intent metadata; it creates no actor or grant.
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::path::Path;
use uuid::Uuid;
const SCHEMA: &str = "CREATE TABLE actor_intents(server TEXT NOT NULL,device TEXT NOT NULL,actor TEXT NOT NULL,owner_revision TEXT NOT NULL,request TEXT UNIQUE NOT NULL,PRIMARY KEY(server,device))";
#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    pub server: String,
    pub device: Uuid,
    pub actor: Uuid,
    pub owner_revision: Uuid,
}
impl Identity {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.server.len() != 64
            || !self
                .server
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
            || [self.device, self.actor, self.owner_revision]
                .iter()
                .any(Uuid::is_nil)
        {
            Err(ErrorCode::Malformed)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone)]
pub struct Intent {
    pub identity: Identity,
    pub request: Uuid,
}
fn schema(db: &Connection) -> Result<bool, ErrorCode> {
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|_| ErrorCode::Storage)?;
    if version == 0 {
        let occupied: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name NOT GLOB 'sqlite_*')",
                [],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Storage)?;
        return if occupied {
            Err(ErrorCode::Malformed)
        } else {
            Ok(false)
        };
    }
    if version != 1 {
        return Err(ErrorCode::Unsupported);
    }
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
        .map_err(|_| ErrorCode::Malformed)?;
    if count != 3 {
        return Err(ErrorCode::Malformed);
    }
    let sql:String=db.query_row("SELECT substr(sql,1,1025) FROM sqlite_master WHERE type='table' AND name='actor_intents'",[],|r|r.get(0)).map_err(|_|ErrorCode::Malformed)?;
    if sql != SCHEMA {
        return Err(ErrorCode::Malformed);
    }
    let mut statement = db
        .prepare("PRAGMA index_list('actor_intents')")
        .map_err(|_| ErrorCode::Malformed)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
    let mut found = [false; 2];
    while let Some(row) = rows.next().map_err(|_| ErrorCode::Malformed)? {
        let (name, unique, origin, partial): (String, i64, String, i64) = (
            row.get(1).map_err(|_| ErrorCode::Malformed)?,
            row.get(2).map_err(|_| ErrorCode::Malformed)?,
            row.get(3).map_err(|_| ErrorCode::Malformed)?,
            row.get(4).map_err(|_| ErrorCode::Malformed)?,
        );
        let index = match (name.as_str(), origin.as_str()) {
            ("sqlite_autoindex_actor_intents_1", "u") => 0,
            ("sqlite_autoindex_actor_intents_2", "pk") => 1,
            _ => return Err(ErrorCode::Malformed),
        };
        if found[index] || unique != 1 || partial != 0 {
            return Err(ErrorCode::Malformed);
        }
        found[index] = true;
    }
    if found != [true; 2] {
        return Err(ErrorCode::Malformed);
    }
    for (query, expected) in [
        (
            "PRAGMA index_info('sqlite_autoindex_actor_intents_1')",
            vec![(4i64, "request")],
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_actor_intents_2')",
            vec![(0, "server"), (1, "device")],
        ),
    ] {
        let mut statement = db.prepare(query).map_err(|_| ErrorCode::Malformed)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
        for (sequence, (column, name)) in expected.into_iter().enumerate() {
            let row = rows
                .next()
                .map_err(|_| ErrorCode::Malformed)?
                .ok_or(ErrorCode::Malformed)?;
            let actual: (i64, i64, String) = (
                row.get(0).map_err(|_| ErrorCode::Malformed)?,
                row.get(1).map_err(|_| ErrorCode::Malformed)?,
                row.get(2).map_err(|_| ErrorCode::Malformed)?,
            );
            if actual != (sequence as i64, column, name.to_owned()) {
                return Err(ErrorCode::Malformed);
            }
        }
        if rows.next().map_err(|_| ErrorCode::Malformed)?.is_some() {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(true)
}
fn read(db: &Connection, identity: &Identity) -> Result<Option<Intent>, ErrorCode> {
    let row:Option<(String,String,String)>=db.query_row("SELECT substr(actor,1,37),substr(owner_revision,1,37),substr(request,1,37) FROM actor_intents WHERE server=?1 AND device=?2",params![identity.server,identity.device.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_|ErrorCode::Storage)?;
    let Some((actor, revision, request)) = row else {
        return Ok(None);
    };
    let request_id = Uuid::parse_str(&request).map_err(|_| ErrorCode::Malformed)?;
    if actor != identity.actor.to_string()
        || revision != identity.owner_revision.to_string()
        || request_id.is_nil()
        || request_id.to_string() != request
    {
        return Err(ErrorCode::Stale);
    }
    Ok(Some(Intent {
        identity: identity.clone(),
        request: request_id,
    }))
}
pub fn load(path: &Path, identity: &Identity) -> Result<Option<Intent>, ErrorCode> {
    identity.validate()?;
    if !path.try_exists().map_err(|_| ErrorCode::Storage)? {
        return Ok(None);
    }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| ErrorCode::Storage)?;
    db.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| ErrorCode::Storage)?;
    if !schema(&db)? {
        return Ok(None);
    }
    read(&db, identity)
}
/// Actual native owner-management coordinator serializes this store. Commit a
/// request before transmission; retries preserve it and never overwrite a row.
pub fn remember(
    path: &Path,
    identity: &Identity,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Intent, ErrorCode> {
    identity.validate()?;
    let mut db = Connection::open(path).map_err(|_| ErrorCode::Storage)?;
    db.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| ErrorCode::Storage)?;
    let initialized = schema(&db)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ErrorCode::Storage)?;
    if !initialized {
        tx.execute_batch(SCHEMA).map_err(|_| ErrorCode::Storage)?;
        tx.execute_batch("PRAGMA user_version=1")
            .map_err(|_| ErrorCode::Storage)?;
        schema(&tx)?;
    }
    let result = match read(&tx, identity)? {
        Some(value) => value,
        None => {
            let count: i64 = tx
                .query_row("SELECT COUNT(*) FROM actor_intents", [], |r| r.get(0))
                .map_err(|_| ErrorCode::Storage)?;
            if count >= 32 {
                return Err(ErrorCode::TooLarge);
            }
            let request = Uuid::new_v4();
            tx.execute("INSERT INTO actor_intents(server,device,actor,owner_revision,request) VALUES(?1,?2,?3,?4,?5)",params![identity.server,identity.device.to_string(),identity.actor.to_string(),identity.owner_revision.to_string(),request.to_string()]).map_err(|_|ErrorCode::Storage)?;
            let value = read(&tx, identity)?.ok_or(ErrorCode::Malformed)?;
            if value.request != request {
                return Err(ErrorCode::Malformed);
            }
            value
        }
    };
    authorize()?;
    tx.commit().map_err(|_| ErrorCode::Storage)?;
    Ok(result)
}
