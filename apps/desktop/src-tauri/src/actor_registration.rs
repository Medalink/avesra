//! Explicit owner configuration; never a voice qualification or planner grant.
use crate::{
    Runtime,
    connection::{self, SessionIdentity},
    setup::ManagementProof,
};
use avesra_contracts::{ErrorCode, actors};
use avesra_core::actor_intents::{self, Identity};
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;
const BUDGET: Duration = Duration::from_secs(15);
#[derive(Clone, Copy)]
enum Operation {
    Status,
    Register,
    Revoke(Uuid),
}
#[derive(Serialize)]
pub struct Status {
    state: &'static str,
    intent: &'static str,
    binding: Option<actors::Binding>,
}
struct Admission {
    started: Instant,
    challenge: u64,
    session: SessionIdentity,
    proof: Option<ManagementProof>,
    present: Arc<AtomicBool>,
}
impl Admission {
    fn current(&self, app: &tauri::AppHandle) -> bool {
        if !self.present.load(Ordering::SeqCst) || self.started.elapsed() >= BUDGET {
            return false;
        }
        let state = app.state::<Runtime>();
        let Ok(local) = state.local.lock() else {
            return false;
        };
        !local.locked
            && local.connected
            && !local.settings.paused
            && state.setup.challenge() == self.challenge
            && state.connection_generation.load(Ordering::SeqCst) == self.session.generation
            && local.action_epoch == self.session.action_epoch
            && self
                .proof
                .as_ref()
                .is_none_or(|p| p.current_locked(&state, &local))
            && state.acknowledged_session.lock().ok().is_some_and(|s| {
                s.is_some_and(|s| {
                    s.id == self.session.id
                        && s.device == self.session.device
                        && s.server_fingerprint == self.session.server_fingerprint
                        && s.generation == self.session.generation
                        && s.action_epoch == self.session.action_epoch
                })
            })
    }
    fn check(&self, app: &tauri::AppHandle) -> Result<(), String> {
        if self.current(app) {
            Ok(())
        } else {
            Err("Owner setup changed or expired; refresh registration to reconcile".into())
        }
    }
}
struct Caller(Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
async fn run(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    operation: Operation,
) -> Result<Status, String> {
    let started = Instant::now();
    let initial_challenge = app.state::<Runtime>().setup.challenge();
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use visible Settings".into());
    }
    let state = app.state::<Runtime>();
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let admission = Arc::new({
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.locked
            || !local.connected
            || local.settings.paused
            || state.setup.challenge() != initial_challenge
        {
            return Err("Connect Spark, unlock Windows and resume Avesra first".into());
        }
        let session = state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .ok_or("Wait for an acknowledged Spark session")?;
        if session.generation != state.connection_generation.load(Ordering::SeqCst)
            || session.action_epoch != local.action_epoch
        {
            return Err("Wait for Spark to acknowledge the current action context".into());
        }
        let proof = if matches!(operation, Operation::Status) {
            None
        } else {
            Some(state.setup.management_proof(&local, session.generation)?)
        };
        Admission {
            started,
            challenge: state.setup.challenge(),
            session,
            proof,
            present: caller.0.clone(),
        }
    });
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    // Dropping the invoking IPC future only withdraws publication. The spawned
    // coordinator keeps the actual slot until blocking work and cancellation end.
    let result = tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let prepared = {
            let directory = directory.clone();
            tokio::task::spawn_blocking(move || {
                let (actor, owner_revision) = crate::owner::identity(&directory).map_err(|_| "Local owner identity unavailable")?;
                let pairing = connection::load(&directory)?;
                let identity = Identity { server: pairing.server_fingerprint()?, device: pairing.device_id, actor, owner_revision };
                let intent = actor_intents::load(&directory.join("actor-intents.db"), &identity);
                Ok::<_, String>((pairing, identity, intent))
            }).await.map_err(|_| "Owner reader stopped")??
        };
        admission.check(&app)?;
        let (pairing, identity, mut intent) = prepared;
        let acknowledged_fingerprint: String = admission.session.server_fingerprint.iter().map(|v| format!("{v:02x}")).collect();
        if pairing.device_id != admission.session.device || identity.server != acknowledged_fingerprint {
            return Err("Saved pairing differs from the acknowledged session. Reconnect explicitly before owner setup.".into());
        }
        let command = match operation {
            Operation::Status => actors::Command::Status,
            Operation::Revoke(registration_revision) => {
                if registration_revision.is_nil() { return Err("Invalid registration revision".into()); }
                app.state::<Runtime>().planner.revoke(identity.actor, registration_revision);
                app.state::<Runtime>().effects.revoke_reply_registration(identity.actor, registration_revision);
                {
                    let state = app.state::<Runtime>();
                    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
                    state.browser.retire_actor_targets(identity.actor, registration_revision);
                }
                actors::Command::Revoke { actor: identity.actor, registration_revision }
            }
            Operation::Register => {
                if intent.is_err() { return Err("Local registration intent is unavailable; refresh remote status for exact recovery".into()); }
                let write_app = app.clone(); let write_admission = admission.clone();
                let write_directory = directory.clone(); let write_identity = identity.clone();
                intent = Ok(Some(tokio::task::spawn_blocking(move || {
                    actor_intents::remember(&write_directory.join("actor-intents.db"), &write_identity, &mut || {
                        if crate::owner::identity(&write_directory)? != (write_identity.actor, write_identity.owner_revision) || !write_admission.current(&write_app) { return Err(ErrorCode::Stale); }
                        Ok(())
                    })
                }).await.map_err(|_| "Registration intent writer stopped")?.map_err(|_| "Registration intent not confirmed; refresh before another explicit attempt")?));
                admission.check(&app)?;
                actors::Command::Register { actor: identity.actor, owner_revision: identity.owner_revision }
            }
        };
        let request = actors::Request {
            version: actors::VERSION,
            request: if matches!(operation, Operation::Register) { intent.as_ref().ok().and_then(|i| i.as_ref()).ok_or("Registration intent unavailable")?.request } else { Uuid::new_v4() },
            attempt: Uuid::new_v4(), session: admission.session.id,
            action_epoch: admission.session.action_epoch, command,
        };
        admission.check(&app)?;
        let request_future = connection::actor_operation(&pairing, &request);
        tokio::pin!(request_future);
        let result = loop {
            tokio::select! {
                value = &mut request_future => break value,
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if !admission.current(&app) { break Err("Owner operation withdrawn or expired; refresh registration to reconcile".into()); }
                }
            }
        };
        // Even an HTTP error may leave a server writer alive. Correlated cancel
        // is withdrawal-only and has its own bounded transport, never a retry.
        if result.is_err() || !admission.current(&app) {
            let _ = connection::actor_cancel(&pairing, &actors::Cancel {version: actors::VERSION, attempt: request.attempt, session: request.session, action_epoch: request.action_epoch}).await;
            return Err("Registration result is unconfirmed. Refresh registration to check whether the change was saved.".into());
        }
        let reply = result?;
        admission.check(&app)?;
        let intent_state = match &intent { Ok(Some(_)) => "saved", Ok(None) => "missing", Err(_) => "unavailable" };
        let state = match &reply.binding {
            None => "unregistered",
            Some(binding) if binding.actor != identity.actor || binding.owner_revision != identity.owner_revision => "different_owner",
            Some(binding) if binding.revoked => "revoked",
            Some(binding) if intent.as_ref().ok().and_then(|i|i.as_ref()).is_some_and(|i| i.request == binding.registered_by) => "registered",
            Some(_) => "unreconciled",
        };
        Ok(Status { state, intent: intent_state, binding: reply.binding })
    }).await.map_err(|_| "Owner registration coordinator stopped")?;
    drop(caller);
    result
}
#[tauri::command]
pub async fn actor_registration_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Status, String> {
    run(window, app, Operation::Status).await
}
#[tauri::command]
pub async fn register_owner_with_spark(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Status, String> {
    run(window, app, Operation::Register).await
}
#[tauri::command]
pub async fn revoke_owner_registration(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    registration_revision: Uuid,
) -> Result<Status, String> {
    run(window, app, Operation::Revoke(registration_revision)).await
}
