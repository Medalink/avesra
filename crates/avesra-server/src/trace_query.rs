//! Paired read-only telemetry. IDs filter rows; they never create accepted work.
use super::{Shared, authenticate_headers};
use avesra_core::trace::{self, Query, Remote};
use axum::http::{HeaderMap, StatusCode};
use std::time::{Duration, Instant};
fn current(state: &Shared, device: uuid::Uuid, request: &Query) -> bool {
    state.sessions.lock().is_ok_and(|s| {
        s.get(&request.session).is_some_and(|s| {
            s.device == device
                && s.action_epoch == request.action_epoch
                && s.updated.elapsed() < Duration::from_secs(30)
        })
    })
}
pub(super) async fn operation(
    state: Shared,
    headers: HeaderMap,
    request: Query,
) -> Result<Remote, StatusCode> {
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let started = Instant::now();
    let permit = state
        .admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let device = authenticate_headers(state.clone(), &headers).await?;
    if !current(&state, device, &request) {
        return Err(StatusCode::CONFLICT);
    }
    let work = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if started.elapsed() > Duration::from_secs(5) || !current(&state, device, &request) {
            return Err(StatusCode::CONFLICT);
        }
        let binding = state
            .auth
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .trace_binding(device, &request)
            .map_err(|_| StatusCode::FORBIDDEN)?;
        let snapshot = trace::snapshot(binding.actor, device, request.turn)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let final_binding = state
            .auth
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .trace_binding(device, &request)
            .map_err(|_| StatusCode::FORBIDDEN)?;
        if binding != final_binding
            || started.elapsed() > Duration::from_secs(5)
            || !current(&state, device, &request)
        {
            return Err(StatusCode::CONFLICT);
        }
        Ok(Remote {
            request: request.request,
            binding,
            snapshot,
        })
    });
    tokio::time::timeout(Duration::from_secs(6), work)
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
}
