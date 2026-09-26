//! Native committed-event output. No model/media input can create a batch.
use crate::{Runtime, connection::SessionIdentity};
use avesra_contracts::ErrorCode;
use avesra_core::notifications::{Batch, Delivery, Event, Kind};
use avesra_windows::{
    audio::{PlaybackFrame, PlaybackRate},
    effects::{NotificationCommand as Command, NotificationResult as ResultValue},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use uuid::Uuid;

#[derive(Default)]
struct State {
    reader: Arc<tokio::sync::Mutex<()>>,
    arbitration: Arc<Mutex<Arbitration>>,
}
#[derive(Default)]
struct Arbitration {
    voice: bool,
    pending: Option<(Uuid, bool)>,
}
pub(crate) struct VoiceOwner(Arc<Mutex<Arbitration>>);
impl Drop for VoiceOwner {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock() {
            state.voice = false;
        }
    }
}
pub(crate) fn voice_owner(app: &tauri::AppHandle) -> Option<VoiceOwner> {
    let shared = app.state::<State>().arbitration.clone();
    {
        let mut state = shared.lock().ok()?;
        if state.voice || state.pending.is_some_and(|(_, granted)| granted) {
            return None;
        }
        state.voice = true;
    }
    Some(VoiceOwner(shared))
}
pub(crate) fn gap_requested(app: &tauri::AppHandle) -> bool {
    app.state::<State>()
        .arbitration
        .lock()
        .is_ok_and(|v| v.pending.is_some_and(|(_, granted)| !granted))
}
pub(crate) fn grant_voice_gap(app: &tauri::AppHandle) {
    if let Ok(mut state) = app.state::<State>().arbitration.lock()
        && state.voice
        && let Some((_, granted)) = &mut state.pending
    {
        *granted = true;
    }
}
struct Gap {
    shared: Arc<Mutex<Arbitration>>,
    id: Uuid,
}
impl Drop for Gap {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.lock()
            && state.pending.is_some_and(|(id, _)| id == self.id)
        {
            state.pending = None;
        }
    }
}
/// Only actual current Personal admission may share notification output with
/// continuous capture. The returned value owns no qualification mutex guard.
fn capture_compatible(state: &Runtime, local: &avesra_core::state::LocalState) -> bool {
    !state.media.capture_owned()
        || (local.capture_allowed()
            && local.voice_ready
            && !local.enrollment_capture
            && !local.microphone_check
            && state
                .qualification
                .admission(state, local)
                .is_some_and(|value| value.kind == avesra_core::voice::AdmissionKind::Personal))
}

