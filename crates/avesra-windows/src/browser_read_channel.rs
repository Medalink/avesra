//! Actual-worker preparation, publication and authenticated content handoff.
//! Metadata from the native consumer still needs ReadExecution.authorize.
use avesra_contracts::{ErrorCode, actors::Binding, browser::reading::Request};
use avesra_core::{
    browser_execution::{Preparation, PublicationPermit, PublishedRequest, ReadExecution},
    ledger::DispatchPermit,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

pub type Current = Box<dyn FnMut() -> Result<(), ErrorCode> + Send>;

/// No public constructor or sender access. NativeEffects creates this for its
/// one actual worker and moves it into that thread for the entire lifetime.
pub struct WorkerOwner {
    send: tokio::sync::mpsc::Sender<Offer>,
    resource: Arc<AtomicU64>,
}
pub struct PreparationReceiver {
    receive: tokio::sync::mpsc::Receiver<Offer>,
}
pub(crate) fn channel(resource: Arc<AtomicU64>) -> (WorkerOwner, PreparationReceiver) {
    let (send, receive) = tokio::sync::mpsc::channel(1);
    (
        WorkerOwner { send, resource },
        PreparationReceiver { receive },
    )
}
struct Shared {
    preparation: Preparation,
    resource: Arc<AtomicU64>,
    // 0: unreserved; MAX: failed/in-progress; otherwise exact blocked word.
    reservation: AtomicU64,
    finished: AtomicBool,
    native_lost: AtomicBool,
    successor_transferred: AtomicBool,
    publication: Mutex<Publication>,
}
#[derive(Default)]
struct Publication {
    context: Option<avesra_contracts::browser::reading::Context>,
    held: Option<PublicationPermit>,
    published: Option<PublishedRequest>,
    terminal: bool,
    attempted: bool,
    successor_registered: bool,
    mailbox_ack: Option<avesra_contracts::browser::mailbox::Ack>,
}
impl Shared {
    fn current(&self) -> Result<u64, ErrorCode> {
        if self.resource.load(Ordering::SeqCst) == u64::MAX {
            return Err(ErrorCode::Unavailable);
        }
        self.preparation.remaining_ms()
    }
    fn reserved(&self) -> Result<u64, ErrorCode> {
        let word = self.reservation.load(Ordering::SeqCst);
        if word == 0 || word == u64::MAX || self.resource.load(Ordering::SeqCst) != word {
            return Err(ErrorCode::Stale);
        }
        Ok(word >> 1)
    }
}
/// One native preparation offer. Dropping withdraws, never releases resources.
pub struct Offer {
    scope: avesra_core::browser_scopes::Grant,
    shared: Arc<Shared>,
    reply: Option<SyncSender<Prepared>>,
    completed: bool,
    content: Option<SyncSender<crate::browser_receive::Content>>,
}
/// Paired endpoint from the original Offer. No raw reply or context constructor.
pub struct NativeEndpoint {
    shared: Arc<Shared>,
    content: Option<SyncSender<crate::browser_receive::Content>>,
}
fn slot_context_invalid(
    shared: &Shared,
    context: &avesra_contracts::browser::reading::Context,
) -> Result<bool, ErrorCode> {
    shared.current()?;
    let slot = shared
        .publication
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    Ok(slot.terminal || slot.context.as_ref() != Some(context))
}
/// Actual native endpoint registration, held until the successor output retires.
/// It keeps cancellation and the original deadline, but grants no new Chrome job.
pub struct ReadSuccessor {
    shared: Arc<Shared>,
}
impl ReadSuccessor {
    pub fn current(&self) -> bool {
        self.shared.current().is_ok() && !self.shared.native_lost.load(Ordering::SeqCst)
    }
}
impl Drop for ReadSuccessor {
    fn drop(&mut self) {
        self.shared.preparation.withdraw();
    }
}
impl NativeEndpoint {
    pub fn successor_transferred(&self) -> bool {
        self.shared.successor_transferred.load(Ordering::SeqCst)
    }
    pub fn mailbox_successor(
        &self,
        context: &avesra_contracts::browser::reading::Context,
    ) -> Result<ReadSuccessor, ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()?;
        let mut slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if !slot.terminal || slot.context.as_ref() != Some(context) || slot.successor_registered {
            return Err(ErrorCode::Stale);
        }
        slot.successor_registered = true;
        Ok(ReadSuccessor {
            shared: self.shared.clone(),
        })
    }

    pub fn mailbox_ack(
        &self,
    ) -> Result<Option<avesra_contracts::browser::mailbox::Ack>, ErrorCode> {
        self.shared.current()?;
        let slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(if slot.terminal {
            None
        } else {
            slot.mailbox_ack.clone()
        })
    }
    /// Last operation of the actual native preparation job, after installation.
    pub fn prepared(&self) {
        self.shared.finished.store(true, Ordering::SeqCst);
    }
    /// Call under the exact native current-context lock immediately before status.
    pub fn request(&mut self) -> Result<Option<Request>, ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()?;
        let mut slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if slot.terminal {
            return Ok(None);
        }
        if let Some(permit) = slot.held.take() {
            slot.published = Some(permit.publish()?);
        }
        slot.published
            .as_ref()
            .map(PublishedRequest::request)
            .transpose()
    }
    /// Actual authenticated proof stops advertising before retirement/ack.
    /// Already-sent content remains eligible under its original authority.
    pub fn settled(&mut self, proof: &crate::browser_receive::Settlement) -> Result<(), ErrorCode> {
        let mut slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if slot.context.as_ref() == Some(proof.context()) {
            slot.terminal = true;
            slot.held.take();
            slot.published.take();
        }
        Ok(())
    }
    pub fn content(&mut self, value: crate::browser_receive::Content) -> Result<(), ErrorCode> {
        if self.shared.current().is_err() {
            return Ok(());
        }
        let slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if slot.context.as_ref() != Some(value.context()) {
            return Ok(());
        }
        drop(slot);
        if value.is_chunk() {
            if slot_context_invalid(&self.shared, value.context())? {
                return Err(ErrorCode::Stale);
            }
            self.content
                .as_ref()
                .ok_or(ErrorCode::Stale)?
                .try_send(value)
                .map_err(|_| ErrorCode::Unavailable)?;
        } else if let Some(send) = self.content.take() {
            send.try_send(value).map_err(|_| ErrorCode::Unavailable)?;
        }
        Ok(())
    }
}
impl Drop for NativeEndpoint {
    fn drop(&mut self) {
        self.shared.finished.store(true, Ordering::SeqCst);
        if !self.shared.successor_transferred.load(Ordering::SeqCst) {
            self.shared.native_lost.store(true, Ordering::SeqCst);
            self.shared.preparation.withdraw();
        }
        if let Ok(mut slot) = self.shared.publication.lock() {
            slot.terminal = true;
            slot.held.take();
            slot.published.take();
        }
    }
}
/// Cloneable withdrawal only; cannot reserve, publish, settle or release.
#[derive(Clone)]
pub struct Withdrawal(Arc<Shared>);
impl Withdrawal {
    pub fn cancel(&self) {
        self.0.preparation.withdraw();
    }
    pub fn remaining_ms(&self) -> Result<u64, ErrorCode> {
        self.0.current()
    }
    pub fn reservation_generation(&self) -> Result<u64, ErrorCode> {
        self.0.reserved()
    }
}
/// Checked response data, NOT a publication/settlement capability.
pub struct Prepared {
    request: Request,
    binding: Binding,
    current: Current,
}
impl Prepared {
    pub fn into_parts(self) -> (Request, Binding, Current) {
        (self.request, self.binding, self.current)
    }
}
/// Actual worker retains this through native preparation and later execution.
/// Its Drop only withdraws. Release requires this token and exact generation.
pub struct WorkerPreparation {
    shared: Arc<Shared>,
    receive: Receiver<Prepared>,
    content: Receiver<crate::browser_receive::Content>,
    released: bool,
}
impl WorkerOwner {
    pub fn offer(
        &self,
        execution: &mut ReadExecution<'_>,
        scope: avesra_core::browser_scopes::Grant,
    ) -> Result<WorkerPreparation, ErrorCode> {
        scope.validate()?;
        if scope.id.uuid() != execution.permit().action.target_id
            || scope.actor.uuid() != execution.permit().action.actor_id
        {
            return Err(ErrorCode::Denied);
        }
        let preparation = execution.take_preparation()?;
        if let Err(error) = preparation.remaining_ms() {
            preparation.withdraw();
            return Err(error);
        }
        let shared = Arc::new(Shared {
            preparation,
            resource: self.resource.clone(),
            reservation: AtomicU64::new(0),
            finished: AtomicBool::new(false),
            native_lost: AtomicBool::new(false),
            successor_transferred: AtomicBool::new(false),
            publication: Mutex::new(Publication::default()),
        });
        let (reply, receive) = mpsc::sync_channel(1);
        let (content_send, content) = mpsc::sync_channel(1);
        let offer = Offer {
            scope,
            shared: shared.clone(),
            reply: Some(reply),
            completed: false,
            content: Some(content_send),
        };
        self.send
            .try_send(offer)
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(WorkerPreparation {
            shared,
            receive,
            content,
            released: false,
        })
    }
}
impl Drop for WorkerOwner {
    fn drop(&mut self) {
        // Terminal worker exit differs from ordinary withdrawal. Invalidate
        // even an already-blocked exact generation so no retained token can
        // later release it after Store/worker ownership has disappeared.
        self.resource.store(u64::MAX, Ordering::SeqCst);
    }
}
impl PreparationReceiver {
    /// One runtime consumer sleeps until an actual offer or worker exit. This
    /// receiver is independent of the authenticated pipe framing owner.
    pub async fn receive(&mut self) -> Result<Offer, ErrorCode> {
        self.receive.recv().await.ok_or(ErrorCode::Unavailable)
    }
}
impl Offer {
    pub fn scope(&self) -> &avesra_core::browser_scopes::Grant {
        &self.scope
    }
    pub fn withdrawal(&self) -> Withdrawal {
        Withdrawal(self.shared.clone())
    }
    pub fn permit(&self) -> &DispatchPermit {
        self.shared.preparation.permit()
    }
    pub fn remaining_ms(&self) -> Result<u64, ErrorCode> {
        self.shared.current()
    }
    /// Called inside the serialized target lookup, BEFORE blocking inspection.
    /// Exactly one attempt; a newer resource word cannot be adopted on failure.
    pub fn reserve(&self, expected_generation: u64) -> Result<u64, ErrorCode> {
        self.shared
            .reservation
            .compare_exchange(0, u64::MAX, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ErrorCode::InvalidTransition)?;
        let result = (|| {
            self.shared.current()?;
            let clear = expected_generation.checked_mul(2).ok_or(ErrorCode::Stale)?;
            let Some(blocked) = clear.checked_add(3).filter(|v| *v != u64::MAX) else {
                // Exhaust only the actual matching clear word, never a
                // different owner or an unrepresentable caller generation.
                let _ = self.shared.resource.compare_exchange(
                    clear,
                    u64::MAX,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
                return Err(ErrorCode::Unavailable);
            };
            self.shared
                .resource
                .compare_exchange(clear, blocked, Ordering::SeqCst, Ordering::SeqCst)
                .map_err(|_| ErrorCode::Stale)?;
            self.shared.reservation.store(blocked, Ordering::SeqCst);
            // A simultaneous withdrawal retains the exact owner for cleanup.
            self.shared.current()?;
            Ok(blocked >> 1)
        })();
        if result.is_err() {
            self.shared.preparation.withdraw();
        }
        result
    }
    pub fn reservation_generation(&self) -> Result<u64, ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()
    }
    /// Consumes the original offer; arbitrary data cannot create an offer.
    /// The actual worker still checks catalog/Store/native authority afterward.
    pub fn complete(
        mut self,
        mut request: Request,
        binding: Binding,
        current: Current,
    ) -> Result<NativeEndpoint, ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()?;
        avesra_core::browser_reading::validate_dispatch(&request, self.permit())?;
        binding.validate()?;
        if binding.revoked
            || binding.actor != self.permit().action.actor_id
            || binding.device != self.permit().device_id
        {
            return Err(ErrorCode::Stale);
        }
        request.remaining_ms = request.remaining_ms.min(self.shared.current()?);
        self.shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?
            .context = Some(request.context.clone());
        self.reply
            .take()
            .ok_or(ErrorCode::Stale)?
            .try_send(Prepared {
                request,
                binding,
                current,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        self.completed = true;
        Ok(NativeEndpoint {
            shared: self.shared.clone(),
            content: self.content.take(),
        })
    }
}
impl Drop for Offer {
    fn drop(&mut self) {
        if !self.completed {
            self.shared.preparation.withdraw();
            self.shared.finished.store(true, Ordering::SeqCst);
        }
        // The consumer must retain Offer inside the actual preparation job.
        // Losing its async caller must not drop it ahead of blocking inspection.
        // Successful preparation now includes installing the paired endpoint;
        // its final operation (or failure Drop) acknowledges actual completion.
    }
}
impl WorkerPreparation {
    pub(crate) fn transfer_mailbox(&self) -> Result<(), ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()?;
        let slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if !slot.terminal
            || !slot.successor_registered
            || self.shared.native_lost.load(Ordering::SeqCst)
            || self.shared.successor_transferred.load(Ordering::SeqCst)
        {
            return Err(ErrorCode::Stale);
        }
        self.shared
            .successor_transferred
            .store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn acknowledge_chunk(
        &self,
        ack: avesra_contracts::browser::mailbox::Ack,
    ) -> Result<(), ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()?;
        let mut slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if slot.terminal || slot.context.as_ref() != Some(&ack.context) {
            return Err(ErrorCode::Stale);
        }
        slot.mailbox_ack = Some(ack);
        Ok(())
    }
    pub(crate) fn publish(&self, permit: PublicationPermit) -> Result<(), ErrorCode> {
        self.shared.current()?;
        self.shared.reserved()?;
        let mut slot = self
            .shared
            .publication
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if slot.terminal || slot.attempted || slot.context.is_none() {
            return Err(ErrorCode::Stale);
        }
        slot.attempted = true;
        slot.held = Some(permit);
        Ok(())
    }
    pub(crate) fn close_publication(&self) {
        if let Ok(mut slot) = self.shared.publication.lock() {
            slot.terminal = true;
            slot.held.take();
            slot.published.take();
        }
    }
    pub(crate) fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::SeqCst)
    }
    pub(crate) fn native_lost(&self) -> bool {
        self.shared.native_lost.load(Ordering::SeqCst)
    }
    pub(crate) fn try_content(&self) -> Result<Option<crate::browser_receive::Content>, ErrorCode> {
        match self.content.try_recv() {
            Ok(value) => Ok(Some(value)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(ErrorCode::Unavailable),
        }
    }
    pub fn try_prepared(&self) -> Result<Option<Prepared>, ErrorCode> {
        match self.receive.try_recv() {
            Ok(prepared) => Ok(Some(prepared)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(ErrorCode::Unavailable),
        }
    }
    pub fn reservation_generation(&self) -> Result<u64, ErrorCode> {
        self.shared.reserved()
    }
    pub fn withdraw(&self) {
        self.shared.preparation.withdraw();
    }
    /// Worker-only lifecycle operation, never called by Offer/receiver Drop.
    /// Future live integration calls only after its Store/content/settlement
    /// obligations finish; preparation completion alone is not that proof.
    pub fn release(&mut self) -> Result<(), ErrorCode> {
        if self.released || !self.shared.finished.load(Ordering::SeqCst) {
            return Err(ErrorCode::InvalidTransition);
        }
        let blocked = self.shared.reservation.load(Ordering::SeqCst);
        if blocked != 0 && blocked != u64::MAX {
            let Some(clear) = blocked
                .checked_add(2)
                .filter(|v| *v != u64::MAX)
                .map(|v| v & !1)
            else {
                let _ = self.shared.resource.compare_exchange(
                    blocked,
                    u64::MAX,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
                return Err(ErrorCode::Unavailable);
            };
            self.shared
                .resource
                .compare_exchange(blocked, clear, Ordering::SeqCst, Ordering::SeqCst)
                .map_err(|_| ErrorCode::Stale)?;
        }
        self.released = true;
        Ok(())
    }
}
impl Drop for WorkerPreparation {
    fn drop(&mut self) {
        if !self.released || !self.shared.successor_transferred.load(Ordering::SeqCst) {
            self.shared.preparation.withdraw();
        }
    }
}
