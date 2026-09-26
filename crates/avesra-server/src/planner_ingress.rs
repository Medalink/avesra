//! Privileged paired-native assertions. No deployment qualifier is installed.
use super::{Shared, authenticate_headers};
use avesra_contracts::{ErrorCode, planner, speech};
use axum::http::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
pub(super) struct Entry {
    context: planner::Context,
    live: Arc<AtomicBool>,
    completed: Option<Completed>,
    owner: Weak<tokio::sync::OwnedSemaphorePermit>,
}
struct Completed {
    digest: [u8; 32],
    at: Instant,
    output: Option<Output>,
    provenance: speech::Provenance,
}
struct Output {
    context: speech::Context,
    live: Arc<AtomicBool>,
    owner: Weak<tokio::sync::OwnedSemaphorePermit>,
    private: Arc<AtomicUsize>,
}
pub(super) struct SpeechLease {
    live: Arc<AtomicBool>,
    pub(super) private: Arc<AtomicUsize>,
}
impl Drop for SpeechLease {
    fn drop(&mut self) {
        self.live.store(false, Ordering::SeqCst);
    }
}
impl Entry {
    pub(super) fn withdraw(&self) {
        self.live.store(false, Ordering::SeqCst);
    }
    fn retired(&self) -> bool {
        if self.owner.strong_count() != 0 {
            return false;
        }
        match &self.completed {
            None => !self.live.load(Ordering::SeqCst),
            Some(completed) => match &completed.output {
                Some(output) => {
                    !output.live.load(Ordering::SeqCst)
                        && output.owner.strong_count() == 0
                        && output.private.load(Ordering::SeqCst) == 0
                }
                None => {
                    !self.live.load(Ordering::SeqCst)
                        || completed.at.elapsed() >= Duration::from_secs(10)
                }
            },
        }
    }
}
struct Lease {
    live: Arc<AtomicBool>,
    completed: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.completed {
            self.live.store(false, Ordering::SeqCst);
        }
    }
}
fn current(state: &Shared, context: &planner::Context) -> bool {
    state.sessions.lock().is_ok_and(|sessions| {
        sessions.get(&context.session).is_some_and(|session| {
            session.device == context.device
                && session.action_epoch == context.action_epoch
                && session.action_enabled
                && session.updated.elapsed() < Duration::from_secs(30)
                && session
                    .planner_requests
                    .get(&context.request)
                    .is_some_and(|entry| {
                        entry.context == *context && entry.live.load(Ordering::SeqCst)
                    })
        })
    })
}
fn authority(
    state: &Shared,
    context: &planner::Context,
    deadline: Instant,
) -> Result<(), ErrorCode> {
    if Instant::now() >= deadline || !current(state, context) {
        return Err(ErrorCode::Stale);
    }
    // Never wait for another auth writer while holding session ownership.
    let store = state.auth.try_lock().map_err(|_| ErrorCode::Unavailable)?;
    store.planner_binding(context)?;
    if Instant::now() >= deadline || !current(state, context) {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
fn collides(
    entries: &std::collections::HashMap<uuid::Uuid, Entry>,
    context: &planner::Context,
) -> bool {
    entries.values().any(|entry| {
        entry.context.request == context.request
            || entry.context.turn == context.turn
            || entry.context.turn_revision == context.turn_revision
            || entry.context.utterance == context.utterance
    })
}
pub(super) fn revoked(
    state: &Shared,
    device: uuid::Uuid,
    binding: &avesra_contracts::actors::Binding,
) {
    if let Ok(sessions) = state.sessions.lock() {
        for session in sessions.values().filter(|session| session.device == device) {
            for entry in session.planner_requests.values().filter(|entry| {
                entry.context.actor == binding.actor
                    && entry.context.registration_revision == binding.registration_revision
            }) {
                entry.live.store(false, Ordering::SeqCst);
            }
        }
    }
}
pub(super) async fn operation(
    state: Shared,
    headers: HeaderMap,
    request: planner::Request,
) -> Result<planner::Reply, StatusCode> {
    let started = Instant::now();
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let deadline = started
        .checked_add(Duration::from_millis(request.remaining_ms))
        .ok_or(StatusCode::BAD_REQUEST)?;
    let admission = Arc::new(
        state
            .planner_admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?,
    );
    let auth_state = state.clone();
    let auth_admission = admission.clone();
    let authentication = tokio::spawn(async move {
        let _admission = auth_admission;
        authenticate_headers(auth_state, &headers).await
    });
    let device = tokio::time::timeout_at(deadline.into(), authentication)
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)??;
    let context = request.context.clone();
    if device != context.device || Instant::now() >= deadline {
        return Err(StatusCode::CONFLICT);
    }
    let mut owner = {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let session = sessions
            .get_mut(&context.session)
            .ok_or(StatusCode::CONFLICT)?;
        session.planner_requests.retain(|_, entry| !entry.retired());
        if session.device != device
            || session.action_epoch != context.action_epoch
            || !session.action_enabled
            || session.updated.elapsed() >= Duration::from_secs(30)
            || session.planner_requests.len() >= 64
            || context.ordinal <= session.planner_ordinal
            || collides(&session.planner_requests, &context)
        {
            return Err(StatusCode::CONFLICT);
        }
        let live = Arc::new(AtomicBool::new(true));
        session.planner_ordinal = context.ordinal;
        session.planner_requests.insert(
            context.request,
            Entry {
                context: context.clone(),
                live: live.clone(),
                completed: None,
                owner: Arc::downgrade(&admission),
            },
        );
        Lease {
            live,
            completed: false,
        }
    };
    let checked = state.clone();
    let exact = context.clone();
    let retained = admission.clone();
    let check: crate::reasoning::http::Authorization = Arc::new(move || {
        let _retained = &retained;
        authority(&checked, &exact, deadline)
    });
    let original = check.clone();
    tokio::time::timeout_at(
        deadline.into(),
        tokio::task::spawn_blocking(move || original()),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    .map_err(|_| StatusCode::CONFLICT)?;
    if !current(&state, &context) || Instant::now() >= deadline {
        return Err(StatusCode::CONFLICT);
    }
    let span = avesra_core::trace::begin(
        avesra_core::trace::Link::planner(&context),
        avesra_core::trace::Stage::ControllerPlanner,
    );
    let result=async {
    let driver = state
        .reasoning
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let answer = driver.answer(request, started, check.clone());
    tokio::pin!(answer);
    let mut inspection = tokio::time::interval(Duration::from_millis(250));
    inspection.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let response = loop {
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline.into()) => return Err(StatusCode::REQUEST_TIMEOUT),
            _ = inspection.tick() => {
                if !current(&state, &context) { return Err(StatusCode::CONFLICT); }
                let original = check.clone();
                tokio::time::timeout_at(deadline.into(), tokio::task::spawn_blocking(move || original())).await
                    .map_err(|_|StatusCode::REQUEST_TIMEOUT)?.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?
                    .map_err(|_|StatusCode::CONFLICT)?;
            }
            result = &mut answer => break result.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?,
        }
    };
    let original = check.clone();
    tokio::time::timeout_at(
        deadline.into(),
        tokio::task::spawn_blocking(move || original()),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    .map_err(|_| StatusCode::CONFLICT)?;
    if !current(&state, &context) || Instant::now() >= deadline {
        return Err(StatusCode::CONFLICT);
    }
    let reply = planner::Reply {
        version: planner::VERSION,
        context: context.clone(),
        terminal: planner::Terminal::Complete,
        response,
    };
    reply
        .validate(&context)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let digest = response_digest(&reply.response).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let session = sessions
            .get_mut(&context.session)
            .ok_or(StatusCode::CONFLICT)?;
        if session.device != context.device
            || session.action_epoch != context.action_epoch
            || !session.action_enabled
            || session.updated.elapsed() >= Duration::from_secs(30)
            || Instant::now() >= deadline
        {
            return Err(StatusCode::CONFLICT);
        }
        let entry = session
            .planner_requests
            .get_mut(&context.request)
            .ok_or(StatusCode::CONFLICT)?;
        if entry.context != context
            || !entry.live.load(Ordering::SeqCst)
            || entry.completed.is_some()
        {
            return Err(StatusCode::CONFLICT);
        }
        entry.completed = Some(Completed {
            digest,
            at: Instant::now(),
            output: None,
            provenance: speech::Provenance::Model,
        });
        owner.completed = true;
    }
    Ok(reply)
    }.await;
    span.finish(
        if result.is_ok() {
            avesra_core::trace::Outcome::Complete
        } else {
            avesra_core::trace::Outcome::Failed
        },
        result.as_ref().err().map(|status| {
            if *status == StatusCode::CONFLICT {
                ErrorCode::Stale
            } else if *status == StatusCode::REQUEST_TIMEOUT {
                ErrorCode::Expired
            } else {
                ErrorCode::Unavailable
            }
        }),
    );
    result
}
pub(super) fn cancel(
    state: Shared,
    headers: HeaderMap,
    request: planner::Cancel,
) -> Result<(), StatusCode> {
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let _admission = state
        .planner_cancellations
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let context = request.context;
    let session = sessions
        .get_mut(&context.session)
        .ok_or(StatusCode::CONFLICT)?;
    if session.device != context.device
        || !bool::from(session.withdrawal_digest.ct_eq(&digest))
        || session.updated.elapsed() >= Duration::from_secs(30)
    {
        return Err(StatusCode::CONFLICT);
    }
    if let Some(entry) = session.planner_requests.get(&context.request) {
        if entry.context != context {
            return Err(StatusCode::CONFLICT);
        }
        entry.live.store(false, Ordering::SeqCst);
    } else {
        session.planner_requests.retain(|_, entry| !entry.retired());
        if session.action_epoch != context.action_epoch
            || session.planner_requests.len() >= 64
            || context.ordinal <= session.planner_ordinal
            || collides(&session.planner_requests, &context)
        {
            return Err(StatusCode::CONFLICT);
        }
        session.planner_ordinal = context.ordinal;
        session.planner_requests.insert(
            context.request,
            Entry {
                context,
                live: Arc::new(AtomicBool::new(false)),
                completed: None,
                owner: Weak::new(),
            },
        );
    }
    Ok(())
}

