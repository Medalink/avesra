//! One companion-owned worker. No device opens until native runtime gates pass.
use avesra_core::state::LocalState;
use avesra_windows::audio::{AudioFrame, Capture, MediaGate, Playback, PlaybackFrame};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

fn device_failed(app: &tauri::AppHandle, epoch: u64, output: bool, reason: &'static str) {
    let Some(state) = app.try_state::<crate::Runtime>() else {
        return;
    };
    let Ok(mut local) = state.local.lock() else {
        return;
    };
    if (if output {
        local.playback_epoch
    } else {
        local.capture_epoch
    }) != epoch
    {
        return;
    }
    local.voice_ready = false;
    if !output {
        local.capture_error = Some(reason.into());
    }
    local.enrollment_capture = false;
    local.microphone_check = false;
    local.capture_epoch = local.capture_epoch.saturating_add(1);
    local.playback_epoch = local.playback_epoch.saturating_add(1);
    local.refresh();
    state.publish(&local);
    let snapshot = local.clone();
    drop(local);
    let _ = app.emit("signal-frame", Option::<SignalFrame>::None);
    let _ = app.emit("runtime-state", snapshot);
    let _ = app.emit("runtime-error", reason);
}

#[derive(Clone)]
struct OutputLease {
    purpose: crate::playback_signal::Purpose,
    id: uuid::Uuid,
    epoch: u64,
    deadline: Instant,
    source: Option<avesra_core::conversations::PlannerCancellation>,
    action_epoch: Option<u64>,
    caller: Arc<AtomicBool>,
}
#[derive(Default)]
struct OutputStatus {
    epoch: u64,
    ready: bool,
    channels: u16,
    submitted: Option<uuid::Uuid>,
    output_lease: Option<uuid::Uuid>,
    drain_until: Option<Instant>,
}
#[derive(Clone, Default)]
struct Configuration {
    revision: u64,
    epoch: u64,
    playback_epoch: u64,
    input: Option<String>,
    output: Option<String>,
    capture: bool,
    microphone_check: bool,
    playback: bool,
    capture_deadline: Option<Instant>,
    voice_window: bool,
    last_voice_epoch: u64,
    output_lease: Option<OutputLease>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignalFrame {
    kind: &'static str,
    source: &'static str,
    samples: Vec<f32>,
    rms: f32,
    peak: f32,
    sequence: u64,
    captured_at: f64,
    capture_epoch: u64,
}
#[derive(Clone, Serialize)]
struct MediaHealth {
    capture: &'static str,
    playback: &'static str,
    output_channels: Option<u16>,
    epoch: u64,
}
#[derive(Clone, Default, Serialize)]
pub struct SoundDiagnostics {
    pub output_channels: Option<u16>,
    /// Voice, effect difference and background: linear peaks after master and
    /// limiter, before device format conversion. Scalar metadata, not recordings.
    pub component_peaks: [f32; 3],
    pub speech_submitted: bool,
    pub mix_submitted: bool,
}
pub struct MediaWorker {
    sound_diagnostics: Arc<Mutex<SoundDiagnostics>>,
    sound: Arc<avesra_windows::sound::SoundControl>,
    telemetry: Arc<crate::playback_signal::Telemetry>,
    configuration: Arc<Mutex<Configuration>>,
    capture_gate: Arc<MediaGate>,
    playback_gate: Arc<MediaGate>,
    outbound: Arc<Mutex<VecDeque<AudioFrame>>>,
    inbound: Arc<Mutex<VecDeque<PlaybackFrame>>>,
    shutdown: Arc<AtomicBool>,
    output_status: Arc<Mutex<OutputStatus>>,
    applied_revision: Arc<AtomicU64>,
}
impl MediaWorker {
    pub fn sound_diagnostics(&self) -> Result<SoundDiagnostics, String> {
        self.sound_diagnostics
            .lock()
            .map(|d| d.clone())
            .map_err(|_| "Sound diagnostics unavailable".into())
    }
    pub fn output_channels(&self) -> Option<u16> {
        self.output_status
            .lock()
            .ok()
            .and_then(|s| s.ready.then_some(s.channels))
    }
    /// Called under native local ownership after explicit Settings validation.
    pub fn open_preview(
        &self,
        local: &LocalState,
        id: uuid::Uuid,
        caller: Arc<AtomicBool>,
    ) -> Result<(), String> {
        self.open_output(
            local,
            id,
            None,
            None,
            caller,
            crate::playback_signal::Purpose::Preview,
        )
    }
    /// Fixed launch greeting, independent of Settings panel lifetime.
    pub fn open_greeting(
        &self,
        local: &LocalState,
        id: uuid::Uuid,
        caller: Arc<AtomicBool>,
    ) -> Result<(), String> {
        self.open_output(
            local,
            id,
            None,
            None,
            caller,
            crate::playback_signal::Purpose::Greeting,
        )
    }
    /// Requires the actual published source, not a reconstructed history row.
    pub fn open_reply(
        &self,
        local: &LocalState,
        id: uuid::Uuid,
        reply: &avesra_windows::effects::PublishedReply,
        caller: Arc<AtomicBool>,
    ) -> Result<(), String> {
        if !local.enrolled
            || !local.voice_ready
            || !reply.current()
            || reply.context().action_epoch != local.action_epoch
        {
            return Err("Accepted output is unavailable".into());
        }
        self.open_output(
            local,
            id,
            Some(reply.cancellation()),
            Some(local.action_epoch),
            caller,
            crate::playback_signal::Purpose::Reply,
        )
    }
    fn open_output(
        &self,
        local: &LocalState,
        id: uuid::Uuid,
        source: Option<avesra_core::conversations::PlannerCancellation>,
        action_epoch: Option<u64>,
        caller: Arc<AtomicBool>,
        purpose: crate::playback_signal::Purpose,
    ) -> Result<(), String> {
        if id.is_nil()
            || !local.connected
            || local.locked
            || local.settings.deafened
            || local.settings.paused
            || local.settings.speaker.is_none()
            || caller.load(Ordering::SeqCst)
        {
            return Err("Assistant output is unavailable".into());
        }
        let mut config = self.configuration.lock().map_err(|_| "Media unavailable")?;
        if config.playback_epoch != local.playback_epoch || config.output_lease.is_some() {
            return Err("Output is already owned or stale".into());
        }
        if let Ok(mut diagnostics) = self.sound_diagnostics.lock() {
            *diagnostics = SoundDiagnostics::default();
        }
        config.output_lease = Some(OutputLease {
            purpose,
            id,
            epoch: local.playback_epoch,
            deadline: Instant::now() + Duration::from_secs(70),
            source,
            action_epoch,
            caller,
        });
        self.telemetry.open(local.playback_epoch, id, purpose);
        config.playback = true;
        config.revision = config.revision.saturating_add(1);
        self.playback_gate.publish(true, local.playback_epoch);
        Ok(())
    }
    /// Read under Runtime.local; setup cleanup cannot revoke accepted output.
    pub fn setup_output_owned(&self, epoch: u64) -> bool {
        self.configuration.lock().is_ok_and(|config| {
            config.output_lease.as_ref().is_some_and(|v| {
                v.epoch == epoch && matches!(v.purpose, crate::playback_signal::Purpose::Preview)
            })
        })
    }
    pub fn retirement_ticket(&self) -> Result<u64, String> {
        Ok(self
            .configuration
            .lock()
            .map_err(|_| "Media unavailable")?
            .revision)
    }
    pub fn retired(&self, ticket: u64) -> bool {
        self.applied_revision.load(Ordering::SeqCst) >= ticket
    }
    pub fn output_state(&self, epoch: u64, id: uuid::Uuid) -> Result<(bool, bool, bool), String> {
        let config = self.configuration.lock().map_err(|_| "Media unavailable")?;
        if !config.output_lease.as_ref().is_some_and(|v| {
            v.id == id
                && v.epoch == epoch
                && Instant::now() < v.deadline
                && !v.caller.load(Ordering::SeqCst)
                && v.source.as_ref().is_none_or(|source| !source.cancelled())
        }) || !self.playback_gate.current(epoch)
        {
            return Err("Output stopped".into());
        }
        let status = self
            .output_status
            .lock()
            .map_err(|_| "Output unavailable")?;
        Ok((
            status.epoch == epoch && status.output_lease == Some(id) && status.ready,
            status.epoch == epoch
                && status.output_lease == Some(id)
                && status.submitted == Some(id),
            status.epoch == epoch
                && status.output_lease == Some(id)
                && status.submitted == Some(id)
                && status
                    .drain_until
                    .is_some_and(|deadline| Instant::now() >= deadline),
        ))
    }
    pub fn output_frame(&self, frame: PlaybackFrame) -> Result<(), String> {
        if !frame.valid() || !self.output_state(frame.epoch, frame.utterance)?.0 {
            return Err("Output device is not ready".into());
        }
        let mut frames = self
            .inbound
            .lock()
            .map_err(|_| "Output queue unavailable")?;
        if frames.len() >= 64 || !self.playback_gate.current(frame.epoch) {
            return Err("Output lost continuity".into());
        }
        frames.push_back(frame);
        Ok(())
    }
    /// Called with Runtime.local held after validating a fresh paired WSS ack.
    pub fn open_voice_window(&self, local: &LocalState) -> Result<(), String> {
        if !local.capture_allowed() || local.enrollment_capture || !local.voice_ready {
            return Err("Normal voice capture is unavailable".into());
        }
        let mut config = self
            .configuration
            .lock()
            .map_err(|_| "Media state unavailable")?;
        if config.epoch != local.capture_epoch
            || config.capture
            || config.input.is_none()
            || local.capture_epoch <= config.last_voice_epoch
        {
            return Err("Media capture already owned or stale".into());
        }
        config.voice_window = true;
        config.last_voice_epoch = local.capture_epoch;
        config.capture = true;
        config.capture_deadline = Some(Instant::now() + Duration::from_secs(11));
        config.revision = config.revision.saturating_add(1);
        self.capture_gate.publish(true, local.capture_epoch);
        Ok(())
    }
    pub fn close_voice_window(&self, epoch: u64) {
        if let Ok(mut config) = self.configuration.lock()
            && config.epoch == epoch
            && config.voice_window
        {
            self.capture_gate.publish(false, epoch);
            config.capture = false;
            config.voice_window = false;
            config.capture_deadline = None;
            config.revision = config.revision.saturating_add(1);
            if let Ok(mut frames) = self.outbound.lock() {
                frames.clear();
            }
        }
    }
    pub fn take_capture_frame(&self, epoch: u64) -> Result<Option<AudioFrame>, String> {
        if self
            .configuration
            .lock()
            .map_err(|_| "Media state unavailable")?
            .microphone_check
        {
            return Err("Microphone check is local only".into());
        }
        if !self.capture_gate.current(epoch) {
            return Err("Capture stopped".into());
        }
        let mut frames = self
            .outbound
            .lock()
            .map_err(|_| "Capture queue unavailable")?;
        let value = frames.pop_front();
        if value.as_ref().is_some_and(|frame| {
            frame.epoch != epoch || frame.captured.elapsed() > Duration::from_millis(500)
        }) {
            frames.clear();
            return Err("Capture lost freshness".into());
        }
        Ok(value)
    }
    pub fn signal_clock(&self) -> f64 {
        self.telemetry.clock()
    }
    pub fn spawn(app: tauri::AppHandle) -> std::io::Result<Self> {
        let sound_diagnostics = Arc::new(Mutex::new(SoundDiagnostics::default()));
        let sound = Arc::new(avesra_windows::sound::SoundControl::default());
        let telemetry = Arc::new(crate::playback_signal::Telemetry::new(app.clone()));
        let configuration = Arc::new(Mutex::new(Configuration::default()));
        let capture_gate = Arc::new(MediaGate::default());
        let playback_gate = Arc::new(MediaGate::default());
        let outbound = Arc::new(Mutex::new(VecDeque::<AudioFrame>::with_capacity(64)));
        let inbound = Arc::new(Mutex::new(VecDeque::<PlaybackFrame>::with_capacity(64)));
        let shutdown = Arc::new(AtomicBool::new(false));
        let output_status = Arc::new(Mutex::new(OutputStatus::default()));
        let applied_revision = Arc::new(AtomicU64::new(0));
        let value = Self {
            sound_diagnostics: sound_diagnostics.clone(),
            sound: sound.clone(),
            telemetry: telemetry.clone(),
            applied_revision: applied_revision.clone(),
            output_status: output_status.clone(),
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
                let mut previous = Configuration::default();
                let mut capture: Option<Capture> = None;
                let mut playback: Option<Playback> = None;
                let mut last_signal = Instant::now();
                while !shutdown.load(Ordering::SeqCst) {
                    let Ok(config) = configuration.lock().map(|value| value.clone()) else {
                        break;
                    };
                    if config.revision != revision {
                        let capture_changed = config.epoch != previous.epoch
                            || config.input != previous.input
                            || config.capture != previous.capture
                            || config.capture_deadline != previous.capture_deadline;
                        let playback_changed = config.playback_epoch != previous.playback_epoch
                            || config.output != previous.output
                            || config.playback != previous.playback
                            || config.output_lease.as_ref().map(|v| v.id)
                                != previous.output_lease.as_ref().map(|v| v.id);
                        // Capture-window churn must not restart an independent playback device.
                        if capture_changed {
                            capture = None;
                            if let Ok(mut queue) = outbound.lock() {
                                queue.clear();
                            }
                            let _ = app.emit("signal-frame", Option::<SignalFrame>::None);
                        }
                        if playback_changed {
                            if previous.playback_epoch != config.playback_epoch {
                                telemetry.retire(previous.playback_epoch);
                            }
                            playback = None;
                            if let Ok(mut queue) = inbound.lock() {
                                queue.clear();
                            }
                        }
                        if capture_changed && config.capture {
                            capture = config.input.as_ref().and_then(|name| {
                                Capture::open_with_gate(
                                    name,
                                    capture_gate.new_attempt_with_deadline(
                                        config.epoch,
                                        config.capture_deadline,
                                    ),
                                )
                                .ok()
                            });
                        }
                        if playback_changed && config.playback {
                            playback = config.output.as_ref().and_then(|name| {
                                Playback::open_with_sound(
                                    name,
                                    playback_gate.output_attempt(
                                        config.playback_epoch,
                                        config.output_lease.as_ref().map(|v| v.deadline),
                                        config.output_lease.as_ref().and_then(|v| v.source.clone()),
                                        config.output_lease.as_ref().map(|v| v.caller.clone()),
                                    ),
                                    avesra_windows::audio::PlaybackRate::Pcm24000,
                                    sound.clone(),
                                )
                                .ok()
                            });
                        }
                        if playback_changed && let Ok(mut status) = output_status.lock() {
                            *status = OutputStatus {
                                epoch: config.playback_epoch,
                                ready: playback.is_some(),
                                channels: playback.as_ref().map_or(0, |p| p.channels),
                                submitted: None,
                                output_lease: config.output_lease.as_ref().map(|v| v.id),
                                drain_until: None,
                            };
                        }
                        applied_revision.store(config.revision, Ordering::SeqCst);
                        revision = config.revision;
                        previous = config.clone();
                        if ((config.capture && capture.is_none())
                            || (config.playback && playback.is_none()))
                            && configuration
                                .lock()
                                .is_ok_and(|value| value.revision == revision)
                        {
                            if config.capture && capture.is_none() {
                                device_failed(&app, config.epoch, false, "Cannot open the selected microphone. Check Windows microphone access and whether another app has exclusive use, then retry.");
                            } else {
                                device_failed(&app, config.playback_epoch, true, "Speaker device unavailable; playback stopped. Review the selected speaker before retrying.");
                            }
                        }
                        let _ = app.emit(
                            "media-health",
                            MediaHealth {
                                output_channels: playback.as_ref().map(|p| p.channels),
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
                                        rms: frame.rms,
                                        peak: frame.peak,
                                        sequence: frame.sequence,
                                        captured_at: frame
                                            .captured
                                            .checked_duration_since(origin)
                                            .unwrap_or_default()
                                            .as_secs_f64()
                                            * 1000.0,
                                        capture_epoch: frame.epoch,
                                    }),
                                );
                                last_signal = Instant::now();
                            }
                            // Level checks publish only reduced visualization/levels.
                            // They never enter enrollment or normal inference queues.
                            if !config.microphone_check && let Ok(mut queue) = outbound.lock() {
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
                        if stream.gate.failed()
                            || config
                                .capture_deadline
                                .is_some_and(|deadline| Instant::now() >= deadline)
                        {
                            if config.microphone_check && !stream.gate.failed() {
                                crate::microphone_check::stop(&app, Some(config.epoch));
                            } else {
                                device_failed(&app, config.epoch, false, if stream.gate.failed() { stream.gate.failure_reason() } else { "Microphone capture exceeded its time limit. Retry the recording." });
                            }
                            let _ = app.emit(
                                "media-health",
                                MediaHealth {
                                    capture: "unavailable",
                                    playback: "disabled",
                                    output_channels: None,
                                    epoch: config.epoch,
                                },
                            );
                            capture = None;
                        }
                    }
                    if let Some(stream) = playback.as_ref() {
                        let expired = config
                            .output_lease
                            .as_ref()
                            .is_some_and(|v| Instant::now() >= v.deadline);
                        let failed = stream.gate.failed() || expired;
                        if failed || !stream.gate.current(config.playback_epoch) {
                            telemetry.retire(config.playback_epoch);
                            if failed {
                                device_failed(&app, config.playback_epoch, true, "Speaker device unavailable; playback stopped. Review the selected speaker before retrying.");
                            }
                            playback = None;
                            if let Ok(mut queue) = inbound.lock() {
                                queue.clear();
                            }
                            if let Ok(mut status) = output_status.lock()
                                && status.epoch == config.playback_epoch
                            {
                                status.ready = false;
                                status.submitted = None;
                                status.drain_until = None;
                            }
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
                                    && frame.epoch == config.playback_epoch
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
                        // Display uses submitted samples only; echo/identity remain unqualified.
                        for _ in 0..64 {
                            let Ok(mut reference) = stream.reference.try_recv() else {
                                break;
                            };
                            if let Ok(current) = configuration.lock()
                                && current.playback
                                && current.playback_epoch == reference.epoch
                                && current.output_lease.as_ref().is_some_and(|lease| {
                                    lease.id == reference.utterance
                                        && Instant::now() < lease.deadline
                                        && !lease.caller.load(Ordering::SeqCst)
                                        && lease
                                            .source
                                            .as_ref()
                                            .is_none_or(|source| !source.cancelled())
                                })
                                && stream.gate.current(reference.epoch)
                            {
                                telemetry.sample(&reference);
                                if let Ok(mut diagnostics) = sound_diagnostics.lock() {
                                    diagnostics.output_channels = Some(reference.channels);
                                    for (peak, value) in diagnostics.component_peaks.iter_mut().zip(reference.component_peaks) { *peak = peak.max(value); }
                                    diagnostics.speech_submitted |= reference.final_submitted;
                                    diagnostics.mix_submitted |= reference.mix_final_submitted;
                                }
                            }
                            if reference.final_submitted
                                && stream.gate.current(reference.epoch)
                                && let Ok(mut status) = output_status.lock()
                                && status.epoch == reference.epoch
                            {
                                status.submitted = Some(reference.utterance);
                            }
                            if reference.mix_final_submitted
                                && stream.gate.current(reference.epoch)
                                && let Ok(mut status) = output_status.lock()
                                && status.epoch == reference.epoch
                            {
                                status.drain_until = reference.drain_until;
                            }
                            reference.valid_samples = 0;
                            if stream.recycle.try_send(reference).is_err() {
                                stream.gate.close_attempt();
                            }
                        }
                        if stream.gate.failed()
                            || discontinuity
                            || config
                                .output_lease
                                .as_ref()
                                .is_some_and(|v| Instant::now() >= v.deadline)
                        {
                            device_failed(&app, config.playback_epoch, true, "Speaker device unavailable; playback stopped. Review the selected speaker before retrying.");
                            telemetry.retire(config.playback_epoch);
                            playback = None;
                            if let Ok(mut status) = output_status.lock()
                                && status.epoch == config.playback_epoch
                            {
                                status.ready = false;
                                status.submitted = None;
                                status.drain_until = None;
                            }
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
        self.sound
            .publish(&local.settings.sound, local.settings.speech_volume);
        let allowed = local.capture_allowed() && local.settings.microphone.is_some();
        let microphone_check = local.microphone_check_allowed();
        let mut clear_capture = true;
        let mut clear_playback = true;
        if let Ok(mut config) = self.configuration.lock() {
            let output_lease = config.output_lease.clone().filter(|v| {
                v.epoch == local.playback_epoch
                    && Instant::now() < v.deadline
                    && local.connected
                    && !local.locked
                    && !local.settings.deafened
                    && !local.settings.paused
                    && local.settings.speaker.is_some()
                    && !v.caller.load(Ordering::SeqCst)
                    && v.source.as_ref().is_none_or(|source| {
                        !source.cancelled() && local.enrolled && local.voice_ready
                    })
                    && v.action_epoch
                        .is_none_or(|epoch| epoch == local.action_epoch)
            });
            let playback = output_lease.is_some();
            let voice_window = config.voice_window
                && config.epoch == local.capture_epoch
                && allowed
                && !local.enrollment_capture;
            let capture =
                microphone_check || (allowed && (local.enrollment_capture || voice_window));
            if config.epoch == local.capture_epoch
                && config.playback_epoch == local.playback_epoch
                && config.input == local.settings.microphone
                && config.output == local.settings.speaker
                && config.capture == capture
                && config.microphone_check == microphone_check
                && config.playback == playback
                && config.output_lease.as_ref().map(|v| v.id) == output_lease.as_ref().map(|v| v.id)
            {
                return;
            }
            clear_capture = config.epoch != local.capture_epoch
                || config.capture != capture
                || config.input != local.settings.microphone;
            clear_playback = config.playback_epoch != local.playback_epoch
                || config.playback != playback
                || config.output != local.settings.speaker;
            if clear_capture {
                self.capture_gate.publish(capture, local.capture_epoch);
            }
            if clear_playback {
                self.telemetry.retire(config.playback_epoch);
                self.playback_gate.publish(playback, local.playback_epoch);
            }
            let capture_deadline = if microphone_check {
                if config.microphone_check && config.epoch == local.capture_epoch {
                    config.capture_deadline
                } else {
                    Some(Instant::now() + Duration::from_secs(30))
                }
            } else if voice_window {
                config.capture_deadline
            } else {
                local
                    .enrollment_capture
                    .then(|| Instant::now() + Duration::from_secs(12))
            };
            *config = Configuration {
                revision: config.revision.saturating_add(1),
                epoch: local.capture_epoch,
                playback_epoch: local.playback_epoch,
                input: local.settings.microphone.clone(),
                output: local.settings.speaker.clone(),
                capture,
                microphone_check,
                playback,
                capture_deadline,
                voice_window,
                last_voice_epoch: config.last_voice_epoch,
                output_lease,
            };
        } else {
            self.capture_gate.publish(false, local.capture_epoch);
            self.playback_gate.publish(false, local.playback_epoch);
        }
        // Dispose data synchronously even if the worker is delayed in an OS call.
        if clear_capture && let Ok(mut queue) = self.outbound.lock() {
            queue.clear();
        }
        if clear_playback && let Ok(mut queue) = self.inbound.lock() {
            queue.clear();
        }
    }
}
impl Drop for MediaWorker {
    fn drop(&mut self) {
        if let Ok(config) = self.configuration.lock() {
            self.telemetry.retire(config.playback_epoch);
        }
        self.capture_gate.publish(false, u64::MAX);
        self.playback_gate.publish(false, u64::MAX);
        self.shutdown.store(true, Ordering::SeqCst);
    }
}
