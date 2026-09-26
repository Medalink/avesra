//! Optional bounded observer of the sole Personal input producer, never capture authority.
use super::ManagementProof;
use crate::{
    Runtime,
    profiles::voice_avatar::{
        self, PortraitSource, View,
        portrait_dsp::{self, Feature},
    },
};
use avesra_core::{
    app_timing::{Operation, Outcome, Portrait, Span, Stage},
    state::LocalState,
    voice,
};
use avesra_windows::audio::AudioFrame;
use serde::Serialize;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;
use zeroize::Zeroizing;

const OPERATION: Duration = Duration::from_secs(12);
const FRAME_SAMPLES: usize = 320;
const BATCH_FRAMES: usize = 400;
fn measured<T>(
    phase: Portrait,
    id: Uuid,
    cancelled: &AtomicBool,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let span = Span::with_operation(Operation::Portrait(phase), Stage::Work, Some(id));
    let result = work();
    span.finish(match &result {
        Ok(_) => Outcome::Complete,
        Err(_) if cancelled.load(Ordering::SeqCst) => Outcome::Withdrawn,
        Err(_) => Outcome::Failed,
    });
    result
}

#[derive(Default)]
pub(crate) struct State {
    session: Mutex<Option<Arc<Session>>>,
    operation: Arc<tokio::sync::Mutex<()>>,
    delivery: Mutex<Option<Delivery>>,
    enabled: AtomicBool,
    missed: AtomicBool,
}
impl State {
    pub(super) fn invalidate(&self) {
        self.enabled.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = self.session.lock()
            && let Some(session) = slot.take()
        {
            session.cancelled.store(true, Ordering::SeqCst);
        }
        // Actual consumer still owns its receiver/operation until retirement.
        if let Ok(mut delivery) = self.delivery.lock() {
            delivery.take();
        }
    }
    fn get(&self, id: Uuid) -> Result<Arc<Session>, String> {
        self.session
            .lock()
            .map_err(|_| "Portrait session unavailable")?
            .as_ref()
            .filter(|value| value.id == id)
            .cloned()
            .ok_or("Portrait session ended; begin again".into())
    }
    fn remove(&self, id: Uuid) {
        if let Ok(mut slot) = self.session.lock()
            && slot.as_ref().is_some_and(|value| value.id == id)
        {
            slot.take();
        }
    }
    /// Producer-only, nonblocking. Any failure affects only this optional observer.
    pub(crate) fn observe(
        &self,
        context: &voice::Context,
        lease: Uuid,
        frame: &AudioFrame,
        known: bool,
        output: bool,
    ) {
        if !self.enabled.load(Ordering::SeqCst) {
            return;
        }
        let Ok(mut slot) = self.delivery.try_lock() else {
            self.missed.store(true, Ordering::SeqCst);
            return;
        };
        let Some(delivery) = slot.as_mut() else {
            return;
        };
        if self.missed.load(Ordering::SeqCst) {
            self.enabled.store(false, Ordering::SeqCst);
            slot.take();
            return;
        }
        if delivery.cancelled.load(Ordering::SeqCst)
            || &delivery.context != context
            || frame.epoch != context.capture_epoch
            || delivery.started.elapsed() >= OPERATION
            || frame.captured.elapsed() > Duration::from_millis(500)
            || delivery.lease.is_some_and(|expected| expected != lease)
            || delivery.previous.is_some_and(|(sequence, captured)| {
                sequence.checked_add(1) != Some(frame.sequence)
                    || frame.captured <= captured
                    || frame.captured.duration_since(captured) > Duration::from_millis(25)
            })
        {
            delivery.cancelled.store(true, Ordering::SeqCst);
            self.enabled.store(false, Ordering::SeqCst);
            slot.take();
            return;
        }
        // Never reuse queued samples that were captured before explicit observation.
        if frame.captured < delivery.started {
            return;
        }
        delivery.lease = Some(lease);
        delivery.previous = Some((frame.sequence, frame.captured));
        let value = ObservedFrame {
            captured: frame.captured,
            samples: (known && !output).then(|| Zeroizing::new(frame.samples)),
        };
        if delivery.sender.try_send(value).is_err() {
            self.missed.store(true, Ordering::SeqCst);
            self.enabled.store(false, Ordering::SeqCst);
            slot.take();
            return;
        }
        delivery.frames += 1;
        if delivery.frames == BATCH_FRAMES {
            self.enabled.store(false, Ordering::SeqCst);
            slot.take();
        }
    }
}
struct Delivery {
    request: Uuid,
    context: voice::Context,
    started: Instant,
    lease: Option<Uuid>,
    previous: Option<(u64, Instant)>,
    frames: usize,
    cancelled: Arc<AtomicBool>,
    sender: SyncSender<ObservedFrame>,
}
struct ObservedFrame {
    captured: Instant,
    samples: Option<Zeroizing<[i16; FRAME_SAMPLES]>>,
}
struct Session {
    id: Uuid,
    proof: ManagementProof,
    context: voice::Context,
    admission: Uuid,
    source: PortraitSource,
    cancelled: Arc<AtomicBool>,
    data: Mutex<Data>,
}
struct Data {
    features: Vec<Option<Feature>>,
    batches: u8,
}
impl Session {
    fn current_locked(&self, state: &Runtime, local: &LocalState) -> Result<(), String> {
        let admission = state.qualification.admission(state, local);
        if self.cancelled.load(Ordering::SeqCst)
            || !self.proof.current_locked(state, local)
            || !local.capture_allowed()
            || !local.voice_ready
            || local.enrollment_capture
            || local.microphone_check
            || !admission.is_some_and(|value| {
                value.kind == voice::AdmissionKind::Personal
                    && value.session == self.admission
                    && value.context == self.context
            })
        {
            self.cancelled.store(true, Ordering::SeqCst);
            return Err(
                "Portrait observation expired or its owner, input or Settings changed".into(),
            );
        }
        Ok(())
    }
    fn current(&self, app: &tauri::AppHandle, started: Instant) -> Result<(), String> {
        if started.elapsed() >= OPERATION {
            self.cancelled.store(true, Ordering::SeqCst);
            return Err("Portrait operation timed out".into());
        }
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        self.current_locked(&state, &local)
    }
    fn summary(&self) -> Result<Summary, String> {
        let data = self.data.lock().map_err(|_| "Portrait state unavailable")?;
        Ok(Summary {
            version: 1,
            session: self.id,
            remaining_ms: self.proof.remaining_ms(),
            batches: data.batches,
            captured_slots: data.features.iter().flatten().count(),
            words: portrait_dsp::WORDS,
            slots: data
                .features
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    if index >= usize::from(data.batches) * 10 {
                        Slot::Pending
                    } else if value.is_some() {
                        Slot::Captured
                    } else {
                        Slot::Missing
                    }
                })
                .collect(),
        })
    }
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Slot {
    Pending,
    Captured,
    Missing,
}
#[derive(Serialize)]
pub(crate) struct Summary {
    version: u16,
    session: Uuid,
    remaining_ms: u64,
    batches: u8,
    captured_slots: usize,
    slots: Vec<Slot>,
    words: [&'static str; 20],
}
#[derive(Clone, Serialize)]
struct Progress {
    version: u16,
    session: Uuid,
    request: Uuid,
    batch: u8,
    received_samples: usize,
    slot: usize,
    phase: &'static str,
    captured_slots: usize,
}
struct Caller {
    cancelled: Arc<AtomicBool>,
    complete: bool,
}
impl Drop for Caller {
    fn drop(&mut self) {
        if !self.complete {
            self.cancelled.store(true, Ordering::SeqCst);
        }
    }
}
fn visible(window: &tauri::WebviewWindow) -> Result<(), String> {
    super::settings_only(window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings visibility unavailable")?
    {
        return Err("Open Settings to observe acoustic prompts".into());
    }
    Ok(())
}
fn budget(started: Instant) -> Result<(), String> {
    if started.elapsed() >= OPERATION {
        Err("Portrait operation timed out".into())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub(crate) async fn begin_voice_portrait(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Summary, String> {
    let started = Instant::now();
    super::settings_only(&window)?;
    let state = app.state::<Runtime>();
    let operation = state
        .setup
        .portrait
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| "A portrait operation is still retiring")?;
    let (proof, admission) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let admission = state.qualification.admission(&state, &local).filter(|v|v.kind == voice::AdmissionKind::Personal)
            .ok_or("Personal listening must already be active; this tool does not start the microphone")?;
        if !local.capture_allowed()
            || !local.voice_ready
            || local.enrollment_capture
            || local.microphone_check
        {
            return Err("Personal input is unavailable".into());
        }
        let proof = state
            .setup
            .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?;
        (proof, admission)
    };
    visible(&window)?;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut caller = Caller {
        cancelled: cancelled.clone(),
        complete: false,
    };
    let result = tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let _owner = owner;
        let state = app.state::<Runtime>();
        let mut current = || {
            budget(started)?;
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if cancelled.load(Ordering::SeqCst)
                || !proof.current_locked(&state, &local)
                || !local.capture_allowed()
                || !state
                    .qualification
                    .admission(&state, &local)
                    .is_some_and(|v| {
                        v.kind == voice::AdmissionKind::Personal
                            && v.session == admission.session
                            && v.context == admission.context
                    })
            {
                return Err("Portrait preparation was withdrawn".into());
            }
            Ok(())
        };
        current()?;
        {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            let mut slot = state
                .setup
                .portrait
                .session
                .lock()
                .map_err(|_| "Portrait state unavailable")?;
            if slot
                .as_ref()
                .is_some_and(|s| s.current_locked(&state, &local).is_ok())
            {
                return Err("Finish or cancel the previous acoustic prompt session".into());
            }
            if let Some(previous) = slot.take() {
                previous.cancelled.store(true, Ordering::SeqCst);
            }
        }
        let source =
            voice_avatar::portrait_source(&directory, &admission.context.microphone, &mut current)?;
        if Some(source.actor()) != admission.context.actor {
            return Err("Portrait owner changed".into());
        }
        current()?;
        let session = Arc::new(Session {
            id: Uuid::new_v4(),
            proof,
            context: admission.context,
            admission: admission.session,
            source,
            cancelled,
            data: Mutex::new(Data {
                features: std::iter::repeat_with(|| None).take(20).collect(),
                batches: 0,
            }),
        });
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        session.current_locked(&state, &local)?;
        let summary = session.summary()?;
        *state
            .setup
            .portrait
            .session
            .lock()
            .map_err(|_| "Portrait state unavailable")? = Some(session);
        Ok(summary)
    })
    .await
    .map_err(|_| "Portrait preparation owner stopped")?;
    caller.complete = result.is_ok();
    result
}

struct TapOwner {
    app: tauri::AppHandle,
    request: Uuid,
}
impl Drop for TapOwner {
    fn drop(&mut self) {
        let state = self.app.state::<Runtime>();
        if let Ok(mut slot) = state.setup.portrait.delivery.lock()
            && slot.as_ref().is_some_and(|v| v.request == self.request)
        {
            state.setup.portrait.enabled.store(false, Ordering::SeqCst);
            slot.take();
        }
    }
}
fn progress(
    app: &tauri::AppHandle,
    session: &Session,
    observation: (Uuid, u8),
    received: usize,
    phase: &'static str,
    captured: usize,
    started: Instant,
) {
    if session.current(app, started).is_err() {
        return;
    }
    let (request, batch) = observation;
    let index = if phase == "slot_complete" {
        received.saturating_sub(1) / 12800
    } else {
        received / 12800
    };
    let _ = app.emit_to(
        "settings",
        "voice-portrait-progress",
        Progress {
            version: 1,
            session: session.id,
            request,
            batch,
            received_samples: received,
            slot: usize::from(batch) * 10 + index.min(9),
            phase,
            captured_slots: captured,
        },
    );
}
#[tauri::command]
pub(crate) async fn record_voice_portrait(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    session: Uuid,
    request: Uuid,
) -> Result<Summary, String> {
    let started = Instant::now();
    visible(&window)?;
    if request.is_nil() {
        return Err("Invalid portrait observation request".into());
    }
    let state = app.state::<Runtime>();
    let operation = state
        .setup
        .portrait
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| "The previous portrait observer is still retiring")?;
    let session = state.setup.portrait.get(session)?;
    session.current(&app, started)?;
    let mut caller = Caller {
        cancelled: session.cancelled.clone(),
        complete: false,
    };
    let result = tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let state = app.state::<Runtime>();
        let timing = Uuid::new_v4();
        measured(Portrait::Observe, timing, &session.cancelled, || {
            let (batch, mut features) = {
                let data = session
                    .data
                    .lock()
                    .map_err(|_| "Portrait state unavailable")?;
                if data.batches >= 2 {
                    return Err("Both acoustic prompt batches are already observed".into());
                }
                (data.batches, data.features.clone())
            };
            let (send, receive) = mpsc::sync_channel(8);
            // Own removal before making the tap visible. No media ownership is changed.
            let tap = TapOwner {
                app: app.clone(),
                request,
            };
            {
                let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                session.current_locked(&state, &local)?;
                let mut delivery = state
                    .setup
                    .portrait
                    .delivery
                    .lock()
                    .map_err(|_| "Portrait observer unavailable")?;
                if delivery.is_some() {
                    return Err("Previous portrait tap has not retired".into());
                }
                state.setup.portrait.missed.store(false, Ordering::SeqCst);
                *delivery = Some(Delivery {
                    request,
                    context: session.context.clone(),
                    started: Instant::now(),
                    lease: None,
                    previous: None,
                    frames: 0,
                    cancelled: session.cancelled.clone(),
                    sender: send,
                });
                state.setup.portrait.enabled.store(true, Ordering::SeqCst);
            }
            measured(Portrait::Capture, timing, &session.cancelled, || {
                let mut slot = Zeroizing::new(Vec::with_capacity(25600));
                let mut eligible = true;
                let mut count = 0usize;
                while count < BATCH_FRAMES {
                    session.current(&app, started)?;
                    if state.setup.portrait.missed.load(Ordering::SeqCst) {
                        return Err(
                    "The optional portrait observer could not keep up; normal listening continued"
                        .into(),
                );
                    }
                    let frame = match receive.recv_timeout(Duration::from_millis(20)) {
                        Ok(frame) => frame,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Err("Portrait input ended before the batch completed".into());
                        }
                    };
                    if frame.captured.elapsed() > Duration::from_millis(500) {
                        return Err("Portrait samples expired before observation".into());
                    }
                    if count.is_multiple_of(40) {
                        progress(
                            &app,
                            &session,
                            (request, batch),
                            count * FRAME_SAMPLES,
                            "observing",
                            features.iter().flatten().count(),
                            started,
                        );
                    }
                    if let Some(samples) = &frame.samples {
                        if eligible {
                            slot.extend(samples.iter().flat_map(|v| v.to_le_bytes()));
                        }
                    } else {
                        eligible = false;
                        use zeroize::Zeroize;
                        slot.zeroize();
                    }
                    count += 1;
                    if count.is_multiple_of(40) {
                        let index = usize::from(batch) * 10 + count / 40 - 1;
                        features[index] = if eligible {
                            measured(Portrait::Extract, timing, &session.cancelled, || {
                                portrait_dsp::extract(&slot, &mut || session.current(&app, started))
                            })?
                        } else {
                            None
                        };
                        progress(
                            &app,
                            &session,
                            (request, batch),
                            count * FRAME_SAMPLES,
                            "slot_complete",
                            features.iter().flatten().count(),
                            started,
                        );
                        use zeroize::Zeroize;
                        slot.zeroize();
                        slot.clear();
                        eligible = true;
                    }
                }
                drop(tap);
                drop(receive); // No outstanding sample holder before exposing completion.
                session.current(&app, started)?;
                {
                    let mut data = session
                        .data
                        .lock()
                        .map_err(|_| "Portrait state unavailable")?;
                    data.features = features;
                    data.batches += 1;
                }
                session.summary()
            })
        })
    })
    .await
    .map_err(|_| "Portrait observer owner stopped")?;
    caller.complete = result.is_ok();
    result
}

