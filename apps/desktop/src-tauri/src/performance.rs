//! Bounded process-local observations; never an admission or qualification input.
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

const CAPACITY: usize = 512;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Preview,
    Greeting,
    Activity,
    Voice,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    VoiceAnalysis,
    VoiceIntent,
    VoiceGate,
    OutputSession,
    Preparation,
    RemoteReady,
    FirstSubmittedSpeech,
    FinalSubmission,
    EstimatedDrain,
    ActivitySession,
    ActivityExchange,
    CaptureAge,
    EndpointToAnalysis,
    EndpointToDirectedness,
}
#[derive(Clone, Copy)]
pub enum Outcome {
    Complete,
    Failed,
    Withdrawn,
    Abandoned,
}
#[derive(Clone, Copy)]
struct Observation {
    operation: Operation,
    stage: Stage,
    duration: Option<Duration>,
    outcome: Outcome,
}
struct Pending {
    id: Uuid,
    operation: Operation,
    started: Instant,
    submitted: bool,
}
struct Inner {
    observations: VecDeque<Observation>,
    outputs: Vec<Pending>,
    evicted: u64,
    observer_loss: u64,
    gates: BTreeMap<GateReason, u64>,
}
pub struct State {
    started: Instant,
    inner: Mutex<Inner>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            inner: Mutex::new(Inner {
                observations: VecDeque::with_capacity(CAPACITY),
                outputs: Vec::with_capacity(8),
                evicted: 0,
                observer_loss: 0,
                gates: BTreeMap::new(),
            }),
        }
    }
}
impl Inner {
    fn record(&mut self, value: Observation) {
        if self.observations.len() == CAPACITY {
            self.observations.pop_front();
            self.evicted = self.evicted.saturating_add(1);
        }
        self.observations.push_back(value);
    }
}
impl State {
    pub fn gate_counts(&self) -> Option<BTreeMap<GateReason, u64>> {
        self.inner.lock().ok().map(|inner| inner.gates.clone())
    }
    pub fn gate(&self, reason: GateReason) {
        if let Ok(mut inner) = self.inner.lock() {
            let count = inner.gates.entry(reason).or_default();
            *count = count.saturating_add(1);
        }
    }
    pub fn begin(self: &Arc<Self>, operation: Operation, stage: Stage, id: Uuid) -> Span {
        let started = Instant::now();
        if stage == Stage::OutputSession
            && let Ok(mut state) = self.inner.lock()
        {
            if state.outputs.len() < 8 && !state.outputs.iter().any(|p| p.id == id) {
                state.outputs.push(Pending {
                    id,
                    operation,
                    started,
                    submitted: false,
                });
            } else {
                state.observer_loss = state.observer_loss.saturating_add(1);
            }
        }
        Span {
            state: self.clone(),
            operation,
            stage,
            id,
            started,
            finished: false,
        }
    }
    pub fn record(&self, operation: Operation, stage: Stage, duration: Duration, outcome: Outcome) {
        if let Ok(mut state) = self.inner.lock() {
            state.record(Observation {
                operation,
                stage,
                duration: Some(duration),
                outcome,
            });
        }
    }
    /// Called only by the existing validated media-worker reference consumer.
    pub fn submitted(&self, id: Uuid, submitted: Instant) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        let Some(pending) = state
            .outputs
            .iter_mut()
            .find(|p| p.id == id && !p.submitted)
        else {
            return;
        };
        let Some(duration) = submitted.checked_duration_since(pending.started) else {
            return;
        };
        pending.submitted = true;
        let operation = pending.operation;
        state.record(Observation {
            operation,
            stage: Stage::FirstSubmittedSpeech,
            duration: Some(duration),
            outcome: Outcome::Complete,
        });
    }
}
pub struct Span {
    state: Arc<State>,
    operation: Operation,
    stage: Stage,
    id: Uuid,
    started: Instant,
    finished: bool,
}
impl Span {
    pub fn finish(mut self, outcome: Outcome) {
        self.complete(outcome);
    }
    fn complete(&mut self, outcome: Outcome) {
        if self.finished {
            return;
        }
        let duration = self.started.elapsed();
        self.finished = true;
        if let Ok(mut state) = self.state.inner.lock() {
            state.record(Observation {
                operation: self.operation,
                stage: self.stage,
                duration: Some(duration),
                outcome,
            });
            if self.stage == Stage::OutputSession
                && let Some(index) = state.outputs.iter().position(|p| p.id == self.id)
            {
                let pending = state.outputs.swap_remove(index);
                if !pending.submitted {
                    state.record(Observation {
                        operation: self.operation,
                        stage: Stage::FirstSubmittedSpeech,
                        duration: None,
                        outcome,
                    });
                }
            }
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        self.complete(Outcome::Abandoned);
    }
}
pub async fn measure<T>(
    state: &Arc<State>,
    operation: Operation,
    stage: Stage,
    id: Uuid,
    future: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let span = state.begin(operation, stage, id);
    let result = future.await;
    span.finish(if result.is_ok() {
        Outcome::Complete
    } else {
        Outcome::Failed
    });
    result
}
#[derive(Default, Serialize)]
pub struct Counts {
    complete: usize,
    failed: usize,
    withdrawn: usize,
    abandoned: usize,
    missing: usize,
}
#[derive(Serialize)]
pub struct Summary {
    operation: Operation,
    stage: Stage,
    counts: Counts,
    timed: usize,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
    max_ms: Option<f64>,
    provisional: bool,
}
#[derive(Serialize)]
pub struct Snapshot {
    uptime_seconds: u64,
    capacity: usize,
    retained: usize,
    evicted: u64,
    observer_loss: u64,
    active_outputs: usize,
    silent_diagnostic: bool,
    summaries: Vec<Summary>,
    gates: BTreeMap<GateReason, u64>,
}
#[tauri::command]
pub fn performance_snapshot(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::Runtime>,
) -> Result<Snapshot, String> {
    if window.label() != "settings"
        || !window.is_visible().unwrap_or(false)
        || state
            .local
            .lock()
            .map_err(|_| "Local state unavailable")?
            .locked
    {
        return Err("Use visible unlocked Settings for performance".into());
    }
    let (observations, evicted, observer_loss, active_outputs, gates) = {
        let inner = state
            .performance
            .inner
            .lock()
            .map_err(|_| "Performance observations unavailable")?;
        (
            inner.observations.iter().copied().collect::<Vec<_>>(),
            inner.evicted,
            inner.observer_loss,
            inner.outputs.len(),
            inner.gates.clone(),
        )
    };
    let mut groups = BTreeMap::<(Operation, Stage), (Counts, Vec<f64>)>::new();
    for value in &observations {
        let (counts, durations) = groups.entry((value.operation, value.stage)).or_default();
        match value.outcome {
            Outcome::Complete => counts.complete += 1,
            Outcome::Failed => counts.failed += 1,
            Outcome::Withdrawn => counts.withdrawn += 1,
            Outcome::Abandoned => counts.abandoned += 1,
        }
        if let Some(duration) = value.duration {
            durations.push(duration.as_secs_f64() * 1000.0);
        } else {
            counts.missing += 1;
        }
    }
    let summaries = groups
        .into_iter()
        .map(|((operation, stage), (counts, mut durations))| {
            durations.sort_by(f64::total_cmp);
            let percentile = |percent: usize| {
                if durations.is_empty() {
                    None
                } else {
                    durations
                        .get((durations.len() * percent).div_ceil(100) - 1)
                        .copied()
                }
            };
            Summary {
                operation,
                stage,
                counts,
                timed: durations.len(),
                p50_ms: percentile(50),
                p95_ms: percentile(95),
                p99_ms: percentile(99),
                max_ms: durations.last().copied(),
                provisional: durations.len() < 30,
            }
        })
        .collect();
    Ok(Snapshot {
        uptime_seconds: state.performance.started.elapsed().as_secs(),
        capacity: CAPACITY,
        retained: observations.len(),
        evicted,
        observer_loss,
        active_outputs,
        silent_diagnostic: avesra_windows::output_recording::enabled(),
        summaries,
        gates,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateReason {
    Accepted,
    Abandoned,
    AnalysisFailed,
    EmptyTranscript,
    ShortSpan,
    ReferenceUnknown,
    QueueExpired,
    QueueOverflow,
    ContextChanged,
    AdmissionFailed,
    Stale,
    Malformed,
    Replay,
    Capacity,
    Unqualified,
    UnknownSpeaker,
    OverlapUnknown,
    Overlap,
    EchoUnknown,
    Echo,
    NotAddressed,
    SignalUnknown,
    InsufficientSignal,
    DirectednessUnknown,
}
impl From<avesra_core::voice::Abstention> for GateReason {
    fn from(value: avesra_core::voice::Abstention) -> Self {
        use avesra_core::voice::Abstention as A;
        match value {
            A::Stale => Self::Stale,
            A::Malformed => Self::Malformed,
            A::Replay => Self::Replay,
            A::Capacity => Self::Capacity,
            A::Unqualified => Self::Unqualified,
            A::UnknownSpeaker => Self::UnknownSpeaker,
            A::OverlapUnknown => Self::OverlapUnknown,
            A::Overlap => Self::Overlap,
            A::EchoUnknown => Self::EchoUnknown,
            A::Echo => Self::Echo,
            A::NotAddressed => Self::NotAddressed,
            A::SignalUnknown => Self::SignalUnknown,
            A::InsufficientSignal => Self::InsufficientSignal,
            A::DirectednessUnknown => Self::DirectednessUnknown,
        }
    }
}
