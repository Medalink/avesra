//! Pending approval stays with the original accepted native coordinator.
use super::*;
pub(super) struct Pending {
    pub context: std::sync::Weak<AcceptedContext>,
    pub action: avesra_contracts::Action,
    pub sender: tokio::sync::oneshot::Sender<()>,
}
fn action_current(action: &avesra_contracts::Action) -> Result<(), ErrorCode> {
    action.validate(
        u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ErrorCode::Expired)?
                .as_millis(),
        )
        .map_err(|_| ErrorCode::Expired)?,
    )
}
pub(super) async fn wait(
    app: &tauri::AppHandle,
    context: &Arc<AcceptedContext>,
    action: &avesra_contracts::Action,
) -> Result<(), String> {
    context
        .current_owner(app)
        .map_err(|_| "Approval context changed")?;
    action_current(action).map_err(|_| "Proposal expired")?;
    let (sender, mut receiver) = tokio::sync::oneshot::channel();
    {
        let state = app.state::<Runtime>();
        let mut pending = state
            .tasks
            .download_pending
            .lock()
            .map_err(|_| "Proposal unavailable")?;
        if pending.is_some() {
            return Err("Another exact proposal is pending".into());
        }
        *pending = Some(Pending {
            context: Arc::downgrade(context),
            action: action.clone(),
            sender,
        });
    }
    let _ = project(app, context.target.actor).await;
    let _=app.emit("runtime-error","Download DNS failure observed. In Settings → Actions, review and approve the exact DNS-cache proposal within its remaining deadline. This is not proof that cache clearing will fix the download.");
    let result = loop {
        tokio::select! {
            value=&mut receiver=>break value.map_err(|_|"Exact approval withdrawn".to_owned()),
            _=tokio::time::sleep(Duration::from_millis(25))=>{
                if context.started.elapsed()>=Duration::from_secs(30)||context.current_owner(app).is_err()||action_current(action).is_err(){break Err("Proposal expired or was withdrawn; no new configuration action is admitted".into());}
            }
        }
    };
    {
        let state = app.state::<Runtime>();
        let mut pending = state
            .tasks
            .download_pending
            .lock()
            .map_err(|_| "Proposal unavailable")?;
        if pending
            .as_ref()
            .is_some_and(|p| p.action.revision == action.revision)
        {
            *pending = None;
        }
    }
    if result.is_err() {
        // This opaque accepted owner may withdraw its original task even when
        // its effect deadline expired. It cannot issue or approve a replacement.
        let target = context.target;
        let directory = app.path().app_data_dir().map_err(|_| "Owner unavailable")?;
        let _ = app.state::<Runtime>().effects.cancel_conversation(
            target.actor,
            target.source,
            target.id,
            target.revision,
            Box::new(move || {
                if crate::owner::matches_actor(&directory, target.actor) {
                    Ok(())
                } else {
                    Err(ErrorCode::Stale)
                }
            }),
        );
    }
    result?;
    context
        .current_owner(app)
        .map_err(|_| "Approved task withdrawn")?;
    action_current(action).map_err(|_| "Approved proposal expired".into())
}
#[tauri::command]
pub async fn grant_download_diagnosis(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    input: avesra_core::download::Input,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    input
        .validate()
        .map_err(|_| "Use one hostname or numeric address, port80/443 and a local drive letter")?;
    let actor = crate::owner::current_actor(&app).await?;
    let context = avesra_core::download::Context {
        id: Uuid::new_v4(),
        revision: Uuid::new_v4(),
        actor,
        input,
    };
    grant(
        app,
        panel,
        Selection::Download {
            context: Box::new(context),
            configuration: false,
        },
    )
    .await
}
#[tauri::command]
pub async fn grant_download_fix(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    id: Uuid,
    revision: Uuid,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let actor = crate::owner::current_actor(&app).await?;
    let receiver = app
        .state::<Runtime>()
        .effects
        .actions(ActionManagement::Read { actor })
        .map_err(|_| "Permission worker busy")?;
    let snapshot = receive(receiver).await?;
    current(&app, panel)?;
    let context = snapshot
        .permissions
        .into_iter()
        .filter(|v| !v.revoked)
        .find_map(|v| match v.permission.target {
            avesra_core::action_permissions::TaskTarget::Download {
                context,
                configuration: false,
            } if context.id == id && context.revision == revision && context.actor == actor => {
                Some(context)
            }
            _ => None,
        })
        .ok_or("Exact diagnosis context changed")?;
    grant(
        app,
        panel,
        Selection::Download {
            context,
            configuration: true,
        },
    )
    .await
}
#[tauri::command]
pub async fn approve_download_fix(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    revision: Uuid,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Approval writer busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        let (context, action) = {
            let state = app.state::<Runtime>();
            let pending = state
                .tasks
                .download_pending
                .lock()
                .map_err(|_| "Proposal unavailable")?;
            let pending = pending
                .as_ref()
                .filter(|p| p.action.revision == revision && p.action.actor_id == actor)
                .ok_or("No live exact proposal; ask again only if you still want it")?;
            (
                pending
                    .context
                    .upgrade()
                    .ok_or("Original accepted owner ended")?,
                pending.action.clone(),
            )
        };
        context
            .current_owner(&app)
            .map_err(|_| "Original owner changed")?;
        action_current(&action).map_err(|_| "Proposal expired")?;
        let mut protected = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        let expected = context.clone();
        let owned_app = app.clone();
        let receiver = app
            .state::<Runtime>()
            .effects
            .actions(ActionManagement::ApproveDownload {
                revision,
                session: context.dispatch(),
                authorize: Box::new(move || {
                    if expected.started.elapsed() >= Duration::from_secs(30) {
                        return Err(ErrorCode::Expired);
                    }
                    action_current(&action)?;
                    expected.current_owner(&owned_app)?;
                    protected()
                }),
            })
            .map_err(|_| "Approval worker busy")?;
        let snapshot = receive(receiver).await?;
        {
            let state = app.state::<Runtime>();
            let mut pending = state
                .tasks
                .download_pending
                .lock()
                .map_err(|_| "Proposal unavailable")?;
            if pending
                .as_ref()
                .is_some_and(|p| p.action.revision == revision)
                && let Some(pending) = pending.take()
            {
                let _ = pending.sender.send(());
            }
        }
        publish(&app, panel, actor, snapshot)
    })
    .await
    .map_err(|_| "Approval coordinator stopped")?
}
