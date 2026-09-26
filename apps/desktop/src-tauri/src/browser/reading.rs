//! Private accepted preparation and status/content publication coordinator.
use super::*;
use crate::connection::{self, SessionIdentity};
use avesra_contracts::actors;
use avesra_windows::browser_read_channel::{NativeEndpoint, Offer, Withdrawal};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Coordinator {
    slot: Arc<Mutex<Slot>>,
}
#[derive(Default)]
struct Slot {
    generation: u64,
    active: Option<Active>,
    successor: Option<(Id, Uuid, Uuid, Withdrawal)>,
}
struct Active {
    id: Uuid,
    actor: Uuid,
    registration: Option<Uuid>,
    signal: Withdrawal,
    admission: std::sync::Weak<Admission>,
    endpoint: Option<NativeEndpoint>,
}
impl Coordinator {
    pub(super) fn invalidate(&self) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some((_, _, _, signal)) = slot.successor.take() {
                signal.cancel();
            }
            slot.generation = slot.generation.saturating_add(1);
            if let Some(active) = slot.active.take() {
                active.signal.cancel();
            }
        }
    }
    /// False means the current native read has already established a different
    /// registration for this actor. Its target metadata must not be retired as
    /// an indirect bypass of this exact-revision withdrawal decision.
    pub(super) fn revoke(&self, actor: Uuid, revision: Uuid) -> bool {
        let Ok(mut slot) = self.slot.lock() else {
            return true;
        };
        if slot
            .successor
            .as_ref()
            .is_some_and(|(_, a, r, _)| *a == actor && *r == revision)
            && let Some((_, _, _, signal)) = slot.successor.take()
        {
            signal.cancel();
        }
        if slot
            .active
            .as_ref()
            .is_some_and(|a| a.actor == actor && a.registration.is_some_and(|v| v != revision))
        {
            return false;
        }
        if slot
            .active
            .as_ref()
            .is_some_and(|a| a.actor == actor && a.registration.is_none_or(|v| v == revision))
        {
            slot.generation = slot.generation.saturating_add(1);
            if let Some(active) = slot.active.take() {
                active.signal.cancel();
            }
        }
        true
    }
    fn claim(&self, actor: Uuid, signal: Withdrawal) -> Result<Owner, ErrorCode> {
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if let Some((_, _, _, signal)) = slot.successor.take() {
            signal.cancel();
        }
        if slot.active.is_some() || slot.generation == u64::MAX {
            return Err(ErrorCode::Unavailable);
        }
        let id = Uuid::new_v4();
        slot.active = Some(Active {
            id,
            actor,
            registration: None,
            signal,
            admission: std::sync::Weak::new(),
            endpoint: None,
        });
        Ok(Owner {
            slot: self.slot.clone(),
            id,
            generation: slot.generation,
        })
    }
    fn install(
        &self,
        admission: &Arc<Admission>,
        endpoint: NativeEndpoint,
    ) -> Result<(), ErrorCode> {
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if slot.generation != admission.owner.generation {
            return Err(ErrorCode::Stale);
        }
        let active = slot
            .active
            .as_mut()
            .filter(|v| v.id == admission.owner.id)
            .ok_or(ErrorCode::Stale)?;
        active.signal.remaining_ms()?;
        if active.endpoint.is_some() {
            return Err(ErrorCode::InvalidTransition);
        }
        active.admission = Arc::downgrade(admission);
        endpoint.prepared();
        active.endpoint = Some(endpoint);
        Ok(())
    }
    /// Caller owns Runtime.local -> browser.inner; no disk/network operations.
    pub(super) fn request(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
    ) -> Result<Option<browser::reading::Request>, ErrorCode> {
        let admission = self
            .slot
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?
            .active
            .as_ref()
            .and_then(|v| v.admission.upgrade());
        let Some(admission) = admission else {
            return Ok(None);
        };
        if admission.current_locked(state, local, inner).is_err() {
            self.invalidate();
            return Ok(None);
        }
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        let Some(active) = slot.active.as_mut().filter(|v| v.id == admission.owner.id) else {
            return Ok(None);
        };
        active
            .endpoint
            .as_mut()
            .map(NativeEndpoint::request)
            .transpose()
            .map(Option::flatten)
    }
    pub(super) fn content(
        &self,
        content: avesra_windows::browser_receive::Content,
    ) -> Result<(), ErrorCode> {
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if let Some(endpoint) = slot.active.as_mut().and_then(|v| v.endpoint.as_mut()) {
            endpoint.content(content)?;
        }
        Ok(())
    }
    pub(super) fn retire_mailbox_successor(&self, request: Id) {
        if let Ok(mut slot) = self.slot.lock()
            && slot
                .successor
                .as_ref()
                .is_some_and(|(id, _, _, _)| *id == request)
            && let Some((_, _, _, signal)) = slot.successor.take()
        {
            signal.cancel();
        }
    }
    pub(super) fn mailbox_successor(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
        context: &browser::reading::Context,
    ) -> Result<avesra_windows::browser_read_channel::ReadSuccessor, ErrorCode> {
        let admission = self
            .slot
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?
            .active
            .as_ref()
            .and_then(|v| v.admission.upgrade())
            .ok_or(ErrorCode::Stale)?;
        admission.current_locked(state, local, inner)?;
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if slot.successor.is_some() {
            return Err(ErrorCode::Unavailable);
        }
        let active = slot
            .active
            .as_ref()
            .filter(|v| v.id == admission.owner.id && v.actor == context.actor.uuid())
            .ok_or(ErrorCode::Stale)?;
        let registration = active.registration.ok_or(ErrorCode::Stale)?;
        let successor = active
            .endpoint
            .as_ref()
            .ok_or(ErrorCode::Stale)?
            .mailbox_successor(context)?;
        slot.successor = Some((
            context.request,
            active.actor,
            registration,
            active.signal.clone(),
        ));
        Ok(successor)
    }
    pub(super) fn mailbox_ack(&self) -> Result<Option<browser::mailbox::Ack>, ErrorCode> {
        let slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        slot.active
            .as_ref()
            .and_then(|a| a.endpoint.as_ref())
            .map(NativeEndpoint::mailbox_ack)
            .transpose()
            .map(Option::flatten)
    }
    pub(super) fn settled(
        &self,
        proof: &avesra_windows::browser_receive::Settlement,
    ) -> Result<(), ErrorCode> {
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if let Some(endpoint) = slot.active.as_mut().and_then(|v| v.endpoint.as_mut()) {
            endpoint.settled(proof)?;
        }
        Ok(())
    }
}
struct Owner {
    slot: Arc<Mutex<Slot>>,
    id: Uuid,
    generation: u64,
}
impl Owner {
    fn current(&self) -> Result<(), ErrorCode> {
        let slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if slot.generation != self.generation
            || !slot
                .active
                .as_ref()
                .is_some_and(|v| v.id == self.id && v.signal.remaining_ms().is_ok())
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    fn bind(&self, binding: &actors::Binding) -> Result<(), ErrorCode> {
        let mut slot = self.slot.lock().map_err(|_| ErrorCode::Unavailable)?;
        if slot.generation != self.generation {
            return Err(ErrorCode::Stale);
        }
        let active = slot
            .active
            .as_mut()
            .filter(|v| v.id == self.id && v.actor == binding.actor)
            .ok_or(ErrorCode::Stale)?;
        active.signal.remaining_ms()?;
        if active.registration.is_some() || binding.revoked {
            return Err(ErrorCode::Stale);
        }
        active.registration = Some(binding.registration_revision);
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.lock()
            && slot.active.as_ref().is_some_and(|v| v.id == self.id)
            && let Some(active) = slot.active.take()
            && !active
                .endpoint
                .as_ref()
                .is_some_and(NativeEndpoint::successor_transferred)
        {
            active.signal.cancel();
        }
    }
}
struct Admission {
    owner: Owner,
    signal: Withdrawal,
    target: documents::PreparedTarget,
    session: SessionIdentity,
    actor: Uuid,
}
impl Admission {
    fn current(&self, app: &tauri::AppHandle) -> Result<(), ErrorCode> {
        self.signal.remaining_ms()?;
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
        let inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        self.current_locked(&state, &local, &inner)
    }
    fn current_locked(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        inner: &Inner,
    ) -> Result<(), ErrorCode> {
        self.signal.remaining_ms()?;
        if !inner.action_allowed
            || inner.action_epoch != self.session.action_epoch
            || !local.enrolled
            || local.action_epoch != self.session.action_epoch
            || state.connection_generation.load(Ordering::SeqCst) != self.session.generation
            || !state
                .acknowledged_session
                .lock()
                .map_err(|_| ErrorCode::Unavailable)?
                .is_some_and(|v| {
                    v.id == self.session.id
                        && v.device == self.session.device
                        && v.server_fingerprint == self.session.server_fingerprint
                        && v.generation == self.session.generation
                        && v.action_epoch == self.session.action_epoch
                })
        {
            return Err(ErrorCode::Stale);
        }
        self.target.current(state, local, inner, &self.signal)?;
        self.owner.current()
    }
}
fn admit(app: &tauri::AppHandle, offer: &Offer) -> Result<Arc<Admission>, ErrorCode> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    let session = state
        .acknowledged_session
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?
        .ok_or(ErrorCode::Stale)?;
    let permit = offer.permit();
    if !inner.action_allowed
        || inner.action_epoch != session.action_epoch
        || !local.enrolled
        || !local.connected
        || local.locked
        || local.settings.paused
        || session.device != permit.device_id
        || session.id != permit.session_id
        || session.action_epoch != permit.action_epoch
        || local.action_epoch != permit.action_epoch
        || session.generation != state.connection_generation.load(Ordering::SeqCst)
    {
        return Err(ErrorCode::Stale);
    }
    let attempt = inner.attempt.as_ref().ok_or(ErrorCode::Stale)?;
    let remaining = offer.remaining_ms()?.saturating_add(1);
    let horizon = Duration::from_secs(300)
        .checked_sub(attempt.started.elapsed())
        .ok_or(ErrorCode::Expired)?;
    if horizon <= Duration::from_millis(remaining) {
        return Err(ErrorCode::Expired);
    }
    let signal = offer.withdrawal();
    let target = documents::reserve_target(&state, &local, &inner, offer)?;
    let owner = state
        .browser
        .reading
        .claim(permit.action.actor_id, signal.clone())?;
    Ok(Arc::new(Admission {
        owner,
        signal,
        target,
        session,
        actor: permit.action.actor_id,
    }))
}

