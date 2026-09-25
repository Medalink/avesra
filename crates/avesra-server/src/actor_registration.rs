//! Authenticated native configuration; registered actors still have no tool grants.
use super::{Shared, authenticate_headers};
use avesra_contracts::{
    ErrorCode,
    actors::{Reply, Request, VERSION},
};
use axum::http::{HeaderMap, StatusCode};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
struct Pending(Arc<AtomicBool>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn current(state: &Shared, device: Uuid, request: &Request) -> bool {
    state.sessions.lock().is_ok_and(|sessions| {
        sessions.get(&request.session).is_some_and(|live| {
            live.device == device
                && live.action_epoch == request.action_epoch
                && live.action_enabled
                && live.actor_attempts.get(&request.attempt) == Some(&(request.action_epoch, false))
                && live.updated.elapsed() < Duration::from_secs(30)
        })
    })
}
pub(super) async fn operation(
    state: Shared,
    headers: HeaderMap,
    request: Request,
) -> Result<Reply, StatusCode> {
    let started = Instant::now();
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let admission = state
        .actor_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let device = tokio::time::timeout(
        Duration::from_secs(5),
        authenticate_headers(state.clone(), &headers),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)??;
    let mut watch = {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let live = sessions
            .get_mut(&request.session)
            .ok_or(StatusCode::CONFLICT)?;
        if live.device != device
            || live.action_epoch != request.action_epoch
            || !live.action_enabled
            || live.updated.elapsed() >= Duration::from_secs(30)
        {
            return Err(StatusCode::CONFLICT);
        }
        if live.actor_attempts.len() >= 128 || live.actor_attempts.contains_key(&request.attempt) {
            return Err(StatusCode::CONFLICT);
        }
        live.actor_attempts
            .insert(request.attempt, (request.action_epoch, false));
        live.action_permission.subscribe()
    };
    if started.elapsed() >= Duration::from_secs(5) {
        return Err(StatusCode::REQUEST_TIMEOUT);
    }
    let owner = Pending(Arc::new(AtomicBool::new(true)));
    let live = owner.0.clone();
    let owned = state.clone();
    let command = request.clone();
    // The actual writer retains its slot even if HTTP, timeout or a watch drops
    // this waiter. The callback observes withdrawal before commit initiation.
    let job = tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let mut store = owned.auth.lock().map_err(|_| ErrorCode::Unavailable)?;
        store.actor_operation(device, &command, &mut || {
            if !live.load(Ordering::SeqCst)
                || started.elapsed() >= Duration::from_secs(5)
                || !current(&owned, device, &command)
            {
                return Err(ErrorCode::Stale);
            }
            Ok(())
        })
    });
    let remaining = Duration::from_secs(5).saturating_sub(started.elapsed());
    let result = tokio::select! {
        biased;
        _=watch.changed()=>return Err(StatusCode::CONFLICT),
        _=tokio::time::sleep(remaining)=>return Err(StatusCode::REQUEST_TIMEOUT),
        value=job=>value.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.map_err(|error|match error {ErrorCode::Unauthenticated=>StatusCode::UNAUTHORIZED,ErrorCode::Denied|ErrorCode::Stale=>StatusCode::CONFLICT,_=>StatusCode::SERVICE_UNAVAILABLE})?,
    };
    if started.elapsed() >= Duration::from_secs(5) || !current(&state, device, &request) {
        return Err(StatusCode::CONFLICT);
    }
    let reply = Reply {
        version: VERSION,
        request: request.request,
        attempt: request.attempt,
        session: request.session,
        action_epoch: request.action_epoch,
        binding: result,
    };
    reply
        .validate(&request, device)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(reply)
}
pub(super) async fn cancel(
    state: Shared,
    headers: HeaderMap,
    request: avesra_contracts::actors::Cancel,
) -> Result<(), StatusCode> {
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let device = tokio::time::timeout(
        Duration::from_secs(5),
        authenticate_headers(state.clone(), &headers),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)??;
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let live = sessions
        .get_mut(&request.session)
        .ok_or(StatusCode::CONFLICT)?;
    if live.device != device || live.updated.elapsed() >= Duration::from_secs(30) {
        return Err(StatusCode::CONFLICT);
    }
    if let Some((epoch, cancelled)) = live.actor_attempts.get_mut(&request.attempt) {
        if *epoch != request.action_epoch {
            return Err(StatusCode::CONFLICT);
        }
        *cancelled = true;
    } else {
        if live.action_epoch != request.action_epoch || live.actor_attempts.len() >= 128 {
            return Err(StatusCode::CONFLICT);
        }
        // Cancel can win the network race before operation admission. Never
        // evict a retired identity; exhaustion requires a new control session.
        live.actor_attempts
            .insert(request.attempt, (request.action_epoch, true));
    }
    Ok(())
}
