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
            if version != (1, 1) {
                return Err(ErrorCode::Unsupported);
            }
        } else {
            let tx = db.transaction().map_err(|_| ErrorCode::Unavailable)?;
            tx.execute_batch("CREATE TABLE schema_version(version INTEGER NOT NULL); INSERT INTO schema_version VALUES(1); CREATE TABLE apps(id TEXT PRIMARY KEY, revision TEXT UNIQUE NOT NULL, body TEXT NOT NULL, revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)));").map_err(|_| ErrorCode::Unavailable)?;
            tx.commit().map_err(|_| ErrorCode::Unavailable)?;
        }
        Ok(Self(db))
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