/// Private synchronous inspector. Constructed in an actual spawn_blocking job;
/// after handoff its checker is called only by the dedicated native worker.
/// Never exposed as an async/pipe callback or run with Runtime.local held.
struct Inspector {
    app: tauri::AppHandle,
    admission: Arc<Admission>,
    directory: PathBuf,
    runtime: tokio::runtime::Handle,
}
// Losing the runtime waiter withdraws immediately without dropping the Offer
// retained inside actual blocking preparation. Successful handoff disarms it.
struct Waiter {
    signal: Withdrawal,
    completed: bool,
}
impl Drop for Waiter {
    fn drop(&mut self) {
        if !self.completed {
            self.signal.cancel();
        }
    }
}
impl Inspector {
    fn status(&self, pairing: &connection::PairingRecord) -> Result<actors::Binding, ErrorCode> {
        self.admission.current(&self.app)?;
        let request = actors::Request {
            version: actors::VERSION,
            request: Uuid::new_v4(),
            attempt: Uuid::new_v4(),
            session: self.admission.session.id,
            action_epoch: self.admission.session.action_epoch,
            command: actors::Command::Status,
        };
        let sampled = Instant::now();
        let remaining = self.admission.signal.remaining_ms()?;
        let deadline = sampled + Duration::from_millis(remaining);
        let result = self
            .runtime
            .block_on(async {
                tokio::time::timeout_at(
                    deadline.into(),
                    connection::actor_operation(pairing, &request),
                )
                .await
            })
            .map_err(|_| ErrorCode::Expired)?
            .map_err(|_| ErrorCode::Unavailable)?;
        self.admission.current(&self.app)?;
        let binding = result.binding.ok_or(ErrorCode::Unauthenticated)?;
        binding.validate()?;
        if binding.revoked
            || binding.actor != self.admission.actor
            || binding.device != self.admission.session.device
        {
            return Err(ErrorCode::Stale);
        }
        Ok(binding)
    }
    fn initial(&self) -> Result<actors::Binding, ErrorCode> {
        self.admission.current(&self.app)?;
        let pairing = connection::load(&self.directory).map_err(|_| ErrorCode::Unavailable)?;
        if pairing.device_id != self.admission.session.device
            || pairing
                .server_fingerprint()
                .map_err(|_| ErrorCode::Malformed)?
                != hex::encode(self.admission.session.server_fingerprint)
        {
            return Err(ErrorCode::Stale);
        }
        self.admission.current(&self.app)?;
        let binding = self.status(&pairing)?;
        crate::planner::paired_owner(&self.directory, &binding, self.admission.session)
            .map_err(|_| ErrorCode::Stale)?;
        self.admission.current(&self.app)?;
        self.admission
            .target
            .saved(&self.directory, &mut || self.admission.current(&self.app))?;
        self.admission.current(&self.app)?;
        self.admission.owner.bind(&binding)?;
        Ok(binding)
    }
    fn current(&self, expected: &actors::Binding) -> Result<(), ErrorCode> {
        self.admission.current(&self.app)?;
        let pairing =
            crate::planner::paired_owner(&self.directory, expected, self.admission.session)
                .map_err(|_| ErrorCode::Stale)?;
        self.admission.current(&self.app)?;
        self.admission
            .target
            .saved(&self.directory, &mut || self.admission.current(&self.app))?;
        self.admission.current(&self.app)?;
        if self.status(&pairing)? != *expected {
            return Err(ErrorCode::Stale);
        }
        self.admission.current(&self.app)
    }
}
pub(super) fn start(app: tauri::AppHandle) -> Result<(), ErrorCode> {
    let mut receive = app.state::<Runtime>().effects.take_browser_preparation()?;
    tauri::async_runtime::spawn(async move {
        loop {
            let offer = match receive.receive().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let Ok(admission) = admit(&app, &offer) else {
                continue;
            };
            let Ok(directory) = app.path().app_data_dir() else {
                continue;
            };
            let owned_app = app.clone();
            let runtime = tokio::runtime::Handle::current();
            let mut waiter = Waiter {
                signal: offer.withdrawal(),
                completed: false,
            };
            // Offer itself lives in the actual blocking job. Cancellation does
            // not detach/drop it before disk/network inspection has returned.
            let result = tokio::task::spawn_blocking(move || {
                let inspector = Inspector {
                    app: owned_app,
                    admission,
                    directory,
                    runtime,
                };
                let binding = inspector.initial()?;
                let request = inspector
                    .admission
                    .target
                    .request(offer.permit(), offer.remaining_ms()?)?;
                let expected = binding.clone();
                let admission = inspector.admission.clone();
                let owned_app = inspector.app.clone();
                let endpoint = offer.complete(
                    request,
                    binding,
                    Box::new(move || inspector.current(&expected)),
                )?;
                admission.current(&owned_app)?;
                owned_app
                    .state::<Runtime>()
                    .browser
                    .reading
                    .install(&admission, endpoint)
            })
            .await;
            waiter.completed = matches!(result, Ok(Ok(())));
        }
    });
    Ok(())
}
