//! Protected setup metadata only. No semantic read or accepted action ingress.
use super::*;
use avesra_contracts::browser::{ScopeOperation, ScopeRef, documents as wire};
use avesra_core::browser_scopes::{Grant, Store as ScopeStore};
use scopes::Context;

#[derive(Default)]
pub(super) struct Coordinator {
    work: Arc<tokio::sync::Mutex<()>>,
    pending: Mutex<Option<Pending>>,
}
struct Pending {
    context: Context,
    proof: Arc<ManagementProof>,
    grant: Grant,
    started: Instant,
    request: Id,
    observation_revision: u64,
    resource_generation: u64,
    mode: wire::Mode,
    phase: Phase,
    _work: tokio::sync::OwnedMutexGuard<()>,
}
#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Phase {
    Waiting,
    Available {
        candidates: Vec<wire::Candidate>,
    },
    Selected {
        identity: Id,
        revision: Id,
        candidate: wire::Candidate,
    },
    Unavailable,
    Expired,
    TooMany,
}
#[derive(Serialize)]
pub struct Status {
    request: Option<Id>,
    scope: Option<ScopeRef>,
    remaining_ms: u64,
    phase: Phase,
}
impl Pending {
    fn current(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
    ) -> bool {
        state
            .effects
            .browser_work_generation()
            .is_ok_and(|v| v == self.resource_generation)
            && self.started.elapsed() < Duration::from_millis(wire::LIFETIME_MS)
            && self.context.matches(state, local, inner)
            && inner
                .attempt
                .as_ref()
                .is_some_and(|v| v.observation_revision == self.observation_revision)
            && self.proof.current_locked(state, local)
    }
    fn wire(&self) -> wire::Request {
        wire::Request {
            request: self.request,
            scope: ScopeRef {
                id: self.grant.id,
                revision: self.grant.revision,
            },
            origin: self.grant.origin.clone(),
            remaining_ms: wire::LIFETIME_MS
                - self
                    .started
                    .elapsed()
                    .as_millis()
                    .min(u128::from(wire::LIFETIME_MS - 1)) as u64,
            mode: self.mode.clone(),
        }
    }
}
impl Coordinator {
    pub(super) fn invalidate(&self) {
        if let Ok(mut slot) = self.pending.lock() {
            *slot = None;
        }
    }
    pub(super) fn observe(&self, local: &avesra_core::state::LocalState) {
        if let Ok(mut slot) = self.pending.lock()
            && slot.as_ref().is_some_and(|p| {
                !local.connected
                    || local.locked
                    || local.settings.paused
                    || local.action_epoch != p.context.action
                    || local.capture_epoch != p.context.capture
                    || p.started.elapsed() >= Duration::from_millis(wire::LIFETIME_MS)
            })
        {
            *slot = None;
        }
    }
    // Caller holds local -> browser.inner. This path never waits for storage.
    pub(super) fn request(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        generation: u64,
        attempt: &Attempt,
    ) -> Result<Option<wire::Request>, ErrorCode> {
        let mut slot = self.pending.lock().map_err(|_| ErrorCode::Unavailable)?;
        if slot.as_ref().is_some_and(|p| {
            !state
                .effects
                .browser_work_generation()
                .is_ok_and(|v| v == p.resource_generation)
                || p.context.transport != generation
                || p.context.attempt != attempt.id
                || p.observation_revision != attempt.observation_revision
                || !attempt.current_time()
                || local.action_epoch != p.context.action
                || local.capture_epoch != p.context.capture
                || !p.proof.current_locked(state, local)
                || p.started.elapsed() >= Duration::from_millis(wire::LIFETIME_MS)
        }) {
            *slot = None;
        }
        Ok(slot
            .as_ref()
            .filter(|p| matches!(p.phase, Phase::Waiting))
            .map(Pending::wire))
    }
}
fn admitted(
    state: &Runtime,
    local: &avesra_core::state::LocalState,
    inner: &Inner,
    generation: u64,
) -> Result<Context, String> {
    if state.effects.browser_work_blocked() {
        return Err("Browser work ownership is unresolved".into());
    }
    let attempt = inner
        .attempt
        .as_ref()
        .ok_or("Connect selected browser first")?;
    let context = Context {
        attempt: attempt.id,
        transport: inner.generation,
        settings: generation,
        connection: attempt.connection,
        session: attempt.session.ok_or("Browser session unavailable")?,
        selection: attempt.selection.ok_or("Use the selected installation")?,
        action: local.action_epoch,
        capture: local.capture_epoch,
        actor: attempt.actor.ok_or("Owner unavailable")?,
    };
    if !context.matches(state, local, inner) {
        return Err("Selected browser context changed".into());
    }
    Ok(context)
}
#[tauri::command]
pub async fn inspect_browser_documents(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    reference: ScopeRef,
) -> Result<Id, String> {
    let generation = visible(&window)?;
    let started = Instant::now();
    let state = app.state::<Runtime>();
    let (work, context, proof, pairing, observation_revision, resource_generation) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| "Browser state unavailable")?;
        state.browser.documents.observe(&local);
        let work = state
            .browser
            .documents
            .work
            .clone()
            .try_lock_owned()
            .map_err(|_| "A document request is still owned")?;
        let context = admitted(&state, &local, &inner, generation)?;
        let proof = Arc::new(state.setup.management_proof(&local, context.connection)?);
        let pairing = inner
            .attempt
            .as_ref()
            .and_then(|v| v.pairing)
            .ok_or("Pairing unavailable")?;
        let observation_revision = inner
            .attempt
            .as_ref()
            .ok_or("Browser unavailable")?
            .observation_revision;
        if observation_revision == 0 {
            return Err("Await browser status acknowledgement".into());
        }
        let resource_generation = state
            .effects
            .browser_work_generation()
            .map_err(|_| "Browser work ownership is unresolved")?;
        (
            work,
            context,
            proof,
            pairing,
            observation_revision,
            resource_generation,
        )
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let owned = app.clone();
    let checked_proof = proof.clone();
    let (work, grant) = tokio::task::spawn_blocking(move || {
        let work = work;
        if !crate::owner::matches_actor(&directory, context.actor) {
            return Err(ErrorCode::Unauthenticated);
        }
        let grant = ScopeStore::open(&directory.join("browser-scopes.db"))?.get(
            Id::new(context.actor)?,
            reference.id,
            reference.revision,
        )?;
        if !grant.operations.contains(&ScopeOperation::Read)
            || grant.selection != context.selection
            || grant.pairing != pairing
        {
            return Err(ErrorCode::Denied);
        }
        let (selection, binding) = Store::open(&directory.join("browser-pairings"))?
            .selected(Id::new(context.actor)?)?
            .ok_or(ErrorCode::Stale)?;
        if selection.revision != grant.selection
            || selection.pairing != grant.pairing
            || binding.browser_app != grant.browser_app
            || binding.browser_revision != grant.browser_revision
        {
            return Err(ErrorCode::Stale);
        }
        let callback_app = owned.clone();
        let reply = owned.state::<Runtime>().effects.with_app(
            context.actor,
            grant.browser_app.uuid(),
            grant.browser_revision.uuid(),
            Box::new(move || {
                let state = callback_app.state::<Runtime>();
                let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
                let inner = state
                    .browser
                    .inner
                    .lock()
                    .map_err(|_| ErrorCode::Unavailable)?;
                if !state
                    .effects
                    .browser_work_generation()
                    .is_ok_and(|v| v == resource_generation)
                    || !context.matches(&state, &local, &inner)
                    || !inner
                        .attempt
                        .as_ref()
                        .is_some_and(|v| v.observation_revision == observation_revision)
                    || !checked_proof.current_locked(&state, &local)
                    || started.elapsed() >= Duration::from_millis(wire::LIFETIME_MS)
                {
                    return Err(ErrorCode::Stale);
                }
                Ok(())
            }),
        )?;
        reply.recv().map_err(|_| ErrorCode::Unavailable)??;
        Ok::<_, ErrorCode>((work, grant))
    })
    .await
    .map_err(|_| "Document admission stopped")?
    .map_err(|_| "Document scope unavailable or original verification expired")?;
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser state unavailable")?;
    if !state
        .effects
        .browser_work_generation()
        .is_ok_and(|v| v == resource_generation)
        || !context.matches(&state, &local, &inner)
        || !inner
            .attempt
            .as_ref()
            .is_some_and(|v| v.observation_revision == observation_revision)
        || !proof.current_locked(&state, &local)
        || started.elapsed() >= Duration::from_millis(wire::LIFETIME_MS)
    {
        return Err("Document admission expired".into());
    }
    let request = Id::new(Uuid::new_v4()).map_err(|_| "Request identity unavailable")?;
    *state
        .browser
        .documents
        .pending
        .lock()
        .map_err(|_| "Document state unavailable")? = Some(Pending {
        context,
        proof,
        grant,
        started,
        request,
        observation_revision,
        resource_generation,
        mode: wire::Mode::Discover,
        phase: Phase::Waiting,
        _work: work,
    });
    Ok(request)
}
#[tauri::command]
pub fn browser_documents_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Status, String> {
    let generation = visible(&window)?;
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser state unavailable")?;
    let mut slot = state
        .browser
        .documents
        .pending
        .lock()
        .map_err(|_| "Document state unavailable")?;
    if slot
        .as_ref()
        .is_some_and(|p| p.context.settings != generation || !p.current(&state, &local, &inner))
    {
        *slot = None;
    }
    Ok(if let Some(p) = slot.as_ref() {
        let request = p.wire();
        Status {
            request: Some(p.request),
            scope: Some(request.scope),
            remaining_ms: request.remaining_ms,
            phase: p.phase.clone(),
        }
    } else {
        Status {
            request: None,
            scope: None,
            remaining_ms: 0,
            phase: Phase::Expired,
        }
    })
}
#[tauri::command]
pub fn cancel_browser_documents(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    request: Id,
) -> Result<(), String> {
    let generation = visible(&window)?;
    let state = app.state::<Runtime>();
    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser state unavailable")?;
    let mut slot = state
        .browser
        .documents
        .pending
        .lock()
        .map_err(|_| "Document state unavailable")?;
    if inner.settings_generation == generation
        && slot.as_ref().is_some_and(|p| p.request == request)
    {
        *slot = None;
    }
    Ok(())
}
#[tauri::command]
pub fn select_browser_document(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    request: Id,
    index: usize,
) -> Result<Id, String> {
    let generation = visible(&window)?;
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser state unavailable")?;
    let mut slot = state
        .browser
        .documents
        .pending
        .lock()
        .map_err(|_| "Document state unavailable")?;
    let p = slot.as_mut().ok_or("Refresh expired documents")?;
    if p.request != request
        || p.context.settings != generation
        || !p.current(&state, &local, &inner)
    {
        return Err("Document roster expired; refresh explicitly".into());
    }
    let Phase::Available { candidates } = &p.phase else {
        return Err("Document roster unavailable".into());
    };
    let candidate = candidates
        .get(index)
        .ok_or("Document choice unavailable")?
        .clone();
    p.request = Id::new(Uuid::new_v4()).map_err(|_| "Request identity unavailable")?;
    p.started = Instant::now();
    p.mode = wire::Mode::Revalidate { candidate };
    p.phase = Phase::Waiting;
    Ok(p.request)
}
pub(super) fn reply(
    app: &tauri::AppHandle,
    attempt: Uuid,
    generation: u64,
    session: Id,
    reply: wire::Reply,
) -> Result<(), ErrorCode> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    let mut slot = state
        .browser
        .documents
        .pending
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    let p = slot.as_mut().ok_or(ErrorCode::Stale)?;
    if !p.current(&state, &local, &inner)
        || p.context.attempt != attempt
        || p.context.transport != generation
        || p.context.session != session
        || !matches!(p.phase, Phase::Waiting)
    {
        return Err(ErrorCode::Stale);
    }
    if reply.observation_revision != p.observation_revision {
        return Err(ErrorCode::Stale);
    }
    reply.validate(&p.wire())?;
    p.phase = match reply.outcome {
        wire::Outcome::Available { candidates } => match &p.mode {
            wire::Mode::Discover => Phase::Available { candidates },
            wire::Mode::Revalidate { candidate } => Phase::Selected {
                identity: Id::new(Uuid::new_v4())?,
                revision: Id::new(Uuid::new_v4())?,
                candidate: candidate.clone(),
            },
        },
        wire::Outcome::Unavailable => Phase::Unavailable,
        wire::Outcome::Expired => Phase::Expired,
        wire::Outcome::TooMany => Phase::TooMany,
    };
    Ok(())
}
