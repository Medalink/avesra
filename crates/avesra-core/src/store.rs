use crate::state::Settings;
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

/// One process owns this connection; callers serialize access.
pub struct Store {
    pub(crate) connection: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, ErrorCode> {
        let mut connection = Connection::open(path).map_err(|_| ErrorCode::Storage)?;
        let has_schema:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_version')",[],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
        let version = if has_schema {
            let (count, version): (i64, Option<i64>) = connection
                .query_row(
                    "SELECT COUNT(*), MAX(version) FROM schema_version",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|_| ErrorCode::Storage)?;
            if count != 1 || !matches!(version, Some(1..=29)) {
                return Err(ErrorCode::Unsupported);
            }
            version.ok_or(ErrorCode::Unsupported)? as u64
        } else {
            let occupied: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name NOT GLOB 'sqlite_*')",
                    [],
                    |r| r.get(0),
                )
                .map_err(|_| ErrorCode::Storage)?;
            if occupied {
                return Err(ErrorCode::Unsupported);
            }
            0
        };
        crate::conversations::check_schema(&connection, version)?;
        crate::browser_jobs::check_schema(&connection, version)?;
        crate::observation_schema::check(&connection, version)?;
        crate::action_permissions::check_schema(&connection, version)?;
        crate::memory::check_schema(&connection, version)?;
        crate::demonstration::check_schema(&connection, version)?;
        crate::demonstration::passive::check_schema(&connection, version)?;
        crate::notifications::check_schema(&connection, version)?;
        if version >= 8 {
            crate::browser_jobs::current(&connection)?;
        }
        connection
            .pragma_update(None, "foreign_keys", version >= 23)
            .map_err(|_| ErrorCode::Storage)?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|_| ErrorCode::Storage)?;
        let tx = connection.transaction().map_err(|_| ErrorCode::Storage)?;
        tx.execute_batch("
          CREATE TABLE IF NOT EXISTS schema_version(version INTEGER PRIMARY KEY);
          INSERT OR IGNORE INTO schema_version VALUES(1);
          CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS tasks(id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, state TEXT NOT NULL, updated_ms INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS steps(id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), state TEXT NOT NULL, target_id TEXT NOT NULL, operation TEXT NOT NULL, updated_ms INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS accepted_intents(task_id TEXT PRIMARY KEY REFERENCES tasks(id), actor_id TEXT NOT NULL, revision TEXT NOT NULL, payloads TEXT NOT NULL, explicit_submit INTEGER NOT NULL, sealed INTEGER NOT NULL DEFAULT 0, cancel_requested INTEGER NOT NULL DEFAULT 0);
          CREATE TABLE IF NOT EXISTS action_revisions(revision TEXT PRIMARY KEY, step_id TEXT NOT NULL REFERENCES steps(id), body TEXT NOT NULL, dispatch_id TEXT UNIQUE, cancel_requested INTEGER NOT NULL DEFAULT 0);
          CREATE TABLE IF NOT EXISTS action_heads(step_id TEXT PRIMARY KEY REFERENCES steps(id), revision TEXT NOT NULL REFERENCES action_revisions(revision));
          CREATE TABLE IF NOT EXISTS dispatch_bindings(dispatch_id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, device_id TEXT NOT NULL, session_id TEXT NOT NULL, capture_epoch TEXT NOT NULL, action_epoch TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS ledger_grants(id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, body TEXT NOT NULL, revoked INTEGER NOT NULL DEFAULT 0);
          CREATE TABLE IF NOT EXISTS ledger_approvals(id TEXT PRIMARY KEY, action_revision TEXT NOT NULL REFERENCES action_revisions(revision), body TEXT NOT NULL, consumed INTEGER NOT NULL DEFAULT 0, revoked INTEGER NOT NULL DEFAULT 0);
          CREATE TABLE IF NOT EXISTS ledger_events(id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL REFERENCES tasks(id), step_id TEXT, kind TEXT NOT NULL, at_ms INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS reconciliation_evidence(dispatch_id TEXT PRIMARY KEY REFERENCES dispatch_bindings(dispatch_id), evidence_id TEXT NOT NULL, actor_id TEXT NOT NULL, outcome TEXT NOT NULL, at_ms INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS native_observations(dispatch_id TEXT PRIMARY KEY REFERENCES dispatch_bindings(dispatch_id), target_id TEXT NOT NULL, action_revision TEXT NOT NULL REFERENCES action_revisions(revision), body TEXT NOT NULL, at_ms INTEGER NOT NULL);
          CREATE INDEX IF NOT EXISTS native_observation_target ON native_observations(target_id);
          CREATE TABLE IF NOT EXISTS native_finalizations(dispatch_id TEXT PRIMARY KEY REFERENCES dispatch_bindings(dispatch_id), target_id TEXT NOT NULL, action_revision TEXT NOT NULL REFERENCES action_revisions(revision), actor_id TEXT NOT NULL, outcome TEXT NOT NULL, at_ms INTEGER NOT NULL);
          CREATE INDEX IF NOT EXISTS native_finalization_lookup ON native_finalizations(target_id,actor_id,outcome);
          DELETE FROM schema_version;
          INSERT INTO schema_version VALUES(29);
        ").map_err(|_|ErrorCode::Storage)?;
        if version < 5 {
            tx.execute_batch(crate::conversations::SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
        }
        if version < 6 {
            tx.execute_batch(crate::conversations::TASK_SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
        }
        if version < 7 {
            tx.execute_batch(crate::conversations::PLAN_SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
            tx.execute_batch(crate::conversations::REPLY_SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
        }
        if version < 8 {
            tx.execute_batch(crate::browser_jobs::SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
        }
        if version < 9 {
            tx.execute_batch(crate::browser_jobs::RETIREMENT_SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
        }
        if version < 11 {
            tx.execute_batch(crate::action_permissions::SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
        }
        if version < 13 {
            for (_, schema) in crate::memory::TABLES {
                tx.execute_batch(schema).map_err(|_| ErrorCode::Storage)?;
            }
        }
        if version < 15 {
            for (_, schema) in crate::notifications::TABLES {
                tx.execute_batch(schema).map_err(|_| ErrorCode::Storage)?;
            }
        }
        if version < 17 {
            tx.execute_batch(crate::conversations::SEQUENCE_SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
            tx.execute(
                "INSERT INTO planner_claim_sequence(id,value) VALUES(1,0)",
                [],
            )
            .map_err(|_| ErrorCode::Storage)?;
        }
        crate::memory::migrate(&tx, version)?;
        if version < 25 {
            for (_, schema) in crate::demonstration::TABLES {
                tx.execute_batch(schema).map_err(|_| ErrorCode::Storage)?;
            }
        }
        crate::demonstration::check_schema(&tx, 25)?;
        if version < 27 {
            for (_, schema) in crate::demonstration::passive::TABLES {
                tx.execute_batch(schema).map_err(|_| ErrorCode::Storage)?;
            }
        }
        crate::demonstration::passive::check_schema(&tx, 27)?;
        crate::notifications::check_schema(&tx, 15)?;
        crate::memory::check_schema(&tx, 23)?;
        crate::action_permissions::check_schema(&tx, 12)?;
        if version < 29 {
            tx.execute_batch(crate::conversations::deletion::SCHEMA)
                .map_err(|_| ErrorCode::Storage)?;
            tx.execute_batch(crate::conversations::deletion::INDEX)
                .map_err(|_| ErrorCode::Storage)?;
        }
        crate::conversations::check_schema(&tx, 29)?;
        crate::browser_jobs::check_schema(&tx, 12)?;
        crate::observation_schema::check(&tx, 12)?;
        crate::browser_jobs::current(&tx)?;
        tx.execute(
            "UPDATE browser_read_owner SET state='uncertain' WHERE state='pending'",
            [],
        )
        .map_err(|_| ErrorCode::Storage)?;
        tx.execute(
            "UPDATE conversation_plans SET state='suspended' WHERE state='pending'",
            [],
        )
        .map_err(|_| ErrorCode::Storage)?;
        tx.execute("UPDATE accepted_conversations SET state='suspended' WHERE state IN ('accepted','planning','waiting_input')",[]).map_err(|_|ErrorCode::Storage)?;
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
        tx.execute("UPDATE steps SET state='\"suspended\"' WHERE state IN ('\"queued\"','\"waiting_for_user\"')",[]).map_err(|_|ErrorCode::Storage)?;
        let invalid: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
                [],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Storage)?;
        if invalid {
            return Err(ErrorCode::Malformed);
        }
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|_| ErrorCode::Storage)?;
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
        let mut settings: Settings = match value {
            Some(value) => serde_json::from_str(&value).map_err(|_| ErrorCode::Malformed)?,
            None => Settings::default(),
        };
        let migrated = settings.audio_device_schema == 0;
        // A damaged optional personal detail must not prevent startup or cause
        // an unvalidated name to be spoken. Preserve the record on disk.
        settings.owner_name = settings.owner_name.filter(|value| {
            !value.actor.is_nil() && avesra_contracts::preview::Greeting::valid_name(&value.name)
        });
        if migrated {
            // Legacy fields were friendly names. Never guess an endpoint from a
            // label: require explicit device reselection and retain deliberate mute.
            settings.microphone = None;
            settings.speaker = None;
            settings.explicit_mute = true;
            settings.audio_device_schema = 1;
        }
        settings.validate()?;
        if migrated {
            let value = serde_json::to_string(&settings).map_err(|_| ErrorCode::Malformed)?;
            self.connection
                .execute("UPDATE settings SET value=?1 WHERE id=1", [value])
                .map_err(|_| ErrorCode::Storage)?;
        }
        Ok(settings)
    }
    pub fn save_settings(&mut self, settings: &Settings) -> Result<(), ErrorCode> {
        settings.validate()?;
        let value = serde_json::to_string(settings).map_err(|_| ErrorCode::Malformed)?;
        self.connection.execute("INSERT INTO settings(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[value]).map_err(|_|ErrorCode::Storage)?;
        Ok(())
    }
}
