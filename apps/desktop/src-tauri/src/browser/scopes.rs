//! Bounded native scope management; polling never waits for database work.
use super::*;
use avesra_contracts::browser::{Origin, ScopeOperation, ScopeRef, ScopeStatus};
use avesra_core::browser_scopes::{Grant, Store as ScopeStore};

#[tauri::command]
pub async fn saved_browser_scopes(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<Grant>, String> {
    let generation = visible(&window)?;
    let state = app.state::<Runtime>();
    let work = state
        .browser
        .scopes
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Scope storage is busy")?;
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, None).map_err(|_| "Settings changed")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let values = tokio::task::spawn_blocking(move || {
        let _work = work;
        ScopeStore::open(&directory.join("browser-scopes.db"))?.list(Id::new(actor)?)
    })
    .await
    .map_err(|_| "Scope reader stopped")?
    .map_err(|_| "Scope database unavailable; preserved without changes")?;
    management_current(&app, generation, None).map_err(|_| "Settings changed")?;
    Ok(values)
}
#[tauri::command]
pub fn cancel_browser_scope(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    reference: ScopeRef,
) -> Result<(), String> {
    let generation = visible(&window)?;
    let state = app.state::<Runtime>();
    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser state unavailable")?;
    if inner.settings_generation != generation {
        return Err("Settings changed".into());
    }
    let mut slot = state
        .browser
        .scopes
        .pending
        .lock()
        .map_err(|_| "Scope state unavailable")?;
    if slot.as_ref().is_some_and(|p| p.reference() == reference) {
        *slot = None;
    }
    Ok(())
}
#[tauri::command]
pub async fn revoke_browser_scope(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    reference: ScopeRef,
) -> Result<(), String> {
    let admitted = visible(&window)?;
    let state = app.state::<Runtime>();
    let (work, generation, proof) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| "Browser state unavailable")?;
        if inner.settings_generation != admitted {
            return Err("Settings changed".into());
        }
        // Withdraw browser admission before removing a grant. Unrelated accepted
        // native tasks retain their independent action authority.
        inner.generation = inner.generation.checked_add(1).ok_or("Restart Avesra")?;
        inner.settings_generation = inner
            .settings_generation
            .checked_add(1)
            .ok_or("Restart Avesra")?;
        inner.attempt = None;
        state.browser.scopes.invalidate();
        let work = state
            .browser
            .scopes
            .work
            .clone()
            .try_lock_owned()
            .map_err(|_| "Scope writer is closing. Refresh before revoking.")?;
        let proof = state
            .setup
            .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?;
        (work, inner.settings_generation, proof)
    };
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, Some(&proof))
        .map_err(|_| "Original verification expired")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    tokio::task::spawn_blocking(move || {
        let _work = work;
        if !crate::owner::matches_actor(&directory, actor) {
            return Err(ErrorCode::Unauthenticated);
        }
        ScopeStore::open(&directory.join("browser-scopes.db"))?.revoke(
            Id::new(actor)?,
            reference.id,
            reference.revision,
            &mut || management_current(&app, generation, Some(&proof)),
        )
    })
    .await
    .map_err(|_| "Scope writer stopped")?
    .map_err(|_| {
        "Revocation is unverified. Refresh saved scopes before another explicit action.".into()
    })
}

