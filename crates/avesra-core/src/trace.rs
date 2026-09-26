//! Content-free, bounded observer. Never an admission, retry or recovery owner.
use avesra_contracts::{ErrorCode, planner};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
const CAP: usize = 32768;
const DAY: u64 = 86_400_000;
pub const QUERY_VERSION: u16 = 3;
static SINK: OnceLock<Sink> = OnceLock::new();
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Host {
    Native,
    Controller,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    VoiceCapture,
    VoiceActivityEndpoint,
    VoiceQueue,
    VoiceAnalysis,
    VoiceIntent,
    VoiceGate,
    Accepted,
    NativePlanner,
    ControllerPlanner,
    Reasoning,
    PrivateReasoning,
    NativeOutput,
    ControllerOutput,
    PrivateTts,
    FirstAudio,
    Submitted,
    EndpointResponseSubmission,
    ToolDispatch,
    ToolResult,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Complete,
    Failed,
    Withdrawn,
    Abandoned,
    Truncated,
    NeedsInput,
    Uncertain,
    Missing,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub turn: Uuid,
    pub actor: Uuid,
    pub device: Uuid,
    pub operation: Uuid,
    pub parent: Option<Uuid>,
}
impl Link {
    pub fn planner(c: &planner::Context) -> Self {
        Self {
            turn: c.turn,
            actor: c.actor,
            device: c.device,
            operation: c.request,
            parent: Some(c.turn),
        }
    }
    pub fn child(self, operation: Uuid) -> Self {
        Self {
            operation,
            parent: Some(self.operation),
            ..self
        }
    }
    pub(crate) fn valid(self) -> bool {
        ![self.turn, self.actor, self.device, self.operation]
            .iter()
            .any(Uuid::is_nil)
            && !self.parent.is_some_and(|id| id.is_nil())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    pub model: Option<String>,
    pub image: Option<String>,
    pub config: Option<String>,
}
impl Deployment {
    pub fn observed(model: &str, image: &str, config: &str) -> Self {
        fn digest(s: &str) -> Option<String> {
            ((s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .then(|| s.to_ascii_lowercase())
        }
        Self {
            model: digest(model),
            image: digest(image),
            config: digest(config),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub link: Link,
    pub host: Host,
    pub process: Uuid,
    pub stage: Stage,
    pub outcome: Outcome,
    pub error: Option<ErrorCode>,
    pub at_ms: u64,
    pub start_us: u64,
    pub duration_us: u64,
    pub queue_us: Option<u64>,
    pub retries: u32,
    pub deployment: Deployment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<avesra_contracts::voice_timing::Analysis>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rollup {
    pub day: u64,
    pub stage: Stage,
    pub outcome: Outcome,
    pub count: u64,
    pub duration_us: u64,
    pub maximum_us: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u16,
    pub host: Host,
    pub process: Uuid,
    pub trace_days: u8,
    pub observer_loss: u64,
    pub collector_starts: u64,
    pub evicted: u64,
    pub truncated: bool,
    pub records: Vec<Record>,
    pub rollups: Vec<Rollup>,
    pub resources: crate::resource_observer::Snapshot,
    pub engine: crate::engine_observer::Snapshot,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub version: u16,
    pub request: Uuid,
    pub session: Uuid,
    pub action_epoch: u64,
    pub actor: Uuid,
    pub owner_revision: Uuid,
    pub registered_by: Uuid,
    pub turn: Option<Uuid>,
}
impl Query {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != QUERY_VERSION
            || [
                self.request,
                self.session,
                self.actor,
                self.owner_revision,
                self.registered_by,
            ]
            .iter()
            .any(Uuid::is_nil)
            || self.turn.is_some_and(|id| id.is_nil())
            || self.action_epoch == 0
            || self.action_epoch > avesra_contracts::browser::MAX_SAFE_COUNTER
        {
            Err(ErrorCode::Malformed)
        } else {
            Ok(())
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    pub request: Uuid,
    pub binding: avesra_contracts::actors::Binding,
    pub snapshot: Snapshot,
}
enum Message {
    AppTiming(crate::app_timing::Record),
    ReadApp {
        reply: mpsc::SyncSender<Result<crate::app_timing::Snapshot, ErrorCode>>,
        deadline: Instant,
        present: Arc<AtomicBool>,
    },
    Record(Record),
    Resource(crate::resource_observer::Sample),
    EngineQueue(crate::engine_observer::QueueRecord),
    Read {
        actor: Uuid,
        device: Uuid,
        turn: Option<Uuid>,
        reply: mpsc::SyncSender<Result<Snapshot, ErrorCode>>,
        deadline: Instant,
        present: Arc<AtomicBool>,
    },
    Retention(u8),
}
struct Sink {
    app_loss: Arc<AtomicU64>,
    frontend_loss: Arc<AtomicU64>,
    tx: mpsc::SyncSender<Message>,
    host: Host,
    process: Uuid,
    origin: Instant,
    loss: Arc<AtomicU64>,
    outputs: Mutex<Vec<(Uuid, Span)>>,
    responses: Mutex<Vec<(Uuid, Span)>>,
    resource_latest: Mutex<Option<(Uuid, u64, Instant)>>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|n| u64::try_from(n.as_millis()).ok())
        .unwrap_or(0)
}
fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}
fn integer<T: TryInto<i64>>(value: T) -> Result<i64, ErrorCode> {
    value.try_into().map_err(|_| ErrorCode::TooLarge)
}
fn sql<T>(r: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    r.map_err(|_| ErrorCode::Storage)
}
pub fn initialize(directory: &Path, host: Host) -> Result<(), ErrorCode> {
    if SINK.get().is_some() {
        return Ok(());
    }
    let mut db = sql(Connection::open(directory.join("telemetry.db")))?;
    let version: i64 = sql(db.query_row("PRAGMA user_version", [], |r| r.get(0)))?;
    if version == 0 {
        let occupied: bool = sql(db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name NOT GLOB 'sqlite_*')",
            [],
            |r| r.get(0),
        ))?;
        if occupied {
            return Err(ErrorCode::Malformed);
        }
    } else if !matches!(version, 1..=7) {
        return Err(ErrorCode::Unsupported);
    }

    sql(db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=1000; CREATE TABLE IF NOT EXISTS spans(id TEXT PRIMARY KEY,actor TEXT NOT NULL,device TEXT NOT NULL,turn TEXT NOT NULL,at_ms INTEGER NOT NULL,body TEXT NOT NULL CHECK(length(body)<=4096)); CREATE INDEX IF NOT EXISTS span_owner ON spans(actor,device,at_ms); CREATE TABLE IF NOT EXISTS rollups(actor TEXT NOT NULL,device TEXT NOT NULL,day INTEGER NOT NULL,stage TEXT NOT NULL,outcome TEXT NOT NULL,count INTEGER NOT NULL,total INTEGER NOT NULL,maximum INTEGER NOT NULL,PRIMARY KEY(actor,device,day,stage,outcome)); CREATE TABLE IF NOT EXISTS retention(id INTEGER PRIMARY KEY CHECK(id=1),days INTEGER NOT NULL CHECK(days BETWEEN 1 AND 7),evicted INTEGER NOT NULL); INSERT OR IGNORE INTO retention VALUES(1,7,0); CREATE TABLE IF NOT EXISTS observer(id INTEGER PRIMARY KEY CHECK(id=1),lost INTEGER NOT NULL,starts INTEGER NOT NULL); INSERT OR IGNORE INTO observer VALUES(1,0,0); UPDATE observer SET starts=starts+1 WHERE id=1;"))?;
    let (tx, rx) = mpsc::sync_channel(256);
    crate::resource_observer::initialize(&db)?;
    crate::engine_observer::initialize(&db)?;
    crate::app_timing::initialize(&db)?;
    sql(db.execute_batch("PRAGMA user_version=7;"))?;
    let loss = Arc::new(AtomicU64::new(0));
    let process = Uuid::new_v4();
    let losses = loss.clone();
    let app_loss = Arc::new(AtomicU64::new(0));
    let frontend_loss = Arc::new(AtomicU64::new(0));
    let app_losses = app_loss.clone();
    let frontend_losses = frontend_loss.clone();
    std::thread::Builder::new()
        .name("avesra-trace-writer".into())
        .spawn(move || {
            while let Ok(message) = rx.recv() {
                let app_delta = app_losses.swap(0, Ordering::Relaxed);
                let frontend_delta = frontend_losses.swap(0, Ordering::Relaxed);
                if (app_delta != 0 || frontend_delta != 0)
                    && crate::app_timing::losses(&db, app_delta, frontend_delta).is_err()
                {
                    app_losses.fetch_add(app_delta.saturating_add(1), Ordering::Relaxed);
                    frontend_losses.fetch_add(frontend_delta, Ordering::Relaxed);
                }
                let delta = losses.swap(0, Ordering::Relaxed);
                if delta != 0
                    && integer(delta)
                        .and_then(|delta| {
                            sql(db.execute("UPDATE observer SET lost=lost+?1 WHERE id=1", [delta]))
                        })
                        .is_err()
                {
                    losses.fetch_add(delta.saturating_add(1), Ordering::Relaxed);
                }
                match message {
                    Message::AppTiming(record) => {
                        if crate::app_timing::write(&mut db, &record).is_err() {
                            app_losses.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Message::ReadApp {
                        reply,
                        deadline,
                        present,
                    } => {
                        if Instant::now() < deadline && present.load(Ordering::SeqCst) {
                            let value = crate::app_timing::read(
                                &mut db,
                                process,
                                now(),
                                app_losses.load(Ordering::Relaxed),
                                frontend_losses.load(Ordering::Relaxed),
                            );
                            if Instant::now() < deadline && present.load(Ordering::SeqCst) {
                                let _ = reply.try_send(value);
                            }
                        }
                    }
                    Message::Record(r) => {
                        if write(&mut db, &r).is_err() {
                            losses.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Message::Resource(sample) => {
                        if crate::resource_observer::write(&mut db, &sample).is_err() {
                            losses.fetch_add(1, Ordering::Relaxed);
                        }
                        if host == Host::Controller
                            && crate::engine_observer::checkpoint(&db, process).is_err()
                        {
                            losses.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Message::EngineQueue(value) => {
                        if crate::engine_observer::write(&mut db, &value).is_err() {
                            losses.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Message::Read {
                        actor,
                        device,
                        turn,
                        reply,
                        deadline,
                        present,
                    } => {
                        if Instant::now() >= deadline || !present.load(Ordering::SeqCst) {
                            continue;
                        }
                        let result =
                            read(&mut db, actor, device, turn, losses.load(Ordering::Relaxed));
                        if Instant::now() < deadline && present.load(Ordering::SeqCst) {
                            let _ = reply.try_send(result);
                        }
                    }
                    Message::Retention(days) => {
                        if db
                            .execute("UPDATE retention SET days=?1 WHERE id=1", [days])
                            .is_err()
                        {
                            losses.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        })
        .map_err(|_| ErrorCode::Unavailable)?;
    SINK.set(Sink {
        app_loss,
        frontend_loss,
        tx,
        host,
        process,
        origin: Instant::now(),
        loss,
        outputs: Mutex::new(Vec::new()),
        responses: Mutex::new(Vec::new()),
        resource_latest: Mutex::new(None),
    })
    .map_err(|_| ErrorCode::Unavailable)
}
fn compact(db: &Connection) -> Result<u8, ErrorCode> {
    let days: u8 = sql(db.query_row("SELECT days FROM retention WHERE id=1", [], |r| r.get(0)))?;
    let removed=sql(db.execute("DELETE FROM spans WHERE at_ms<?1 OR id IN(SELECT id FROM spans ORDER BY rowid DESC LIMIT -1 OFFSET ?2)",params![integer(now().saturating_sub(DAY*u64::from(days)))?,integer(CAP)?]))?;
    sql(db.execute(
        "UPDATE retention SET evicted=evicted+?1 WHERE id=1",
        [integer(removed)?],
    ))?;
    sql(db.execute("DELETE FROM rollups WHERE day<?1 OR rowid IN(SELECT rowid FROM rollups ORDER BY day DESC,rowid DESC LIMIT -1 OFFSET 8192)",[integer(now().saturating_sub(DAY*30)/DAY)?]))?;
    Ok(days)
}
fn write(db: &mut Connection, r: &Record) -> Result<(), ErrorCode> {
    let body = serde_json::to_string(r).map_err(|_| ErrorCode::Malformed)?;
    let tx = sql(db.transaction())?;
    sql(tx.execute(
        "INSERT INTO spans VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            r.id.to_string(),
            r.link.actor.to_string(),
            r.link.device.to_string(),
            r.link.turn.to_string(),
            integer(r.at_ms)?,
            body
        ],
    ))?;
    sql(tx.execute("INSERT INTO rollups VALUES(?1,?2,?3,?4,?5,1,?6,?6) ON CONFLICT(actor,device,day,stage,outcome) DO UPDATE SET count=count+1,total=total+excluded.total,maximum=max(maximum,excluded.maximum)",params![r.link.actor.to_string(),r.link.device.to_string(),integer(r.at_ms/DAY)?,serde_json::to_string(&r.stage).map_err(|_|ErrorCode::Malformed)?,serde_json::to_string(&r.outcome).map_err(|_|ErrorCode::Malformed)?,integer(r.duration_us)?]))?;
    compact(&tx)?;
    sql(tx.commit())
}
fn read(
    db: &mut Connection,
    actor: Uuid,
    device: Uuid,
    turn: Option<Uuid>,
    loss: u64,
) -> Result<Snapshot, ErrorCode> {
    let sink = SINK.get().ok_or(ErrorCode::Unavailable)?;
    let (host, process) = (sink.host, sink.process);
    let tx = sql(db.transaction())?;
    let days = compact(&tx)?;
    let mut q=sql(tx.prepare("SELECT body FROM spans WHERE actor=?1 AND device=?2 AND (?3 IS NULL OR turn=?3) ORDER BY rowid DESC LIMIT 2049"))?;
    let bodies = sql(q.query_map(
        params![
            actor.to_string(),
            device.to_string(),
            turn.map(|id| id.to_string())
        ],
        |r| r.get::<_, String>(0),
    ))?;
    let mut records = Vec::new();
    for body in bodies {
        records.push(serde_json::from_str(&sql(body)?).map_err(|_| ErrorCode::Malformed)?);
    }
    let truncated = records.len() > 2048;
    records.truncate(2048);
    records.reverse();
    drop(q);
    let mut q=sql(tx.prepare("SELECT day,stage,outcome,count,total,maximum FROM rollups WHERE actor=?1 AND device=?2 ORDER BY day DESC LIMIT 8192"))?;
    let mut rows = sql(q.query(params![actor.to_string(), device.to_string()]))?;
    let mut rollups = Vec::new();
    while let Some(r) = sql(rows.next())? {
        let n = |i| -> Result<u64, ErrorCode> {
            u64::try_from(sql(r.get::<_, i64>(i))?).map_err(|_| ErrorCode::Malformed)
        };
        rollups.push(Rollup {
            day: n(0)?,
            stage: serde_json::from_str(&sql(r.get::<_, String>(1))?)
                .map_err(|_| ErrorCode::Malformed)?,
            outcome: serde_json::from_str(&sql(r.get::<_, String>(2))?)
                .map_err(|_| ErrorCode::Malformed)?,
            count: n(3)?,
            duration_us: n(4)?,
            maximum_us: n(5)?,
        });
    }
    drop(rows);
    drop(q);
    let (stored_loss, starts): (i64, i64) = sql(tx.query_row(
        "SELECT lost,starts FROM observer WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ))?;
    let evicted: i64 =
        sql(tx.query_row("SELECT evicted FROM retention WHERE id=1", [], |r| r.get(0)))?;
    let resources = crate::resource_observer::read(&tx, days, host)?;
    let engine = crate::engine_observer::read(&tx, host, process, actor, device, turn, days)?;
    sql(tx.commit())?;
    let snapshot = Snapshot {
        version: QUERY_VERSION,
        host,
        process,
        trace_days: days,
        observer_loss: u64::try_from(stored_loss)
            .map_err(|_| ErrorCode::Malformed)?
            .saturating_add(loss),
        collector_starts: u64::try_from(starts).map_err(|_| ErrorCode::Malformed)?,
        evicted: u64::try_from(evicted).map_err(|_| ErrorCode::Malformed)?,
        truncated,
        records,
        rollups,
        resources,
        engine,
    };
    snapshot.validate(host, actor, device, turn)?;
    Ok(snapshot)
}
pub fn snapshot(actor: Uuid, device: Uuid, turn: Option<Uuid>) -> Result<Snapshot, ErrorCode> {
    let sink = SINK.get().ok_or(ErrorCode::Unavailable)?;
    let (tx, rx) = mpsc::sync_channel(1);
    let present = Arc::new(AtomicBool::new(true));
    sink.tx
        .try_send(Message::Read {
            actor,
            device,
            turn,
            reply: tx,
            deadline: Instant::now() + Duration::from_secs(2),
            present: present.clone(),
        })
        .map_err(|_| ErrorCode::Unavailable)?;
    let result = rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| ErrorCode::Expired);
    present.store(false, Ordering::SeqCst);
    result?
}
pub fn retention(days: u8) -> Result<(), ErrorCode> {
    if !(1..=7).contains(&days) {
        return Err(ErrorCode::Malformed);
    }
    SINK.get()
        .ok_or(ErrorCode::Unavailable)?
        .tx
        .try_send(Message::Retention(days))
        .map_err(|_| ErrorCode::Unavailable)
}
pub(crate) fn resource(sample: crate::resource_observer::Sample, observed: Instant) {
    let Some(sink) = SINK.get() else {
        return;
    };
    if sample.validate(sink.host).is_err() {
        sink.loss.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let Ok(mut latest) = sink.resource_latest.lock() else {
        return;
    };
    if latest
        .as_ref()
        .is_some_and(|(_, _, at)| at.elapsed() < Duration::from_secs(1))
    {
        sink.loss.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let stamp = (sample.process, sample.sequence, observed);
    if sink.tx.try_send(Message::Resource(sample)).is_err() {
        sink.loss.fetch_add(1, Ordering::Relaxed);
    } else {
        *latest = Some(stamp);
    }
}
pub(crate) fn resource_clock(host: Host) -> Option<(Uuid, Instant)> {
    let sink = SINK.get().filter(|sink| sink.host == host)?;
    Some((sink.process, sink.origin))
}
pub(crate) fn observer_loss() {
    if let Some(sink) = SINK.get() {
        sink.loss.fetch_add(1, Ordering::Relaxed);
    }
}
pub(crate) fn engine_queue(value: crate::engine_observer::QueueRecord) {
    if let Some(sink) = SINK.get()
        && sink.tx.try_send(Message::EngineQueue(value)).is_err()
    {
        sink.loss.fetch_add(1, Ordering::Relaxed);
    }
}
pub(crate) fn resource_age(sample: Option<&crate::resource_observer::Sample>) -> Option<u64> {
    let sample = sample?;
    let latest = SINK.get()?.resource_latest.lock().ok()?;
    let (process, sequence, at) = latest.as_ref()?;
    (*process == sample.process && *sequence == sample.sequence)
        .then(|| crate::resource_observer::millis(at.elapsed()))
}
pub struct Span {
    record: Option<Record>,
    started: Instant,
}
pub fn begin(link: Link, stage: Stage) -> Span {
    let started = Instant::now();
    let record = SINK.get().filter(|_| link.valid()).map(|s| Record {
        id: Uuid::new_v4(),
        link,
        host: s.host,
        process: s.process,
        stage,
        outcome: Outcome::Abandoned,
        error: None,
        at_ms: now(),
        start_us: micros(started.duration_since(s.origin)),
        duration_us: 0,
        queue_us: None,
        retries: 0,
        deployment: Deployment::default(),
        analysis: None,
    });
    Span { record, started }
}
impl Span {
    pub fn queued(&mut self, queued: Instant) {
        if let Some(r) = &mut self.record {
            r.queue_us = self.started.checked_duration_since(queued).map(micros);
        }
    }
    pub fn deployment(&mut self, value: Deployment) {
        if let Some(r) = &mut self.record {
            r.deployment = value;
        }
    }
    pub fn finish(mut self, outcome: Outcome, error: Option<ErrorCode>) {
        if let Some(r) = &mut self.record {
            r.outcome = outcome;
            r.error = error;
        }
        self.emit();
    }
    pub fn effect(self, outcome: avesra_contracts::Outcome) {
        use avesra_contracts::Outcome as Effect;
        self.finish(
            match outcome {
                Effect::Success | Effect::AlreadySatisfied => Outcome::Complete,
                Effect::NeedsInput => Outcome::NeedsInput,
                Effect::UnknownEffect => Outcome::Uncertain,
                Effect::Cancelled => Outcome::Withdrawn,
                Effect::Failed | Effect::Unsupported => Outcome::Failed,
            },
            None,
        );
    }
    pub fn result<T>(self, result: &Result<T, ErrorCode>) {
        let (outcome, error) = match result {
            Ok(_) => (Outcome::Complete, None),
            Err(e) => (
                if *e == ErrorCode::Stale {
                    Outcome::Withdrawn
                } else {
                    Outcome::Failed
                },
                Some(*e),
            ),
        };
        self.finish(outcome, error);
    }
    fn emit(&mut self) {
        self.emit_at(Instant::now());
    }
    fn emit_at(&mut self, end: Instant) {
        if let Some(mut r) = self.record.take() {
            r.duration_us = micros(end.saturating_duration_since(self.started));
            if let Some(s) = SINK.get()
                && s.tx.try_send(Message::Record(r)).is_err()
            {
                s.loss.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        self.emit();
    }
}

/// Transient only: it has no accepted turn or actor identifier and cannot emit
/// until the actual native durable acceptance object is available.
pub struct Deferred {
    stage: Stage,
    started: Instant,
    ended: Instant,
    analysis: Option<avesra_contracts::voice_timing::Analysis>,
}
impl Deferred {
    pub fn interval(stage: Stage, started: Instant, ended: Instant) -> Self {
        Self {
            stage,
            started,
            ended,
            analysis: None,
        }
    }
    pub fn analysis(&mut self, value: Option<avesra_contracts::voice_timing::Analysis>) {
        self.analysis = value;
    }
    pub fn promote(self, turn: &crate::conversations::DurableTurn) {
        let Some(s) = SINK.get().filter(|s| s.host == Host::Native) else {
            return;
        };
        let Some(start_us) = self.started.checked_duration_since(s.origin).map(micros) else {
            return;
        };
        let Some(duration) = self.ended.checked_duration_since(self.started) else {
            return;
        };
        let Some(age) = Instant::now().checked_duration_since(self.started) else {
            return;
        };
        let source = turn.source();
        if self
            .analysis
            .as_ref()
            .is_some_and(|v| v.validate(source.utterance).is_err())
        {
            s.loss.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let r = Record {
            id: Uuid::new_v4(),
            link: Link {
                turn: turn.id(),
                actor: turn.actor(),
                device: source.device,
                operation: source.utterance,
                parent: Some(turn.id()),
            },
            host: s.host,
            process: s.process,
            stage: self.stage,
            outcome: Outcome::Complete,
            error: None,
            at_ms: now().saturating_sub(u64::try_from(age.as_millis()).unwrap_or(u64::MAX)),
            start_us,
            duration_us: micros(duration),
            queue_us: None,
            retries: 0,
            deployment: Deployment::default(),
            analysis: self.analysis,
        };
        if s.tx.try_send(Message::Record(r)).is_err() {
            s.loss.fetch_add(1, Ordering::Relaxed);
        }
    }
}
/// Controller-local clock observation, not a retained preacceptance span.
pub struct AudioTimer {
    started: Instant,
    stamp: Option<(Uuid, u64, u64)>,
}
impl AudioTimer {
    pub fn start() -> Self {
        let started = Instant::now();
        let stamp = SINK.get().filter(|s| s.host == Host::Controller).map(|s| {
            (
                s.process,
                now(),
                micros(started.saturating_duration_since(s.origin)),
            )
        });
        Self { started, stamp }
    }
    pub fn finish(
        self,
        worker: Uuid,
        lane: avesra_contracts::voice_timing::Lane,
        revision: &str,
    ) -> Option<avesra_contracts::voice_timing::Receipt> {
        let (process, at_ms, start_us) = self.stamp?;
        Some(avesra_contracts::voice_timing::Receipt {
            worker,
            host: avesra_contracts::voice_timing::Host::Controller,
            process,
            lane,
            model_revision: revision.into(),
            at_ms,
            start_us,
            duration_us: micros(self.started.elapsed()),
        })
    }
}

/// OTLP/HTTP JSON projection. Full native records accompany this projection in
/// exports; operation links are attributes, never fabricated parent span IDs.
pub fn otlp(snapshots: &[&Snapshot]) -> serde_json::Value {
    let mut resources=snapshots.iter().map(|snapshot| {
        let spans=snapshot.records.iter().map(|r| {
            let attr=|key:&str,value:String|serde_json::json!({"key":key,"value":{"stringValue":value}});
            let mut attrs=vec![attr("avesra.record.id",r.id.to_string()),attr("avesra.operation.id",r.link.operation.to_string()),attr("avesra.process.id",r.process.to_string()),attr("avesra.outcome",serde_json::to_string(&r.outcome).unwrap_or_default()),attr("avesra.duration.us",r.duration_us.to_string()),attr("avesra.clock.offset","unavailable".into()),attr("avesra.clock.monotonic.start_us",r.start_us.to_string()),attr("avesra.retry.count",r.retries.to_string())];
            if let Some(parent)=r.link.parent{attrs.push(attr("avesra.parent.operation.id",parent.to_string()));}
            if let Some(queue)=r.queue_us{attrs.push(attr("avesra.queue.us",queue.to_string()));}
            if let Some(error)=r.error{attrs.push(attr("avesra.error.code",serde_json::to_string(&error).unwrap_or_default()));}
            for (key,value) in [("avesra.model.revision",&r.deployment.model),("avesra.engine.image",&r.deployment.image),("avesra.config.digest",&r.deployment.config)] {attrs.push(attr(key,value.clone().unwrap_or_else(||"unavailable".into())));}
            let start=u128::from(r.at_ms)*1_000_000;
            serde_json::json!({"traceId":r.link.turn.simple().to_string(),"spanId":&r.id.simple().to_string()[..16],"name":serde_json::to_string(&r.stage).unwrap_or_default().trim_matches('"'),"kind":1,"startTimeUnixNano":start.to_string(),"endTimeUnixNano":(start+u128::from(r.duration_us)*1000).to_string(),"attributes":attrs,"status":{"code":if matches!(r.outcome,Outcome::Complete){1}else{2}}})
        }).collect::<Vec<_>>();
        serde_json::json!({"resource":{"attributes":[{"key":"service.name","value":{"stringValue":match snapshot.host{Host::Native=>"avesra-native",Host::Controller=>"avesra-controller"}}}]},"scopeSpans":[{"scope":{"name":"avesra.accepted","version":"1"},"spans":spans}]})
    }).collect::<Vec<_>>();
    for record in snapshots.iter().flat_map(|snapshot| &snapshot.records) {
        if let Some(analysis) = &record.analysis {
            let spans=analysis.receipts.iter().map(|r|{
                let attr=|key:&str,value:String|serde_json::json!({"key":key,"value":{"stringValue":value}});
                let start=u128::from(r.at_ms)*1_000_000;
                serde_json::json!({"traceId":record.link.turn.simple().to_string(),"spanId":&r.worker.simple().to_string()[..16],"name":match r.lane{avesra_contracts::voice_timing::Lane::Asr=>"asr_driver_round_trip",avesra_contracts::voice_timing::Lane::Speaker=>"speaker_driver_round_trip"},"kind":1,"startTimeUnixNano":start.to_string(),"endTimeUnixNano":(start+u128::from(r.duration_us)*1000).to_string(),"attributes":[attr("avesra.operation.id",r.worker.to_string()),attr("avesra.parent.operation.id",analysis.request.to_string()),attr("avesra.process.id",r.process.to_string()),attr("avesra.clock.monotonic.start_us",r.start_us.to_string()),attr("avesra.clock.offset","unavailable".into()),attr("avesra.model.revision",r.model_revision.clone()),attr("avesra.measurement","controller_driver_round_trip".into())],"status":{"code":1}})
            }).collect::<Vec<_>>();
            resources.push(serde_json::json!({"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"avesra-controller"}}]},"scopeSpans":[{"scope":{"name":"avesra.voice_analysis","version":"1"},"spans":spans}]}));
        }
    }
    serde_json::json!({"resourceSpans":resources})
}

/// Nonserialized observer owned by one genuinely accepted native voice caller.
/// Moving it to the actual output coordinator never renews the endpoint clock.
pub struct ResponseTiming {
    pending: Option<Span>,
    output: Option<Uuid>,
}
impl ResponseTiming {
    pub fn accepted(turn: &crate::conversations::DurableTurn, endpoint: Instant) -> Self {
        let source = turn.source();
        let link = Link {
            turn: turn.id(),
            actor: turn.actor(),
            device: source.device,
            operation: source.utterance,
            parent: Some(turn.id()),
        };
        let mut span = begin(link, Stage::EndpointResponseSubmission);
        if let Some(s) = SINK.get()
            && let Some(record) = &mut span.record
        {
            if let (Some(start), Some(age)) = (
                endpoint.checked_duration_since(s.origin),
                Instant::now().checked_duration_since(endpoint),
            ) {
                span.started = endpoint;
                record.start_us = micros(start);
                record.at_ms =
                    now().saturating_sub(u64::try_from(age.as_millis()).unwrap_or(u64::MAX));
            } else {
                span.record = None;
                s.loss.fetch_add(1, Ordering::Relaxed);
            }
        }
        Self {
            pending: Some(span),
            output: None,
        }
    }
    /// The caller supplies the actual PublishedReply context and output UUID.
    /// A mismatched turn cannot consume this observation.
    pub fn bind_output(&mut self, link: Link) {
        let Some(s) = SINK.get() else {
            return;
        };
        let Some(record) = self.pending.as_ref().and_then(|span| span.record.as_ref()) else {
            return;
        };
        if !link.valid()
            || record.link.turn != link.turn
            || record.link.actor != link.actor
            || record.link.device != link.device
        {
            return;
        }
        let Ok(mut pending) = s.responses.lock() else {
            return;
        };
        if pending.len() >= 8 || pending.iter().any(|(id, _)| *id == link.operation) {
            s.loss.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if let Some(mut span) = self.pending.take() {
            if let Some(record) = &mut span.record {
                record.link = link;
            }
            pending.push((link.operation, span));
            self.output = Some(link.operation);
        }
    }
    fn take(&mut self) -> Option<Span> {
        if let Some(id) = self.output.take()
            && let Some(s) = SINK.get()
            && let Ok(mut pending) = s.responses.lock()
            && let Some(index) = pending.iter().position(|(key, _)| *key == id)
        {
            return Some(pending.swap_remove(index).1);
        }
        self.pending.take()
    }
    pub fn finish(mut self, outcome: Outcome) {
        if let Some(span) = self.take() {
            span.finish(outcome, None);
        }
    }
}
impl Drop for ResponseTiming {
    fn drop(&mut self) {
        drop(self.take());
    }
}
/// Only an observed nonzero speech component may call this; the exact live
/// output epoch and finite postmix samples are checked by the native media owner.
pub fn response_submitted(id: Uuid, at: Instant) {
    if let Some(s) = SINK.get()
        && let Ok(mut pending) = s.responses.lock()
        && let Some(index) = pending.iter().position(|(key, _)| *key == id)
        && at >= pending[index].1.started
    {
        let (_, mut span) = pending.swap_remove(index);
        if let Some(record) = &mut span.record {
            record.outcome = Outcome::Complete;
        }
        span.emit_at(at);
    }
}

/// Off-callback first-submission correlation, held by the actual output owner.
pub struct OutputGuard(Uuid);
pub fn output(link: Link) -> Option<OutputGuard> {
    let s = SINK.get()?;
    let mut pending = s.outputs.lock().ok()?;
    if pending.len() >= 8 || pending.iter().any(|(id, _)| *id == link.operation) {
        s.loss.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    pending.push((link.operation, begin(link, Stage::Submitted)));
    Some(OutputGuard(link.operation))
}
pub fn submitted(id: Uuid, at: Instant) {
    if let Some(s) = SINK.get()
        && let Ok(mut pending) = s.outputs.lock()
        && let Some(i) = pending.iter().position(|(key, _)| *key == id)
    {
        let (_, mut span) = pending.swap_remove(i);
        if at >= span.started {
            if let Some(record) = &mut span.record {
                record.outcome = Outcome::Complete;
            }
            span.emit_at(at);
        }
    }
}
impl Drop for OutputGuard {
    fn drop(&mut self) {
        if let Some(s) = SINK.get()
            && let Ok(mut pending) = s.outputs.lock()
            && let Some(i) = pending.iter().position(|(id, _)| *id == self.0)
        {
            pending.swap_remove(i);
        }
    }
}

impl Snapshot {
    pub fn validate(
        &self,
        host: Host,
        actor: Uuid,
        device: Uuid,
        turn: Option<Uuid>,
    ) -> Result<(), ErrorCode> {
        let digest = |v: &Option<String>| {
            v.as_ref().is_none_or(|v| {
                (v.len() == 40 || v.len() == 64)
                    && v.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        };
        self.resources.validate(host)?;
        self.engine.validate(host, actor, device, turn)?;
        if self.version != QUERY_VERSION
            || self.engine.process != self.process
            || self.host != host
            || self.process.is_nil()
            || !(1..=7).contains(&self.trace_days)
            || self.records.len() > 2048
            || self.rollups.len() > 8192
            || self.records.iter().any(|r| {
                r.host != host
                    || r.id.is_nil()
                    || r.id.as_bytes()[..8].iter().all(|b| *b == 0)
                    || r.process.is_nil()
                    || !r.link.valid()
                    || r.link.actor != actor
                    || r.link.device != device
                    || turn.is_some_and(|id| r.link.turn != id)
                    || !digest(&r.deployment.model)
                    || !digest(&r.deployment.image)
                    || !digest(&r.deployment.config)
                    || r.analysis.as_ref().is_some_and(|value| {
                        !matches!(r.stage, Stage::VoiceAnalysis)
                            || r.host != Host::Native
                            || value.validate(r.link.operation).is_err()
                    })
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}

/// Local installation telemetry only. Never exposed through accepted trace query.
pub fn app_snapshot() -> Result<crate::app_timing::Snapshot, ErrorCode> {
    let sink = SINK.get().ok_or(ErrorCode::Unavailable)?;
    let (reply, receiver) = mpsc::sync_channel(1);
    let present = Arc::new(AtomicBool::new(true));
    sink.tx
        .try_send(Message::ReadApp {
            reply,
            deadline: Instant::now() + Duration::from_secs(2),
            present: present.clone(),
        })
        .map_err(|_| ErrorCode::Unavailable)?;
    let result = receiver
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| ErrorCode::Expired);
    present.store(false, Ordering::SeqCst);
    result?
}
pub fn app_frontend_loss(value: u64) {
    if let Some(sink) = SINK.get() {
        sink.frontend_loss
            .fetch_add(value.min(1_000_000), Ordering::Relaxed);
    }
}
pub(crate) fn app_observe(
    operation: (crate::app_timing::Operation, Option<Uuid>),
    stage: crate::app_timing::Stage,
    outcome: crate::app_timing::Outcome,
    origin: crate::app_timing::Origin,
    started: Option<Instant>,
    duration: Duration,
) {
    let Some(sink) = SINK.get() else { return };
    if !operation.0.valid() || micros(duration) > crate::app_timing::MAX_DURATION_US {
        sink.app_loss.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let record = crate::app_timing::Record {
        version: 1,
        id: Uuid::new_v4(),
        process: sink.process,
        package_version: env!("CARGO_PKG_VERSION").into(),
        build_fingerprint: None,
        operation: operation.0,
        native_operation: operation.1,
        stage,
        outcome,
        origin,
        at_ms: now(),
        start_us: started
            .and_then(|at| at.checked_duration_since(sink.origin))
            .map(micros),
        duration_us: micros(duration),
    };
    if sink.tx.try_send(Message::AppTiming(record)).is_err() {
        sink.app_loss.fetch_add(1, Ordering::Relaxed);
    }
}
