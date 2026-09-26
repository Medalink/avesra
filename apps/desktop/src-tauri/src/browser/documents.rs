//! Protected setup metadata only. No semantic read or accepted action ingress.
use super::*;
use avesra_contracts::browser::{ScopeOperation, ScopeRef, documents as wire};
use avesra_core::browser_scopes::{Grant, Store as ScopeStore};
use avesra_windows::browser_read_channel::{Offer, Withdrawal};
use scopes::Context;
pub(super) const MAX_TARGETS: usize = 16;

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
    target_generation: u64,
    mode: wire::Mode,
    phase: Phase,
    _work: tokio::sync::OwnedMutexGuard<()>,
}
// Immutable native configuration metadata. Only this module's actual reply
// path constructs it; no Deserialize, Settings-fields constructor or persistence.
#[derive(Clone)]
pub(super) struct Target {
    reference: ScopeRef,
    actor: Uuid,
    grant: Grant,
    session: Id,
    transport: u64,
    connection: u64,
    observation_revision: u64,
    resource_generation: u64,
    target_generation: u64,
    candidate: Option<wire::Candidate>,
    retired: bool,
}
impl Target {
    fn selected(pending: &Pending, candidate: wire::Candidate) -> Result<Self, ErrorCode> {
        pending.grant.validate()?;
        candidate.validate(&pending.grant.origin)?;
        if !pending.grant.operations.contains(&ScopeOperation::Read)
            || pending.grant.actor.uuid() != pending.context.actor
            || pending.grant.selection != pending.context.selection
        {
            return Err(ErrorCode::Stale);
        }
        Ok(Self {
            reference: ScopeRef {
                id: Id::new(Uuid::new_v4())?,
                revision: Id::new(Uuid::new_v4())?,
            },
            actor: pending.context.actor,
            grant: pending.grant.clone(),
            session: pending.context.session,
            transport: pending.context.transport,
            connection: pending.context.connection,
            observation_revision: pending.observation_revision,
            resource_generation: pending.resource_generation,
            target_generation: pending.target_generation,
            candidate: Some(candidate),
            retired: false,
        })
    }
    pub(super) fn retire(&mut self) {
        self.retired = true;
    }
    pub(super) fn retire_actor(&mut self, actor: Uuid) {
        if self.actor == actor {
            self.retire();
        }
    }
}
/// Bounded metadata resolver only. Accepted use must separately establish
/// current durable grant/registration/native owner and exact Chrome identity.
/// Caller holds Runtime.local -> browser.inner; no I/O or management proof here.
pub(super) fn resolve_target(
    state: &Runtime,
    local: &avesra_core::state::LocalState,
    inner: &Inner,
    reference: ScopeRef,
    actor: Uuid,
) -> Result<Target, ErrorCode> {
    resolve_target_resource(state, local, inner, reference, actor, None)
}
fn resolve_target_resource(
    state: &Runtime,
    local: &avesra_core::state::LocalState,
    inner: &Inner,
    reference: ScopeRef,
    actor: Uuid,
    reservation: Option<(&Withdrawal, u64)>,
) -> Result<Target, ErrorCode> {
    let attempt = inner.attempt.as_ref().ok_or(ErrorCode::Stale)?;
    let target = attempt
        .targets
        .iter()
        .find(|v| v.reference == reference)
        .ok_or(ErrorCode::Stale)?;
    validate_target(state, local, inner, target, actor, reservation)
}
fn validate_target(
    state: &Runtime,
    local: &avesra_core::state::LocalState,
    inner: &Inner,
    target: &Target,
    actor: Uuid,
    reservation: Option<(&Withdrawal, u64)>,
) -> Result<Target, ErrorCode> {
    let attempt = inner.attempt.as_ref().ok_or(ErrorCode::Stale)?;
    if target.retired
        || target.actor != actor
        || !local.connected
        || local.locked
        || local.settings.paused
        || !attempt.current_time()
        || attempt.state != "authenticated_no_scopes"
        || attempt.actor != Some(actor)
        || attempt.session != Some(target.session)
        || inner.generation != target.transport
        || attempt.connection != target.connection
        || state.connection_generation.load(Ordering::SeqCst) != target.connection
        || attempt.observation_revision != target.observation_revision
        || attempt.target_generation == u64::MAX
        || attempt.target_generation != target.target_generation
        || attempt.selection != Some(target.grant.selection)
        || attempt.pairing != Some(target.grant.pairing)
        || !attempt.selected.as_ref().is_some_and(|app| {
            app.id == target.grant.browser_app.uuid()
                && app.revision == target.grant.browser_revision.uuid()
                && app.selected_by == actor
        })
        || !match reservation {
            Some((signal, generation)) => {
                signal.remaining_ms().is_ok()
                    && signal
                        .reservation_generation()
                        .is_ok_and(|v| v == generation)
                    && target.resource_generation.checked_add(1) == Some(generation)
            }
            None => state
                .effects
                .browser_work_generation()
                .is_ok_and(|v| v == target.resource_generation),
        }
    {
        return Err(ErrorCode::Stale);
    }
    if let Some(candidate) = &target.candidate {
        candidate.validate(&target.grant.origin)?;
    }
    Ok(target.clone())
}
/// Exact private metadata plus this offer's own resource transition. Not a grant.
pub(super) struct PreparedTarget {
    target: Target,
    generation: u64,
}
pub(super) fn reserve_target(
    state: &Runtime,
    local: &avesra_core::state::LocalState,
    inner: &Inner,
    offer: &Offer,
) -> Result<PreparedTarget, ErrorCode> {
    let action = &offer.permit().action;
    if matches!(
        action.payload,
        avesra_contracts::ActionPayload::OpenX { .. }
    ) {
        let scope = offer.scope();
        scope.validate()?;
        if scope.origin.as_str() != avesra_contracts::browser::provider::Provider::X.origin()
            || scope.operations != [ScopeOperation::Read, ScopeOperation::Navigate]
            || scope.id.uuid() != action.target_id
            || scope.actor.uuid() != action.actor_id
        {
            return Err(ErrorCode::Denied);
        }
        let attempt = inner.attempt.as_ref().ok_or(ErrorCode::Stale)?;
        let target = Target {
            reference: ScopeRef {
                id: Id::new(Uuid::new_v4())?,
                revision: Id::new(Uuid::new_v4())?,
            },
            actor: action.actor_id,
            grant: scope.clone(),
            session: attempt.session.ok_or(ErrorCode::Stale)?,
            transport: inner.generation,
            connection: attempt.connection,
            observation_revision: attempt.observation_revision,
            resource_generation: state.effects.browser_work_generation()?,
            target_generation: attempt.target_generation,
            candidate: None,
            retired: false,
        };
        validate_target(state, local, inner, &target, action.actor_id, None)?;
        let generation = offer.reserve(target.resource_generation)?;
        state.browser.documents.invalidate();
        return Ok(PreparedTarget { target, generation });
    }
    let mut matches = inner
        .attempt
        .as_ref()
        .ok_or(ErrorCode::Stale)?
        .targets
        .iter()
        .filter(|v| !v.retired && v.grant.id.uuid() == action.target_id);
    let reference = matches.next().ok_or(ErrorCode::Stale)?.reference;
    if matches.next().is_some() {
        return Err(ErrorCode::Denied);
    }
    let target = resolve_target(state, local, inner, reference, action.actor_id)?;
    let origin = match &action.payload {
        avesra_contracts::ActionPayload::ReadPage { origin, .. } => origin.as_str(),
        avesra_contracts::ActionPayload::ReadInbox { .. } => "https://mail.google.com",
        avesra_contracts::ActionPayload::InspectBrowserProvider { provider } => provider.origin(),
        avesra_contracts::ActionPayload::OpenX { .. } => {
            avesra_contracts::browser::provider::Provider::X.origin()
        }
        _ => return Err(ErrorCode::Denied),
    };
    if avesra_contracts::browser::Origin::parse(origin)? != target.grant.origin {
        return Err(ErrorCode::Stale);
    }
    if matches!(
        action.payload,
        avesra_contracts::ActionPayload::OpenX { .. }
            | avesra_contracts::ActionPayload::ReadInbox { .. }
    ) && !target.grant.operations.contains(&ScopeOperation::Navigate)
    {
        return Err(ErrorCode::Denied);
    }
    let generation = offer.reserve(target.resource_generation)?;
    state.browser.documents.invalidate();
    Ok(PreparedTarget { target, generation })
}
impl PreparedTarget {
    pub(super) fn current(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
        signal: &Withdrawal,
    ) -> Result<(), ErrorCode> {
        if self.target.candidate.is_none() {
            return validate_target(
                state,
                local,
                inner,
                &self.target,
                self.target.actor,
                Some((signal, self.generation)),
            )
            .map(|_| ());
        }
        resolve_target_resource(
            state,
            local,
            inner,
            self.target.reference,
            self.target.actor,
            Some((signal, self.generation)),
        )
        .map(|_| ())
    }
    pub(super) fn saved(
        &self,
        directory: &std::path::Path,
        current: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        current()?;
        let expected = &self.target.grant;
        let grant = ScopeStore::open_read_only(&directory.join("browser-scopes.db"))?.get(
            expected.actor,
            expected.id,
            expected.revision,
        )?;
        if serde_json::to_vec(&grant).map_err(|_| ErrorCode::Malformed)?
            != serde_json::to_vec(expected).map_err(|_| ErrorCode::Malformed)?
        {
            return Err(ErrorCode::Stale);
        }
        current()?;
        let store = Store::open_existing(&directory.join("browser-pairings"))?;
        current()?;
        let (selected, binding) = store.selected(expected.actor)?.ok_or(ErrorCode::Stale)?;
        if selected.revision != expected.selection
            || selected.actor != expected.actor
            || selected.pairing != expected.pairing
            || binding.actor != expected.actor
            || binding.pairing != expected.pairing
            || binding.browser_app != expected.browser_app
            || binding.browser_revision != expected.browser_revision
        {
            return Err(ErrorCode::Stale);
        }
        current()
    }
    pub(super) fn request(
        &self,
        permit: &avesra_core::ledger::DispatchPermit,
        remaining_ms: u64,
    ) -> Result<avesra_contracts::browser::reading::Request, ErrorCode> {
        use avesra_contracts::browser::reading::{Context, Mode, Request, Source};
        let (message_limit, mode) = match &permit.action.payload {
            avesra_contracts::ActionPayload::ReadPage { message_limit, .. } => {
                (*message_limit, Mode::Excerpt)
            }
            avesra_contracts::ActionPayload::InspectBrowserProvider { provider } => (
                1,
                Mode::ProviderInspection {
                    provider: *provider,
                },
            ),
            avesra_contracts::ActionPayload::ReadInbox { account, count } => (
                *count,
                Mode::Inbox {
                    account: account.clone(),
                },
            ),
            avesra_contracts::ActionPayload::OpenX { account } => (
                1,
                Mode::XReady {
                    account: account.clone(),
                },
            ),
            _ => return Err(ErrorCode::Denied),
        };
        let action = &permit.action;
        let target = &self.target;
        let grant = &target.grant;
        let request = Request {
            context: Context {
                request: Id::new(Uuid::new_v4())?,
                dispatch: Id::new(permit.dispatch_id)?,
                task: Id::new(action.task_id)?,
                step: Id::new(action.step_id)?,
                actor: Id::new(action.actor_id)?,
                action_revision: Id::new(action.revision)?,
                intent_revision: Id::new(action.intent_revision)?,
                grant: Id::new(action.grant_id)?,
                target: ScopeRef {
                    id: grant.id,
                    revision: grant.revision,
                },
                scope: ScopeRef {
                    id: grant.id,
                    revision: grant.revision,
                },
                pairing: grant.pairing,
                selection: grant.selection,
                browser_app: ScopeRef {
                    id: grant.browser_app,
                    revision: grant.browser_revision,
                },
                browser_session: target.session,
                browser_generation: target.transport,
                observation_revision: target.observation_revision,
                source: Source {
                    device: Id::new(permit.device_id)?,
                    session: Id::new(permit.session_id)?,
                    capture_epoch: permit.capture_epoch,
                    action_epoch: permit.action_epoch,
                },
            },
            origin: grant.origin.clone(),
            document: target.candidate.clone(),
            message_limit,
            mode,
            remaining_ms,
        };
        avesra_core::browser_reading::validate_dispatch(&request, permit)?;
        Ok(request)
    }
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
            && inner.attempt.as_ref().is_some_and(|v| {
                v.observation_revision == self.observation_revision
                    && v.target_generation != u64::MAX
                    && v.target_generation == self.target_generation
            })
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
                || attempt.target_generation == u64::MAX
                || p.target_generation != attempt.target_generation
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
    if attempt.target_generation == u64::MAX || !context.matches(state, local, inner) {
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
    let (
        work,
        context,
        proof,
        pairing,
        observation_revision,
        resource_generation,
        target_generation,
    ) = {
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
            inner
                .attempt
                .as_ref()
                .ok_or("Browser unavailable")?
                .target_generation,
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
                    || !inner.attempt.as_ref().is_some_and(|v| {
                        v.target_generation != u64::MAX && v.target_generation == target_generation
                    })
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
        || !inner.attempt.as_ref().is_some_and(|v| {
            v.target_generation != u64::MAX && v.target_generation == target_generation
        })
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
        target_generation,
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
    if let Some(p) = slot.as_mut()
        && let Phase::Selected {
            identity, revision, ..
        } = &p.phase
        && resolve_target(
            &state,
            &local,
            &inner,
            ScopeRef {
                id: *identity,
                revision: *revision,
            },
            p.context.actor,
        )
        .is_err()
    {
        p.phase = Phase::Unavailable;
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
    let mut inner = state
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
            wire::Mode::Revalidate { candidate } => {
                if inner
                    .attempt
                    .as_ref()
                    .ok_or(ErrorCode::Stale)?
                    .targets
                    .len()
                    >= MAX_TARGETS
                {
                    Phase::Unavailable
                } else {
                    let target = Target::selected(p, candidate.clone())?;
                    if !p.current(&state, &local, &inner) {
                        return Err(ErrorCode::Stale);
                    }
                    let attempt = inner.attempt.as_mut().ok_or(ErrorCode::Stale)?;
                    if attempt.targets.iter().any(|v| {
                        v.reference.id == target.reference.id
                            || v.reference.revision == target.reference.revision
                    }) {
                        return Err(ErrorCode::Malformed);
                    }
                    let phase = Phase::Selected {
                        identity: target.reference.id,
                        revision: target.reference.revision,
                        candidate: target.candidate.clone().ok_or(ErrorCode::Malformed)?,
                    };
                    // An explicit new selection replaces only inactive metadata
                    // for this scope; actual owner admission already excludes a
                    // pending/uncertain job before this path can be reached.
                    for previous in &mut attempt.targets {
                        if previous.grant.id == target.grant.id {
                            previous.retire();
                        }
                    }
                    attempt.targets.push(target);
                    phase
                }
            }
        },
        wire::Outcome::Unavailable => Phase::Unavailable,
        wire::Outcome::Expired => Phase::Expired,
        wire::Outcome::TooMany => Phase::TooMany,
    };
    Ok(())
}
