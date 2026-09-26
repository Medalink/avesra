//! Paired transient classifier ingress. No accepted request or action authority.
use super::{Shared, authenticate_headers, voice_setup};
use avesra_contracts::{
    ErrorCode,
    directedness::{Context, Reply, Request},
};
use axum::http::{HeaderMap, StatusCode};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn current(state: &Shared, context: &Context) -> bool {
    voice_setup::current(
        state,
        context.device,
        context.session,
        context.capture_epoch,
        false,
    ) && state.sessions.lock().is_ok_and(|sessions| {
        sessions
            .get(&context.session)
            .is_some_and(|s| s.action_epoch == context.action_epoch)
    })
}
pub(super) async fn operation(
    state: Shared,
    headers: HeaderMap,
    request: Request,
) -> Result<Reply, StatusCode> {
    let started = Instant::now();
    request.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let deadline = started + Duration::from_millis(request.remaining_ms);
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
    if device != request.context.device || !current(&state, &request.context) {
        return Err(StatusCode::CONFLICT);
    }
    let mut permission = voice_setup::admit(
        &state,
        device,
        request.context.session,
        request.context.capture_epoch,
        request.request,
        false,
    )?;
    let driver = state
        .reasoning
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .clone();
    let owner_state = state.clone();
    let context = request.context.clone();
    let authorize: crate::reasoning::http::Authorization = Arc::new(move || {
        let _admission = &admission;
        if Instant::now() >= deadline || !current(&owner_state, &context) {
            return Err(ErrorCode::Stale);
        }
        let store = owner_state
            .auth
            .try_lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if !store.active(device).map_err(|_| ErrorCode::Unavailable)? {
            return Err(ErrorCode::Denied);
        }
        if Instant::now() >= deadline || !current(&owner_state, &context) {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    });
    let context = request.context.clone();
    let monitor = async {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if !current(&state, &context) {
                return;
            }
        }
    };
    tokio::select! {biased;
        _=tokio::time::sleep_until(deadline.into())=>Err(StatusCode::GATEWAY_TIMEOUT),
        _=permission.changed()=>Err(StatusCode::CONFLICT),
        _=monitor=>Err(StatusCode::CONFLICT),
        value=driver.directedness(request,started,authorize)=>value.map_err(|_|StatusCode::SERVICE_UNAVAILABLE),
    }
}
