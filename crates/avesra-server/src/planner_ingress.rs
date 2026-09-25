//! Privileged paired-native assertions. No deployment qualifier is installed.
use super::{Shared, authenticate_headers};
use avesra_contracts::{ErrorCode, planner, speech};
use axum::http::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
pub(super) struct Entry {
    context: planner::Context,
    live: Arc<AtomicBool>,
    completed: Option<Completed>,
}
struct Completed {
    digest: [u8; 32],
    at: Instant,
    output: Option<Output>,
}
struct Output {
    context: speech::Context,
    live: Arc<AtomicBool>,
}
pub(super) struct SpeechLease(Arc<AtomicBool>);
impl Drop for SpeechLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
impl Entry {
    pub(super) fn withdraw(&self) {
        self.live.store(false, Ordering::SeqCst);
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
        if session.device != device
            || session.action_epoch != context.action_epoch
            || !session.action_enabled
            || session.updated.elapsed() >= Duration::from_secs(30)
            || session.planner_requests.len() >= 64
            || collides(&session.planner_requests, &context)
        {
            return Err(StatusCode::CONFLICT);
        }
        let live = Arc::new(AtomicBool::new(true));
        session.planner_requests.insert(
            context.request,
            Entry {
                context: context.clone(),
                live: live.clone(),
                completed: None,
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
    let (driver, qualification) = state
        .reasoning
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let answer = driver.answer(qualification.clone(), request, started, check.clone());
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
        });
        owner.completed = true;
    }
    Ok(reply)
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
        if session.action_epoch != context.action_epoch
            || session.planner_requests.len() >= 64
            || collides(&session.planner_requests, &context)
        {
            return Err(StatusCode::CONFLICT);
        }
        session.planner_requests.insert(
            context.request,
            Entry {
                context,
                live: Arc::new(AtomicBool::new(false)),
                completed: None,
            },
        );
    }
    Ok(())
}

fn response_digest(response: &planner::Response) -> Result<[u8; 32], ErrorCode> {
    let encoded = serde_json::to_vec(response).map_err(|_| ErrorCode::Malformed)?;
    Ok(Sha256::digest(encoded).into())
}
/// Consumes the server completion's one output opportunity. This is paired
/// native assertion, not permission for frontend text or history reconstruction.
pub(super) fn reserve_speech(
    state: &Shared,
    device: uuid::Uuid,
    request: &speech::Request,
) -> Result<SpeechLease, ErrorCode> {
    request.validate()?;
    let context = &request.source.planner;
    if device != context.device {
        return Err(ErrorCode::Denied);
    }
    let digest = response_digest(&request.source.response)?;
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
    let entry = session
        .planner_requests
        .get_mut(&context.request)
        .ok_or(ErrorCode::Stale)?;
    if entry.context != *context || !entry.live.load(Ordering::SeqCst) {
        return Err(ErrorCode::Stale);
    }
    let completed = entry.completed.as_mut().ok_or(ErrorCode::Stale)?;
    if completed.output.is_some()
        || completed.at.elapsed() >= Duration::from_secs(10)
        || !bool::from(completed.digest.ct_eq(&digest))
    {
        return Err(ErrorCode::Stale);
    }
    let live = Arc::new(AtomicBool::new(true));
    completed.output = Some(Output {
        context: request.stream_context(),
        live: live.clone(),
    });
    Ok(SpeechLease(live))
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
