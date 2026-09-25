use crate::state::Settings;
use avesra_contracts::{ErrorCode, TaskState};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use uuid::Uuid;

/// One process owns this connection; callers serialize access.
pub struct Store {
    connection: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, ErrorCode> {
        let mut connection = Connection::open(path).map_err(|_| ErrorCode::Storage)?;
        let has_schema:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_version')",[],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
        if has_schema {
            let version: Option<i64> = connection
                .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
                .map_err(|_| ErrorCode::Storage)?;
            if version != Some(1) {
                return Err(ErrorCode::Unsupported);
            }
        }
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;",
            )
            .map_err(|_| ErrorCode::Storage)?;
        let tx = connection.transaction().map_err(|_| ErrorCode::Storage)?;
        tx.execute_batch("
          CREATE TABLE IF NOT EXISTS schema_version(version INTEGER PRIMARY KEY);
          INSERT OR IGNORE INTO schema_version VALUES(1);
          CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS tasks(id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, state TEXT NOT NULL, updated_ms INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS steps(id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), state TEXT NOT NULL, target_id TEXT NOT NULL, operation TEXT NOT NULL, updated_ms INTEGER NOT NULL);
        ").map_err(|_|ErrorCode::Storage)?;
        // A crash cannot prove a running mutation failed or succeeded.
        tx.execute(
            "UPDATE steps SET state='\"unknown_effect\"' WHERE state='\"running\"'",
            [],
        )
        .map_err(|_| ErrorCode::Storage)?;
        tx.execute(
            "UPDATE tasks SET state='\"unknown_effect\"' WHERE state='\"running\"'",
            [],
        )
        .map_err(|_| ErrorCode::Storage)?;
        tx.execute("UPDATE tasks SET state='\"suspended\"' WHERE state IN ('\"queued\"','\"waiting_for_user\"')",[]).map_err(|_|ErrorCode::Storage)?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(Self { connection })
    }
    pub fn settings(&self) -> Result<Settings, ErrorCode> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT value FROM settings WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| ErrorCode::Storage)?;
        let settings = match value {
            Some(value) => serde_json::from_str(&value).map_err(|_| ErrorCode::Malformed)?,
            None => Settings::default(),
        };
        settings.validate()?;
        Ok(settings)
    }
    pub fn save_settings(&mut self, settings: &Settings) -> Result<(), ErrorCode> {
        settings.validate()?;
        let value = serde_json::to_string(settings).map_err(|_| ErrorCode::Malformed)?;
        self.connection.execute("INSERT INTO settings(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[value]).map_err(|_|ErrorCode::Storage)?;
        Ok(())
    }
    pub fn create_task(&mut self, id: Uuid, actor_id: Uuid, now_ms: u64) -> Result<(), ErrorCode> {
        let now_ms = i64::try_from(now_ms).map_err(|_| ErrorCode::Malformed)?;
        if id.is_nil() || actor_id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        self.connection
            .execute(
                "INSERT INTO tasks(id,actor_id,state,updated_ms) VALUES(?1,?2,?3,?4)",
                params![
                    id.to_string(),
                    actor_id.to_string(),
                    serde_json::to_string(&TaskState::Proposed)
                        .map_err(|_| ErrorCode::Malformed)?,
                    now_ms
                ],
            )
            .map_err(|_| ErrorCode::Storage)?;
        Ok(())
    }
    pub fn transition(&mut self, id: Uuid, next: TaskState, now_ms: u64) -> Result<(), ErrorCode> {
        let now_ms = i64::try_from(now_ms).map_err(|_| ErrorCode::Malformed)?;
        let tx = self
            .connection
            .transaction()
            .map_err(|_| ErrorCode::Storage)?;
        let current: String = tx
            .query_row(
                "SELECT state FROM tasks WHERE id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Storage)?;
        let current: TaskState =
            serde_json::from_str(&current).map_err(|_| ErrorCode::Malformed)?;
        if !current.can_transition_to(next) {
            return Err(ErrorCode::InvalidTransition);
        }
        tx.execute(
            "UPDATE tasks SET state=?2,updated_ms=?3 WHERE id=?1",
            params![
                id.to_string(),
                serde_json::to_string(&next).map_err(|_| ErrorCode::Malformed)?,
                now_ms
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
}
