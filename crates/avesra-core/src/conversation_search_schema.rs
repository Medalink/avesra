//! Exact application schema plus the fixed shadow layout emitted by bundled FTS5.
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OptionalExtension};
const DOCS: &str = "CREATE TABLE conversation_search_docs(id INTEGER PRIMARY KEY,turn TEXT UNIQUE NOT NULL REFERENCES accepted_conversations(id),revision TEXT NOT NULL,actor TEXT NOT NULL,device TEXT NOT NULL,task TEXT,app TEXT)";
const APPS: &str =
    "CREATE INDEX conversation_search_app ON conversation_search_docs(actor,device,app,id)";
const PROGRESS: &str = "CREATE TABLE conversation_search_progress(actor TEXT NOT NULL,device TEXT NOT NULL,through INTEGER NOT NULL CHECK(through>=0),revision INTEGER NOT NULL CHECK(revision>0),PRIMARY KEY(actor,device))";
const FTS: &str = "CREATE VIRTUAL TABLE conversation_fts USING fts5(owner_scope,original,response,tokenize='unicode61')";
const TABLES: [(&str, &str); 5] = [
    ("conversation_search_docs", DOCS),
    ("conversation_search_progress", PROGRESS),
    ("conversation_search_app", APPS),
    ("conversation_fts", FTS),
    (
        "conversation_search_owner",
        "CREATE INDEX conversation_search_owner ON accepted_conversations(actor,device)",
    ),
];
pub(crate) fn create(db: &Connection) -> Result<(), ErrorCode> {
    for (_, sql) in TABLES {
        db.execute_batch(sql).map_err(|_| ErrorCode::Storage)?;
    }
    db.execute(
        "INSERT INTO conversation_fts(conversation_fts,rank) VALUES('secure-delete',1)",
        [],
    )
    .map_err(|_| ErrorCode::Unsupported)?;
    Ok(())
}
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    for (name, sql) in TABLES {
        let actual: Option<String> = db
            .query_row(
                "SELECT substr(sql,1,2049) FROM sqlite_master WHERE name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::Storage)?;
        if (version < 30 && actual.is_some()) || (version >= 30 && actual.as_deref() != Some(sql)) {
            return Err(ErrorCode::Malformed);
        }
    }
    let names = [
        "conversation_fts_data",
        "conversation_fts_idx",
        "conversation_fts_content",
        "conversation_fts_docsize",
        "conversation_fts_config",
    ];
    let definitions = [
        "id INTEGER PRIMARY KEY, block BLOB",
        "segid, term, pgno, PRIMARY KEY(segid, term)",
        "id INTEGER PRIMARY KEY, c0, c1, c2",
        "id INTEGER PRIMARY KEY, sz BLOB",
        "k PRIMARY KEY, v",
    ];
    for (index, name) in names.iter().enumerate() {
        let actual: Option<String> = db
            .query_row(
                "SELECT substr(sql,1,2049) FROM sqlite_master WHERE name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::Storage)?;
        let suffix = if index == 1 || index == 4 {
            " WITHOUT ROWID"
        } else {
            ""
        };
        let expected = format!("CREATE TABLE '{name}'({}){suffix}", definitions[index]);
        if (version < 30 && actual.is_some())
            || (version >= 30 && actual.as_deref() != Some(&expected))
        {
            return Err(ErrorCode::Malformed);
        }
    }
    let count:i64=db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name GLOB 'conversation_fts*' OR tbl_name GLOB 'conversation_fts*' OR name GLOB 'conversation_search_*' OR tbl_name GLOB 'conversation_search_*'",[],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
    if count != if version >= 30 { 12 } else { 0 } {
        return Err(ErrorCode::Malformed);
    }
    if version >= 30 {
        let secure: i64 = db
            .query_row(
                "SELECT v FROM conversation_fts_config WHERE k='secure-delete'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Malformed)?;
        if secure != 1 {
            return Err(ErrorCode::Malformed);
        }
        let mut owner = db
            .prepare("PRAGMA index_info('conversation_search_owner')")
            .map_err(|_| ErrorCode::Storage)?;
        let columns = owner
            .query_map([], |r| Ok((r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))
            .map_err(|_| ErrorCode::Storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ErrorCode::Storage)?;
        if columns != vec![(2, "actor".to_owned()), (3, "device".to_owned())] {
            return Err(ErrorCode::Malformed);
        }
        for (table, expected) in [
            (
                "conversation_search_docs",
                vec![
                    (
                        "sqlite_autoindex_conversation_search_docs_1",
                        1,
                        "u",
                        vec![(1, "turn")],
                    ),
                    (
                        "conversation_search_app",
                        0,
                        "c",
                        vec![(3, "actor"), (4, "device"), (6, "app"), (0, "id")],
                    ),
                ],
            ),
            (
                "conversation_search_progress",
                vec![(
                    "sqlite_autoindex_conversation_search_progress_1",
                    1,
                    "pk",
                    vec![(0, "actor"), (1, "device")],
                )],
            ),
        ] {
            let mut statement = db
                .prepare(&format!("PRAGMA index_list('{table}')"))
                .map_err(|_| ErrorCode::Storage)?;
            let mut rows = statement.query([]).map_err(|_| ErrorCode::Storage)?;
            let mut seen = Vec::new();
            while let Some(row) = rows.next().map_err(|_| ErrorCode::Storage)? {
                let (name, unique, origin, partial): (String, i64, String, i64) = (
                    row.get(1).map_err(|_| ErrorCode::Malformed)?,
                    row.get(2).map_err(|_| ErrorCode::Malformed)?,
                    row.get(3).map_err(|_| ErrorCode::Malformed)?,
                    row.get(4).map_err(|_| ErrorCode::Malformed)?,
                );
                let entry = expected
                    .iter()
                    .find(|e| e.0 == name)
                    .ok_or(ErrorCode::Malformed)?;
                if unique != entry.1 || origin != entry.2 || partial != 0 || seen.contains(&name) {
                    return Err(ErrorCode::Malformed);
                }
                let mut info = db
                    .prepare(&format!("PRAGMA index_info('{}')", entry.0))
                    .map_err(|_| ErrorCode::Storage)?;
                let values = info
                    .query_map([], |r| Ok((r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))
                    .map_err(|_| ErrorCode::Storage)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ErrorCode::Storage)?;
                if values
                    != entry
                        .3
                        .iter()
                        .map(|(n, v)| (*n, v.to_string()))
                        .collect::<Vec<_>>()
                {
                    return Err(ErrorCode::Malformed);
                }
                seen.push(name);
            }
            if seen.len() != expected.len() {
                return Err(ErrorCode::Malformed);
            }
        }
    }
    Ok(())
}
