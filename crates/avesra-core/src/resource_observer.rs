//! Fixed-field host telemetry, never an authority or accepted-turn record.
use crate::trace::Host;
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use uuid::Uuid;
pub const SAMPLE_CAP: usize = 8192;
pub const VIEW_CAP: usize = 720;
const DAY: u64 = 86_400_000;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    ProcessCpu,
    ProcessWorkingBytes,
    HostTotalBytes,
    HostAvailableBytes,
    HostSwapTotalBytes,
    HostSwapFreeBytes,
    HostUptimeMs,
    CgroupCpu,
    CgroupMemoryBytes,
    CgroupMemoryLimitBytes,
    CgroupSwapBytes,
    CgroupSwapLimitBytes,
    GpuBusy,
    GpuTemperatureMilliC,
    GpuPowerMilliW,
    GpuMemoryBytes,
    GpuMemoryTotalBytes,
}
impl Metric {
    pub fn gpu(self) -> bool {
        matches!(
            self,
            Self::GpuBusy
                | Self::GpuTemperatureMilliC
                | Self::GpuPowerMilliW
                | Self::GpuMemoryBytes
                | Self::GpuMemoryTotalBytes
        )
    }
    fn max(self) -> u64 {
        match self {
            Self::ProcessCpu | Self::CgroupCpu | Self::GpuBusy => 10_000,
            Self::GpuTemperatureMilliC => 200_000,
            Self::GpuPowerMilliW => 10_000_000,
            Self::HostUptimeMs => 31_536_000_000_000,
            _ => 1 << 46,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Unsupported,
    Unavailable,
    Invalid,
    Warmup,
    Reset,
    Gap,
    Timeout,
    Unlimited,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reading {
    Value { value: u64 },
    Unavailable { reason: Reason },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Value {
    pub metric: Metric,
    pub gpu: Option<u8>,
    pub reading: Reading,
}
impl Value {
    pub fn measured(metric: Metric, value: u64) -> Self {
        Self {
            metric,
            gpu: None,
            reading: if value <= metric.max() {
                Reading::Value { value }
            } else {
                Reading::Unavailable {
                    reason: Reason::Invalid,
                }
            },
        }
    }
    pub fn unavailable(metric: Metric, reason: Reason) -> Self {
        Self {
            metric,
            gpu: None,
            reading: Reading::Unavailable { reason },
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub host: Host,
    pub process: Uuid,
    pub boot: Option<Uuid>,
    pub process_started: Option<u64>,
    pub sequence: u64,
    pub at_ms: u64,
    pub monotonic_ms: u64,
    pub window_ms: Option<u64>,
    pub collection_ms: u64,
    pub values: Vec<Value>,
}
impl Sample {
    pub fn validate(&self, host: Host) -> Result<(), ErrorCode> {
        let mut keys = std::collections::BTreeSet::new();
        if self.host != host
            || self.process.is_nil()
            || self.boot.is_some_and(|v| v.is_nil())
            || self.sequence == 0
            || self.sequence > avesra_contracts::browser::MAX_SAFE_COUNTER
            || self.at_ms > avesra_contracts::browser::MAX_SAFE_COUNTER
            || self.monotonic_ms > avesra_contracts::browser::MAX_SAFE_COUNTER
            || self
                .process_started
                .is_some_and(|v| v == 0 || v > avesra_contracts::browser::MAX_SAFE_COUNTER)
            || self.values.is_empty()
            || self.values.len() > 32
            || self.window_ms.is_some_and(|v| v > 86_400_000)
            || self.collection_ms > 86_400_000
            || self.values.iter().any(|v| {
                !keys.insert((v.metric, v.gpu))
                    || (!v.metric.gpu() && v.gpu.is_some())
                    || (v.metric.gpu()
                        && v.gpu.is_none()
                        && matches!(v.reading, Reading::Value { .. }))
                    || v.gpu.is_some_and(|v| v >= 4)
                    || matches!(v.reading,Reading::Value{value} if value>v.metric.max())
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rollup {
    pub day: u64,
    pub metric: Metric,
    pub gpu: Option<u8>,
    pub reason: Option<Reason>,
    pub count: u64,
    pub total: u64,
    pub minimum: Option<u64>,
    pub maximum: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub samples: Vec<Sample>,
    pub rollups: Vec<Rollup>,
    pub evicted: u64,
    pub truncated: bool,
    pub latest_age_ms: Option<u64>,
}
impl Snapshot {
    pub fn validate(&self, host: Host) -> Result<(), ErrorCode> {
        if self.samples.len() > VIEW_CAP || self.rollups.len() > 8192 {
            return Err(ErrorCode::TooLarge);
        }
        if self.latest_age_ms.is_some() && self.samples.is_empty() {
            return Err(ErrorCode::Malformed);
        }
        for v in &self.samples {
            v.validate(host)?;
        }
        for v in &self.rollups {
            if v.day > 100_000_000
                || (!v.metric.gpu() && v.gpu.is_some())
                || (v.metric.gpu() && v.gpu.is_none() && v.reason.is_none())
                || v.gpu.is_some_and(|n| n >= 4)
                || v.count == 0
                || v.count > 100_000
                || v.minimum.is_some_and(|n| n > v.metric.max())
                || v.maximum.is_some_and(|n| n > v.metric.max())
                || (v.reason.is_some()
                    && (v.total != 0 || v.minimum.is_some() || v.maximum.is_some()))
                || (v.reason.is_none()
                    && (v.minimum.is_none() || v.maximum.is_none() || v.minimum > v.maximum))
                || v.total > v.metric.max().saturating_mul(v.count)
                || v.minimum
                    .is_some_and(|n| v.total < n.saturating_mul(v.count))
                || v.maximum
                    .is_some_and(|n| v.total > n.saturating_mul(v.count))
            {
                return Err(ErrorCode::Malformed);
            }
        }
        Ok(())
    }
}
pub struct Clock {
    host: Host,
    process: Uuid,
    origin: Instant,
    sequence: u64,
}
impl Clock {
    pub fn new(host: Host) -> Self {
        let (process, origin) =
            crate::trace::resource_clock(host).unwrap_or_else(|| (Uuid::new_v4(), Instant::now()));
        Self {
            host,
            process,
            origin,
            sequence: 0,
        }
    }
    pub fn sample(
        &mut self,
        observed: Instant,
        boot: Option<Uuid>,
        process_started: Option<u64>,
        window: Option<Duration>,
        values: Vec<Value>,
    ) {
        self.sequence = self.sequence.saturating_add(1);
        let collection_ms = millis(observed.elapsed());
        let sample = Sample {
            host: self.host,
            process: self.process,
            boot,
            process_started,
            sequence: self.sequence,
            at_ms: now().saturating_sub(collection_ms),
            monotonic_ms: millis(observed.saturating_duration_since(self.origin)),
            window_ms: window.map(millis),
            collection_ms,
            values,
        };
        crate::trace::resource(sample, observed);
    }
}
pub fn millis(v: Duration) -> u64 {
    u64::try_from(v.as_millis()).unwrap_or(u64::MAX)
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(millis)
        .unwrap_or(0)
}
/// Counter units are CPU nanoseconds; value is basis points of all logical CPUs.
pub fn cpu(
    metric: Metric,
    previous: Option<(u64, Instant)>,
    current: u64,
    at: Instant,
    processors: u32,
) -> Value {
    let Some((before, start)) = previous else {
        return Value::unavailable(metric, Reason::Warmup);
    };
    let Some(delta) = current.checked_sub(before) else {
        return Value::unavailable(metric, Reason::Reset);
    };
    let Some(window) = at.checked_duration_since(start) else {
        return Value::unavailable(metric, Reason::Reset);
    };
    if !(Duration::from_secs(1)..=Duration::from_secs(15)).contains(&window) || processors == 0 {
        return Value::unavailable(metric, Reason::Gap);
    };
    let denominator = window.as_nanos() * u128::from(processors);
    let value = u128::from(delta) * 10_000 / denominator;
    u64::try_from(value).map_or_else(
        |_| Value::unavailable(metric, Reason::Invalid),
        |v| Value::measured(metric, v),
    )
}
fn sql<T>(v: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    v.map_err(|_| ErrorCode::Storage)
}
fn integer(v: u64) -> Result<i64, ErrorCode> {
    i64::try_from(v).map_err(|_| ErrorCode::TooLarge)
}
pub(crate) fn initialize(db: &Connection) -> Result<(), ErrorCode> {
    sql(db.execute_batch("CREATE TABLE IF NOT EXISTS resource_samples(id INTEGER PRIMARY KEY,at_ms INTEGER NOT NULL,body TEXT NOT NULL CHECK(length(body)<=8192)); CREATE TABLE IF NOT EXISTS resource_rollups(day INTEGER NOT NULL,metric TEXT NOT NULL,gpu INTEGER NOT NULL,reason TEXT NOT NULL,count INTEGER NOT NULL,total INTEGER NOT NULL,minimum INTEGER,maximum INTEGER,PRIMARY KEY(day,metric,gpu,reason)); CREATE TABLE IF NOT EXISTS resource_retention(id INTEGER PRIMARY KEY CHECK(id=1),evicted INTEGER NOT NULL); INSERT OR IGNORE INTO resource_retention VALUES(1,0);"))
}
fn compact(db: &Connection, days: u8) -> Result<(), ErrorCode> {
    let count=sql(db.execute("DELETE FROM resource_samples WHERE at_ms<?1 OR id IN(SELECT id FROM resource_samples ORDER BY id DESC LIMIT -1 OFFSET 8192)",[integer(now().saturating_sub(DAY*u64::from(days)))?]))?;
    sql(db.execute(
        "UPDATE resource_retention SET evicted=evicted+?1 WHERE id=1",
        [i64::try_from(count).map_err(|_| ErrorCode::TooLarge)?],
    ))?;
    sql(db.execute("DELETE FROM resource_rollups WHERE day<?1 OR rowid IN(SELECT rowid FROM resource_rollups ORDER BY day DESC,rowid DESC LIMIT -1 OFFSET 8192)",[integer(now().saturating_sub(DAY*30)/DAY)?]))?;
    Ok(())
}
pub(crate) fn write(db: &mut Connection, sample: &Sample) -> Result<(), ErrorCode> {
    sample.validate(sample.host)?;
    let tx = sql(db.transaction())?;
    let body = serde_json::to_string(sample).map_err(|_| ErrorCode::Malformed)?;
    sql(tx.execute(
        "INSERT INTO resource_samples(at_ms,body) VALUES(?1,?2)",
        params![integer(sample.at_ms)?, body],
    ))?;
    for v in &sample.values {
        let (reason, value) = match v.reading {
            Reading::Value { value } => (String::new(), Some(integer(value)?)),
            Reading::Unavailable { reason } => (
                serde_json::to_string(&reason).map_err(|_| ErrorCode::Malformed)?,
                None,
            ),
        };
        sql(tx.execute("INSERT INTO resource_rollups VALUES(?1,?2,?3,?4,1,?5,?6,?6) ON CONFLICT(day,metric,gpu,reason) DO UPDATE SET count=count+1,total=total+excluded.total,minimum=min(minimum,excluded.minimum),maximum=max(maximum,excluded.maximum)",params![integer(sample.at_ms/DAY)?,serde_json::to_string(&v.metric).map_err(|_|ErrorCode::Malformed)?,v.gpu.map(i64::from).unwrap_or(-1),reason,value.unwrap_or(0),value]))?;
    }
    let days = sql(tx.query_row("SELECT days FROM retention WHERE id=1", [], |r| r.get(0)))?;
    compact(&tx, days)?;
    sql(tx.commit())
}
pub(crate) fn read(db: &Connection, days: u8, host: Host) -> Result<Snapshot, ErrorCode> {
    compact(db, days)?;
    let mut query =
        sql(db.prepare("SELECT body FROM resource_samples ORDER BY id DESC LIMIT 721"))?;
    let rows = sql(query.query_map([], |r| r.get::<_, String>(0)))?;
    let mut samples = Vec::new();
    for row in rows {
        samples.push(serde_json::from_str::<Sample>(&sql(row)?).map_err(|_| ErrorCode::Malformed)?);
    }
    let truncated = samples.len() > VIEW_CAP;
    samples.truncate(VIEW_CAP);
    samples.reverse();
    let mut query=sql(db.prepare("SELECT day,metric,gpu,reason,count,total,minimum,maximum FROM resource_rollups ORDER BY day DESC LIMIT 8192"))?;
    let mut rows = sql(query.query([]))?;
    let mut rollups = Vec::new();
    while let Some(row) = sql(rows.next())? {
        let n = |i| -> Result<u64, ErrorCode> {
            u64::try_from(sql(row.get::<_, i64>(i))?).map_err(|_| ErrorCode::Malformed)
        };
        let optional = |i| -> Result<Option<u64>, ErrorCode> {
            sql(row.get::<_, Option<i64>>(i))?
                .map(|v| u64::try_from(v).map_err(|_| ErrorCode::Malformed))
                .transpose()
        };
        let reason: String = sql(row.get(3))?;
        let gpu: i64 = sql(row.get(2))?;
        rollups.push(Rollup {
            day: n(0)?,
            metric: serde_json::from_str(&sql(row.get::<_, String>(1))?)
                .map_err(|_| ErrorCode::Malformed)?,
            gpu: if gpu == -1 {
                None
            } else {
                Some(u8::try_from(gpu).map_err(|_| ErrorCode::Malformed)?)
            },
            reason: if reason.is_empty() {
                None
            } else {
                Some(serde_json::from_str(&reason).map_err(|_| ErrorCode::Malformed)?)
            },
            count: n(4)?,
            total: n(5)?,
            minimum: optional(6)?,
            maximum: optional(7)?,
        });
    }
    let evicted: i64 = sql(db.query_row(
        "SELECT evicted FROM resource_retention WHERE id=1",
        [],
        |r| r.get(0),
    ))?;
    // Current-process monotonic freshness is supplied independently by the sink.
    let latest_age_ms = crate::trace::resource_age(samples.last());
    let value = Snapshot {
        samples,
        rollups,
        evicted: u64::try_from(evicted).map_err(|_| ErrorCode::Malformed)?,
        truncated,
        latest_age_ms,
    };
    value.validate(host)?;
    Ok(value)
}
