//! Durable uncertainty survives caller loss and process restart. No reset escape.
use super::{deployment::ReadyIncarnation, stream::CompletedStream};
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    time::Duration,
};
use uuid::Uuid;
const SQL: &str = "CREATE TABLE reasoning_job(id INTEGER PRIMARY KEY CHECK(id=1),body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=4096))";
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Record {
    request: Uuid,
    model: String,
    incarnation: String,
    state: State,
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Outstanding,
    Uncertain,
}
impl Record {
    fn validate(&self) -> Result<(), ErrorCode> {
        if self.request.is_nil()
            || self.model.is_empty()
            || self.model.len() > 128
            || !self
                .model
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_' | b'.'))
            || self.incarnation.len() != 64
            || !self
                .incarnation
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Status {
    pub request: Uuid,
    pub model: String,
    pub state: State,
}
/// Created only after durable begin; deliberately no Clone/Deserialize.
pub struct JobLease {
    owner: Uuid,
    record: Record,
}
impl JobLease {
    pub fn request(&self) -> Uuid {
        self.record.request
    }
    pub fn model(&self) -> &str {
        &self.record.model
    }
}
pub struct Jobs {
    connection: Connection,
    owner: Uuid,
    _lock: File,
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
    let actual:String=db.query_row("SELECT substr(sql,1,1025) FROM sqlite_master WHERE type='table' AND name='reasoning_job'",[],|r|r.get(0)).map_err(|_|ErrorCode::Malformed)?;
    if count != 1 || actual != SQL {
        return Err(ErrorCode::Malformed);
    }
    Ok(true)
}
fn read(db: &Connection) -> Result<Option<Record>, ErrorCode> {
    let rows: i64 = db
        .query_row("SELECT COUNT(*) FROM reasoning_job", [], |r| r.get(0))
        .map_err(|_| ErrorCode::Storage)?;
    if rows > 1 {
        return Err(ErrorCode::Malformed);
    }
    let value: Option<Vec<u8>> = db
        .query_row(
            "SELECT substr(CAST(body AS BLOB),1,4097) FROM reasoning_job WHERE id=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    let Some(value) = value else {
        return if rows == 0 {
            Ok(None)
        } else {
            Err(ErrorCode::Malformed)
        };
    };
    if value.len() > 4096 {
        return Err(ErrorCode::Malformed);
    }
    let record: Record = serde_json::from_slice(&value).map_err(|_| ErrorCode::Malformed)?;
    record.validate()?;
    Ok(Some(record))
}
fn encode(value: &Record) -> Result<String, ErrorCode> {
    value.validate()?;
    let body = serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)?;
    if body.len() > 4096 {
        return Err(ErrorCode::TooLarge);
    }
    Ok(body)
}
impl Jobs {
    /// The controller's existing private Avesra directory only; no arbitrary
    /// frontend path and no Local Studio files are mutated.
    pub fn open(directory: &Path) -> Result<Self, ErrorCode> {
        let meta = std::fs::symlink_metadata(directory).map_err(|_| ErrorCode::Storage)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(ErrorCode::Denied);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(ErrorCode::Denied);
            }
        }
        let lock_path = directory.join("reasoning-owner.lock");
        if lock_path.try_exists().map_err(|_| ErrorCode::Storage)?
            && std::fs::symlink_metadata(&lock_path)
                .map_err(|_| ErrorCode::Storage)?
                .file_type()
                .is_symlink()
        {
            return Err(ErrorCode::Denied);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path).map_err(|_| ErrorCode::Storage)?;
        lock.try_lock().map_err(|_| ErrorCode::Unavailable)?;
        let path = directory.join("reasoning-jobs.db");
        if path.try_exists().map_err(|_| ErrorCode::Storage)?
            && std::fs::symlink_metadata(&path)
                .map_err(|_| ErrorCode::Storage)?
                .file_type()
                .is_symlink()
        {
            return Err(ErrorCode::Denied);
        }
        let mut connection = Connection::open(path).map_err(|_| ErrorCode::Storage)?;
        connection
            .busy_timeout(Duration::from_secs(2))
            .map_err(|_| ErrorCode::Storage)?;
        let initialized = schema(&connection)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        if !initialized {
            tx.execute_batch(SQL).map_err(|_| ErrorCode::Storage)?;
            tx.execute_batch("PRAGMA user_version=1")
                .map_err(|_| ErrorCode::Storage)?;
        }
        schema(&tx)?;
        if let Some(mut record) = read(&tx)? {
            record.state = State::Uncertain;
            tx.execute(
                "UPDATE reasoning_job SET body=?1 WHERE id=1",
                [encode(&record)?],
            )
            .map_err(|_| ErrorCode::Storage)?;
        }
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(Self {
            connection,
            owner: Uuid::new_v4(),
            _lock: lock,
        })
    }
    pub fn status(&self) -> Result<Option<Status>, ErrorCode> {
        Ok(read(&self.connection)?.map(|r| Status {
            request: r.request,
            model: r.model,
            state: r.state,
        }))
    }
    /// Bookkeeping only. The actual caller must separately own qualified model
    /// admission before invoking begin or sending any accepted text.
    pub fn begin(
        &mut self,
        incarnation: &ReadyIncarnation,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<JobLease, ErrorCode> {
        incarnation.current()?;
        let record = Record {
            request: Uuid::new_v4(),
            model: incarnation.model().into(),
            incarnation: incarnation.fingerprint()?,
            state: State::Outstanding,
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        if read(&tx)?.is_some() {
            return Err(ErrorCode::Unavailable);
        }
        tx.execute(
            "INSERT INTO reasoning_job(id,body) VALUES(1,?1)",
            [encode(&record)?],
        )
        .map_err(|_| ErrorCode::Storage)?;
        if read(&tx)?.as_ref() != Some(&record) {
            return Err(ErrorCode::Malformed);
        }
        authorize()?;
        incarnation.current()?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        incarnation.current()?;
        Ok(JobLease {
            owner: self.owner,
            record,
        })
    }
    fn owns(&self, lease: &JobLease) -> Result<(), ErrorCode> {
        if lease.owner != self.owner || read(&self.connection)?.as_ref() != Some(&lease.record) {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    pub fn uncertain(&mut self, lease: JobLease) -> Result<(), ErrorCode> {
        self.owns(&lease)?;
        let original = encode(&lease.record)?;
        let mut record = lease.record;
        record.state = State::Uncertain;
        if self
            .connection
            .execute(
                "UPDATE reasoning_job SET body=?1 WHERE id=1 AND body=?2",
                params![encode(&record)?, original],
            )
            .map_err(|_| ErrorCode::Storage)?
            != 1
        {
            return Err(ErrorCode::Stale);
        }
        if read(&self.connection)?.as_ref() != Some(&record) {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    /// A qualifying adapter may retire only its own correlated complete stream.
    /// No nonce, config change, timeout or process restart can call this instead.
    pub fn complete(
        &mut self,
        lease: JobLease,
        stream: CompletedStream,
    ) -> Result<CompletedStream, ErrorCode> {
        self.owns(&lease)?;
        if stream.request() != lease.request() || stream.model() != lease.model() {
            return Err(ErrorCode::Stale);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        if tx
            .execute(
                "DELETE FROM reasoning_job WHERE id=1 AND body=?1",
                params![encode(&lease.record)?],
            )
            .map_err(|_| ErrorCode::Storage)?
            != 1
        {
            return Err(ErrorCode::Stale);
        }
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(stream)
    }
}
