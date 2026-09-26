//! Manual observer reports: annotations never become measured provenance or authority.
use crate::trace::{Deployment, Host, Outcome, Record, Snapshot, Stage};
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
pub const CAPACITY: usize = 16;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Temperature {
    Cold,
    Warm,
    Unknown,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    Idle,
    Observation,
    Background,
    LanImpairment,
    Gaming,
    Unknown,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    pub scenario: String,
    pub version: u16,
    pub temperature: Temperature,
    pub condition: Condition,
}
impl Annotation {
    fn validate(&self) -> Result<(), ErrorCode> {
        if self.scenario.is_empty()
            || self.scenario.len() > 48
            || !self
                .scenario
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || self.version == 0
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub annotation: Annotation,
    pub turns: Vec<Uuid>,
}
impl Request {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.annotation.validate()?;
        turns_valid(&self.turns)
    }
}
fn turns_valid(turns: &[Uuid]) -> Result<(), ErrorCode> {
    if turns.is_empty()
        || turns.len() > 64
        || turns.iter().any(Uuid::is_nil)
        || turns.iter().collect::<BTreeSet<_>>().len() != turns.len()
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub host: Host,
    pub query_process: Uuid,
    pub observer_loss: u64,
    pub evicted: u64,
    pub collector_starts: u64,
    pub truncated: bool,
    pub trace_days: u8,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub version: u16,
    pub id: Uuid,
    pub actor: Uuid,
    pub device: Uuid,
    pub created_ms: u64,
    pub annotation: Annotation,
    pub turns: Vec<Uuid>,
    pub coverage: Vec<Coverage>,
    pub records: Vec<Record>,
}
#[derive(Serialize)]
pub struct Summary {
    pub id: Uuid,
    pub created_ms: u64,
    pub annotation: Annotation,
    pub turns: usize,
    pub spans: usize,
    pub controller_available: bool,
    pub coverage: Vec<Coverage>,
}
impl Report {
    pub fn capture(
        actor: Uuid,
        device: Uuid,
        created_ms: u64,
        request: Request,
        local: &Snapshot,
        controller: Option<&Snapshot>,
    ) -> Result<Self, ErrorCode> {
        request.validate()?;
        local.validate(Host::Native, actor, device, None)?;
        if let Some(remote) = controller {
            remote.validate(Host::Controller, actor, device, None)?;
        }
        let snapshots = std::iter::once(local).chain(controller);
        let mut records = Vec::new();
        let mut coverage = Vec::new();
        for snapshot in snapshots {
            records.extend(
                snapshot
                    .records
                    .iter()
                    .filter(|r| request.turns.contains(&r.link.turn))
                    .cloned(),
            );
            coverage.push(Coverage {
                host: snapshot.host,
                query_process: snapshot.process,
                observer_loss: snapshot.observer_loss,
                evicted: snapshot.evicted,
                collector_starts: snapshot.collector_starts,
                truncated: snapshot.truncated,
                trace_days: snapshot.trace_days,
            });
        }
        let report = Self {
            version: 1,
            id: Uuid::new_v4(),
            actor,
            device,
            created_ms,
            annotation: request.annotation,
            turns: request.turns,
            coverage,
            records,
        };
        report.validate(actor, device)?;
        Ok(report)
    }
    pub fn validate(&self, actor: Uuid, device: Uuid) -> Result<(), ErrorCode> {
        self.annotation.validate()?;
        turns_valid(&self.turns)?;
        let digest = |value: &Option<String>| {
            value.as_ref().is_none_or(|v| {
                (v.len() == 40 || v.len() == 64)
                    && v.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        };
        let limit = avesra_contracts::browser::MAX_SAFE_COUNTER;
        if self.version != 1
            || self.id.is_nil()
            || actor.is_nil()
            || device.is_nil()
            || self.actor != actor
            || self.device != device
            || self.created_ms == 0
            || self.created_ms > limit
            || self.records.len() > 4096
            || self.coverage.is_empty()
            || self.coverage.len() > 2
            || self
                .coverage
                .iter()
                .filter(|c| c.host == Host::Native)
                .count()
                != 1
            || self
                .coverage
                .iter()
                .filter(|c| c.host == Host::Controller)
                .count()
                > 1
            || self.coverage.iter().any(|c| {
                c.query_process.is_nil()
                    || !(1..=7).contains(&c.trace_days)
                    || c.observer_loss > limit
                    || c.evicted > limit
                    || c.collector_starts > limit
            })
            || self
                .records
                .iter()
                .map(|r| r.id)
                .collect::<BTreeSet<_>>()
                .len()
                != self.records.len()
            || self.records.iter().any(|r| {
                r.id.is_nil()
                    || r.id.as_bytes()[..8].iter().all(|b| *b == 0)
                    || r.process.is_nil()
                    || r.link.actor != actor
                    || r.link.device != device
                    || r.link.operation.is_nil()
                    || r.link.parent.is_some_and(|v| v.is_nil())
                    || !self.turns.contains(&r.link.turn)
                    || !self.coverage.iter().any(|c| c.host == r.host)
                    || r.duration_us > limit
                    || r.at_ms > limit
                    || r.start_us > limit
                    || r.queue_us.is_some_and(|v| v > limit)
                    || !digest(&r.deployment.model)
                    || !digest(&r.deployment.image)
                    || !digest(&r.deployment.config)
                    || r.analysis.as_ref().is_some_and(|a| {
                        r.host != Host::Native
                            || !matches!(r.stage, Stage::VoiceAnalysis)
                            || a.validate(r.link.operation).is_err()
                    })
            })
            || self.turns.iter().any(|turn| {
                !self
                    .records
                    .iter()
                    .any(|r| r.host == Host::Native && r.link.turn == *turn)
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn summary(&self) -> Summary {
        Summary {
            id: self.id,
            created_ms: self.created_ms,
            annotation: self.annotation.clone(),
            turns: self.turns.len(),
            spans: self.records.len(),
            coverage: self.coverage.clone(),
            controller_available: self.coverage.iter().any(|c| c.host == Host::Controller),
        }
    }
}
#[derive(Serialize)]
pub struct Stats {
    pub turns: usize,
    pub spans: usize,
    pub timed_turns: usize,
    pub counts: BTreeMap<String, usize>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub failure_elapsed_p50_ms: Option<f64>,
    pub successful_submission_only: bool,
    pub provisional: bool,
    pub processes: Vec<Uuid>,
    pub deployments: Vec<Deployment>,
}
#[derive(Serialize)]
pub struct Row {
    pub scope: String,
    pub comparable: bool,
    pub issues: Vec<&'static str>,
    pub baseline: Option<Stats>,
    pub candidate: Option<Stats>,
}
#[derive(Serialize)]
pub struct Comparison {
    pub baseline: Summary,
    pub candidate: Summary,
    pub issues: Vec<&'static str>,
    pub rows: Vec<Row>,
}
fn name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unavailable".into())
}
fn groups(report: &Report) -> BTreeMap<String, Vec<&Record>> {
    let mut groups = BTreeMap::<String, Vec<&Record>>::new();
    for r in &report.records {
        groups
            .entry(format!("{} / {}", name(&r.host), name(&r.stage)))
            .or_default()
            .push(r);
    }
    groups
}
fn percentile(sorted: &[u64], numerator: usize) -> Option<f64> {
    (!sorted.is_empty())
        .then(|| sorted[(sorted.len() * numerator).div_ceil(100) - 1] as f64 / 1000.0)
}
fn stats(records: &[&Record]) -> Stats {
    let response = matches!(records[0].stage, Stage::EndpointResponseSubmission);
    let measured = records
        .iter()
        .copied()
        .filter(|r| !response || matches!(r.outcome, Outcome::Complete))
        .collect::<Vec<_>>();
    let mut durations = measured.iter().map(|r| r.duration_us).collect::<Vec<_>>();
    durations.sort_unstable();
    let mut failed = records
        .iter()
        .filter(|r| !matches!(r.outcome, Outcome::Complete))
        .map(|r| r.duration_us)
        .collect::<Vec<_>>();
    failed.sort_unstable();
    let mut counts = BTreeMap::new();
    let mut deployments = Vec::new();
    let mut seen = BTreeSet::new();
    for r in records {
        *counts.entry(name(&r.outcome)).or_default() += 1;
        let key = serde_json::to_string(&r.deployment).unwrap_or_default();
        if seen.insert(key) {
            deployments.push(r.deployment.clone());
        }
    }
    let timed_turns = measured
        .iter()
        .map(|r| r.link.turn)
        .collect::<BTreeSet<_>>()
        .len();
    Stats {
        turns: records
            .iter()
            .map(|r| r.link.turn)
            .collect::<BTreeSet<_>>()
            .len(),
        spans: records.len(),
        timed_turns,
        counts,
        p50_ms: percentile(&durations, 50),
        p95_ms: percentile(&durations, 95),
        p99_ms: percentile(&durations, 99),
        max_ms: durations.last().map(|v| *v as f64 / 1000.0),
        failure_elapsed_p50_ms: percentile(&failed, 50),
        successful_submission_only: response,
        provisional: timed_turns < 30,
        processes: records
            .iter()
            .map(|r| r.process)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        deployments,
    }
}
pub fn compare(baseline: &Report, candidate: &Report) -> Result<Comparison, ErrorCode> {
    baseline.validate(baseline.actor, baseline.device)?;
    candidate.validate(baseline.actor, baseline.device)?;
    if baseline.id == candidate.id {
        return Err(ErrorCode::Malformed);
    }
    let mut issues = vec![
        "Historical build/profile/boot provenance unavailable; descriptive comparison only",
        "Scenario conditions and independent repetitions are owner annotations, not measured facts",
    ];
    if baseline.annotation != candidate.annotation {
        issues.push("Declared scenario/version/conditions differ");
    }
    if matches!(baseline.annotation.temperature, Temperature::Unknown)
        || matches!(candidate.annotation.temperature, Temperature::Unknown)
        || matches!(baseline.annotation.condition, Condition::Unknown)
        || matches!(candidate.annotation.condition, Condition::Unknown)
    {
        issues.push("Declared conditions unavailable");
    }
    if baseline.turns.iter().any(|v| candidate.turns.contains(v)) {
        issues.push("Cohorts overlap in accepted turns");
    }
    for report in [baseline, candidate] {
        if !report.coverage.iter().any(|c| c.host == Host::Controller) {
            issues.push("Controller coverage unavailable");
        }
        if report
            .coverage
            .iter()
            .any(|c| c.truncated || c.observer_loss > 0 || c.evicted > 0)
        {
            issues.push("Query truncation or cumulative observer loss/eviction may omit evidence");
        }
    }
    issues.sort_unstable();
    issues.dedup();
    let a = groups(baseline);
    let b = groups(candidate);
    let mut keys = a.keys().chain(b.keys()).cloned().collect::<BTreeSet<_>>();
    keys.insert(format!(
        "{} / {}",
        name(&Host::Native),
        name(&Stage::EndpointResponseSubmission)
    ));
    let rows=keys.into_iter().map(|scope|{
        let left=a.get(&scope).map(|v|stats(v));let right=b.get(&scope).map(|v|stats(v));let mut row_issues=issues.clone();
        for (value,total) in [(left.as_ref(),baseline.turns.len()),(right.as_ref(),candidate.turns.len())]{
            match value {
                None=>row_issues.push("Stage missing from one cohort"),
                Some(v)=>{
                    if v.spans>v.turns{row_issues.push("Repeated stage attempts weight these span-level percentiles; not independent turns");}
                    if v.turns!=total{row_issues.push("Some selected turns lack this stage");}
                    if v.processes.len()!=1{row_issues.push("Multiple process identities in stage cohort");}
                    if v.deployments.len()!=1{row_issues.push("Multiple deployment tuples in stage cohort");}
                    if v.deployments.iter().any(|d|d.model.is_none()||d.image.is_none()||d.config.is_none()){row_issues.push("Deployment metadata incomplete");}
                }
            }
        }
        row_issues.sort_unstable();row_issues.dedup();
        Row{scope,comparable:row_issues.is_empty(),issues:row_issues,baseline:left,candidate:right}
    }).collect();
    Ok(Comparison {
        baseline: baseline.summary(),
        candidate: candidate.summary(),
        issues,
        rows,
    })
}
