//! Actual controller admission and channel events. Never an authority.
use crate::trace::{Host, Link, Outcome};
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Instant,
};
use uuid::Uuid;
const DAY: u64 = 86_400_000;
const SAFE: u64 = avesra_contracts::browser::MAX_SAFE_COUNTER;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Reasoning,
    Asr,
    Speaker,
    Tts,
    Activity,
    VoiceDesign,
    UnspecifiedAudio,
}
impl Lane {
    pub fn audio(value: Option<&str>) -> Self {
        match value {
            Some("asr") => Self::Asr,
            Some("speaker") => Self::Speaker,
            Some("tts") => Self::Tts,
            Some("activity") => Self::Activity,
            Some("voice-design") => Self::VoiceDesign,
            _ => Self::UnspecifiedAudio,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Reasoning,
    Load,
    Infer,
    AsrStream,
    TtsStream,
    ActivityStream,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub lane: Lane,
    pub operation: Operation,
    pub attempts: u64,
    pub busy: u64,
    pub closed: u64,
}
struct State {
    admissions: BTreeMap<(Lane, Operation), Admission>,
    daily: BTreeMap<(u64, Lane, Operation), Admission>,
    dirty: BTreeSet<(u64, Lane, Operation)>,
    queues: BTreeMap<Uuid, Weak<Mutex<Inner>>>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            admissions: BTreeMap::new(),
            daily: BTreeMap::new(),
            dirty: BTreeSet::new(),
            queues: BTreeMap::new(),
        })
    })
}
fn loss() {
    crate::trace::observer_loss();
}
pub fn admission(lane: Lane, operation: Operation, busy: bool, closed: bool) {
    if busy && closed {
        loss();
        return;
    }
    let Ok(mut state) = state().lock() else {
        loss();
        return;
    };
    let update = |value: &mut Admission| {
        if value.attempts >= SAFE {
            loss();
            return;
        }
        value.attempts += 1;
        if busy {
            value.busy += 1;
        }
        if closed {
            value.closed += 1;
        }
    };
    let empty = || Admission {
        lane,
        operation,
        attempts: 0,
        busy: 0,
        closed: 0,
    };
    update(
        state
            .admissions
            .entry((lane, operation))
            .or_insert_with(empty),
    );
    let key = (now() / DAY, lane, operation);
    update(state.daily.entry(key).or_insert_with(empty));
    state.dirty.insert(key);
    while state.daily.len() > 1260 {
        if let Some((key, _)) = state.daily.pop_first()
            && state.dirty.remove(&key)
        {
            loss();
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Times {
    pub count: u64,
    pub total_us: u64,
    pub maximum_us: u64,
}
impl Times {
    fn add(&mut self, elapsed: u64) {
        self.count = self.count.saturating_add(1);
        self.total_us = self.total_us.saturating_add(elapsed);
        self.maximum_us = self.maximum_us.max(elapsed);
    }
    fn valid(&self) -> bool {
        self.count <= 512
            && self.maximum_us <= SAFE
            && self.total_us <= SAFE
            && self.maximum_us <= self.total_us
            && (self.count != 0 || (self.total_us == 0 && self.maximum_us == 0))
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub enqueued: u64,
    pub dequeued: u64,
    pub dropped: u64,
    pub maximum_outstanding: u16,
    pub dwell: Times,
    pub discarded_dwell: Times,
    pub capacity_wait: Times,
    pub failed_wait: Times,
    pub abandoned_wait: Times,
}
impl Stats {
    fn valid(&self) -> bool {
        self.enqueued <= 512
            && self.dequeued <= self.enqueued
            && self.dropped <= self.enqueued
            && self.dequeued + self.dropped <= self.enqueued
            && self.maximum_outstanding <= 65
            && self.dwell.count == self.dequeued
            && self.discarded_dwell.count == self.dropped
            && [
                &self.dwell,
                &self.discarded_dwell,
                &self.capacity_wait,
                &self.failed_wait,
                &self.abandoned_wait,
            ]
            .iter()
            .all(|v| v.valid())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueRecord {
    pub id: Uuid,
    pub process: Uuid,
    pub link: Link,
    pub at_ms: u64,
    pub elapsed_us: u64,
    pub outcome: Outcome,
    pub stats: Stats,
}
impl QueueRecord {
    fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.process.is_nil()
            && self.link.valid()
            && self.at_ms <= SAFE
            && self.elapsed_us <= SAFE
            && self.stats.valid()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveQueue {
    pub id: Uuid,
    pub process: Uuid,
    pub link: Link,
    pub capacity: u16,
    pub outstanding: u16,
    pub oldest_outstanding_age_us: Option<u64>,
    pub stats: Stats,
}
struct Inner {
    id: Uuid,
    process: Uuid,
    link: Link,
    started: Instant,
    at_ms: u64,
    next: u64,
    pending: BTreeMap<u64, Instant>,
    stats: Stats,
    outcome: Outcome,
}
impl Drop for Inner {
    fn drop(&mut self) {
        crate::trace::engine_queue(QueueRecord {
            id: self.id,
            process: self.process,
            link: self.link,
            at_ms: self.at_ms,
            elapsed_us: elapsed(self.started),
            outcome: self.outcome,
            stats: self.stats.clone(),
        });
    }
}
/// Only actual normal-speech channel ownership constructs this observer.
pub struct Queue(Arc<Mutex<Inner>>);
impl Queue {
    pub fn new(link: Link) -> Option<Self> {
        let (process, _) = crate::trace::resource_clock(Host::Controller)?;
        if !link.valid() {
            loss();
            return None;
        }
        let mut state = state().lock().map_err(|_| loss()).ok()?;
        state.queues.retain(|_, v| v.strong_count() > 0);
        if state.queues.len() >= 8 {
            loss();
            return None;
        }
        let id = Uuid::new_v4();
        let inner = Arc::new(Mutex::new(Inner {
            id,
            process,
            link,
            started: Instant::now(),
            at_ms: now(),
            next: 0,
            pending: BTreeMap::new(),
            stats: Stats::default(),
            outcome: Outcome::Abandoned,
        }));
        state.queues.insert(id, Arc::downgrade(&inner));
        Some(Self(inner))
    }
    pub fn waiting(&self) -> Wait {
        Wait {
            owner: self.0.clone(),
            started: Instant::now(),
            finished: false,
        }
    }
    /// Called after actual channel capacity was reserved, immediately before send.
    pub fn enqueue(&self) -> Option<Ticket> {
        let mut inner = self.0.lock().map_err(|_| loss()).ok()?;
        if inner.pending.len() >= 65 || inner.next >= 512 {
            loss();
            return None;
        }
        inner.next += 1;
        let id = inner.next;
        inner.pending.insert(id, Instant::now());
        inner.stats.enqueued += 1;
        inner.stats.maximum_outstanding = inner
            .stats
            .maximum_outstanding
            .max(u16::try_from(inner.pending.len()).unwrap_or(65));
        Some(Ticket {
            owner: self.0.clone(),
            id,
        })
    }
    pub fn finish(&self, success: bool) {
        if let Ok(mut value) = self.0.lock() {
            value.outcome = if success {
                Outcome::Complete
            } else {
                Outcome::Failed
            };
        } else {
            loss();
        }
    }
}
pub struct Wait {
    owner: Arc<Mutex<Inner>>,
    started: Instant,
    finished: bool,
}
impl Wait {
    pub fn finish(mut self, success: bool) {
        if let Ok(mut value) = self.owner.lock() {
            if success {
                value.stats.capacity_wait.add(elapsed(self.started));
            } else {
                value.stats.failed_wait.add(elapsed(self.started));
            }
        } else {
            loss();
        }
        self.finished = true;
    }
}
impl Drop for Wait {
    fn drop(&mut self) {
        if !self.finished {
            if let Ok(mut value) = self.owner.lock() {
                value.stats.abandoned_wait.add(elapsed(self.started));
            } else {
                loss();
            }
        }
    }
}
pub struct Ticket {
    owner: Arc<Mutex<Inner>>,
    id: u64,
}
impl Ticket {
    pub fn received(mut self) {
        self.remove(true);
        self.id = 0;
    }
    fn remove(&mut self, received: bool) {
        if self.id == 0 {
            return;
        }
        if let Ok(mut value) = self.owner.lock() {
            if let Some(started) = value.pending.remove(&self.id) {
                if received {
                    value.stats.dequeued += 1;
                    value.stats.dwell.add(elapsed(started));
                } else {
                    value.stats.dropped += 1;
                    value.stats.discarded_dwell.add(elapsed(started));
                }
            } else {
                loss();
            }
        } else {
            loss();
        }
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.remove(false);
    }
}
fn elapsed(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros())
        .unwrap_or(SAFE)
        .min(SAFE)
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|v| u64::try_from(v.as_millis()).ok())
        .unwrap_or(0)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyAdmission {
    pub day: u64,
    pub process: Uuid,
    pub lane: Lane,
    pub operation: Operation,
    pub attempts: u64,
    pub busy: u64,
    pub closed: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rollup {
    pub day: u64,
    pub outcome: Outcome,
    pub queues: u64,
    pub enqueued: u64,
    pub dequeued: u64,
    pub dropped: u64,
    pub dwell_us: u64,
    pub maximum_dwell_us: u64,
    pub capacity_wait_us: u64,
    pub discarded_dwell_us: u64,
    pub failed_wait_us: u64,
    pub abandoned_wait_us: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub supported: bool,
    pub process: Uuid,
    pub admissions: Vec<Admission>,
    pub daily_admissions: Vec<DailyAdmission>,
    pub live: Vec<LiveQueue>,
    pub queues: Vec<QueueRecord>,
    pub rollups: Vec<Rollup>,
    pub evicted: u64,
    pub admission_evicted: u64,
    pub rollup_evicted: u64,
    pub truncated: bool,
}
impl Snapshot {
    pub fn validate(
        &self,
        host: Host,
        actor: Uuid,
        device: Uuid,
        turn: Option<Uuid>,
    ) -> Result<(), ErrorCode> {
        let matches = |v: Link| {
            v.valid()
                && v.actor == actor
                && v.device == device
                && turn.is_none_or(|id| id == v.turn)
        };
        if self.process.is_nil()
            || self.evicted > SAFE
            || self.admission_evicted > SAFE
            || self.rollup_evicted > SAFE
            || self.supported != (host == Host::Controller)
            || self.admissions.len() > 42
            || self.daily_admissions.len() > 8192
            || self.live.len() > 8
            || self.queues.len() > 256
            || self.rollups.len() > 8192
        {
            return Err(ErrorCode::Malformed);
        }
        if !self.supported
            && (!self.admissions.is_empty()
                || !self.daily_admissions.is_empty()
                || !self.live.is_empty()
                || !self.queues.is_empty()
                || !self.rollups.is_empty())
        {
            return Err(ErrorCode::Malformed);
        }
        if self
            .admissions
            .iter()
            .any(|v| v.attempts > SAFE || v.busy > v.attempts || v.closed > v.attempts - v.busy)
            || self.daily_admissions.iter().any(|v| {
                v.process.is_nil()
                    || v.attempts > SAFE
                    || v.busy > v.attempts
                    || v.closed > v.attempts - v.busy
                    || v.day > 100_000_000
            })
            || self.queues.iter().any(|v| {
                !v.valid()
                    || !matches(v.link)
                    || v.stats.dequeued + v.stats.dropped != v.stats.enqueued
            })
            || self.live.iter().any(|v| {
                v.process != self.process
                    || v.id.is_nil()
                    || !matches(v.link)
                    || v.capacity != 64
                    || v.outstanding > 65
                    || !v.stats.valid()
                    || u64::from(v.outstanding)
                        != v.stats.enqueued - v.stats.dequeued - v.stats.dropped
                    || (v.outstanding == 0) != v.oldest_outstanding_age_us.is_none()
                    || v.oldest_outstanding_age_us.is_some_and(|v| v > SAFE)
            })
            || self.rollups.iter().any(|v| {
                v.day > 100_000_000
                    || v.queues == 0
                    || [
                        v.queues,
                        v.enqueued,
                        v.dequeued,
                        v.dropped,
                        v.dwell_us,
                        v.maximum_dwell_us,
                        v.capacity_wait_us,
                        v.discarded_dwell_us,
                        v.failed_wait_us,
                        v.abandoned_wait_us,
                    ]
                    .iter()
                    .any(|n| *n > SAFE)
                    || v.dequeued > v.enqueued
                    || v.dropped > v.enqueued
                    || v.dequeued + v.dropped != v.enqueued
                    || v.maximum_dwell_us > v.dwell_us
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
fn current(actor: Uuid, device: Uuid, turn: Option<Uuid>) -> (Vec<Admission>, Vec<LiveQueue>) {
    let Ok(mut state) = state().lock() else {
        loss();
        return (Vec::new(), Vec::new());
    };
    state.queues.retain(|_, value| value.strong_count() > 0);
    let admissions = state.admissions.values().cloned().collect();
    let owners = state
        .queues
        .values()
        .filter_map(Weak::upgrade)
        .collect::<Vec<_>>();
    drop(state);
    let live = owners
        .into_iter()
        .filter_map(|owner| {
            let v = owner.lock().map_err(|_| loss()).ok()?;
            if v.link.actor != actor
                || v.link.device != device
                || turn.is_some_and(|id| id != v.link.turn)
            {
                return None;
            }
            Some(LiveQueue {
                id: v.id,
                process: v.process,
                link: v.link,
                capacity: 64,
                outstanding: u16::try_from(v.pending.len()).ok()?,
                oldest_outstanding_age_us: v.pending.values().next().copied().map(elapsed),
                stats: v.stats.clone(),
            })
        })
        .collect();
    (admissions, live)
}
fn sql<T>(v: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    v.map_err(|_| ErrorCode::Storage)
}
fn i(v: u64) -> Result<i64, ErrorCode> {
    i64::try_from(v).map_err(|_| ErrorCode::TooLarge)
}
pub(crate) fn initialize(db: &Connection) -> Result<(), ErrorCode> {
    sql(db.execute_batch("CREATE TABLE IF NOT EXISTS engine_admissions(day INTEGER NOT NULL,process TEXT NOT NULL,lane TEXT NOT NULL,operation TEXT NOT NULL,attempts INTEGER NOT NULL,busy INTEGER NOT NULL,closed INTEGER NOT NULL,PRIMARY KEY(day,process,lane,operation));CREATE TABLE IF NOT EXISTS engine_queues(id TEXT PRIMARY KEY,actor TEXT NOT NULL,device TEXT NOT NULL,turn TEXT NOT NULL,at_ms INTEGER NOT NULL,body TEXT NOT NULL CHECK(length(body)<=8192));CREATE TABLE IF NOT EXISTS engine_rollups(actor TEXT NOT NULL,device TEXT NOT NULL,day INTEGER NOT NULL,outcome TEXT NOT NULL,queues INTEGER NOT NULL,enqueued INTEGER NOT NULL,dequeued INTEGER NOT NULL,dropped INTEGER NOT NULL,dwell INTEGER NOT NULL,max_dwell INTEGER NOT NULL,capacity_wait INTEGER NOT NULL,discarded INTEGER NOT NULL,failed_wait INTEGER NOT NULL,abandoned_wait INTEGER NOT NULL,PRIMARY KEY(actor,device,day,outcome));CREATE TABLE IF NOT EXISTS engine_retention(id INTEGER PRIMARY KEY CHECK(id=1),evicted INTEGER NOT NULL,admission_evicted INTEGER NOT NULL,rollup_evicted INTEGER NOT NULL);INSERT OR IGNORE INTO engine_retention VALUES(1,0,0,0);"))
}
fn compact(db: &Connection, days: u8) -> Result<(), ErrorCode> {
    let removed=sql(db.execute("DELETE FROM engine_queues WHERE at_ms<?1 OR rowid IN(SELECT rowid FROM engine_queues ORDER BY rowid DESC LIMIT -1 OFFSET 8192)",[i(now().saturating_sub(DAY*u64::from(days)))?]))?;
    sql(db.execute(
        "UPDATE engine_retention SET evicted=evicted+?1 WHERE id=1",
        [i64::try_from(removed).map_err(|_| ErrorCode::TooLarge)?],
    ))?;
    for (table, counter) in [
        ("engine_admissions", "admission_evicted"),
        ("engine_rollups", "rollup_evicted"),
    ] {
        let removed=sql(db.execute(&format!("DELETE FROM {table} WHERE day<?1 OR rowid IN(SELECT rowid FROM {table} ORDER BY day DESC,rowid DESC LIMIT -1 OFFSET 8192)"),[i(now().saturating_sub(DAY*30)/DAY)?]))?;
        sql(db.execute(
            &format!("UPDATE engine_retention SET {counter}={counter}+?1 WHERE id=1"),
            [i64::try_from(removed).map_err(|_| ErrorCode::TooLarge)?],
        ))?;
    }
    Ok(())
}
pub(crate) fn checkpoint(db: &Connection, process: Uuid) -> Result<(), ErrorCode> {
    let observed = state().lock().map_err(|_| ErrorCode::Unavailable)?;
    let rows = observed
        .dirty
        .iter()
        .filter_map(|key| observed.daily.get(key).map(|v| (*key, v.clone())))
        .collect::<Vec<_>>();
    drop(observed);
    for (key, value) in rows {
        sql(db.execute("INSERT INTO engine_admissions VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(day,process,lane,operation) DO UPDATE SET attempts=excluded.attempts,busy=excluded.busy,closed=excluded.closed",params![i(key.0)?,process.to_string(),serde_json::to_string(&value.lane).map_err(|_|ErrorCode::Malformed)?,serde_json::to_string(&value.operation).map_err(|_|ErrorCode::Malformed)?,i(value.attempts)?,i(value.busy)?,i(value.closed)?]))?;
        let mut state = state().lock().map_err(|_| ErrorCode::Unavailable)?;
        if state.daily.get(&key) == Some(&value) {
            state.dirty.remove(&key);
        }
    }
    let days = sql(
        db.query_row("SELECT days FROM retention WHERE id=1", [], |row| {
            row.get(0)
        }),
    )?;
    compact(db, days)
}
pub(crate) fn write(db: &mut Connection, value: &QueueRecord) -> Result<(), ErrorCode> {
    if !value.valid() || value.stats.enqueued != value.stats.dequeued + value.stats.dropped {
        return Err(ErrorCode::Malformed);
    }
    let tx = sql(db.transaction())?;
    let s = &value.stats;
    sql(tx.execute(
        "INSERT INTO engine_queues VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            value.id.to_string(),
            value.link.actor.to_string(),
            value.link.device.to_string(),
            value.link.turn.to_string(),
            i(value.at_ms)?,
            serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)?
        ],
    ))?;
    sql(tx.execute("INSERT INTO engine_rollups VALUES(?1,?2,?3,?4,1,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(actor,device,day,outcome) DO UPDATE SET queues=queues+1,enqueued=enqueued+excluded.enqueued,dequeued=dequeued+excluded.dequeued,dropped=dropped+excluded.dropped,dwell=dwell+excluded.dwell,max_dwell=max(max_dwell,excluded.max_dwell),capacity_wait=capacity_wait+excluded.capacity_wait,discarded=discarded+excluded.discarded,failed_wait=failed_wait+excluded.failed_wait,abandoned_wait=abandoned_wait+excluded.abandoned_wait",params![value.link.actor.to_string(),value.link.device.to_string(),i(value.at_ms/DAY)?,serde_json::to_string(&value.outcome).map_err(|_|ErrorCode::Malformed)?,i(s.enqueued)?,i(s.dequeued)?,i(s.dropped)?,i(s.dwell.total_us)?,i(s.dwell.maximum_us)?,i(s.capacity_wait.total_us)?,i(s.discarded_dwell.total_us)?,i(s.failed_wait.total_us)?,i(s.abandoned_wait.total_us)?]))?;
    let days = sql(
        tx.query_row("SELECT days FROM retention WHERE id=1", [], |row| {
            row.get(0)
        }),
    )?;
    compact(&tx, days)?;
    sql(tx.commit())
}
pub(crate) fn read(
    db: &Connection,
    host: Host,
    process: Uuid,
    actor: Uuid,
    device: Uuid,
    turn: Option<Uuid>,
    days: u8,
) -> Result<Snapshot, ErrorCode> {
    compact(db, days)?;
    let (admissions, live) = if host == Host::Controller {
        current(actor, device, turn)
    } else {
        (Vec::new(), Vec::new())
    };
    let mut query=sql(db.prepare("SELECT body FROM engine_queues WHERE actor=?1 AND device=?2 AND (?3 IS NULL OR turn=?3) ORDER BY rowid DESC LIMIT 257"))?;
    let rows = sql(query.query_map(
        params![
            actor.to_string(),
            device.to_string(),
            turn.map(|id| id.to_string())
        ],
        |row| row.get::<_, String>(0),
    ))?;
    let mut queues = Vec::new();
    for row in rows {
        queues.push(serde_json::from_str(&sql(row)?).map_err(|_| ErrorCode::Malformed)?);
    }
    let truncated = queues.len() > 256;
    queues.truncate(256);
    let mut query=sql(db.prepare("SELECT day,process,lane,operation,attempts,busy,closed FROM engine_admissions ORDER BY day DESC LIMIT 8192"))?;
    let mut rows = sql(query.query([]))?;
    let mut daily_admissions = Vec::new();
    while let Some(row) = sql(rows.next())? {
        let n =
            |index| u64::try_from(sql(row.get::<_, i64>(index))?).map_err(|_| ErrorCode::Malformed);
        daily_admissions.push(DailyAdmission {
            day: n(0)?,
            process: Uuid::parse_str(&sql(row.get::<_, String>(1))?)
                .map_err(|_| ErrorCode::Malformed)?,
            lane: serde_json::from_str(&sql(row.get::<_, String>(2))?)
                .map_err(|_| ErrorCode::Malformed)?,
            operation: serde_json::from_str(&sql(row.get::<_, String>(3))?)
                .map_err(|_| ErrorCode::Malformed)?,
            attempts: n(4)?,
            busy: n(5)?,
            closed: n(6)?,
        });
    }
    let mut query=sql(db.prepare("SELECT day,outcome,queues,enqueued,dequeued,dropped,dwell,max_dwell,capacity_wait,discarded,failed_wait,abandoned_wait FROM engine_rollups WHERE actor=?1 AND device=?2 ORDER BY day DESC LIMIT 8192"))?;
    let mut rows = sql(query.query(params![actor.to_string(), device.to_string()]))?;
    let mut rollups = Vec::new();
    while let Some(row) = sql(rows.next())? {
        let n =
            |index| u64::try_from(sql(row.get::<_, i64>(index))?).map_err(|_| ErrorCode::Malformed);
        rollups.push(Rollup {
            day: n(0)?,
            outcome: serde_json::from_str(&sql(row.get::<_, String>(1))?)
                .map_err(|_| ErrorCode::Malformed)?,
            queues: n(2)?,
            enqueued: n(3)?,
            dequeued: n(4)?,
            dropped: n(5)?,
            dwell_us: n(6)?,
            maximum_dwell_us: n(7)?,
            capacity_wait_us: n(8)?,
            discarded_dwell_us: n(9)?,
            failed_wait_us: n(10)?,
            abandoned_wait_us: n(11)?,
        });
    }
    let (evicted, admission_evicted, rollup_evicted): (i64, i64, i64) = sql(db.query_row(
        "SELECT evicted,admission_evicted,rollup_evicted FROM engine_retention WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ))?;
    let snapshot = Snapshot {
        supported: host == Host::Controller,
        process,
        admissions,
        daily_admissions,
        live,
        queues,
        rollups,
        evicted: u64::try_from(evicted).map_err(|_| ErrorCode::Malformed)?,
        admission_evicted: u64::try_from(admission_evicted).map_err(|_| ErrorCode::Malformed)?,
        rollup_evicted: u64::try_from(rollup_evicted).map_err(|_| ErrorCode::Malformed)?,
        truncated,
    };
    snapshot.validate(host, actor, device, turn)?;
    Ok(snapshot)
}
