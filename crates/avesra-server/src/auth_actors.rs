//! Device-bound immutable native owner registration; no inference or tool grant.
#[cfg(unix)]
use super::AuthStore;
#[cfg(unix)]
use avesra_contracts::{
    ErrorCode,
    actors::{Binding, Command, Request},
};
use rusqlite::Connection;
#[cfg(unix)]
use rusqlite::{OptionalExtension, params};
#[cfg(unix)]
use uuid::Uuid;
pub(super) const SCHEMA: &str = "CREATE TABLE actors(device TEXT PRIMARY KEY NOT NULL REFERENCES devices(id),actor TEXT NOT NULL,owner_revision TEXT NOT NULL,registration_revision TEXT UNIQUE NOT NULL,registered_by TEXT UNIQUE NOT NULL,revoked INTEGER NOT NULL CHECK(revoked IN (0,1)))";
pub(super) fn check_schema(db: &Connection, version: i64) -> Result<(), String> {
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='actors' OR tbl_name='actors'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Actor schema unavailable")?;
    if version < 2 {
        return if count == 0 {
            Ok(())
        } else {
            Err("Unexpected actor schema".into())
        };
    }
    if count != 4 {
        return Err("Malformed actor schema".into());
    }
    let schema: String = db
        .query_row(
            "SELECT substr(sql,1,2049) FROM sqlite_master WHERE type='table' AND name='actors'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Actor schema unavailable")?;
    if schema != SCHEMA {
        return Err("Unsupported actor schema".into());
    }
    let mut stmt = db
        .prepare("PRAGMA index_list('actors')")
        .map_err(|_| "Actor schema unavailable")?;
    let mut rows = stmt.query([]).map_err(|_| "Actor schema unavailable")?;
    let mut seen = [false; 3];
    while let Some(row) = rows.next().map_err(|_| "Actor schema unavailable")? {
        let (name, unique, origin, partial): (String, i64, String, i64) = (
            row.get(1).map_err(|_| "Actor schema unavailable")?,
            row.get(2).map_err(|_| "Actor schema unavailable")?,
            row.get(3).map_err(|_| "Actor schema unavailable")?,
            row.get(4).map_err(|_| "Actor schema unavailable")?,
        );
        let index = match (name.as_str(), origin.as_str()) {
            ("sqlite_autoindex_actors_1", "pk") => 0,
            ("sqlite_autoindex_actors_2", "u") => 1,
            ("sqlite_autoindex_actors_3", "u") => 2,
            _ => return Err("Malformed actor indexes".into()),
        };
        if seen[index] || unique != 1 || partial != 0 {
            return Err("Malformed actor indexes".into());
        }
        seen[index] = true;
    }
    if seen != [true; 3] {
        return Err("Missing actor indexes".into());
    }
    for (query, column, name) in [
        (
            "PRAGMA index_info('sqlite_autoindex_actors_1')",
            0i64,
            "device",
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_actors_2')",
            3,
            "registration_revision",
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_actors_3')",
            4,
            "registered_by",
        ),
    ] {
        let mut stmt = db.prepare(query).map_err(|_| "Actor index unavailable")?;
        let mut rows = stmt.query([]).map_err(|_| "Actor index unavailable")?;
        let row = rows
            .next()
            .map_err(|_| "Actor index unavailable")?
            .ok_or("Actor index missing")?;
        let actual: (i64, i64, String) = (
            row.get(0).map_err(|_| "Actor index unavailable")?,
            row.get(1).map_err(|_| "Actor index unavailable")?,
            row.get(2).map_err(|_| "Actor index unavailable")?,
        );
        if actual != (0, column, name.to_owned())
            || rows
                .next()
                .map_err(|_| "Actor index unavailable")?
                .is_some()
        {
            return Err("Malformed actor index".into());
        }
    }
    Ok(())
}
#[cfg(unix)]
fn read(db: &Connection, device: Uuid) -> Result<Option<Binding>, ErrorCode> {
    let row:Option<(String,String,String,String,i64)>=db.query_row("SELECT substr(actor,1,37),substr(owner_revision,1,37),substr(registration_revision,1,37),substr(registered_by,1,37),revoked FROM actors WHERE device=?1",[device.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(|_|ErrorCode::Storage)?;
    let Some((actor, owner, revision, request, revoked)) = row else {
        return Ok(None);
    };
    if !matches!(revoked, 0 | 1) {
        return Err(ErrorCode::Malformed);
    }
    let parse = |v: &str| -> Result<Uuid, ErrorCode> {
        let id = Uuid::parse_str(v).map_err(|_| ErrorCode::Malformed)?;
        if id.to_string() != v {
            return Err(ErrorCode::Malformed);
        }
        Ok(id)
    };
    let value = Binding {
        device,
        actor: parse(&actor)?,
        owner_revision: parse(&owner)?,
        registration_revision: parse(&revision)?,
        registered_by: parse(&request)?,
        revoked: revoked == 1,
    };
    value.validate()?;
    Ok(Some(value))
}
#[cfg(unix)]
impl AuthStore {
    pub fn actor_operation(
        &mut self,
        device: Uuid,
        request: &Request,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Option<Binding>, ErrorCode> {
        request.validate()?;
        if device.is_nil() || !self.active(device).map_err(|_| ErrorCode::Storage)? {
            return Err(ErrorCode::Unauthenticated);
        }
        let tx = self
            .connection
            .transaction()
            .map_err(|_| ErrorCode::Storage)?;
        match request.command {
            Command::Register {
                actor,
                owner_revision,
            } => {
                if read(&tx, device)?.is_some() {
                    return Err(ErrorCode::Denied);
                }
                let revision = Uuid::new_v4();
                tx.execute("INSERT INTO actors(device,actor,owner_revision,registration_revision,registered_by,revoked) VALUES(?1,?2,?3,?4,?5,0)",params![device.to_string(),actor.to_string(),owner_revision.to_string(),revision.to_string(),request.request.to_string()]).map_err(|_|ErrorCode::Storage)?;
            }
            Command::Status => {}
            Command::Revoke {
                actor,
                registration_revision,
            } => {
                let prior = read(&tx, device)?.ok_or(ErrorCode::Stale)?;
                if prior.actor != actor || prior.registration_revision != registration_revision {
                    return Err(ErrorCode::Stale);
                }
                if tx.execute("UPDATE actors SET revoked=1 WHERE device=?1 AND actor=?2 AND registration_revision=?3",params![device.to_string(),actor.to_string(),registration_revision.to_string()]).map_err(|_|ErrorCode::Storage)?!=1{return Err(ErrorCode::Stale);}
            }
        }
        let result = read(&tx, device)?;
        let active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE id=?1 AND revoked=0)",
                [device.to_string()],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Storage)?;
        if !active {
            return Err(ErrorCode::Unauthenticated);
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(result)
    }
}
