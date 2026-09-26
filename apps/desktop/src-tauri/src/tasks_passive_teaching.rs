//! Optional bounded native observer; no accepted-turn or ambient-text ingress.
use super::*;
use crate::connection::SessionIdentity;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Attempt {
    actor: Uuid,
    setting: Uuid,
    session: Uuid,
    generation: u64,
    action_epoch: u64,
}
fn current(
    app: &tauri::AppHandle,
    actor: Uuid,
    session: SessionIdentity,
    generation: u64,
    verify_owner: bool,
) -> Result<bool, ErrorCode> {
    let state = app.state::<Runtime>();
    let _owner = state
        .owner_setup
        .try_lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    if verify_owner {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| ErrorCode::Unavailable)?;
        if !crate::owner::matches_actor(&directory, actor) {
            return Err(ErrorCode::Stale);
        }
    }
    let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    if state
        .tasks
        .teaching
        .passive_generation
        .load(Ordering::SeqCst)
        != generation
        || !local.enrolled
        || !local.connected
        || local.locked
        || local.settings.paused
        || local.action_epoch != session.action_epoch
        || state.connection_generation.load(Ordering::SeqCst) != session.generation
        || !state
            .acknowledged_session
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?
            .is_some_and(|s| {
                s.id == session.id
                    && s.device == session.device
                    && s.generation == session.generation
                    && s.action_epoch == session.action_epoch
                    && s.server_fingerprint == session.server_fingerprint
            })
    {
        return Err(ErrorCode::Stale);
    }
    Ok(local.active_task
        || local.enrollment_capture
        || local.microphone_check
        || state.media.no_output(&local).is_none()
        || state.tasks.accepted.try_lock().is_err())
}
fn authorization(
    app: tauri::AppHandle,
    actor: Uuid,
    session: SessionIdentity,
    generation: u64,
) -> avesra_windows::effects::CatalogAuthorization {
    Box::new(move || {
        if current(&app, actor, session, generation, true)? {
            Err(ErrorCode::Unavailable)
        } else {
            Ok(())
        }
    })
}
async fn background(app: &tauri::AppHandle, request: Request) -> Result<ResultValue, String> {
    let receive = app
        .state::<Runtime>()
        .effects
        .teaching_background(request)
        .map_err(|_| "Background teaching deferred")?;
    super::receive(receive).await
}
#[tauri::command]
pub async fn set_passive_teaching(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    scope: Uuid,
    revision: Uuid,
    enabled: bool,
) -> Result<(), String> {
    visible(&window)?;
    super::current(&app, panel)?;
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
        let authorize = super::authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        app.state::<Runtime>().tasks.teaching.invalidate_learning();
        receive(command(
            &app,
            actor,
            Operation::Passive {
                scope,
                revision,
                enabled,
            },
            authorize,
        )?)
        .await?;
        app.state::<Runtime>()
            .tasks
            .teaching
            .passive_generation
            .fetch_add(1, Ordering::SeqCst);
        Ok(())
    })
    .await
    .map_err(|_| "Passive setting writer stopped")?
}
pub fn start_passive(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut attempted = None;
        let mut inspected: Option<(Uuid, u64, u64, u64, Instant)> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let state = app.state::<Runtime>();
            let session = {
                let Ok(local) = state.local.lock() else {
                    continue;
                };
                if !local.enrolled
                    || !local.connected
                    || local.locked
                    || local.settings.paused
                    || local.active_task
                    || local.enrollment_capture
                    || local.microphone_check
                    || state.media.no_output(&local).is_none()
                    || state.tasks.accepted.try_lock().is_err()
                {
                    continue;
                }
                let Ok(active) = state.tasks.teaching.active.lock() else {
                    continue;
                };
                if active.is_some() {
                    continue;
                }
                let Ok(session) = state.acknowledged_session.lock() else {
                    continue;
                };
                let Some(session) = *session else { continue };
                session
            };
            let generation = state
                .tasks
                .teaching
                .passive_generation
                .load(Ordering::SeqCst);
            if inspected.as_ref().is_some_and(|(s, e, g, c, at)| {
                *s == session.id
                    && *e == session.action_epoch
                    && *g == generation
                    && *c == session.generation
                    && at.elapsed() < Duration::from_secs(30)
            }) {
                continue;
            }
            inspected = Some((
                session.id,
                session.action_epoch,
                generation,
                session.generation,
                Instant::now(),
            ));
            let Ok(actor) = crate::owner::current_actor(&app).await else {
                continue;
            };
            let prepared = background(
                &app,
                Request {
                    actor,
                    operation: Operation::PassiveRead,
                    authorize: authorization(app.clone(), actor, session, generation),
                },
            )
            .await;
            let Ok(ResultValue::Passive(Some(prepared))) = prepared else {
                continue;
            };
            let key = Attempt {
                actor,
                setting: prepared.setting.revision,
                session: session.id,
                generation: session.generation,
                action_epoch: session.action_epoch,
            };
            if attempted == Some(key)
                || !matches!(current(&app, actor, session, generation, true), Ok(false))
            {
                continue;
            }
            let started = Instant::now();
            let id = Uuid::new_v4();
            let control = Arc::new(AtomicU8::new(0));
            {
                let Ok(mut active) = state.tasks.teaching.active.lock() else {
                    continue;
                };
                if active.is_some() {
                    continue;
                }
                *active = Some(Active {
                    app_name: prepared.setup.scope.name.clone(),
                    passive: true,
                    started,
                    id,
                    panel: Uuid::nil(),
                    actor,
                    control: control.clone(),
                    qualified: false,
                    transitions: 0,
                });
            }
            attempted = Some(key);
            if let Ok(mut last) = state.tasks.teaching.last.lock() {
                *last = Some("Passive app learning is active for this bounded session.");
            }
            let observed_app = app.clone();
            let worker_control = control.clone();
            let observed = tokio::task::spawn_blocking(move || {
                std::thread::Builder::new()
                    .name("avesra-passive-app".into())
                    .spawn(move || {
                        let mut check = || {
                            if worker_control.load(Ordering::SeqCst) == 2 {
                                return Err(ErrorCode::Stale);
                            }
                            current(&observed_app, actor, session, generation, false)
                        };
                        let mut progress = |qualified, transitions| {
                            if let Ok(mut active) =
                                observed_app.state::<Runtime>().tasks.teaching.active.lock()
                                && let Some(active) = active.as_mut().filter(|v| v.id == id)
                            {
                                active.qualified = qualified;
                                active.transitions = transitions;
                            }
                        };
                        avesra_windows::apps::demonstration::observe_passive(
                            *prepared,
                            session.device,
                            avesra_windows::apps::demonstration::Admission {
                                started,
                                control: &worker_control,
                            },
                            &mut check,
                            &mut progress,
                        )
                    })
                    .map_err(|_| ErrorCode::Unavailable)?
                    .join()
                    .map_err(|_| ErrorCode::Unavailable)?
            })
            .await;
            let saved = if let Ok(Ok(Some(evidence))) = observed {
                let writer_app = app.clone();
                let authorize = Box::new(move || {
                    if current(&writer_app, actor, session, generation, true)? {
                        return Err(ErrorCode::Unavailable);
                    }
                    let state = writer_app.state::<Runtime>();
                    let active = state
                        .tasks
                        .teaching
                        .active
                        .lock()
                        .map_err(|_| ErrorCode::Unavailable)?;
                    if !active.as_ref().is_some_and(|v| {
                        v.id == id
                            && v.passive
                            && v.started.elapsed() < Duration::from_secs(300)
                            && v.control.load(Ordering::SeqCst) == 1
                    }) {
                        return Err(ErrorCode::Stale);
                    }
                    Ok(())
                });
                background(
                    &app,
                    Request {
                        actor,
                        operation: Operation::PassiveSave(Box::new(evidence)),
                        authorize,
                    },
                )
                .await
                .is_ok()
            } else {
                false
            };
            if let Ok(mut active) = state.tasks.teaching.active.lock()
                && active.as_ref().is_some_and(|v| v.id == id)
            {
                *active = None;
            }
            if let Ok(mut last) = state.tasks.teaching.last.lock() {
                *last = Some(if saved {
                    "Saved a passive app-opening candidate. It is not verified execution."
                } else {
                    "Passive observation ended without saving. This session will not restart it automatically."
                });
            }
            let _ = app.emit("demonstration-changed", ());
        }
    });
}