/// Read-only closure observation; never consumes an ordinal or withdraws work.
pub(super) async fn inspect_retirement(
    state: Shared,
    headers: HeaderMap,
    request: planner::retirement::Request,
) -> Result<planner::retirement::Reply, StatusCode> {
    use planner::retirement::{Observation, Reply, Status, VERSION};
    let started = Instant::now();
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let deadline = started + Duration::from_millis(request.remaining_ms);
    let permit = state
        .planner_inspections
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_owned();
    let work = tokio::task::spawn_blocking(move || {
        // Actual metadata ownership survives a dropped/timed-out HTTP waiter.
        let _permit = permit;
        if Instant::now() >= deadline {
            return Err(StatusCode::REQUEST_TIMEOUT);
        }
        let auth = state
            .auth
            .try_lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let device = auth
            .authenticate(&token)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        if device != request.current.device {
            return Err(StatusCode::FORBIDDEN);
        }
        // Validation requires every item to have this same device/actor/registration.
        auth.planner_binding(&request.contexts[0])
            .map_err(|_| StatusCode::FORBIDDEN)?;
        let current = |session: &super::LiveSession| {
            Instant::now() < deadline
                && session.device == device
                && session.action_epoch == request.current.action_epoch
                && session.updated.elapsed() < Duration::from_secs(30)
        };
        let observations = {
            let sessions = state
                .sessions
                .try_lock()
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            let session = sessions
                .get(&request.current.session)
                .ok_or(StatusCode::CONFLICT)?;
            if !current(session) {
                return Err(StatusCode::CONFLICT);
            }
            request
                .contexts
                .iter()
                .map(|context| {
                    let status = match session.planner_requests.get(&context.request) {
                        Some(entry) if entry.context == *context => match &entry.completed {
                            Some(completed)
                                if matches!(completed.provenance, speech::Provenance::Model) =>
                            {
                                if entry.retired() {
                                    Status::Retired
                                } else if completed.output.as_ref().is_some_and(|output| {
                                    output.owner.strong_count() == 0
                                        && output.private.load(Ordering::SeqCst) != 0
                                }) {
                                    Status::PrivateRetirementUnconfirmed
                                } else {
                                    Status::Busy
                                }
                            }
                            _ => Status::Unknown,
                        },
                        None if context.ordinal <= session.planner_ordinal
                            && !collides(&session.planner_requests, context)
                            && !session
                                .planner_requests
                                .values()
                                .any(|entry| entry.context.ordinal == context.ordinal) =>
                        {
                            Status::ClosedAndCompacted
                        }
                        _ => Status::Unknown,
                    };
                    Observation {
                        context: context.clone(),
                        status,
                    }
                })
                .collect()
        };
        // No session lock spans SQLite work; withdrawal never waits for this read.
        // Retirement/ordinal closure is irreversible within the same live session.
        auth.planner_binding(&request.contexts[0])
            .map_err(|_| StatusCode::FORBIDDEN)?;
        if auth
            .authenticate(&token)
            .map_err(|_| StatusCode::UNAUTHORIZED)?
            != device
        {
            return Err(StatusCode::CONFLICT);
        }
        let sessions = state
            .sessions
            .try_lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if !sessions.get(&request.current.session).is_some_and(current) {
            return Err(StatusCode::CONFLICT);
        }
        let reply = Reply {
            version: VERSION,
            request: request.request,
            current: request.current.clone(),
            observations,
        };
        reply
            .validate(&request)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        Ok(reply)
    });
    tokio::time::timeout_at(deadline.into(), work)
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
}

