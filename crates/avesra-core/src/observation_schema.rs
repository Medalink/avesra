//! Exact layouts newly relied on by schema 10 read finalization. No row scan.
use avesra_contracts::ErrorCode;
use rusqlite::Connection;

fn sql<T>(value: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    value.map_err(|_| ErrorCode::Storage)
}
pub(crate) fn check(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    table(
        db,
        version,
        Layout {
            introduced: 3,
            name: "native_observations",
            expected: "CREATE TABLE native_observations(dispatch_id TEXT PRIMARY KEY REFERENCES dispatch_bindings(dispatch_id), target_id TEXT NOT NULL, action_revision TEXT NOT NULL REFERENCES action_revisions(revision), body TEXT NOT NULL, at_ms INTEGER NOT NULL)",
            index: "native_observation_target",
            index_sql: "CREATE INDEX native_observation_target ON native_observations(target_id)",
            columns: &[(1, "target_id")],
        },
    )?;
    table(
        db,
        version,
        Layout {
            introduced: 4,
            name: "native_finalizations",
            expected: "CREATE TABLE native_finalizations(dispatch_id TEXT PRIMARY KEY REFERENCES dispatch_bindings(dispatch_id), target_id TEXT NOT NULL, action_revision TEXT NOT NULL REFERENCES action_revisions(revision), actor_id TEXT NOT NULL, outcome TEXT NOT NULL, at_ms INTEGER NOT NULL)",
            index: "native_finalization_lookup",
            index_sql: "CREATE INDEX native_finalization_lookup ON native_finalizations(target_id,actor_id,outcome)",
            columns: &[(1, "target_id"), (3, "actor_id"), (4, "outcome")],
        },
    )
}
struct Layout<'a> {
    introduced: u64,
    name: &'a str,
    expected: &'a str,
    index: &'a str,
    index_sql: &'a str,
    columns: &'a [(i64, &'a str)],
}
fn table(db: &Connection, version: u64, layout: Layout<'_>) -> Result<(), ErrorCode> {
    let Layout {
        introduced,
        name,
        expected,
        index,
        index_sql,
        columns,
    } = layout;
    let automatic = format!("sqlite_autoindex_{name}_1");
    let mut query = sql(db.prepare("SELECT substr(CAST(type AS BLOB),1,17),substr(CAST(name AS BLOB),1,129),substr(CAST(sql AS BLOB),1,2049) FROM sqlite_master WHERE name=?1 OR tbl_name=?1 OR name=?2 LIMIT 4"))?;
    let mut rows = sql(query.query([name, index]))?;
    let mut found = [false; 3];
    while let Some(row) = sql(rows.next())? {
        if version < introduced {
            return Err(ErrorCode::Malformed);
        }
        let kind: Vec<u8> = sql(row.get(0))?;
        let actual: Vec<u8> = sql(row.get(1))?;
        let body: Option<Vec<u8>> = sql(row.get(2))?;
        let slot = if kind == b"table"
            && actual == name.as_bytes()
            && body.as_deref() == Some(expected.as_bytes())
        {
            0
        } else if kind == b"index" && actual == automatic.as_bytes() && body.is_none() {
            1
        } else if kind == b"index"
            && actual == index.as_bytes()
            && body.as_deref() == Some(index_sql.as_bytes())
        {
            2
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
    if found != [true; 3] {
        return Err(ErrorCode::Malformed);
    }
    let mut query = sql(db.prepare("SELECT substr(CAST(name AS BLOB),1,129),\"unique\",origin,partial FROM pragma_index_list(?1) LIMIT 3"))?;
    let mut rows = sql(query.query([name]))?;
    let mut seen = [false; 2];
    while let Some(row) = sql(rows.next())? {
        let actual: Vec<u8> = sql(row.get(0))?;
        let slot = if actual == automatic.as_bytes() {
            0
        } else if actual == index.as_bytes() {
            1
        } else {
            return Err(ErrorCode::Malformed);
        };
        if seen[slot]
            || sql::<i64>(row.get(1))? != if slot == 0 { 1 } else { 0 }
            || sql::<String>(row.get(2))? != if slot == 0 { "pk" } else { "c" }
            || sql::<i64>(row.get(3))? != 0
        {
            return Err(ErrorCode::Malformed);
        }
        seen[slot] = true;
    }
    if seen != [true; 2] {
        return Err(ErrorCode::Malformed);
    }
    for (index, columns) in [
        (automatic.as_str(), &[(0, "dispatch_id")][..]),
        (index, columns),
    ] {
        let mut query = sql(db.prepare(
            "SELECT seqno,cid,substr(CAST(name AS BLOB),1,129) FROM pragma_index_info(?1) LIMIT 4",
        ))?;
        let mut rows = sql(query.query([index]))?;
        for (position, (column, name)) in columns.iter().enumerate() {
            let row = sql(rows.next())?.ok_or(ErrorCode::Malformed)?;
            if sql::<i64>(row.get(0))? != position as i64
                || sql::<i64>(row.get(1))? != *column
                || sql::<Vec<u8>>(row.get(2))? != name.as_bytes()
            {
                return Err(ErrorCode::Malformed);
            }
        }
        if sql(rows.next())?.is_some() {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
