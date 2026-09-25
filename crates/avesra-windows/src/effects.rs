//! One process-owned native effect worker. No webview/remote authority ingress.
use crate::volume::{self, VolumeOutcome, VolumeTarget};
use avesra_contracts::{Action, ActionPayload, ErrorCode, Outcome};
use avesra_core::{
    execution::{
        Cancellation, EffectAdapter, EffectObservation, EffectResult, ExecutionController,
        ExecutionReceipt, VolumeLevel,
    },
    ledger::{AcceptedIntent, DispatchPermit, DispatchSession},
    policy::{Approval, Grant},
    store::Store,
};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};
use uuid::Uuid;

/// Only a native authenticated intent/permission adapter may construct these.
/// Deliberately not Deserialize and not part of any webview/network command.
pub enum Management {
    Accept(AcceptedIntent),
    Grant(Grant),
    Propose(Action),
    Approve(Approval),
    Seal { task: Uuid, actor: Uuid },
    RevokeGrant(Uuid),
    RevokeApproval(Uuid),
    RegisterApp(avesra_core::apps::AppRecord),
    RevokeApp(Uuid),
}
impl Management {
    fn apply(
        self,
        store: &mut Store,
        apps: &mut avesra_core::apps::AppCatalog,
    ) -> Result<(), ErrorCode> {
        let now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ErrorCode::Expired)?
                .as_millis(),
        )
        .map_err(|_| ErrorCode::Expired)?;
        match self {
            Self::Accept(intent) => store.accept_intent(&intent, now),
            Self::Grant(grant) => store.record_grant(&grant),
            Self::Propose(action) => store.propose_action(&action, now),
            Self::Approve(approval) => store.record_approval(&approval, now),
            Self::Seal { task, actor } => store.seal_task(task, actor),
            Self::RevokeGrant(id) => store.revoke_grant(id),
            Self::RevokeApproval(id) => store.revoke_approval(id),
            Self::RegisterApp(record) => apps.register(&record),
            Self::RevokeApp(id) => apps.revoke(id),
        }
    }
}
enum Command {
    Execute(Job),
    Manage(Management, SyncSender<Result<(), ErrorCode>>),
}

static PROCESS_OWNER: AtomicBool = AtomicBool::new(false);
struct WorkerOwnership {
    _file: File,
}
impl WorkerOwnership {
    fn acquire(path: &std::path::Path) -> Result<Self, ErrorCode> {
        PROCESS_OWNER
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ErrorCode::Unavailable)?;
        // Sharing zero is an OS-held exclusive lease, not a stale PID marker.
        // Never delete this file: the open handle, including through COM stalls,
        // owns exclusion. Process death releases it automatically.
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path.with_extension("owner.lock"))
        {
            Ok(file) => Ok(Self { _file: file }),
            Err(_) => {
                PROCESS_OWNER.store(false, Ordering::SeqCst);
                Err(ErrorCode::Unavailable)
            }
        }
    }
}
impl Drop for WorkerOwnership {
    fn drop(&mut self) {
        PROCESS_OWNER.store(false, Ordering::SeqCst);
    }
}

