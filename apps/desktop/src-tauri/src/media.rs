//! One companion-owned worker. No device opens until native runtime gates pass.
use avesra_core::state::LocalState;
use avesra_windows::audio::{AudioFrame, Capture, MediaGate, Playback};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

fn device_failed(app: &tauri::AppHandle, epoch: u64) {
    let Some(state) = app.try_state::<crate::Runtime>() else {
        return;
    };
    let Ok(mut local) = state.local.lock() else {
        return;
    };
    if local.capture_epoch != epoch {
        return;
    }
    local.voice_ready = false;
    local.capture_epoch = local.capture_epoch.saturating_add(1);
    local.action_epoch = local.action_epoch.saturating_add(1);
    local.refresh();
    state.publish(&local);
    let snapshot = local.clone();
    drop(local);
    let _ = app.emit("signal-frame", Option::<SignalFrame>::None);
    let _ = app.emit("runtime-state", snapshot);
    let _ = app.emit("runtime-error", "Audio device unavailable; listening and playback stopped. Review devices before retrying setup.");
}

#[derive(Clone, Default)]
struct Configuration {
    revision: u64,
    epoch: u64,
    input: Option<String>,
    output: Option<String>,
    capture: bool,
    playback: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignalFrame {
    kind: &'static str,
    source: &'static str,
    samples: Vec<f32>,
    sequence: u64,
    captured_at: f64,
    capture_epoch: u64,
}
#[derive(Clone, Serialize)]
struct MediaHealth {
    capture: &'static str,
    playback: &'static str,
    epoch: u64,
}
pub struct MediaWorker {
    configuration: Arc<Mutex<Configuration>>,
    capture_gate: Arc<MediaGate>,
    playback_gate: Arc<MediaGate>,
    outbound: Arc<Mutex<VecDeque<AudioFrame>>>,
    inbound: Arc<Mutex<VecDeque<AudioFrame>>>,
    shutdown: Arc<AtomicBool>,
}
impl MediaWorker {
    pub fn spawn(app: tauri::AppHandle) -> std::io::Result<Self> {
        let configuration = Arc::new(Mutex::new(Configuration::default()));
        let capture_gate = Arc::new(MediaGate::default());
        let playback_gate = Arc::new(MediaGate::default());
        let outbound = Arc::new(Mutex::new(VecDeque::<AudioFrame>::with_capacity(64)));
        let inbound = Arc::new(Mutex::new(VecDeque::<AudioFrame>::with_capacity(64)));
        let shutdown = Arc::new(AtomicBool::new(false));
        let value = Self {
            configuration: configuration.clone(),
            capture_gate: capture_gate.clone(),
            playback_gate: playback_gate.clone(),
            outbound: outbound.clone(),
            inbound: inbound.clone(),
            shutdown: shutdown.clone(),
        };
        std::thread::Builder::new()
            .name("avesra-media".into())
            .spawn(move || {
                let origin = Instant::now();
                let mut revision = 0;
                let mut capture: Option<Capture> = None;
                let mut playback: Option<Playback> = None;
                let mut last_signal = Instant::now();
                while !shutdown.load(Ordering::SeqCst) {
                    let Ok(config) = configuration.lock().map(|value| value.clone()) else {
                        break;
                    };
                    if config.revision != revision {
                        // Dispose the previous streams and queues before any slow device open.
                        capture = None;
                        playback = None;
                        if let Ok(mut queue) = outbound.lock() {
                            queue.clear();
                        }
                        if let Ok(mut queue) = inbound.lock() {
                            queue.clear();
                        }
                        let _ = app.emit("signal-frame", Option::<SignalFrame>::None);
                        if config.capture {
                            capture = config.input.as_ref().and_then(|name| {
                                Capture::open_with_gate(
                                    name,
                                    capture_gate.new_attempt(config.epoch),
                                )
                                .ok()
                            });
                        }
                        if config.playback {
                            playback = config.output.as_ref().and_then(|name| {
                                Playback::open_with_gate(
                                    name,
                                    playback_gate.new_attempt(config.epoch),
                                )
                                .ok()
                            });
                        }
                        revision = config.revision;
                        if (config.capture && capture.is_none())
                            || (config.playback && playback.is_none())
                        {
                            device_failed(&app, config.epoch);
                        }
                        let _ = app.emit(
                            "media-health",
                            MediaHealth {
                                capture: if !config.capture {
                                    "disabled"
                                } else if capture.is_some() {
                                    "open"
                                } else {
                                    "unavailable"
                                },
                                playback: if !config.playback {
                                    "disabled"
                                } else if playback.is_some() {
                                    "open"
                                } else {
                                    "unavailable"
                                },
                                epoch: config.epoch,
                            },
                        );
                    }
                    if let Some(stream) = capture.as_ref() {
                        for _ in 0..64 {
                            let Ok(frame) = stream.frames.try_recv() else {
                                break;
                            };
                            if !config.capture
                                || frame.epoch != config.epoch
                                || frame.captured.elapsed() > Duration::from_millis(500)
                            {
                                continue;
                            }
                            let current = configuration.lock().is_ok_and(|value| {
                                value.capture
                                    && value.epoch == frame.epoch
                                    && value.revision == revision
                            });
                            if !current || !stream.gate.current(frame.epoch) {
                                continue;
                            }
                            if last_signal.elapsed() >= Duration::from_millis(50) {
                                let samples = frame
                                    .samples
                                    .chunks_exact(10)
                                    .map(|chunk| {
                                        chunk
                                            .iter()
                                            .map(|sample| f32::from(*sample) / 32768.0)
                                            .sum::<f32>()
                                            / 10.0
                                    })
                                    .collect();
                                let _ = app.emit(
                                    "signal-frame",
                                    Some(SignalFrame {
                                        kind: "human",
                                        source: "background",
                                        samples,
                                        sequence: frame.sequence,
                                        captured_at: origin.elapsed().as_secs_f64() * 1000.0,
                                        capture_epoch: frame.epoch,
                                    }),
                                );
                                last_signal = Instant::now();
                            }
                            if let Ok(mut queue) = outbound.lock() {
                                if !stream.gate.current(frame.epoch) {
                                    continue;
                                }
                                if queue.len() < 64 {
                                    queue.push_back(frame);
                                } else {
                                    // Fail this attempt only; a late old worker must not
                                    // revoke a newer device attempt's permission.
                                    stream.gate.close_attempt();
                                    queue.clear();
                                    let _ = app.emit(
                                        "runtime-error",
                                        "Capture queue overflow; media gate closed.",
                                    );
                                }
                            }
                        }
                        if stream.gate.failed() {
                            device_failed(&app, config.epoch);
                            let _ = app.emit(
                                "media-health",
                                MediaHealth {
                                    capture: "unavailable",
                                    playback: "disabled",
                                    epoch: config.epoch,
                                },
                            );
                            capture = None;
                        }
                    }
                    if let Some(stream) = playback.as_ref() {
                        let mut discontinuity = false;
                        if let Ok(mut queue) = inbound.lock() {
                            for _ in 0..64 {
                                let Some(frame) = queue.pop_front() else {
                                    break;
                                };
                                if config.playback
                                    && stream.gate.current(frame.epoch)
                                    && frame.epoch == config.epoch
                                    && frame.captured.elapsed() < Duration::from_millis(500)
                                    && stream.frames.try_send(frame).is_err()
                                {
                                    discontinuity = true;
                                    stream.gate.close_attempt();
                                    queue.clear();
                                    break;
                                }
                            }
                        }
                        // Echo cancellation is not qualified: references are drained,
                        // never retained or interpreted as owner speech.
                        for _ in 0..64 {
                            if stream.reference.try_recv().is_err() {
                                break;
                            }
                        }
                        if stream.gate.failed() || discontinuity {
                            device_failed(&app, config.epoch);
                            playback = None;
                        }
                    }
                    if let Ok(mut queue) = outbound.lock() {
                        while queue.front().is_some_and(|frame| {
                            frame.captured.elapsed() > Duration::from_millis(500)
                        }) {
                            queue.pop_front();
                        }
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })?;
        Ok(value)
    }
    /// Called inside the serialized local-state update, before disk/network IO.
    pub fn publish(&self, local: &LocalState) {
        let capture = local.capture_allowed() && local.settings.microphone.is_some();
        let playback = local.connected
            && local.enrolled
            && local.voice_ready
            && !local.locked
            && !local.settings.deafened
            && !local.settings.paused
            && local.settings.speaker.is_some();
        if let Ok(mut config) = self.configuration.lock() {
            if config.epoch == local.capture_epoch
                && config.input == local.settings.microphone
                && config.output == local.settings.speaker
                && config.capture == capture
                && config.playback == playback
            {
                return;
            }
            self.capture_gate.publish(capture, local.capture_epoch);
            self.playback_gate.publish(playback, local.capture_epoch);
            *config = Configuration {
                revision: config.revision.saturating_add(1),
                epoch: local.capture_epoch,
                input: local.settings.microphone.clone(),
                output: local.settings.speaker.clone(),
                capture,
                playback,
            };
        } else {
            self.capture_gate.publish(false, local.capture_epoch);
            self.playback_gate.publish(false, local.capture_epoch);
        }
        // Dispose data synchronously even if the worker is delayed in an OS call.
        if let Ok(mut queue) = self.outbound.lock() {
            queue.clear();
        }
        if let Ok(mut queue) = self.inbound.lock() {
            queue.clear();
        }
    }
}
impl Drop for MediaWorker {
    fn drop(&mut self) {
        self.capture_gate.publish(false, u64::MAX);
        self.playback_gate.publish(false, u64::MAX);
        self.shutdown.store(true, Ordering::SeqCst);
    }
}
