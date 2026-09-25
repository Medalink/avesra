//! Specialized disclosure authorization on the existing borrowed ledger owner.
//! No native transport or settlement authority is constructed in this module.
use crate::{
    browser_jobs,
    execution::{Cancellation, EffectObservation, now_ms},
    ledger::{DispatchPermit, DispatchSession},
    store::Store,
};
use avesra_contracts::{
    ActionPayload, ErrorCode, Outcome,
    browser::reading::{Context, LIFETIME_MS, Reply, Request},
};
use rusqlite::{TransactionBehavior, params};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

const NO_PERMIT: u8 = 0;
const HELD: u8 = 1;
const POSSIBLY_PUBLISHED: u8 = 2;
const RETURNED: u8 = 3;
const SETTLED: u8 = 4;

/// One-use preparation provenance from an actual claimed read. Not authority
/// to publish, and not reconstructible from the serialized permit/history.
pub struct Preparation {
    permit: DispatchPermit,
    cancellation: Cancellation,
    started: Instant,
    admitted_ms: u64,
    deadline: Instant,
}
impl Preparation {
    pub fn permit(&self) -> &DispatchPermit {
        &self.permit
    }
    pub fn remaining_ms(&self) -> Result<u64, ErrorCode> {
        let now = now_ms()?;
        let elapsed =
            u64::try_from(self.started.elapsed().as_millis()).map_err(|_| ErrorCode::Expired)?;
        if self.cancellation.is_cancelled()
            || now.abs_diff(self.admitted_ms.saturating_add(elapsed)) > 1000
        {
            return Err(ErrorCode::Stale);
        }
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if remaining == 0 {
            return Err(ErrorCode::Expired);
        }
        u64::try_from(remaining).map_err(|_| ErrorCode::Expired)
    }
    pub fn withdraw(&self) {
        self.cancellation.cancel();
    }
}

/// Actual worker retains this lease. No reconstruction from wire/history.
pub struct MarkerLease {
    revision: Uuid,
    context: Context,
    phase: Arc<AtomicU8>,
}
impl MarkerLease {
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn revision(&self) -> Uuid {
        self.revision
    }
}
/// Owns the right to make this exact request possibly visible once.
pub struct PublicationPermit {
    request: Option<Request>,
    deadline: Instant,
    cancellation: Cancellation,
    phase: Arc<AtomicU8>,
}
/// Already possibly published; dropping it is never settlement evidence.
pub struct PublishedRequest {
    request: Request,
    deadline: Instant,
    cancellation: Cancellation,
    phase: Arc<AtomicU8>,
}
impl PublicationPermit {
    /// The native coordinator calls only after its exact current context check,
    /// immediately before the first status serialization/send. Any later error
    /// retains uncertainty. This transition does not authenticate a browser.
    pub fn publish(mut self) -> Result<PublishedRequest, ErrorCode> {
        if self.cancellation.is_cancelled() || Instant::now() >= self.deadline {
            return Err(ErrorCode::Stale);
        }
        self.phase
            .compare_exchange(HELD, POSSIBLY_PUBLISHED, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ErrorCode::Stale)?;
        Ok(PublishedRequest {
            request: self.request.take().ok_or(ErrorCode::Stale)?,
            deadline: self.deadline,
            cancellation: self.cancellation.clone(),
            phase: self.phase.clone(),
        })
    }
}
impl Drop for PublicationPermit {
    fn drop(&mut self) {
        // CAS cannot undo a concurrent/previous possibly-published transition.
        let _ = self
            .phase
            .compare_exchange(HELD, RETURNED, Ordering::SeqCst, Ordering::SeqCst);
    }
}
impl PublishedRequest {
    pub fn request(&self) -> Result<Request, ErrorCode> {
        if self.cancellation.is_cancelled()
            || self.phase.load(Ordering::SeqCst) != POSSIBLY_PUBLISHED
        {
            return Err(ErrorCode::Stale);
        }
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if remaining == 0 {
            return Err(ErrorCode::Expired);
        }
        let mut request = self.request.clone();
        request.remaining_ms = request
            .remaining_ms
            .min(u64::try_from(remaining).map_err(|_| ErrorCode::Expired)?);
        request.validate()?;
        Ok(request)
    }
}