struct NativeAdapter {
    targets: HashMap<Uuid, VolumeTarget>,
    apps: avesra_core::apps::AppCatalog,
}
impl EffectAdapter for NativeAdapter {
    fn execute(
        &mut self,
        permit: &DispatchPermit,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<EffectResult, ErrorCode> {
        if let ActionPayload::LaunchApp { app_id } = permit.action.payload {
            if app_id != permit.action.target_id {
                return Err(ErrorCode::Denied);
            }
            let record = self.apps.get(app_id)?;
            return crate::apps::launch(&record, authorize);
        }
        let ActionPayload::SetVolume { percent } = permit.action.payload else {
            return Ok(EffectResult {
                outcome: Outcome::Unsupported,
                observation: None,
            });
        };
        let target = self
            .targets
            .get(&permit.action.target_id)
            .ok_or(ErrorCode::Denied)?;
        let change = volume::set_percent(target, percent, permit.dispatch_id, authorize)?;
        let outcome = match change.outcome {
            VolumeOutcome::Verified => Outcome::Success,
            VolumeOutcome::Uncertain => Outcome::UnknownEffect,
        };
        Ok(EffectResult {
            outcome,
            observation: Some(EffectObservation::Volume {
                before: VolumeLevel {
                    scalar: change.before.scalar,
                    muted: change.before.muted,
                },
                after: change.after.map(|value| VolumeLevel {
                    scalar: value.scalar,
                    muted: value.muted,
                }),
            }),
        })
    }
}
struct Job {
    step: Uuid,
    session: DispatchSession,
    cancellation: Cancellation,
    reply: SyncSender<Result<ExecutionReceipt, ErrorCode>>,
}
struct State {
    action_epoch: u64,
    allowed: bool,
    active: Option<Cancellation>,
}
/// Thread lifetime owns admission, including after receiver timeout/drop. Target
/// records are immutable for its lifetime; replacement requires a new worker.
pub struct NativeEffects {
    send: SyncSender<Command>,
    state: Arc<Mutex<State>>,
}
impl NativeEffects {
    pub fn spawn(path: PathBuf, targets: Vec<VolumeTarget>) -> Result<Self, ErrorCode> {
        if targets.len() > 32 {
            return Err(ErrorCode::TooLarge);
        }
        let mut registry = HashMap::new();
        for target in targets {
            if registry.insert(target.id, target).is_some() {
                return Err(ErrorCode::Malformed);
            }
        }
        let ownership = WorkerOwnership::acquire(&path)?;
        let (send, receive) = mpsc::sync_channel::<Command>(16);
        let state = Arc::new(Mutex::new(State {
            action_epoch: 0,
            allowed: false,
            active: None,
        }));
        let owned = state.clone();
        std::thread::Builder::new()
            .name("avesra-effects".into())
            .spawn(move || {
                let _ownership = ownership;
                let Ok(store) = Store::open(&path) else {
                    return;
                };
                let Ok(apps) = avesra_core::apps::AppCatalog::open(&path.with_extension("apps.db"))
                else {
                    return;
                };
                let mut controller = ExecutionController::new(store);
                let mut adapter = NativeAdapter {
                    targets: registry,
                    apps,
                };
                while let Ok(command) = receive.recv() {
                    let job = match command {
                        Command::Execute(job) => job,
                        Command::Manage(command, reply) => {
                            let _ = reply.try_send(
                                command.apply(controller.management(), &mut adapter.apps),
                            );
                            continue;
                        }
                    };
                    let result =
                        controller.execute(job.step, &job.session, &job.cancellation, &mut adapter);
                    // Ownership ends only after native calls AND durable finalization
                    // have returned. A dropped receiver cannot overlap another job.
                    if let Ok(mut state) = owned.lock() {
                        state.active = None;
                    } else {
                        break;
                    }
                    let _ = job.reply.try_send(result);
                }
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self { send, state })
    }
    /// Accepted work retains its frozen capture provenance. Only current action
    /// authority is observed here; input/output listening controls are independent.
    /// Called synchronously with local state changes, before any storage await.
    pub fn observe(&self, action_epoch: u64, allowed: bool) {
        if let Ok(mut state) = self.state.lock() {
            if (action_epoch != state.action_epoch || !allowed)
                && let Some(active) = &state.active
            {
                active.cancel();
            }
            state.action_epoch = action_epoch;
            state.allowed = allowed;
        }
    }
    /// Native authenticated controller only; step must already exist in this
    /// worker's durable ledger. Public/network messages cannot create authority.
    pub fn submit(
        &self,
        step: Uuid,
        session: DispatchSession,
    ) -> Result<Receiver<Result<ExecutionReceipt, ErrorCode>>, ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
        if !state.allowed
            || session.capture_epoch == 0
            || session.action_epoch == 0
            || state.action_epoch != session.action_epoch
        {
            return Err(ErrorCode::Stale);
        }
        if state.active.is_some() {
            return Err(ErrorCode::Unavailable);
        }
        let cancellation = Cancellation::default();
        let (reply, receive) = mpsc::sync_channel(1);
        state.active = Some(cancellation.clone());
        if self
            .send
            .try_send(Command::Execute(Job {
                step,
                session,
                cancellation,
                reply,
            }))
            .is_err()
        {
            state.active = None;
            return Err(ErrorCode::Unavailable);
        }
        Ok(receive)
    }
    /// Invalidates in-flight admission before queuing a permission/intent change.
    /// Its durable mutation waits for the same owner; a blocked OS call cannot
    /// be bypassed by a second Store or worker.
    pub fn manage(
        &self,
        command: Management,
    ) -> Result<Receiver<Result<(), ErrorCode>>, ErrorCode> {
        let state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
        if let Some(active) = &state.active {
            active.cancel();
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::Manage(command, reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
}
impl Drop for NativeEffects {
    fn drop(&mut self) {
        self.observe(0, false);
    }
}
