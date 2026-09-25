//! Native-only opaque claim transport. No webview command or voice activation.
use crate::{
    Runtime,
    connection::{self, PairingRecord, SessionIdentity},
};
use avesra_contracts::{ErrorCode, actors, planner};
use avesra_core::{
    actor_intents::{self, Identity},
    conversations::{PlannerCancellation, PlannerClaim, StoredReply},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;
struct Active {
    context: planner::Context,
    cancellation: PlannerCancellation,
    withdrawn: Arc<AtomicBool>,
}
#[derive(Default)]
pub(crate) struct NativePlanner {
    slot: Arc<Mutex<Option<Active>>>,
}
impl NativePlanner {
    fn reserve(
        &self,
        context: planner::Context,
        cancellation: PlannerCancellation,
        withdrawn: Arc<AtomicBool>,
    ) -> Result<Owner, String> {
        let mut slot = self.slot.lock().map_err(|_| "Planner owner unavailable")?;
        if slot.is_some() {
            return Err("Planner is busy".into());
        }
        let request = context.request;
        *slot = Some(Active {
            context,
            cancellation,
            withdrawn,
        });
        Ok(Owner {
            slot: self.slot.clone(),
            request,
            completed: false,
        })
    }
    pub(crate) fn revoke(&self, actor: Uuid, revision: Uuid) {
        if let Ok(slot) = self.slot.lock()
            && let Some(active) = slot.as_ref()
            && active.context.actor == actor
            && active.context.registration_revision == revision
        {
            active.withdrawn.store(true, Ordering::SeqCst);
            active.cancellation.cancel();
        }
    }
}
struct Owner {
    slot: Arc<Mutex<Option<Active>>>,
    request: Uuid,
    completed: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.lock()
            && slot
                .as_ref()
                .is_some_and(|active| active.context.request == self.request)
            && let Some(active) = slot.take()
            && !self.completed
        {
            active.cancellation.cancel();
        }
    }
}
struct Caller {
    armed: bool,
    withdrawn: Arc<AtomicBool>,
    cancellation: PlannerCancellation,
}
impl Drop for Caller {
    fn drop(&mut self) {
        if self.armed {
            self.withdrawn.store(true, Ordering::SeqCst);
            self.cancellation.cancel();
        }
    }
}
struct Admission {
    context: planner::Context,
    binding: actors::Binding,
    session: SessionIdentity,
    deadline: Instant,
    cancellation: PlannerCancellation,
    withdrawn: Arc<AtomicBool>,
}
impl Admission {
    fn current_locked(&self, state: &Runtime, local: &avesra_core::state::LocalState) -> bool {
        if Instant::now() >= self.deadline
            || self.withdrawn.load(Ordering::SeqCst)
            || self.cancellation.cancelled()
        {
            return false;
        }
        local.connected
            && local.enrolled
            && !local.locked
            && !local.settings.paused
            && local.action_epoch == self.context.action_epoch
            && state.connection_generation.load(Ordering::SeqCst) == self.session.generation
            && state.acknowledged_session.lock().ok().is_some_and(|ack| {
                ack.is_some_and(|s| {
                    s.id == self.session.id
                        && s.device == self.session.device
                        && s.server_fingerprint == self.session.server_fingerprint
                        && s.generation == self.session.generation
                        && s.action_epoch == self.context.action_epoch
                })
            })
    }
    fn current(&self, app: &tauri::AppHandle) -> bool {
        let state = app.state::<Runtime>();
        state
            .local
            .lock()
            .is_ok_and(|local| self.current_locked(&state, &local))
    }
    fn check(&self, app: &tauri::AppHandle) -> Result<(), String> {
        if self.current(app) {
            Ok(())
        } else {
            Err("Accepted planner context changed or expired".into())
        }
    }
}
async fn live<T>(
    app: &tauri::AppHandle,
    admission: &Admission,
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::pin!(future);
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(admission.deadline.into()) => return Err("Planner deadline expired".into()),
            _ = tick.tick() => admission.check(app)?,
            value = &mut future => { admission.check(app)?; return value; }
        }
    }
}
async fn registration(
    app: &tauri::AppHandle,
    admission: &Admission,
    pairing: &PairingRecord,
) -> Result<(), String> {
    let request = actors::Request {
        version: actors::VERSION,
        request: Uuid::new_v4(),
        attempt: Uuid::new_v4(),
        session: admission.context.session,
        action_epoch: admission.context.action_epoch,
        command: actors::Command::Status,
    };
    let reply = live(
        app,
        admission,
        connection::actor_operation(pairing, &request),
    )
    .await?;
    if reply.binding.as_ref() != Some(&admission.binding) || admission.binding.revoked {
        return Err("Original actor registration is no longer current".into());
    }
    Ok(())
}
fn prepared(directory: &std::path::Path, expected: &Admission) -> Result<PairingRecord, String> {
    let (actor, owner_revision) =
        crate::owner::identity(directory).map_err(|_| "Current native owner unavailable")?;
    let pairing = connection::load(directory)?;
    let server = pairing.server_fingerprint()?;
    if actor != expected.binding.actor
        || owner_revision != expected.binding.owner_revision
        || pairing.device_id != expected.session.device
        || server != hex::encode(expected.session.server_fingerprint)
    {
        return Err("Original owner or paired device changed".into());
    }
    let identity = Identity {
        server,
        device: pairing.device_id,
        actor,
        owner_revision,
    };
    let intent = actor_intents::load(&directory.join("actor-intents.db"), &identity)
        .map_err(|_| "Original registration request unavailable")?
        .ok_or("Original registration request unavailable")?;
    if intent.request != expected.binding.registered_by {
        return Err("Original registration request changed".into());
    }
    Ok(pairing)
}
fn final_authority(
    app: tauri::AppHandle,
    admission: Arc<Admission>,
    directory: PathBuf,
) -> avesra_windows::effects::PlannerAuthorization {
    Box::new(move |authority| {
        if authority.context != &admission.context
            || authority.binding != &admission.binding
            || !admission.current(&app)
        {
            return Err(ErrorCode::Stale);
        }
        let state = app.state::<Runtime>();
        let _owner = state
            .owner_setup
            .clone()
            .try_lock_owned()
            .map_err(|_| ErrorCode::Unavailable)?;
        prepared(&directory, &admission).map_err(|_| ErrorCode::Stale)?;
        if !admission.current(&app) {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    })
}
async fn retire(
    app: &tauri::AppHandle,
    retirement: avesra_core::conversations::PlannerRetirement,
) -> Result<(), String> {
    let receive = app
        .state::<Runtime>()
        .effects
        .retire_planner(retirement)
        .map_err(
            |_| "Planner cleanup queue unavailable; pending history requires reconciliation",
        )?;
    tokio::task::spawn_blocking(move || receive.recv())
        .await
        .map_err(|_| "Planner cleanup reader stopped")?
        .map_err(|_| "Planner cleanup owner stopped")?
        .map_err(|_| {
            "Planner cleanup unconfirmed; pending history requires reconciliation".to_owned()
        })
}
async fn run(
    app: &tauri::AppHandle,
    claim: PlannerClaim,
    withdrawn: Arc<AtomicBool>,
) -> Result<StoredReply, String> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(
            claim.remaining_ms().map_err(|_| "Planner claim expired")?,
        ))
        .ok_or("Planner claim expired")?;
    let context = claim.context().clone();
    let binding = claim.binding().clone();
    let cancellation = claim.cancellation();
    let state = app.state::<Runtime>();
    let session = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let session = state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .ok_or("Session unavailable")?;
        if local.locked
            || !local.connected
            || !local.enrolled
            || local.settings.paused
            || local.action_epoch != context.action_epoch
            || session.action_epoch != context.action_epoch
            || session.device != context.device
            || session.id != context.session
            || session.generation != state.connection_generation.load(Ordering::SeqCst)
        {
            return Err("Accepted planner source is no longer current".into());
        }
        session
    };
    let admission = Arc::new(Admission {
        context,
        binding,
        session,
        deadline,
        cancellation,
        withdrawn,
    });
    admission.check(app)?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Native owner directory unavailable")?;
    let owner_slot = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let path = directory.clone();
    let expected = admission.clone();
    let pairing = tokio::task::spawn_blocking(move || {
        let _owner_slot = owner_slot;
        prepared(&path, &expected)
    })
    .await
    .map_err(|_| "Native planner preparation stopped")??;
    admission.check(app)?;
    registration(app, &admission, &pairing).await?;
    let request = claim.transport().map_err(|_| "Planner claim expired")?;
    admission.check(app)?;
    // Every error from the first possible planner send through native publication
    // owns exactly one correlated withdrawal, including post-response failures.
    let result = async {
        let reply = live(
            app,
            &admission,
            connection::planner_operation(&pairing, &request),
        )
        .await?;
        registration(app, &admission, &pairing).await?;
        admission.check(app)?;
        let receiver = state
            .effects
            .finish_planner(
                claim,
                reply,
                final_authority(app.clone(), admission.clone(), directory),
            )
            .map_err(|_| "Planner finalization unavailable")?;
        let stored = tokio::task::spawn_blocking(move || receiver.recv())
            .await
            .map_err(|_| "Planner result reader stopped")?
            .map_err(|_| "Planner ledger owner stopped")?
            .map_err(|_| "Planner reply was not confirmed")?;
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !admission.current_locked(&state, &local) {
            return Err("Planner publication was withdrawn".into());
        }
        // Same local -> effects ordering as Runtime.publish. Exact cancellation
        // and final handoff linearize on the actual effects owner's state mutex.
        state
            .effects
            .publish_planner(stored)
            .map_err(|_| "Planner publication was withdrawn".into())
    }
    .await;
    if result.is_err() {
        let cancel = planner::Cancel {
            version: planner::VERSION,
            context: admission.context.clone(),
        };
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            connection::planner_cancel(&pairing, &cancel),
        )
        .await;
    }
    result
}
/// Called only by an eventual qualified native producer holding a durable claim.
/// Reading saved history or passing arbitrary text cannot invoke this signature.
pub async fn answer(app: tauri::AppHandle, claim: PlannerClaim) -> Result<StoredReply, String> {
    let cancellation = claim.cancellation();
    let withdrawn = Arc::new(AtomicBool::new(false));
    let mut caller = Caller {
        armed: true,
        withdrawn: withdrawn.clone(),
        cancellation: cancellation.clone(),
    };
    let result = tauri::async_runtime::spawn(async move {
        let retirement = claim.retirement();
        let owner = app.state::<Runtime>().planner.reserve(
            claim.context().clone(),
            cancellation,
            withdrawn.clone(),
        );
        let mut owner = match owner {
            Ok(owner) => owner,
            Err(error) => {
                retire(&app, retirement).await?;
                return Err(error);
            }
        };
        let result = run(&app, claim, withdrawn).await;
        if result.is_err() {
            retire(&app, retirement).await?;
        }
        owner.completed = result.is_ok();
        drop(owner);
        result
    })
    .await
    .map_err(|_| "Native planner coordinator stopped; reconcile pending history")?;
    caller.armed = result.is_err();
    drop(caller);
    result
}
