//! Display-only submission telemetry. Never called from an audio callback.
use avesra_windows::audio::PlaybackReference;
use serde::Serialize;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Emitter;
use uuid::Uuid;

const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Preview,
    Reply,
}
#[derive(Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Event {
    Sample {
        epoch: u64,
        output: Uuid,
        sequence: u64,
        purpose: Purpose,
        submitted_at: f64,
        expires_at: f64,
        samples: [f32; 32],
    },
    Clear {
        epoch: u64,
        output: Uuid,
        sequence: u64,
    },
}
#[derive(Default)]
struct State {
    sequence: u64,
    last_epoch: u64,
    active: Option<(u64, Uuid, Purpose)>,
    last_sample: Option<Instant>,
}
pub struct Telemetry {
    app: tauri::AppHandle,
    origin: Instant,
    state: Mutex<State>,
}
impl Telemetry {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            origin: Instant::now(),
            state: Mutex::new(State::default()),
        }
    }
    pub fn clock(&self) -> f64 {
        self.origin.elapsed().as_secs_f64() * 1000.0
    }
    /// Admission/sample share media configuration ownership. Exact-epoch retirement
    /// also runs on worker teardown; this lane mutex serializes its sequence.
    pub fn open(&self, epoch: u64, output: Uuid, purpose: Purpose) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if epoch <= state.last_epoch || state.sequence >= MAX_SEQUENCE {
            return;
        }
        state.last_epoch = epoch;
        state.active = Some((epoch, output, purpose));
        state.last_sample = None;
    }
    pub fn retire(&self, epoch: u64) {
        let event = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let Some((owned, output, _)) = state.active else {
                return;
            };
            if epoch != owned {
                return;
            }
            state.active = None;
            if state.sequence >= MAX_SEQUENCE {
                return;
            }
            state.sequence += 1;
            Event::Clear {
                epoch,
                output,
                sequence: state.sequence,
            }
        };
        let _ = self.app.emit("playback-signal", event);
    }
    pub fn sample(&self, reference: &PlaybackReference) {
        let now = Instant::now();
        if reference.valid_samples == 0
            || reference.valid_samples > reference.samples.len()
            || reference.submitted > now
            || now.duration_since(reference.submitted) >= Duration::from_millis(250)
        {
            return;
        }
        let raw = &reference.samples[..reference.valid_samples];
        if raw.iter().any(|v| !v.is_finite() || v.abs() > 1.0) {
            return;
        }
        let event = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let Some((epoch, output, purpose)) = state.active else {
                return;
            };
            if epoch != reference.epoch
                || output != reference.utterance
                || state.sequence >= MAX_SEQUENCE
                || state
                    .last_sample
                    .is_some_and(|last| now.duration_since(last) < Duration::from_millis(50))
            {
                return;
            }
            let Some(submitted) = reference.submitted.checked_duration_since(self.origin) else {
                return;
            };
            let samples = std::array::from_fn(|i| {
                let start = i * raw.len() / 32;
                let end = ((i + 1) * raw.len() / 32).max(start + 1).min(raw.len());
                raw[start..end].iter().sum::<f32>() / (end - start) as f32
            });
            state.sequence += 1;
            state.last_sample = Some(now);
            let submitted_at = submitted.as_secs_f64() * 1000.0;
            Event::Sample {
                epoch,
                output,
                sequence: state.sequence,
                purpose,
                submitted_at,
                expires_at: submitted_at + 250.0,
                samples,
            }
        };
        let _ = self.app.emit("playback-signal", event);
    }
}

#[tauri::command]
pub fn playback_signal_clock(state: tauri::State<'_, crate::Runtime>) -> f64 {
    state.media.signal_clock()
}
