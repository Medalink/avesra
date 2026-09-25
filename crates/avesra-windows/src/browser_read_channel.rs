//! Dormant preparation plumbing. Only the actual worker owns a sender/token;
//! metadata returned by the native consumer still needs ReadExecution.authorize.
use avesra_contracts::{ErrorCode, actors::Binding, browser::reading::Request};
use avesra_core::{
    browser_execution::{Preparation, ReadExecution},
    ledger::DispatchPermit,
};
use std::sync::{
    Arc,
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
    shared: Arc<Shared>,
    reply: Option<SyncSender<Prepared>>,
    completed: bool,
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
    released: bool,
}
impl WorkerOwner {
    pub fn offer(&self, execution: &mut ReadExecution<'_>) -> Result<WorkerPreparation, ErrorCode> {
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
        });
        let (reply, receive) = mpsc::sync_channel(1);
        let offer = Offer {
            shared: shared.clone(),
            reply: Some(reply),
            completed: false,
        };
        self.send
            .try_send(offer)
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(WorkerPreparation {
            shared,
            receive,
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
    ) -> Result<(), ErrorCode> {
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
        Ok(())
    }
}
impl Drop for Offer {
    fn drop(&mut self) {
        if !self.completed {
            self.shared.preparation.withdraw();
        }
        // The consumer must retain Offer inside the actual preparation job.
        // Losing its async caller must not drop it ahead of blocking inspection.
        self.shared.finished.store(true, Ordering::SeqCst);
    }
}
impl WorkerPreparation {
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
        self.shared.preparation.withdraw();
    }
}