/// Borrows the one Store through preparation and eventual native completion.
/// Dropping never clears a marker or proves that external work ended.
pub struct ReadExecution<'a> {
    store: &'a mut Store,
    permit: DispatchPermit,
    session: DispatchSession,
    cancellation: Cancellation,
    started: Instant,
    admitted_ms: u64,
    deadline: Instant,
    attempted: bool,
    preparation_taken: bool,
    marker: Option<(Uuid, Context)>,
    authorized: Option<Request>,
    phase: Arc<AtomicU8>,
    finished: bool,
    finalization_attempted: bool,
}
impl<'a> ReadExecution<'a> {
    pub(crate) fn begin(
        store: &'a mut Store,
        step: Uuid,
        session: &DispatchSession,
        cancellation: &Cancellation,
    ) -> Result<Self, ErrorCode> {
        if cancellation.is_cancelled() {
            return Err(ErrorCode::Stale);
        }
        let admitted_ms = now_ms()?;
        let started = Instant::now();
        let permit = store.claim_action(step, session, admitted_ms)?;
        if !matches!(permit.action.payload, ActionPayload::ReadPage { .. }) {
            store.finish_action(permit.dispatch_id, session, Outcome::Unsupported, now_ms()?)?;
            return Err(ErrorCode::Unsupported);
        }
        let lifetime = permit
            .action
            .expires_at_ms
            .saturating_sub(admitted_ms)
            .min(LIFETIME_MS);
        Ok(Self {
            store,
            permit,
            session: DispatchSession {
                actor_id: session.actor_id,
                device_id: session.device_id,
                session_id: session.session_id,
                capture_epoch: session.capture_epoch,
                action_epoch: session.action_epoch,
                active: session.active,
            },
            cancellation: cancellation.clone(),
            started,
            admitted_ms,
            deadline: started + Duration::from_millis(lifetime),
            attempted: false,
            preparation_taken: false,
            marker: None,
            authorized: None,
            phase: Arc::new(AtomicU8::new(NO_PERMIT)),
            finished: false,
            finalization_attempted: false,
        })
    }
    pub fn permit(&self) -> &DispatchPermit {
        &self.permit
    }
    /// The sole worker transfers this once into its native preparation channel.
    /// Failed delivery/expiry does not permit a replacement preparation attempt.
    pub fn take_preparation(&mut self) -> Result<Preparation, ErrorCode> {
        if self.preparation_taken || self.attempted {
            return Err(ErrorCode::InvalidTransition);
        }
        self.preparation_taken = true;
        if let Err(error) = self.current() {
            self.cancellation.cancel();
            return Err(error);
        }
        let permit = &self.permit;
        Ok(Preparation {
            permit: DispatchPermit {
                dispatch_id: permit.dispatch_id,
                action: permit.action.clone(),
                device_id: permit.device_id,
                session_id: permit.session_id,
                capture_epoch: permit.capture_epoch,
                action_epoch: permit.action_epoch,
            },
            cancellation: self.cancellation.clone(),
            started: self.started,
            admitted_ms: self.admitted_ms,
            deadline: self.deadline,
        })
    }
    /// Read-only exact historical lookup while this owner still borrows Store.
    /// It cannot acknowledge wire data, retire this marker, or release resources.
    pub fn retirement(
        &self,
        context: &Context,
    ) -> Result<Option<browser_jobs::Retirement>, ErrorCode> {
        self.store.browser_read_retirement(context)
    }
    pub fn remaining_ms(&self) -> Result<u64, ErrorCode> {
        self.current()?;
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if remaining == 0 {
            return Err(ErrorCode::Expired);
        }
        u64::try_from(remaining).map_err(|_| ErrorCode::Expired)
    }
    fn current(&self) -> Result<(), ErrorCode> {
        let now = now_ms()?;
        let elapsed =
            u64::try_from(self.started.elapsed().as_millis()).map_err(|_| ErrorCode::Expired)?;
        if self.cancellation.is_cancelled()
            || Instant::now() >= self.deadline
            || now.abs_diff(self.admitted_ms.saturating_add(elapsed)) > 1000
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    /// Exactly one attempted authorization, including failure. The callback is
    /// native current-context validation only, not a UI/serialized permission.
    pub fn authorize(
        &mut self,
        mut request: Request,
        native_current: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(MarkerLease, PublicationPermit), ErrorCode> {
        if self.attempted {
            return Err(ErrorCode::InvalidTransition);
        }
        self.attempted = true;
        crate::browser_reading::validate_dispatch(&request, &self.permit)?;
        self.deadline = self
            .deadline
            .min(Instant::now() + Duration::from_millis(request.remaining_ms));
        self.current()?;
        self.store
            .validate_dispatch(&self.permit, &self.session, now_ms()?)?;
        self.current()?;
        native_current()?;
        self.current()?;
        let revision = Uuid::new_v4();
        let body = serde_json::to_vec(&request.context).map_err(|_| ErrorCode::Malformed)?;
        if body.is_empty() || body.len() > 4096 {
            return Err(ErrorCode::TooLarge);
        }
        let body = String::from_utf8(body).map_err(|_| ErrorCode::Malformed)?;
        // Retain exact candidate identity BEFORE a commit with an uncertain return.
        self.marker = Some((revision, request.context.clone()));
        {
            let tx = self
                .store
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| ErrorCode::Storage)?;
            if browser_jobs::current(&tx)?.is_some() {
                return Err(ErrorCode::InvalidTransition);
            }
            tx.execute("INSERT INTO browser_read_owner(id,revision,dispatch,action_revision,context,state) VALUES(1,?1,?2,?3,?4,'pending')", params![revision.to_string(),request.context.dispatch.uuid().to_string(),request.context.action_revision.uuid().to_string(),body]).map_err(|_| ErrorCode::Storage)?;
            let stored = browser_jobs::current(&tx)?.ok_or(ErrorCode::Malformed)?;
            if stored.revision != revision || stored.context != request.context {
                return Err(ErrorCode::Malformed);
            }
            // No Store callback reentrancy; context checker must not reopen/queue
            // this ledger. Withdrawal can occur while SQLite was blocked.
            if self.cancellation.is_cancelled() || Instant::now() >= self.deadline {
                return Err(ErrorCode::Stale);
            }
            native_current()?;
            if self.cancellation.is_cancelled() || Instant::now() >= self.deadline {
                return Err(ErrorCode::Stale);
            }
            tx.commit().map_err(|_| ErrorCode::Storage)?;
        }
        let stored = browser_jobs::current(&self.store.connection)?.ok_or(ErrorCode::Malformed)?;
        if stored.revision != revision || stored.context != request.context {
            return Err(ErrorCode::Malformed);
        }
        self.current()?;
        native_current()?;
        request.remaining_ms = request.remaining_ms.min(self.remaining_ms()?);
        self.authorized = Some(request.clone());
        self.phase.store(HELD, Ordering::SeqCst);
        Ok((
            MarkerLease {
                revision,
                context: request.context.clone(),
                phase: self.phase.clone(),
            },
            PublicationPermit {
                request: Some(request),
                deadline: self.deadline,
                cancellation: self.cancellation.clone(),
                phase: self.phase.clone(),
            },
        ))
    }
    /// Resource retirement only. The native worker must match its opaque
    /// received Settlement to this exact lease before consuming it here.
    /// Expired/cancelled content authority does not prevent withdrawal.
    pub fn retire_published(
        &mut self,
        lease: MarkerLease,
    ) -> Result<browser_jobs::Retirement, ErrorCode> {
        if !Arc::ptr_eq(&lease.phase, &self.phase)
            || self.phase.load(Ordering::SeqCst) != POSSIBLY_PUBLISHED
            || self.marker.as_ref() != Some(&(lease.revision, lease.context.clone()))
        {
            return Err(ErrorCode::Stale);
        }
        let receipt = browser_jobs::retire(
            &mut self.store.connection,
            lease.revision,
            &lease.context,
            false,
        )?;
        self.marker = None;
        self.phase.store(SETTLED, Ordering::SeqCst);
        Ok(receipt)
    }
    /// Original content authority only. After durable finalization this remains
    /// checked until the borrowed native reply is consumed; no renewed budget.
    pub fn content_current(&self) -> Result<(), ErrorCode> {
        self.current()?;
        if self.phase.load(Ordering::SeqCst) != SETTLED || self.marker.is_some() {
            return Err(ErrorCode::InvalidTransition);
        }
        Ok(())
    }
    pub fn withdraw_content(&self) {
        self.cancellation.cancel();
    }
    /// Correlates untrusted data and records the exact result on this actual
    /// claimed owner. This is NOT native receive authentication and produces no
    /// text capability. Native Content must retain its own private Reply.
    pub fn finalize_content(&mut self, reply: &Reply) -> Result<(), ErrorCode> {
        if self.finished || self.finalization_attempted {
            return Err(ErrorCode::InvalidTransition);
        }
        self.finalization_attempted = true;
        self.content_current()?;
        let request = self.authorized.as_ref().ok_or(ErrorCode::Stale)?;
        crate::browser_reading::validate_reply(request, reply, &self.permit)?;
        let observation = EffectObservation::BrowserRead {
            observation: Box::new(crate::browser_reading::Observation::from_reply(
                request, reply,
            )?),
        };
        observation.validate(&self.permit.action, Outcome::Success)?;
        self.store
            .validate_dispatch(&self.permit, &self.session, now_ms()?)?;
        self.content_current()?;
        let cancellation = self.cancellation.clone();
        let (started, admitted_ms, deadline) = (self.started, self.admitted_ms, self.deadline);
        let mut current = || {
            let elapsed =
                u64::try_from(started.elapsed().as_millis()).map_err(|_| ErrorCode::Expired)?;
            if cancellation.is_cancelled()
                || Instant::now() >= deadline
                || now_ms()?.abs_diff(admitted_ms.saturating_add(elapsed)) > 1000
            {
                return Err(ErrorCode::Stale);
            }
            Ok(())
        };
        self.store.finish_observed_action_checked(
            self.permit.dispatch_id,
            &self.session,
            Outcome::Success,
            Some(&observation),
            now_ms()?,
            Some(crate::ledger::ReadFinalCheck {
                permit: &self.permit,
                current: &mut current,
            }),
        )?;
        self.finished = true;
        // Withdrawal may race commit. Preserve immutable history but withhold
        // the native transient handle when withdrawal wins this later check.
        self.content_current()
    }
    /// Proven never-issued/returned publication creates no settlement receipt.
    pub fn finish_unpublished(
        mut self,
        lease: Option<MarkerLease>,
        outcome: Outcome,
    ) -> Result<(), ErrorCode> {
        if !matches!(
            outcome,
            Outcome::Failed | Outcome::Cancelled | Outcome::Unsupported | Outcome::NeedsInput
        ) {
            return Err(ErrorCode::Denied);
        }
        let phase = self.phase.load(Ordering::SeqCst);
        if !matches!(phase, NO_PERMIT | RETURNED) {
            return Err(ErrorCode::InvalidTransition);
        }
        if phase == RETURNED {
            let lease = lease.ok_or(ErrorCode::Stale)?;
            if !Arc::ptr_eq(&lease.phase, &self.phase)
                || self.marker.as_ref() != Some(&(lease.revision, lease.context))
            {
                return Err(ErrorCode::Stale);
            }
        } else if lease.is_some() {
            return Err(ErrorCode::Stale);
        }
        if let Some((revision, context)) = &self.marker {
            let tx = self
                .store
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| ErrorCode::Storage)?;
            if let Some(stored) = browser_jobs::current(&tx)? {
                if stored.revision != *revision || stored.context != *context {
                    return Err(ErrorCode::Stale);
                }
                if tx
                    .execute(
                        "DELETE FROM browser_read_owner WHERE id=1 AND revision=?1 AND dispatch=?2",
                        [revision.to_string(), context.dispatch.uuid().to_string()],
                    )
                    .map_err(|_| ErrorCode::Storage)?
                    != 1
                {
                    return Err(ErrorCode::Stale);
                }
            } else if phase == RETURNED {
                // A successfully armed lease must not silently adopt missing storage.
                return Err(ErrorCode::Malformed);
            }
            tx.commit().map_err(|_| ErrorCode::Storage)?;
            self.marker = None;
        }
        self.store
            .finish_action(self.permit.dispatch_id, &self.session, outcome, now_ms()?)?;
        self.finished = true;
        Ok(())
    }
}
impl Drop for ReadExecution<'_> {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if !self.finished {
            if let Some((revision, context)) = &self.marker
                && let Ok(Some(stored)) = browser_jobs::current(&self.store.connection)
                && stored.revision == *revision
                && stored.context == *context
            {
                let _ = self.store.connection.execute(
                    "UPDATE browser_read_owner SET state='uncertain' WHERE id=1 AND revision=?1 AND dispatch=?2",
                    [revision.to_string(), context.dispatch.uuid().to_string()],
                );
            }
            // Resource ownership stays in the slot even if outcome persistence
            // fails. Startup recovery cannot turn it into a new effect permit.
            if let Ok(now) = now_ms() {
                let outcome = if self.marker.is_some()
                    || matches!(
                        self.phase.load(Ordering::SeqCst),
                        POSSIBLY_PUBLISHED | SETTLED
                    ) {
                    Outcome::UnknownEffect
                } else {
                    Outcome::Cancelled
                };
                let _ =
                    self.store
                        .finish_action(self.permit.dispatch_id, &self.session, outcome, now);
            }
        }
    }
}