#[derive(Default)]
pub(super) struct Coordinator {
    work: Arc<tokio::sync::Mutex<()>>,
    pending: Mutex<Option<Pending>>,
}
#[derive(Clone, Copy)]
struct Context {
    attempt: Uuid,
    transport: u64,
    settings: u64,
    connection: u64,
    session: Id,
    selection: Id,
    action: u64,
    capture: u64,
    actor: Uuid,
}
impl Context {
    fn matches(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
    ) -> bool {
        inner.generation == self.transport
            && inner.settings_generation == self.settings
            && local.connected
            && !local.locked
            && !local.settings.paused
            && local.action_epoch == self.action
            && local.capture_epoch == self.capture
            && state.connection_generation.load(Ordering::SeqCst) == self.connection
            && inner
                .attempt
                .as_ref()
                .is_some_and(|v| self.matches_attempt(v))
    }
    fn matches_attempt(&self, attempt: &Attempt) -> bool {
        attempt.id == self.attempt
            && attempt.connection == self.connection
            && attempt.current_time()
            && attempt.state == "authenticated_no_scopes"
            && attempt.session == Some(self.session)
            && attempt.selection == Some(self.selection)
            && attempt.actor == Some(self.actor)
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Pending,
    Saving,
    Saved,
    Declined,
    Unavailable,
}
struct Pending {
    context: Context,
    started: Instant,
    proof: Arc<ManagementProof>,
    grant: Grant,
    phase: Phase,
    work: Option<tokio::sync::OwnedMutexGuard<()>>,
}
impl Pending {
    fn reference(&self) -> ScopeRef {
        ScopeRef {
            id: self.grant.id,
            revision: self.grant.revision,
        }
    }
    fn current(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
    ) -> bool {
        self.started.elapsed() < Duration::from_secs(45)
            && self.context.matches(state, local, inner)
            && self.proof.current_locked(state, local)
    }
}
impl Coordinator {
    pub(super) fn invalidate(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
    }
    pub(super) fn observe(&self, local: &avesra_core::state::LocalState) {
        if let Ok(mut slot) = self.pending.lock()
            && slot.as_ref().is_some_and(|p| {
                !local.connected
                    || local.locked
                    || local.settings.paused
                    || p.context.action != local.action_epoch
                    || p.context.capture != local.capture_epoch
                    || p.started.elapsed() >= Duration::from_secs(45)
            })
        {
            *slot = None;
        }
    }
    // Parent holds Runtime.local -> browser.inner. Never reacquire either here.
    pub(super) fn status(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        generation: u64,
        attempt: &Attempt,
    ) -> Result<Option<ScopeStatus>, ErrorCode> {
        let mut slot = self.pending.lock().map_err(|_| ErrorCode::Unavailable)?;
        if slot.as_ref().is_some_and(|p| {
            p.context.transport != generation
                || !p.context.matches_attempt(attempt)
                || p.context.action != local.action_epoch
                || p.context.capture != local.capture_epoch
                || !p.proof.current_locked(state, local)
                || local.settings.paused
                || p.started.elapsed() >= Duration::from_secs(45)
        }) {
            *slot = None;
        }
        Ok(slot.as_ref().map(|p| {
            let reference = p.reference();
            match p.phase {
                Phase::Pending => ScopeStatus::Pending {
                    reference,
                    origin: p.grant.origin.clone(),
                    operations: p.grant.operations.clone(),
                    remaining_ms: 45_000 - p.started.elapsed().as_millis().min(44_999) as u64,
                },
                Phase::Saving => ScopeStatus::Saving { reference },
                Phase::Saved => ScopeStatus::Saved { reference },
                Phase::Declined => ScopeStatus::Declined { reference },
                Phase::Unavailable => ScopeStatus::Unavailable { reference },
            }
        }))
    }
}
fn authorize(
    app: &tauri::AppHandle,
    context: Context,
    reference: ScopeRef,
) -> Result<(), ErrorCode> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    let slot = state
        .browser
        .scopes
        .pending
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    let p = slot.as_ref().ok_or(ErrorCode::Stale)?;
    if p.reference() != reference
        || p.context.transport != context.transport
        || !matches!(p.phase, Phase::Saving)
        || !p.current(&state, &local, &inner)
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
#[tauri::command]
pub async fn propose_browser_scope(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    origin: Origin,
    operations: Vec<ScopeOperation>,
) -> Result<ScopeRef, String> {
    browser::validate_operations(&operations).map_err(|_| "Choose supported operations")?;
    let admitted = visible(&window)?;
    let started = Instant::now();
    let state = app.state::<Runtime>();
    let (work, context, proof, pairing, record) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| "Browser state unavailable")?;
        state.browser.scopes.observe(&local);
        let work = state
            .browser
            .scopes
            .work
            .clone()
            .try_lock_owned()
            .map_err(|_| "A scope operation is still pending")?;
        let attempt = inner
            .attempt
            .as_ref()
            .ok_or("Connect the selected installation first")?;
        if inner.settings_generation != admitted
            || local.locked
            || !local.connected
            || local.settings.paused
            || !attempt.current_time()
            || attempt.state != "authenticated_no_scopes"
        {
            return Err("Selected connection changed".into());
        }
        let context = Context {
            attempt: attempt.id,
            transport: inner.generation,
            settings: admitted,
            connection: attempt.connection,
            session: attempt.session.ok_or("Selected session unavailable")?,
            selection: attempt.selection.ok_or("Use the selected connection")?,
            action: local.action_epoch,
            capture: local.capture_epoch,
            actor: attempt.actor.ok_or("Owner unavailable")?,
        };
        if !context.matches(&state, &local, &inner) {
            return Err("Selected connection changed".into());
        }
        let proof = Arc::new(state.setup.management_proof(&local, context.connection)?);
        (
            work,
            context,
            proof,
            attempt.pairing.ok_or("Pairing unavailable")?,
            attempt.selected.clone().ok_or("Application unavailable")?,
        )
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let prepared = tokio::task::spawn_blocking(move || {
        let work = work;
        if !crate::owner::matches_actor(&directory, context.actor) {
            return Err(ErrorCode::Unauthenticated);
        }
        let (selected, binding) = Store::open(&directory.join("browser-pairings"))?
            .selected(Id::new(context.actor)?)?
            .ok_or(ErrorCode::Stale)?;
        if selected.revision != context.selection
            || selected.pairing != pairing
            || binding.browser_app.uuid() != record.id
            || binding.browser_revision.uuid() != record.revision
        {
            return Err(ErrorCode::Stale);
        }
        let existing = ScopeStore::open(&directory.join("browser-scopes.db"))?
            .list(Id::new(context.actor)?)?;
        if existing.len() >= 32
            || existing
                .iter()
                .any(|v| v.selection == context.selection && v.origin == origin)
        {
            return Err(ErrorCode::Denied);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ErrorCode::Expired)?
            .as_millis();
        let grant = Grant {
            id: Id::new(Uuid::new_v4())?,
            revision: Id::new(Uuid::new_v4())?,
            actor: Id::new(context.actor)?,
            selection: context.selection,
            pairing,
            browser_app: Id::new(record.id)?,
            browser_revision: Id::new(record.revision)?,
            origin,
            operations,
            created_at_ms: u64::try_from(now).map_err(|_| ErrorCode::Expired)?,
        };
        grant.validate()?;
        Ok::<_, ErrorCode>((work, grant))
    })
    .await
    .map_err(|_| "Scope preparation stopped")?
    .map_err(|_| "Scope proposal unavailable; inspect saved grants")?;
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser state unavailable")?;
    if !context.matches(&state, &local, &inner)
        || !proof.current_locked(&state, &local)
        || started.elapsed() >= Duration::from_secs(45)
    {
        return Err("Original scope approval expired".into());
    }
    let (work, grant) = prepared;
    let reference = ScopeRef {
        id: grant.id,
        revision: grant.revision,
    };
    *state
        .browser
        .scopes
        .pending
        .lock()
        .map_err(|_| "Scope state unavailable")? = Some(Pending {
        context,
        started,
        proof,
        grant,
        phase: Phase::Pending,
        work: Some(work),
    });
    Ok(reference)
}
pub(super) fn decision(
    app: &tauri::AppHandle,
    attempt: Uuid,
    generation: u64,
    session: Id,
    reference: ScopeRef,
    action_epoch: u64,
    permitted: bool,
) -> Result<(), ErrorCode> {
    let state = app.state::<Runtime>();
    let (work, grant, context) = {
        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
        let inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        let mut slot = state
            .browser
            .scopes
            .pending
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        let p = slot.as_mut().ok_or(ErrorCode::Stale)?;
        if p.reference() != reference
            || p.context.attempt != attempt
            || p.context.transport != generation
            || p.context.session != session
            || p.context.action != action_epoch
            || !matches!(p.phase, Phase::Pending)
            || !p.current(&state, &local, &inner)
        {
            return Err(ErrorCode::Stale);
        }
        if !permitted {
            p.phase = Phase::Declined;
            p.work.take();
            return Ok(());
        }
        let work = p.work.take().ok_or(ErrorCode::Stale)?;
        p.phase = Phase::Saving;
        (work, p.grant.clone(), p.context)
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let writer_app = app.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _work = work;
            let directory = writer_app
                .path()
                .app_data_dir()
                .map_err(|_| ErrorCode::Unavailable)?;
            if !crate::owner::matches_actor(&directory, context.actor) {
                return Err(ErrorCode::Unauthenticated);
            }
            let owned = writer_app.clone();
            let reply = writer_app.state::<Runtime>().effects.with_app(
                context.actor,
                grant.browser_app.uuid(),
                grant.browser_revision.uuid(),
                Box::new(move || {
                    let mut store = ScopeStore::open(&directory.join("browser-scopes.db"))?;
                    store.publish(&grant, &mut || authorize(&owned, context, reference))?;
                    store.get(grant.actor, grant.id, grant.revision)?;
                    Ok(())
                }),
            )?;
            reply.recv().map_err(|_| ErrorCode::Unavailable)?
        })
        .await;
        let state = app.state::<Runtime>();
        let Ok(local) = state.local.lock() else {
            return;
        };
        let Ok(inner) = state.browser.inner.lock() else {
            return;
        };
        let Ok(mut slot) = state.browser.scopes.pending.lock() else {
            return;
        };
        if let Some(p) = slot.as_mut()
            && p.reference() == reference
            && p.current(&state, &local, &inner)
        {
            p.phase = if matches!(result, Ok(Ok(()))) {
                Phase::Saved
            } else {
                Phase::Unavailable
            };
        }
    });
    Ok(())
}