#[tauri::command]
pub(crate) async fn save_voice_portrait(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    session: Uuid,
) -> Result<View, String> {
    let started = Instant::now();
    visible(&window)?;
    let state = app.state::<Runtime>();
    let operation = state
        .setup
        .portrait
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| "Portrait observation is still retiring")?;
    let session = state.setup.portrait.get(session)?;
    session.current(&app, started)?;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let features = {
        let data = session
            .data
            .lock()
            .map_err(|_| "Portrait state unavailable")?;
        if data.batches != 2 || !data.features.iter().any(Option::is_some) {
            return Err(
                "Observe both batches with at least one measured acoustic slot before saving"
                    .into(),
            );
        }
        data.features.clone()
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let mut caller = Caller {
        cancelled: session.cancelled.clone(),
        complete: false,
    };
    let result = tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let _owner = owner;
        let result = measured(Portrait::Save, Uuid::new_v4(), &session.cancelled, || {
            voice_avatar::save_portrait(
                &directory,
                &session.context.microphone,
                &session.source,
                features,
                &mut || session.current(&app, started),
                &mut |temporary: &Path, destination: &Path| {
                    let state = app.state::<Runtime>();
                    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                    budget(started).inspect_err(|_| {
                        session.cancelled.store(true, Ordering::SeqCst);
                    })?;
                    session.current_locked(&state, &local)?;
                    std::fs::rename(temporary, destination).map_err(|_| {
                        "Portrait publication failed; refresh saved state before retrying".into()
                    })
                },
                &mut || {
                    session.cancelled.store(true, Ordering::SeqCst);
                },
            )
        });
        app.state::<Runtime>().setup.portrait.remove(session.id);
        result.and_then(|view| {
            session.current(&app, started)?;
            Ok(view)
        })
    })
    .await
    .map_err(|_| "Portrait writer stopped; refresh saved state before retrying")?;
    caller.complete = result.is_ok();
    result
}
#[tauri::command]
pub(crate) fn cancel_voice_portrait(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    session: Uuid,
) -> Result<(), String> {
    super::settings_only(&window)?;
    let state = app.state::<Runtime>();
    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if let Ok(value) = state.setup.portrait.get(session) {
        value.cancelled.store(true, Ordering::SeqCst);
        state.setup.portrait.remove(session);
    }
    Ok(())
}