async fn gap(
    app: &tauri::AppHandle,
    context: &Context,
    kind: Kind,
    volume: u8,
) -> Result<Gap, String> {
    let shared = app.state::<State>().arbitration.clone();
    let id = Uuid::new_v4();
    let compatible = {
        let runtime = app.state::<Runtime>();
        let local = runtime
            .local
            .lock()
            .map_err(|_| "Local state unavailable")?;
        capture_compatible(&runtime, &local)
    };
    {
        let mut state = shared
            .lock()
            .map_err(|_| "Notification handoff unavailable")?;
        if state.pending.is_some() {
            return Err("Notification handoff busy".into());
        }
        let granted = !state.voice && compatible;
        state.pending = Some((id, granted));
    }
    let owner = Gap { shared, id };
    let started = Instant::now();
    loop {
        context
            .current(app)
            .map_err(|_| "Notification owner changed")?;
        if settings(app, kind)? != Some(volume) || started.elapsed() >= Duration::from_secs(9) {
            return Err("Notification idle handoff expired".into());
        }
        let state = app.state::<Runtime>();
        if state.local.lock().map_or(true, |local| local.active_task) {
            return Err("Foreground task takes priority".into());
        }
        let granted = owner
            .shared
            .lock()
            .map_err(|_| "Notification handoff unavailable")?
            .pending
            == Some((id, true));
        if granted {
            let compatible = {
                let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                capture_compatible(&state, &local)
            };
            if compatible {
                return Ok(owner);
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
struct Worker(tauri::async_runtime::JoinHandle<()>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[derive(Clone)]
struct Context {
    actor: Uuid,
    session: SessionIdentity,
    challenge: u64,
}
impl Context {
    fn current(&self, app: &tauri::AppHandle) -> Result<(), ErrorCode> {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
        if !local.connected
            || local.locked
            || local.action_epoch != self.session.action_epoch
            || local.capture_epoch != self.session.epoch
            || state.connection_generation.load(Ordering::SeqCst) != self.session.generation
            || state.setup.challenge() != self.challenge
            || !state.acknowledged_session.lock().is_ok_and(|v| {
                v.is_some_and(|v| {
                    v.id == self.session.id
                        && v.device == self.session.device
                        && v.action_epoch == self.session.action_epoch
                        && v.epoch == self.session.epoch
                        && v.generation == self.session.generation
                })
            })
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
}
async fn context(app: &tauri::AppHandle) -> Result<Context, String> {
    let state = app.state::<Runtime>();
    let session = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected || local.locked {
            return Err("Owner events unavailable".into());
        }
        state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .ok_or("Session unavailable")?
    };
    let challenge = state.setup.challenge();
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let actor = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        crate::owner::identity(&directory).map(|v| v.0)
    })
    .await
    .map_err(|_| "Owner reader stopped")?
    .map_err(|_| "Owner unavailable")?;
    let value = Context {
        actor,
        session,
        challenge,
    };
    value.current(app).map_err(|_| "Owner changed")?;
    Ok(value)
}
async fn call(
    app: &tauri::AppHandle,
    context: &Context,
    command: Command,
) -> Result<ResultValue, String> {
    let guard = app
        .state::<State>()
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Event writer busy")?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(cancelled.clone());
    let expected = context.clone();
    let owner_app = app.clone();
    let receiver = app
        .state::<Runtime>()
        .effects
        .notification(
            command,
            Box::new(move || {
                if cancelled.load(Ordering::SeqCst) {
                    return Err(ErrorCode::Stale);
                }
                expected.current(&owner_app)
            }),
        )
        .map_err(|_| "Event writer busy")?;
    tokio::task::spawn_blocking(move || {
        let _guard = guard;
        receiver.recv()
    })
    .await
    .map_err(|_| "Event reader stopped")?
    .map_err(|_| "Event writer stopped")?
    .map_err(|_| "Event operation was not committed".into())
}
fn settings(app: &tauri::AppHandle, kind: Kind) -> Result<Option<u8>, String> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if !local.connected
        || local.locked
        || local.settings.paused
        || local.settings.deafened
        || local.settings.speaker.is_none()
    {
        return Ok(None);
    }
    let (enabled, volume) = match kind {
        Kind::Learning => (
            local.settings.learning_chime,
            local.settings.learning_chime_volume,
        ),
        Kind::Action => (
            local.settings.action_chime,
            local.settings.action_chime_volume,
        ),
    };
    Ok(enabled
        .then_some(volume.unwrap_or(local.settings.chime_volume))
        .filter(|v| *v > 0))
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|value| u64::try_from(value.as_millis()).ok())
        .unwrap_or(u64::MAX)
}
pub fn start(app: tauri::AppHandle) {
    app.manage(State::default());
    let owned = app.clone();
    let worker = tauri::async_runtime::spawn(async move {
        if avesra_windows::output_recording::enabled()
            && !avesra_windows::output_recording::notifications_enabled()
        {
            return;
        }
        let mut previous = None;
        let mut since = now_ms();
        let mut learning = None;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let Ok(context) = context(&owned).await else {
                previous = None;
                continue;
            };
            let key = (
                context.actor,
                context.session.id,
                context.session.action_epoch,
                context.session.epoch,
                context.challenge,
            );
            if previous != Some(key) {
                since = now_ms();
                if call(
                    &owned,
                    &context,
                    Command::Suppress {
                        actor: context.actor,
                        device: context.session.device,
                        before: since,
                    },
                )
                .await
                .is_err()
                {
                    continue;
                }
                previous = Some(key);
            }
            for kind in [Kind::Learning, Kind::Action] {
                let Ok(volume) = settings(&owned, kind) else {
                    continue;
                };
                if volume.is_some() {
                    let state = owned.state::<Runtime>();
                    if state.preview.try_lock().is_err()
                        || state.local.lock().map_or(true, |v| v.active_task)
                    {
                        continue;
                    }
                    if kind == Kind::Learning
                        && learning
                            .is_some_and(|last: Instant| last.elapsed() < Duration::from_secs(60))
                    {
                        continue;
                    }
                }
                let Ok(ResultValue::Batch(Some(batch))) = call(
                    &owned,
                    &context,
                    Command::Claim {
                        actor: context.actor,
                        device: context.session.device,
                        kind,
                        since,
                    },
                )
                .await
                else {
                    continue;
                };
                let delivery = match volume {
                    None => Delivery::Suppressed,
                    Some(volume) => {
                        // An uncertain partial tone also consumes the budget.
                        if kind == Kind::Learning {
                            learning = Some(Instant::now());
                        }
                        match play(&owned, &context, &batch, volume).await {
                            Ok(()) => {
                                if avesra_windows::output_recording::enabled() {
                                    Delivery::Uncertain
                                } else {
                                    Delivery::Submitted
                                }
                            }
                            Err(_) => Delivery::Uncertain,
                        }
                    }
                };
                let _ = call(&owned, &context, Command::Finish { batch, delivery }).await;
            }
        }
    });
    app.manage(Worker(worker));
}
async fn play(
    app: &tauri::AppHandle,
    context: &Context,
    batch: &Batch,
    volume: u8,
) -> Result<(), String> {
    context
        .current(app)
        .map_err(|_| "Notification source changed")?;
    if batch.actor() != context.actor || batch.device() != context.session.device {
        return Err("Notification owner changed".into());
    }
    let _gap = gap(app, context, batch.kind(), volume).await?;
    let mut owner = crate::output::Owner::reserve(app)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(cancelled.clone());
    let epoch = {
        let state = app.state::<Runtime>();
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.action_epoch != context.session.action_epoch
            || local.active_task
            || !capture_compatible(&state, &local)
        {
            return Err("Notification suppressed".into());
        }
        local.playback_epoch = local.playback_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        owner.bind(local.playback_epoch, context.session.generation);
        state
            .media
            .open_notification(&local, batch.id(), cancelled, volume)?;
        let _ = app.emit("runtime-state", local.clone());
        local.playback_epoch
    };
    let started = Instant::now();
    let current = || -> Result<(), String> {
        context
            .current(app)
            .map_err(|_| "Notification source changed")?;
        let state = app.state::<Runtime>();
        let compatible = {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            !local.active_task && capture_compatible(&state, &local)
        };
        if !compatible {
            return Err("Foreground voice takes priority".into());
        }
        if settings(app, batch.kind())? != Some(volume)
            || started.elapsed() >= Duration::from_secs(6)
        {
            return Err("Notification output suppressed".into());
        }
        app.state::<Runtime>()
            .media
            .output_state(epoch, batch.id())
            .map(|_| ())
    };
    let result = async {
        while !app
            .state::<Runtime>()
            .media
            .output_state(epoch, batch.id())?
            .0
        {
            current()?;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // Fixed bounded notification tone; no text/model/remote audio input.
        let origin = Instant::now();
        let frequency = match batch.kind() {
            Kind::Learning => 660.0f32,
            Kind::Action => 880.0f32,
        };
        for sequence in 1..=12u64 {
            current()?;
            let due = Duration::from_millis((sequence - 1) * 20);
            tokio::time::sleep_until(tokio::time::Instant::from_std(origin + due)).await;
            current()?;
            let samples = std::array::from_fn(|index| {
                let at = (sequence as usize - 1) * 480 + index;
                let phase = at as f32 / 5760.0;
                let envelope = (std::f32::consts::PI * phase).sin().powi(2);
                ((std::f32::consts::TAU * frequency * at as f32 / 24000.0).sin()
                    * envelope
                    * 0.18
                    * f32::from(i16::MAX)) as i16
            });
            app.state::<Runtime>().media.output_frame(PlaybackFrame {
                epoch,
                utterance: batch.id(),
                sequence,
                captured: origin + due,
                deadline: origin + Duration::from_secs(4),
                device_time: None,
                rate: PlaybackRate::Pcm24000,
                samples,
                valid_samples: 480,
                final_frame: sequence == 12,
            })?;
        }
        loop {
            current()?;
            let (_, submitted, drained) = app
                .state::<Runtime>()
                .media
                .output_state(epoch, batch.id())?;
            if submitted && drained {
                return Ok::<_, String>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    .await;
    owner.finish().await?;
    result
}
#[tauri::command]
pub async fn notification_status(window: tauri::WebviewWindow) -> Result<Vec<Event>, String> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to inspect committed events".into());
    }
    let app = window.app_handle();
    let context = context(app).await?;
    let ResultValue::Events(events) = call(
        app,
        &context,
        Command::Recent {
            actor: context.actor,
            device: context.session.device,
        },
    )
    .await?
    else {
        return Err("Event reader changed".into());
    };
    if !window.is_visible().unwrap_or(false) {
        return Err("Settings closed".into());
    }
    Ok(events)
}
