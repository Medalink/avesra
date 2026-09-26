//! Visible scoped teaching owns its actual native observer until retirement.
use super::*;
use avesra_windows::effects::teaching::{Operation, Request, ResultValue};
#[derive(Default)]
pub struct State {
    generation: std::sync::atomic::AtomicU64,
    active: Mutex<Option<Active>>,
    last: Mutex<Option<&'static str>>,
}
struct Active {
    started: Instant,
    id: Uuid,
    panel: Uuid,
    actor: Uuid,
    control: Arc<AtomicU8>,
    qualified: bool,
    transitions: u16,
}
#[derive(Serialize)]
pub struct View {
    snapshot: avesra_core::demonstration::Snapshot,
    active: Option<ActiveView>,
    last: Option<&'static str>,
}
#[derive(Serialize)]
struct ActiveView {
    id: Uuid,
    qualified: bool,
    transitions: u16,
    finishing: bool,
}
impl State {
    pub(super) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(active) = self.active.lock()
            && let Some(active) = active.as_ref()
        {
            active.control.store(2, Ordering::SeqCst);
        }
    }
}
fn command(
    app: &tauri::AppHandle,
    actor: Uuid,
    operation: Operation,
    authorize: avesra_windows::effects::CatalogAuthorization,
) -> Result<std::sync::mpsc::Receiver<Result<ResultValue, ErrorCode>>, String> {
    app.state::<Runtime>()
        .effects
        .teaching(Request {
            actor,
            operation,
            authorize,
        })
        .map_err(|_| "Teaching writer is busy".into())
}
#[tauri::command]
pub async fn teaching_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<View, String> {
    visible(&window)?;
    current(&app, panel)?;
    let actor = crate::owner::current_actor(&app).await?;
    let value = receive(command(
        &app,
        actor,
        Operation::Read,
        authorize(app.clone(), panel, actor, None)?,
    )?)
    .await?;
    current(&app, panel)?;
    let ResultValue::Snapshot(snapshot) = value else {
        return Err("Unexpected teaching state".into());
    };
    let state = app.state::<Runtime>();
    let active = state
        .tasks
        .teaching
        .active
        .lock()
        .map_err(|_| "Teaching unavailable")?
        .as_ref()
        .filter(|v| v.actor == actor && v.panel == panel)
        .map(|v| ActiveView {
            id: v.id,
            qualified: v.qualified,
            transitions: v.transitions,
            finishing: v.control.load(Ordering::SeqCst) != 0,
        });
    let last = *state
        .tasks
        .teaching
        .last
        .lock()
        .map_err(|_| "Teaching unavailable")?;
    Ok(View {
        snapshot,
        active,
        last,
    })
}
#[tauri::command]
pub async fn set_teaching_scope(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    alias: Option<Uuid>,
    scope: Option<Uuid>,
    revision: Uuid,
    excluded: bool,
) -> Result<(), String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        let authorization = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        app.state::<Runtime>().tasks.teaching.invalidate();
        receive(command(
            &app,
            actor,
            match (alias, scope, excluded) {
                (Some(alias), None, excluded) => Operation::Scope {
                    alias,
                    revision,
                    excluded,
                },
                (None, Some(scope), true) => Operation::Exclude { scope, revision },
                _ => return Err("Choose one exact observation scope".into()),
            },
            authorization,
        )?)
        .await?;
        Ok(())
    })
    .await
    .map_err(|_| "Teaching scope coordinator stopped")?
}
#[tauri::command]
pub async fn change_demonstration(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    id: Uuid,
    revision: Uuid,
    name: Option<String>,
) -> Result<(), String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        let authorization = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        app.state::<Runtime>().tasks.teaching.invalidate();
        receive(command(
            &app,
            actor,
            Operation::Change { id, revision, name },
            authorization,
        )?)
        .await?;
        Ok(())
    })
    .await
    .map_err(|_| "Teaching correction coordinator stopped")?
}
#[tauri::command]
pub async fn start_demonstration(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    scope: Uuid,
    revision: Uuid,
    name: String,
) -> Result<Uuid, String> {
    visible(&window)?;
    current(&app, panel)?;
    avesra_core::apps::alias_phrase(&name).map_err(|_| "Use a short routine name")?;
    let actor = crate::owner::current_actor(&app).await?;
    let generation = app
        .state::<Runtime>()
        .tasks
        .teaching
        .generation
        .load(Ordering::SeqCst);
    let setup = receive(command(
        &app,
        actor,
        Operation::Prepare { scope, revision },
        authorize(app.clone(), panel, actor, None)?,
    )?)
    .await?;
    let ResultValue::Setup(setup) = setup else {
        return Err("Teaching scope unavailable".into());
    };
    current(&app, panel)?;
    let state = app.state::<Runtime>();
    if state.tasks.teaching.generation.load(Ordering::SeqCst) != generation {
        return Err("Observation scope changed".into());
    }
    let session = state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?
        .ok_or("Session unavailable")?;
    let id = Uuid::new_v4();
    let control = Arc::new(AtomicU8::new(0));
    {
        let mut active = state
            .tasks
            .teaching
            .active
            .lock()
            .map_err(|_| "Teaching unavailable")?;
        if active.is_some() {
            return Err("Wait for the previous observer to finish".into());
        }
        *active = Some(Active {
            started: Instant::now(),
            id,
            panel,
            actor,
            control: control.clone(),
            qualified: false,
            transitions: 0,
        });
    }
    *state
        .tasks
        .teaching
        .last
        .lock()
        .map_err(|_| "Teaching unavailable")? = None;
    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let observed = tokio::task::spawn_blocking(move || {
            let mut authority =
                authorize(worker_app.clone(), panel, actor, None).map_err(|_| ErrorCode::Stale)?;
            let mut check = || {
                authority()?;
                let state = worker_app.state::<Runtime>();
                let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
                if state.tasks.teaching.generation.load(Ordering::SeqCst) != generation
                    || control.load(Ordering::SeqCst) == 2
                    || local.settings.paused
                    || local.action_epoch != session.action_epoch
                    || state.connection_generation.load(Ordering::SeqCst) != session.generation
                {
                    return Err(ErrorCode::Stale);
                }
                let busy = local.active_task || state.tasks.accepted.try_lock().is_err();
                Ok(busy)
            };
            let mut progress = |qualified, transitions| {
                let state = worker_app.state::<Runtime>();
                if let Ok(mut active) = state.tasks.teaching.active.lock()
                    && let Some(active) = active.as_mut().filter(|v| v.id == id)
                {
                    active.qualified = qualified;
                    active.transitions = transitions;
                }
            };
            avesra_windows::apps::demonstration::observe(
                *setup,
                session.device,
                name,
                &control,
                &mut check,
                &mut progress,
            )
        })
        .await;
        let result = match observed {
            Ok(Ok(Some(evidence))) => {
                let saved_app = app.clone();
                let authorize: avesra_windows::effects::CatalogAuthorization =
                    Box::new(move || {
                        let mut authority = super::authorize(saved_app.clone(), panel, actor, None)
                            .map_err(|_| ErrorCode::Stale)?;
                        authority()?;
                        let state = saved_app.state::<Runtime>();
                        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
                        if state.tasks.teaching.generation.load(Ordering::SeqCst) != generation
                            || local.settings.paused
                            || local.action_epoch != session.action_epoch
                            || state.connection_generation.load(Ordering::SeqCst)
                                != session.generation
                        {
                            return Err(ErrorCode::Stale);
                        }
                        let active = state
                            .tasks
                            .teaching
                            .active
                            .lock()
                            .map_err(|_| ErrorCode::Unavailable)?;
                        if !active.as_ref().is_some_and(|v| {
                            v.id == id
                                && v.started.elapsed() < Duration::from_secs(300)
                                && v.control.load(Ordering::SeqCst) == 1
                        }) {
                            return Err(ErrorCode::Stale);
                        }
                        Ok(())
                    });
                match command(&app, actor, Operation::Save(Box::new(evidence)), authorize) {
                    Ok(receiver) => receive(receiver).await.map(|_| ()),
                    Err(error) => Err(error),
                }
            }
            Ok(Ok(None)) => Err("No candidate saved".into()),
            _ => Err("Observation stopped without saving".into()),
        };
        let state = app.state::<Runtime>();
        if let Ok(mut active) = state.tasks.teaching.active.lock()
            && active.as_ref().is_some_and(|v| v.id == id)
        {
            *active = None;
        }
        if let Ok(mut last) = state.tasks.teaching.last.lock() {
            *last = Some(if result.is_ok() {
                "Saved an observed app-opening candidate. It has not been validated by execution."
            } else {
                "No candidate saved. Check the selected app, scope and routine name before starting again."
            });
        }
        let _ = app.emit("demonstration-changed", ());
    });
    Ok(id)
}
#[tauri::command]
pub fn stop_demonstration(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    id: Uuid,
    save: bool,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    if save {
        visible(&window)?;
        current(&app, panel)?;
    }
    let state = app.state::<Runtime>();
    let active = state
        .tasks
        .teaching
        .active
        .lock()
        .map_err(|_| "Teaching unavailable")?;
    let active = active
        .as_ref()
        .filter(|v| v.id == id && v.panel == panel)
        .ok_or("Teaching session changed")?;
    if !save {
        active.control.store(2, Ordering::SeqCst);
    } else {
        let _ = active
            .control
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
    }
    Ok(())
}

#[derive(Serialize)]
pub struct Progress {
    active: Option<ActiveView>,
    last: Option<&'static str>,
}
#[tauri::command]
pub fn teaching_progress(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<Progress, String> {
    visible(&window)?;
    current(&app, panel)?;
    let state = app.state::<Runtime>();
    let active = state
        .tasks
        .teaching
        .active
        .lock()
        .map_err(|_| "Teaching unavailable")?
        .as_ref()
        .filter(|v| v.panel == panel)
        .map(|v| ActiveView {
            id: v.id,
            qualified: v.qualified,
            transitions: v.transitions,
            finishing: v.control.load(Ordering::SeqCst) != 0,
        });
    let last = *state
        .tasks
        .teaching
        .last
        .lock()
        .map_err(|_| "Teaching unavailable")?;
    Ok(Progress { active, last })
}
