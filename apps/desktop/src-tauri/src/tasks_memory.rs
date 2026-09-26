//! Accepted fact output and one exact, protected deletion proposal.
use super::*;
use avesra_core::{
    conversations::{MemoryAnswer, PendingMemory, PlannerClaim, StoredReply},
    memory::conversation::DeleteView,
};
use avesra_windows::effects::PlannerAuthorization;
pub(super) struct Pending {
    context: std::sync::Weak<AcceptedContext>,
    view: DeleteView,
    sender: tokio::sync::oneshot::Sender<PlannerAuthorization>,
}
fn current_context(app: &tauri::AppHandle, context: &AcceptedContext) -> Result<(), ErrorCode> {
    if context.started.elapsed() >= Duration::from_secs(30) {
        return Err(ErrorCode::Expired);
    }
    context.current_owner(app)
}
fn source_authority(app: tauri::AppHandle, context: Arc<AcceptedContext>) -> PlannerAuthorization {
    Box::new(move |authority| {
        if authority.context.turn != context.target.id
            || authority.context.turn_revision != context.target.revision
            || authority.binding != &context.binding
        {
            return Err(ErrorCode::Stale);
        }
        current_context(&app, &context)
    })
}
pub(super) async fn answer(
    app: &tauri::AppHandle,
    context: Arc<AcceptedContext>,
    claim: PlannerClaim,
) -> Result<AcceptedResult, String> {
    let state = app.state::<Runtime>();
    let receiver = state
        .effects
        .memory_answer(claim, None, source_authority(app.clone(), context.clone()))
        .map_err(|_| "Memory writer unavailable")?;
    let stored = match receive(receiver).await? {
        MemoryAnswer::Reply(reply) => *reply,
        MemoryAnswer::Pending(pending) => wait(app, &context, *pending).await?,
    };
    current_context(app, &context).map_err(|_| "Memory reply context changed")?;
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if local.action_epoch != context.session.action_epoch || local.locked || !local.connected {
        return Err("Memory reply withdrawn".into());
    }
    let reply = state
        .effects
        .publish_planner(stored)
        .map_err(|_| "Memory reply publication withdrawn")?;
    let _ = app.emit("private-memory-changed", ());
    Ok(AcceptedResult::Reply(Box::new(reply)))
}
async fn wait(
    app: &tauri::AppHandle,
    context: &Arc<AcceptedContext>,
    pending: PendingMemory,
) -> Result<StoredReply, String> {
    current_context(app, context).map_err(|_| "Memory proposal context changed")?;
    pending
        .claim
        .remaining_ms()
        .map_err(|_| "Memory proposal expired")?;
    let view = pending.deletion.view();
    let (sender, mut receiver) = tokio::sync::oneshot::channel();
    {
        let state = app.state::<Runtime>();
        let mut slot = state
            .tasks
            .memory_pending
            .lock()
            .map_err(|_| "Memory proposal unavailable")?;
        if slot.is_some() {
            return Err("Another exact memory proposal is pending".into());
        }
        *slot = Some(Pending {
            context: Arc::downgrade(context),
            view: view.clone(),
            sender,
        });
    }
    let _ = app.emit("memory-forget-changed", ());
    let _=app.emit("runtime-error","Review the exact saved fact in Settings → Memory to approve deletion before this request expires.");
    let authorization = loop {
        tokio::select! {
            value=&mut receiver=>break value.map_err(|_|"Memory deletion approval withdrawn".to_owned()),
            _=tokio::time::sleep(Duration::from_millis(25))=>{
                if current_context(app,context).is_err() || pending.claim.remaining_ms().is_err(){break Err("Memory deletion expired or was withdrawn; nothing was deleted".into());}
            }
        }
    };
    {
        let state = app.state::<Runtime>();
        let mut slot = state
            .tasks
            .memory_pending
            .lock()
            .map_err(|_| "Memory proposal unavailable")?;
        if slot
            .as_ref()
            .is_some_and(|v| v.view.proposal == view.proposal)
        {
            *slot = None;
        }
    }
    drop(view);
    let _ = app.emit("memory-forget-changed", ());
    let authorize = match authorization {
        Ok(value) => value,
        Err(error) => {
            let _ = app
                .state::<Runtime>()
                .effects
                .retire_planner(pending.claim.retirement());
            return Err(error);
        }
    };
    let receiver = app
        .state::<Runtime>()
        .effects
        .memory_answer(pending.claim, Some(pending.deletion), authorize)
        .map_err(|_| "Memory deletion writer unavailable")?;
    match receive(receiver).await? {
        MemoryAnswer::Reply(reply) => Ok(*reply),
        MemoryAnswer::Pending(_) => Err("Memory deletion was not approved".into()),
    }
}
#[tauri::command]
pub async fn memory_forget_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<Option<DeleteView>, String> {
    visible(&window)?;
    current(&app, panel)?;
    let actor = crate::owner::current_actor(&app).await?;
    let (context, view) = {
        let state = app.state::<Runtime>();
        let pending = state
            .tasks
            .memory_pending
            .lock()
            .map_err(|_| "Memory proposal unavailable")?;
        let Some(pending) = pending.as_ref() else {
            return Ok(None);
        };
        let Some(context) = pending.context.upgrade() else {
            return Ok(None);
        };
        (context, pending.view.clone())
    };
    if context.target.actor != actor || current_context(&app, &context).is_err() {
        return Ok(None);
    }
    Ok(Some(view))
}
#[tauri::command]
pub async fn approve_memory_forget(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    proposal: Uuid,
) -> Result<(), String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    tauri::async_runtime::spawn(async move {
        let actor = crate::owner::current_actor(&app).await?;
        let context = {
            let state = app.state::<Runtime>();
            let pending = state
                .tasks
                .memory_pending
                .lock()
                .map_err(|_| "Memory proposal unavailable")?;
            let pending = pending
                .as_ref()
                .filter(|v| v.view.proposal == proposal)
                .ok_or("Memory proposal changed or expired")?;
            pending
                .context
                .upgrade()
                .ok_or("Memory proposal withdrawn")?
        };
        if context.target.actor != actor {
            return Err("Memory owner changed".into());
        }
        current_context(&app, &context).map_err(|_| "Memory proposal expired")?;
        let mut protected = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        let mut original = source_authority(app.clone(), context);
        let authorization: PlannerAuthorization = Box::new(move |authority| {
            let _guard = &guard;
            original(authority)?;
            protected()
        });
        let state = app.state::<Runtime>();
        let mut pending = state
            .tasks
            .memory_pending
            .lock()
            .map_err(|_| "Memory proposal unavailable")?;
        if !pending
            .as_ref()
            .is_some_and(|v| v.view.proposal == proposal)
        {
            return Err("Memory proposal changed or expired".into());
        }
        let pending = pending.take().ok_or("Memory proposal withdrawn")?;
        pending
            .sender
            .send(authorization)
            .map_err(|_| "Memory proposal withdrawn".to_owned())
    })
    .await
    .map_err(|_| "Memory approval coordinator stopped")?
}
#[tauri::command]
pub async fn cancel_memory_forget(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    proposal: Uuid,
) -> Result<(), String> {
    visible(&window)?;
    current(&app, panel)?;
    let actor = crate::owner::current_actor(&app).await?;
    let state = app.state::<Runtime>();
    let mut pending = state
        .tasks
        .memory_pending
        .lock()
        .map_err(|_| "Memory proposal unavailable")?;
    let value = pending
        .as_ref()
        .filter(|v| v.view.proposal == proposal)
        .ok_or("Memory proposal expired")?;
    let context = value.context.upgrade().ok_or("Memory proposal withdrawn")?;
    if context.target.actor != actor {
        return Err("Memory owner changed".into());
    }
    context.withdrawn.store(true, Ordering::SeqCst);
    *pending = None;
    Ok(())
}