fn response_digest(response: &planner::Response) -> Result<[u8; 32], ErrorCode> {
    let encoded = serde_json::to_vec(response).map_err(|_| ErrorCode::Malformed)?;
    Ok(Sha256::digest(encoded).into())
}
fn source_digest(source: &speech::Source) -> Result<[u8; 32], ErrorCode> {
    if source.provenance == speech::Provenance::Model {
        response_digest(&source.response)
    } else {
        // Local derivation binds its exact ledger revision and provenance too;
        // there is no model-result digest to borrow or relabel.
        let encoded = serde_json::to_vec(source).map_err(|_| ErrorCode::Malformed)?;
        Ok(Sha256::digest(encoded).into())
    }
}
/// Consumes the server completion's one output opportunity. This is paired
/// native assertion, not permission for frontend text or history reconstruction.
pub(super) fn reserve_speech(
    state: &Shared,
    device: uuid::Uuid,
    request: &speech::Request,
    owner: &Arc<tokio::sync::OwnedSemaphorePermit>,
) -> Result<SpeechLease, ErrorCode> {
    request.validate()?;
    let context = &request.source.planner;
    if device != context.device {
        return Err(ErrorCode::Denied);
    }
    let digest = source_digest(&request.source)?;
    let mut sessions = state.sessions.lock().map_err(|_| ErrorCode::Unavailable)?;
    let session = sessions.get_mut(&context.session).ok_or(ErrorCode::Stale)?;
    if session.device != device
        || session.action_epoch != context.action_epoch
        || !session.action_enabled
        || session.output_epoch != request.playback_epoch
        || !session.output_enabled
        || session.updated.elapsed() >= Duration::from_secs(30)
        || session.planner_requests.values().any(|entry| {
            entry
                .completed
                .as_ref()
                .and_then(|c| c.output.as_ref())
                .is_some_and(|output| output.context.request == request.request)
        })
    {
        return Err(ErrorCode::Stale);
    }
    if !matches!(request.source.provenance, speech::Provenance::Model) {
        session.planner_requests.retain(|_, entry| !entry.retired());
        if context.ordinal <= session.planner_ordinal
            || session.planner_requests.len() >= 64
            || collides(&session.planner_requests, context)
        {
            return Err(ErrorCode::Stale);
        }
        session.planner_ordinal = context.ordinal;
        session.planner_requests.insert(
            context.request,
            Entry {
                context: context.clone(),
                live: Arc::new(AtomicBool::new(true)),
                owner: Arc::downgrade(owner),
                completed: Some(Completed {
                    digest,
                    at: Instant::now(),
                    output: None,
                    provenance: request.source.provenance.clone(),
                }),
            },
        );
    }
    let entry = session
        .planner_requests
        .get_mut(&context.request)
        .ok_or(ErrorCode::Stale)?;
    if entry.context != *context || !entry.live.load(Ordering::SeqCst) {
        return Err(ErrorCode::Stale);
    }
    let completed = entry.completed.as_mut().ok_or(ErrorCode::Stale)?;
    if completed.output.is_some()
        || completed.provenance != request.source.provenance
        || completed.at.elapsed() >= Duration::from_secs(10)
        || !bool::from(completed.digest.ct_eq(&digest))
    {
        return Err(ErrorCode::Stale);
    }
    let live = Arc::new(AtomicBool::new(true));
    let private = Arc::new(AtomicUsize::new(0));
    completed.output = Some(Output {
        context: request.stream_context(),
        live: live.clone(),
        owner: Arc::downgrade(owner),
        private: private.clone(),
    });
    Ok(SpeechLease { live, private })
}
pub(super) fn speech_current(state: &Shared, context: &speech::Context) -> bool {
    state.sessions.lock().is_ok_and(|sessions| {
        sessions
            .get(&context.planner.session)
            .is_some_and(|session| {
                session.device == context.planner.device
                    && session.action_epoch == context.planner.action_epoch
                    && session.action_enabled
                    && session.output_enabled
                    && session.output_epoch == context.playback_epoch
                    && session.updated.elapsed() < Duration::from_secs(30)
                    && session
                        .planner_requests
                        .get(&context.planner.request)
                        .is_some_and(|entry| {
                            entry.context == context.planner
                                && entry.live.load(Ordering::SeqCst)
                                && entry
                                    .completed
                                    .as_ref()
                                    .and_then(|c| c.output.as_ref())
                                    .is_some_and(|output| {
                                        output.context == *context
                                            && output.live.load(Ordering::SeqCst)
                                    })
                        })
            })
    })
}
pub(super) fn speech_authority(
    state: &Shared,
    context: &speech::Context,
    deadline: Instant,
) -> Result<(), ErrorCode> {
    if Instant::now() >= deadline || !speech_current(state, context) {
        return Err(ErrorCode::Stale);
    }
    let store = state.auth.try_lock().map_err(|_| ErrorCode::Unavailable)?;
    store.planner_binding(&context.planner)?;
    if Instant::now() >= deadline || !speech_current(state, context) {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
