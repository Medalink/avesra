//! Single durable actor envelope. Principal derivation/protection stay native.
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::{path::Path, time::Duration};
use uuid::Uuid;
pub struct ProtectedOwner {
    pub actor: Uuid,
    pub revision: Uuid,
    pub protected: Vec<u8>,
}
fn existing(db: &Connection) -> Result<bool, ErrorCode> {
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
    let version: (i64, i64) = db
        .query_row(
            "SELECT count(*), min(version) FROM schema_version",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ErrorCode::Malformed)?;
    if version != (1, 1) {
        return Err(ErrorCode::Unsupported);
    }
    Ok(true)
}
fn read(db: &Connection) -> Result<Option<ProtectedOwner>, ErrorCode> {
    if !existing(db)? {
        return Err(ErrorCode::Malformed);
    }
    let count: i64 = db
        .query_row("SELECT count(*) FROM owner", [], |r| r.get(0))
        .map_err(|_| ErrorCode::Malformed)?;
    if count != 1 {
        return Err(ErrorCode::Malformed);
    }
    let row: Option<(String,String,Vec<u8>,i64)> = db.query_row("SELECT actor, revision, substr(protected,1,16385), length(protected) FROM owner WHERE singleton=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_| ErrorCode::Malformed)?;
    let Some((actor, revision, protected, length)) = row else {
        return Err(ErrorCode::Malformed);
    };
    let value = ProtectedOwner {
        actor: actor.parse().map_err(|_| ErrorCode::Malformed)?,
        revision: revision.parse().map_err(|_| ErrorCode::Malformed)?,
        protected,
    };
    if value.actor.is_nil()
        || value.revision.is_nil()
        || !(1..=16384).contains(&length)
        || length as usize != value.protected.len()
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(value))
}
pub fn load(path: &Path) -> Result<Option<ProtectedOwner>, ErrorCode> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Ok(value) if value.is_file() && !value.is_symlink() => {}
        _ => return Err(ErrorCode::Unavailable),
    }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| ErrorCode::Unavailable)?;
    db.busy_timeout(Duration::from_secs(2))
        .map_err(|_| ErrorCode::Unavailable)?;
    read(&db)
}
pub fn create(
    path: &Path,
    value: &ProtectedOwner,
    authorize_commit: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<(), ErrorCode> {
    if value.actor.is_nil()
        || value.revision.is_nil()
        || value.protected.is_empty()
        || value.protected.len() > 16384
    {
        return Err(ErrorCode::Malformed);
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("owner.lock"))
        .map_err(|_| ErrorCode::Unavailable)?;
    lock.try_lock().map_err(|_| ErrorCode::Unavailable)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => {
            load(path)?;
            return Err(ErrorCode::Denied);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(ErrorCode::Unavailable),
    }
    let temporary = path.with_extension("owner.pending");
    for stale in [
        temporary.clone(),
        path.with_extension("owner.pending-journal"),
    ] {
        match std::fs::remove_file(stale) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ErrorCode::Unavailable),
        }
    }
    let result = (|| {
        let mut db = Connection::open(&temporary).map_err(|_| ErrorCode::Unavailable)?;
        db.busy_timeout(Duration::from_secs(2))
            .map_err(|_| ErrorCode::Unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Unavailable)?;
        let initialized = existing(&tx)?;
        if initialized && read(&tx)?.is_some() {
            return Err(ErrorCode::Denied);
        }
        if !initialized {
            tx.execute_batch("CREATE TABLE schema_version(version INTEGER NOT NULL); INSERT INTO schema_version VALUES(1); CREATE TABLE owner(singleton INTEGER PRIMARY KEY CHECK(singleton=1), actor TEXT NOT NULL, revision TEXT NOT NULL, protected BLOB NOT NULL);").map_err(|_| ErrorCode::Unavailable)?;
        }
        tx.execute(
            "INSERT INTO owner VALUES(1,?1,?2,?3)",
            params![
                value.actor.to_string(),
                value.revision.to_string(),
                value.protected
            ],
        )
        .map_err(|_| ErrorCode::Unavailable)?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)?;
        drop(db);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|_| ErrorCode::Unavailable)?;
        authorize_commit()?;
        std::fs::hard_link(&temporary, path).map_err(|_| ErrorCode::Unavailable)
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}
