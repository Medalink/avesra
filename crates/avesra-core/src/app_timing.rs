//! Installation-scoped content-free observations, never accepted-turn authority.
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const CAPACITY: usize = 8192;
pub const MAX_DURATION_US: u64 = 600_000_000;
const DAY_MS: u64 = 86_400_000;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Setup,
    Audio,
    Models,
    Profiles,
    People,
    Awareness,
    Memory,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "name",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Operation {
    NativeStartup,
    SettingsStore,
    AppMount,
    UiReady,
    View(View),
    Command(String),
    History,
}
impl Operation {
    pub fn valid(&self) -> bool {
        match self {
            Self::Command(name) => COMMANDS.contains(&name.as_str()),
            _ => true,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Initialize,
    InvokeRoundTrip,
    MountCommit,
    NextFrame,
    QueueWait,
    Work,
    Retired,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Complete,
    Failed,
    Withdrawn,
    Abandoned,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Window {
    Settings,
    Overlay,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Origin {
    Native,
    Frontend { window: Window, page: Uuid },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u16,
    pub id: Uuid,
    pub process: Uuid,
    pub native_operation: Option<Uuid>,
    pub package_version: String,
    pub build_fingerprint: Option<String>,
    pub operation: Operation,
    pub stage: Stage,
    pub outcome: Outcome,
    pub origin: Origin,
    pub at_ms: u64,
    pub start_us: Option<u64>,
    pub duration_us: u64,
}
impl Record {
    fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != 1
            || self.id.is_nil()
            || self.process.is_nil()
            || !self.operation.valid()
            || self.duration_us > MAX_DURATION_US
            || self.native_operation.is_some_and(|v| v.is_nil())
            || (matches!(self.origin, Origin::Frontend { .. }) && self.native_operation.is_some())
            || self.package_version.len() > 64
            || self.build_fingerprint.is_some()
            || matches!(self.origin, Origin::Frontend {page,..} if page.is_nil())
            || (matches!(self.origin, Origin::Frontend { .. }) && self.start_us.is_some())
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Snapshot {
    pub version: u16,
    pub scope: &'static str,
    pub process: Uuid,
    pub capacity: usize,
    pub retention_days: u8,
    pub evicted: u64,
    pub native_observer_loss: u64,
    pub frontend_reported_loss: u64,
    pub records: Vec<Record>,
}
pub struct Span {
    operation: Operation,
    stage: Stage,
    started: Instant,
    finished: bool,
    operation_id: Option<Uuid>,
}
impl Span {
    pub fn start(operation: Operation, stage: Stage) -> Self {
        Self {
            operation,
            stage,
            started: Instant::now(),
            finished: false,
            operation_id: None,
        }
    }
    pub fn with_operation(operation: Operation, stage: Stage, id: Option<Uuid>) -> Self {
        let mut span = Self::start(operation, stage);
        span.operation_id = id;
        span
    }
    pub fn operation_id(&self) -> Option<Uuid> {
        self.operation_id
    }
    pub fn finish(mut self, outcome: Outcome) {
        self.end(outcome);
    }
    fn end(&mut self, outcome: Outcome) {
        if self.finished {
            return;
        }
        self.finished = true;
        crate::trace::app_observe(
            (self.operation.clone(), self.operation_id),
            self.stage,
            outcome,
            Origin::Native,
            Some(self.started),
            self.started.elapsed(),
        );
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        self.end(Outcome::Abandoned);
    }
}
pub fn frontend(
    operation: Operation,
    stage: Stage,
    outcome: Outcome,
    window: Window,
    page: Uuid,
    duration_us: u64,
) {
    crate::trace::app_observe(
        (operation, None),
        stage,
        outcome,
        Origin::Frontend { window, page },
        None,
        Duration::from_micros(duration_us),
    );
}
pub(crate) fn initialize(db: &Connection) -> Result<(), ErrorCode> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS app_timings(id TEXT PRIMARY KEY,at_ms INTEGER NOT NULL,body TEXT NOT NULL CHECK(length(body)<=1024)); CREATE TABLE IF NOT EXISTS app_timing_state(id INTEGER PRIMARY KEY CHECK(id=1),evicted INTEGER NOT NULL,native_loss INTEGER NOT NULL,frontend_loss INTEGER NOT NULL); INSERT OR IGNORE INTO app_timing_state VALUES(1,0,0,0);")
        .map_err(|_|ErrorCode::Storage)?;
    let columns = |table: &str| -> Result<Vec<String>, ErrorCode> {
        let mut query = db
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|_| ErrorCode::Storage)?;
        query
            .query_map([], |row| row.get(1))
            .map_err(|_| ErrorCode::Storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| ErrorCode::Storage)
    };
    if columns("app_timings")? != ["id", "at_ms", "body"]
        || columns("app_timing_state")? != ["id", "evicted", "native_loss", "frontend_loss"]
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(())
}
fn integer<T: TryInto<i64>>(value: T) -> Result<i64, ErrorCode> {
    value.try_into().map_err(|_| ErrorCode::TooLarge)
}
fn unsigned(value: i64) -> Result<u64, ErrorCode> {
    value.try_into().map_err(|_| ErrorCode::Malformed)
}
fn compact(db: &Connection, now: u64) -> Result<(), ErrorCode> {
    let removed=db.execute("DELETE FROM app_timings WHERE at_ms<?1 OR rowid IN(SELECT rowid FROM app_timings ORDER BY rowid DESC LIMIT -1 OFFSET ?2)",params![integer(now.saturating_sub(7*DAY_MS))?,integer(CAPACITY)?]).map_err(|_|ErrorCode::Storage)?;
    db.execute(
        "UPDATE app_timing_state SET evicted=CASE WHEN evicted>9223372036854775807-?1 THEN 9223372036854775807 ELSE evicted+?1 END WHERE id=1",
        [integer(removed)?],
    )
    .map_err(|_| ErrorCode::Storage)?;
    Ok(())
}
pub(crate) fn write(db: &mut Connection, record: &Record) -> Result<(), ErrorCode> {
    record.validate()?;
    let body = serde_json::to_string(record).map_err(|_| ErrorCode::Malformed)?;
    if body.len() > 1024 {
        return Err(ErrorCode::TooLarge);
    }
    let tx = db.transaction().map_err(|_| ErrorCode::Storage)?;
    tx.execute(
        "INSERT INTO app_timings VALUES(?1,?2,?3)",
        params![record.id.to_string(), integer(record.at_ms)?, body],
    )
    .map_err(|_| ErrorCode::Storage)?;
    compact(&tx, record.at_ms)?;
    tx.commit().map_err(|_| ErrorCode::Storage)
}
pub(crate) fn losses(db: &Connection, native: u64, frontend: u64) -> Result<(), ErrorCode> {
    db.execute("UPDATE app_timing_state SET native_loss=CASE WHEN native_loss>9223372036854775807-?1 THEN 9223372036854775807 ELSE native_loss+?1 END,frontend_loss=CASE WHEN frontend_loss>9223372036854775807-?2 THEN 9223372036854775807 ELSE frontend_loss+?2 END WHERE id=1",params![integer(native.min(i64::MAX as u64))?,integer(frontend.min(i64::MAX as u64))?]).map_err(|_|ErrorCode::Storage)?;
    Ok(())
}
pub(crate) fn read(
    db: &mut Connection,
    process: Uuid,
    now: u64,
    loss: u64,
    frontend_loss: u64,
) -> Result<Snapshot, ErrorCode> {
    let tx = db.transaction().map_err(|_| ErrorCode::Storage)?;
    compact(&tx, now)?;
    tx.commit().map_err(|_| ErrorCode::Storage)?;
    let mut query = db
        .prepare("SELECT body FROM app_timings ORDER BY rowid DESC LIMIT 8192")
        .map_err(|_| ErrorCode::Storage)?;
    let mut rows = query.query([]).map_err(|_| ErrorCode::Storage)?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(|_| ErrorCode::Storage)? {
        let raw = row
            .get_ref(0)
            .map_err(|_| ErrorCode::Storage)?
            .as_str()
            .map_err(|_| ErrorCode::Malformed)?;
        if raw.len() > 1024 {
            return Err(ErrorCode::TooLarge);
        }
        let value: Record = serde_json::from_str(raw).map_err(|_| ErrorCode::Malformed)?;
        value.validate()?;
        records.push(value);
    }
    let (evicted, stored_loss, stored_frontend): (i64, i64, i64) = db
        .query_row(
            "SELECT evicted,native_loss,frontend_loss FROM app_timing_state WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| ErrorCode::Storage)?;
    Ok(Snapshot {
        version: 1,
        scope: "windows_user_installation",
        process,
        capacity: CAPACITY,
        retention_days: 7,
        evicted: unsigned(evicted)?,
        native_observer_loss: unsigned(stored_loss)?.saturating_add(loss),
        frontend_reported_loss: unsigned(stored_frontend)?.saturating_add(frontend_loss),
        records,
    })
}

// Closed compile-time roster. Observer commands are deliberately excluded.
const COMMANDS: &[&str] = &[
    "begin_microphone_check",
    "stop_microphone_check",
    "check_saved_voice",
    "cancel_voice_check",
    "freeze_voice_calibration",
    "discard_voice_calibration",
    "voice_calibration_status",
    "annotate_voice_activity",
    "review_voice_admission",
    "review_development_voice",
    "revalidate_voice_admission",
    "revoke_voice_admission",
    "notification_status",
    "reasoning_health",
    "discover_sparks",
    "cancel_spark_discovery",
    "pair_discovered_spark",
    "begin_browser_pairing",
    "connect_selected_browser",
    "inspect_browser_documents",
    "browser_documents_status",
    "cancel_browser_documents",
    "select_browser_document",
    "propose_browser_scope",
    "saved_browser_scopes",
    "cancel_browser_scope",
    "revoke_browser_scope",
    "browser_pairing_status",
    "approve_browser_pairing",
    "cancel_browser_pairing",
    "release_browser_management",
    "saved_browser_pairings",
    "revoke_browser_pairing",
    "browser_selections",
    "select_browser_pairing",
    "clear_browser_selection",
    "hide_window",
    "open_app_catalog",
    "close_app_catalog",
    "scan_app_catalog",
    "choose_app_folder",
    "choose_app_executable",
    "observe_app_windows",
    "choose_app_window",
    "app_aliases",
    "remember_app",
    "forget_app_alias",
    "open_action_panel",
    "close_action_panel",
    "action_status",
    "grant_app_action",
    "grant_volume_action",
    "grant_diagnostic_action",
    "inspect_vpn_profile",
    "grant_vpn_action",
    "grant_download_diagnosis",
    "grant_download_fix",
    "approve_download_fix",
    "grant_browser_read_action",
    "change_private_memory",
    "teaching_status",
    "teaching_progress",
    "set_teaching_scope",
    "set_passive_teaching",
    "start_demonstration",
    "stop_demonstration",
    "change_demonstration",
    "inspect_conversation_history",
    "prepare_conversation_deletion",
    "confirm_conversation_deletion",
    "memory_forget_status",
    "approve_memory_forget",
    "cancel_memory_forget",
    "inspect_prompt_surface",
    "bind_prompt_project",
    "revoke_action_permission",
    "cancel_action_task",
    "shortcut_status",
    "owner_status",
    "create_owner",
    "remember_owner_name",
    "actor_registration_status",
    "register_owner_with_spark",
    "revoke_owner_registration",
    "set_shortcut",
    "begin_shortcut_recording",
    "end_shortcut_recording",
    "preview_voice",
    "open_voice_panel",
    "close_voice_panel",
    "voice_operation",
    "setup_status",
    "verify_setup",
    "cancel_setup",
    "begin_enrollment",
    "finish_enrollment",
    "speaker_candidates",
    "delete_speaker_candidate",
    "select_speaker_candidate",
    "clear_speaker_selection",
    "record_enrollment",
    "runtime_snapshot",
    "audio_devices",
    "audio_lane_health",
    "local_control",
    // Retained for historical observations after the old IPC surface is removed.
    "save_settings",
    "apply_preferences",
    "begin_preferences_editor",
    "retire_preferences_editor",
    "answer_preferences_close",
    "show_settings",
    "update_sound",
    "sound_output_channels",
    "sound_diagnostics",
    "pair_spark",
    "connect_spark",
    "saved_pairing",
    "forget_spark",
    "disconnect_spark",
];
