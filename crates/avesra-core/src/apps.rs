//! Immutable native identities, independent of model text and action authority.
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableIdentity {
    pub path: String,
    pub arguments: String,
    pub working_directory: String,
    pub sha256: String,
    pub bytes: u64,
    pub volume: u32,
    pub file_index: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LaunchIdentity {
    Executable(ExecutableIdentity),
    Packaged {
        app_id: String,
        package_full_name: String,
        publisher_id: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppRecord {
    pub id: Uuid,
    pub revision: Uuid,
    pub selected_by: Uuid,
    pub name: String,
    pub source: AppSource,
    #[serde(default)]
    pub source_identity: Option<String>,
    pub publisher: Option<String>,
    pub window_class: Option<String>,
    pub launch: LaunchIdentity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppSource {
    ExplicitExecutable,
    StartMenu,
    WindowsRegistration,
    PackageRegistration,
}
#[derive(Clone, Debug, Serialize)]
pub struct AppAlias {
    pub id: Uuid,
    pub revision: Uuid,
    pub phrase: String,
    pub target: Uuid,
    pub target_revision: Uuid,
    pub selected_by: Uuid,
    pub name: String,
    pub detail: String,
    pub available: bool,
    /// Populated only from the owning execution ledger's validated observation.
    pub last_success_ms: Option<u64>,
}
/// Lookup is a planning input, never an accepted intent or executable grant.
pub struct ResolvedApp {
    pub alias_id: Uuid,
    pub alias_revision: Uuid,
    pub record: AppRecord,
}
/// Canonical lookup key; a phrase never chooses an executable or grants rights.
pub fn alias_phrase(value: &str) -> Result<String, ErrorCode> {
    if value.len() > 256 || value.chars().any(char::is_control) {
        return Err(ErrorCode::Malformed);
    }
    let value = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if value.is_empty()
        || value.len() > 256
        || value.chars().count() > 64
        || !value
            .chars()
            .all(|v| v.is_alphanumeric() || matches!(v, ' ' | '-' | '\''))
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}
const ALIASES_SCHEMA: &str = "CREATE TABLE aliases(id TEXT PRIMARY KEY, revision TEXT UNIQUE NOT NULL, phrase TEXT UNIQUE NOT NULL, target TEXT NOT NULL REFERENCES apps(id), target_revision TEXT NOT NULL, selected_by TEXT NOT NULL);";
fn text(value: &str, max: usize, empty: bool) -> bool {
    (empty || !value.is_empty()) && value.len() <= max && !value.chars().any(char::is_control)
}
pub fn local_path(value: &str) -> bool {
    let value = value.strip_prefix(r"\\?\").unwrap_or(value);
    let bytes = value.as_bytes();
    text(value, 4096, false)
        && bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\'
        && !value.contains('"')
        && !value[3..].contains(':')
        && !value.split('\\').any(|v| matches!(v, "." | ".."))
}
impl AppRecord {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let valid = !self.id.is_nil()
            && !self.revision.is_nil()
            && !self.selected_by.is_nil()
            && text(&self.name, 256, false)
            && self
                .source_identity
                .as_ref()
                .is_none_or(|v| text(v, 4096, false))
            && self.publisher.as_ref().is_none_or(|v| text(v, 512, false))
            && self
                .window_class
                .as_ref()
                .is_none_or(|v| text(v, 256, false))
            && match &self.launch {
                LaunchIdentity::Executable(v) => {
                    local_path(&v.path)
                        && v.path.to_ascii_lowercase().ends_with(".exe")
                        && local_path(&v.working_directory)
                        && text(&v.arguments, 8192, true)
                        && v.bytes > 0
                        && v.bytes <= 1_073_741_824
                        && v.file_index != 0
                        && v.sha256.len() == 64
                        && v.sha256
                            .bytes()
                            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
                }
                LaunchIdentity::Packaged {
                    app_id,
                    package_full_name,
                    publisher_id,
                } => {
                    text(app_id, 512, false)
                        && app_id.contains('!')
                        && text(package_full_name, 512, false)
                        && text(publisher_id, 128, false)
                }
            };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        }
    }
}

/// Owned only by the native effect thread. Registration requires a trusted
/// native owner-selected record; this API is not exposed over IPC or transport.
pub struct AppCatalog(Connection);
impl AppCatalog {
    pub fn open(path: &Path) -> Result<Self, ErrorCode> {
        let mut db = Connection::open(path).map_err(|_| ErrorCode::Unavailable)?;
        db.busy_timeout(Duration::from_secs(2))
            .map_err(|_| ErrorCode::Unavailable)?;
        let tables: i64 = db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Unavailable)?;
        if tables != 0 {
            let version: (i64, i64) = db
                .query_row(
                    "SELECT count(*), min(version) FROM schema_version",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|_| ErrorCode::Malformed)?;
            if !matches!(version, (1, 1) | (1, 2)) {
                return Err(ErrorCode::Unsupported);
            }
            if version == (1, 1) {
                db.prepare("SELECT id,revision,body,revoked FROM apps LIMIT 0")
                    .map_err(|_| ErrorCode::Malformed)?;
                // Exact, transactional upgrade preserves every immutable app.
                let tx = db.transaction().map_err(|_| ErrorCode::Unavailable)?;
                tx.execute_batch(ALIASES_SCHEMA)
                    .map_err(|_| ErrorCode::Malformed)?;
                tx.execute("UPDATE schema_version SET version=2", [])
                    .map_err(|_| ErrorCode::Unavailable)?;
                tx.commit().map_err(|_| ErrorCode::Unavailable)?;
            }
        } else {
            let tx = db.transaction().map_err(|_| ErrorCode::Unavailable)?;
            tx.execute_batch("CREATE TABLE schema_version(version INTEGER NOT NULL); INSERT INTO schema_version VALUES(2); CREATE TABLE apps(id TEXT PRIMARY KEY, revision TEXT UNIQUE NOT NULL, body TEXT NOT NULL, revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)));").map_err(|_| ErrorCode::Unavailable)?;
            tx.execute_batch(ALIASES_SCHEMA)
                .map_err(|_| ErrorCode::Unavailable)?;
            tx.commit().map_err(|_| ErrorCode::Unavailable)?;
        }
        Ok(Self(db))
    }
    pub fn remember(
        &mut self,
        record: &AppRecord,
        phrase: &str,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        record.validate()?;
        let phrase = alias_phrase(phrase)?;
        let body = serde_json::to_string(record).map_err(|_| ErrorCode::Malformed)?;
        if body.len() > 32768 {
            return Err(ErrorCode::TooLarge);
        }
        let tx = self.0.transaction().map_err(|_| ErrorCode::Unavailable)?;
        let count: (i64, i64) = tx
            .query_row(
                "SELECT (SELECT count(*) FROM apps), (SELECT count(*) FROM aliases)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| ErrorCode::Unavailable)?;
        if count.0 >= 512 || count.1 >= 256 {
            return Err(ErrorCode::TooLarge);
        }
        tx.execute(
            "INSERT INTO apps(id,revision,body) VALUES(?1,?2,?3)",
            params![record.id.to_string(), record.revision.to_string(), body],
        )
        .map_err(|_| ErrorCode::Denied)?;
        tx.execute(
            "INSERT INTO aliases VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                phrase,
                record.id.to_string(),
                record.revision.to_string(),
                record.selected_by.to_string()
            ],
        )
        .map_err(|_| ErrorCode::Denied)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)
    }
    pub fn aliases(&self) -> Result<Vec<AppAlias>, ErrorCode> {
        let mut statement = self.0.prepare("SELECT substr(id,1,37),substr(revision,1,37),substr(phrase,1,257),substr(target,1,37),substr(target_revision,1,37),substr(selected_by,1,37) FROM aliases ORDER BY phrase LIMIT 257").map_err(|_|ErrorCode::Malformed)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Unavailable)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(|_| ErrorCode::Malformed)? {
            if result.len() >= 256 {
                return Err(ErrorCode::TooLarge);
            }
            let parse = |index| -> Result<Uuid, ErrorCode> {
                let value: String = row.get(index).map_err(|_| ErrorCode::Malformed)?;
                let id: Uuid = value.parse().map_err(|_| ErrorCode::Malformed)?;
                if id.is_nil() || id.to_string() != value {
                    return Err(ErrorCode::Malformed);
                }
                Ok(id)
            };
            let phrase: String = row.get(2).map_err(|_| ErrorCode::Malformed)?;
            if alias_phrase(&phrase)? != phrase {
                return Err(ErrorCode::Malformed);
            }
            let mut value = AppAlias {
                id: parse(0)?,
                revision: parse(1)?,
                phrase,
                target: parse(3)?,
                target_revision: parse(4)?,
                selected_by: parse(5)?,
                name: String::new(),
                detail: String::new(),
                available: false,
                last_success_ms: None,
            };
            let (body, length, revoked): (String,i64,i64) = self.0.query_row("SELECT substr(body,1,32769),length(CAST(body AS BLOB)),revoked FROM apps WHERE id=?1 AND revision=?2",params![value.target.to_string(),value.target_revision.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|ErrorCode::Malformed)?;
            if length > 32768 || !matches!(revoked, 0 | 1) {
                return Err(ErrorCode::Malformed);
            }
            let app: AppRecord = serde_json::from_str(&body).map_err(|_| ErrorCode::Malformed)?;
            app.validate()?;
            if app.id != value.target
                || app.revision != value.target_revision
                || app.selected_by != value.selected_by
            {
                return Err(ErrorCode::Malformed);
            }
            value.name = app.name;
            value.detail = match app.launch {
                LaunchIdentity::Executable(v) => v.path,
                LaunchIdentity::Packaged { app_id, .. } => app_id,
            };
            value.available = revoked == 0;
            result.push(value);
        }
        Ok(result)
    }
    pub fn resolve(&self, actor: Uuid, phrase: &str) -> Result<ResolvedApp, ErrorCode> {
        if actor.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let phrase = alias_phrase(phrase)?;
        let mut matches = self.aliases()?.into_iter().filter(|v| v.phrase == phrase);
        let alias = matches.next().ok_or(ErrorCode::Denied)?;
        if matches.next().is_some() {
            return Err(ErrorCode::Malformed);
        }
        if alias.selected_by != actor || !alias.available {
            return Err(ErrorCode::Denied);
        }
        let record = self.get(alias.target)?;
        if record.revision != alias.target_revision || record.selected_by != actor {
            return Err(ErrorCode::Stale);
        }
        Ok(ResolvedApp {
            alias_id: alias.id,
            alias_revision: alias.revision,
            record,
        })
    }
    pub fn forget(
        &mut self,
        id: Uuid,
        revision: Uuid,
        actor: Uuid,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if id.is_nil() || revision.is_nil() || actor.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let tx = self.0.transaction().map_err(|_| ErrorCode::Unavailable)?;
        if tx
            .execute(
                "DELETE FROM aliases WHERE id=?1 AND revision=?2 AND selected_by=?3",
                params![id.to_string(), revision.to_string(), actor.to_string()],
            )
            .map_err(|_| ErrorCode::Unavailable)?
            != 1
        {
            return Err(ErrorCode::Stale);
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)
    }
    pub fn register(&mut self, record: &AppRecord) -> Result<(), ErrorCode> {
        record.validate()?;
        let body = serde_json::to_string(record).map_err(|_| ErrorCode::Malformed)?;
        if body.len() > 32768 {
            return Err(ErrorCode::TooLarge);
        }
        let tx = self.0.transaction().map_err(|_| ErrorCode::Unavailable)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM apps", [], |r| r.get(0))
            .map_err(|_| ErrorCode::Unavailable)?;
        if count >= 512 {
            return Err(ErrorCode::TooLarge);
        }
        tx.execute(
            "INSERT INTO apps(id,revision,body) VALUES(?1,?2,?3)",
            params![record.id.to_string(), record.revision.to_string(), body],
        )
        .map_err(|_| ErrorCode::Denied)?;
        tx.commit().map_err(|_| ErrorCode::Unavailable)
    }
    pub fn get(&self, id: Uuid) -> Result<AppRecord, ErrorCode> {
        if id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let row: Option<(String, String)> = self
            .0
            .query_row(
                "SELECT revision,body FROM apps WHERE id=?1 AND revoked=0",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| ErrorCode::Unavailable)?;
        let (revision, body) = row.ok_or(ErrorCode::Denied)?;
        if body.len() > 32768 {
            return Err(ErrorCode::TooLarge);
        }
        let record: AppRecord = serde_json::from_str(&body).map_err(|_| ErrorCode::Malformed)?;
        record.validate()?;
        if record.id != id || record.revision.to_string() != revision {
            return Err(ErrorCode::Malformed);
        }
        Ok(record)
    }
    pub fn revoke(&mut self, id: Uuid) -> Result<(), ErrorCode> {
        if id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        if self
            .0
            .execute("UPDATE apps SET revoked=1 WHERE id=?1", [id.to_string()])
            .map_err(|_| ErrorCode::Unavailable)?
            != 1
        {
            return Err(ErrorCode::Denied);
        }
        Ok(())
    }
}
