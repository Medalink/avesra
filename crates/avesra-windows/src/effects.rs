//! One process-owned native effect worker. No webview/remote authority ingress.
use crate::volume::{self, VolumeOutcome, VolumeTarget};
use avesra_contracts::{Action, ActionPayload, ErrorCode, Outcome};
use avesra_core::{
    conversations::{
        AppTaskRequest, CancellationTarget, DurableTurn, PlannerAuthority, PlannerCancellation,
        PlannerClaim, PlannerRequest, PlannerRetirement, Source as ConversationSource, StoredReply,
        Summary as ConversationSummary, TaskAuthority, TaskResolution,
    },
    execution::{
        Cancellation, EffectAdapter, EffectObservation, EffectResult, ExecutionController,
        ExecutionReceipt, VolumeLevel,
    },
    ledger::{AcceptedIntent, DispatchPermit, DispatchSession},
    policy::{Approval, Grant},
    store::Store,
    voice::Conversation,
};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    BrowserCompletionWake,
    RetirePlanner {
        retirement: PlannerRetirement,
        reply: SyncSender<Result<(), ErrorCode>>,
    },
    ClaimPlanner {
        request: PlannerRequest,
        authorize: PlannerAuthorization,
        reply: SyncSender<Result<PlannerClaim, ErrorCode>>,
    },
    FinishPlanner {
        claim: Box<PlannerClaim>,
        result: avesra_contracts::planner::Reply,
        authorize: PlannerAuthorization,
        reply: SyncSender<Result<StoredReply, ErrorCode>>,
    },
    AcceptAppTask {
        request: AppTaskRequest,
        authorize: TaskAuthorization,
        reply: SyncSender<Result<TaskResolution, ErrorCode>>,
    },
    AcceptConversation {
        conversation: Conversation,
        authorize: ConversationAuthorization,
        reply: SyncSender<Result<DurableTurn, ErrorCode>>,
    },
    ConversationStatus {
        actor: Uuid,
        source: ConversationSource,
        reply: SyncSender<Result<Option<ConversationSummary>, ErrorCode>>,
    },
    CancelConversation {
        actor: Uuid,
        source: ConversationSource,
        id: Uuid,
        revision: Uuid,
        authorize: CatalogAuthorization,
        reply: SyncSender<Result<Vec<Uuid>, ErrorCode>>,
    },
    Execute(Job),
    Manage(Management, SyncSender<Result<(), ErrorCode>>),
    Catalog(
        CatalogCommand,
        SyncSender<Result<Vec<avesra_core::apps::AppAlias>, ErrorCode>>,
    ),
    ResolveApp(
        Uuid,
        String,
        SyncSender<Result<avesra_core::apps::ResolvedApp, ErrorCode>>,
    ),
    WithApp {
        actor: Uuid,
        id: Uuid,
        revision: Uuid,
        operation: CatalogAuthorization,
        reply: SyncSender<Result<(), ErrorCode>>,
    },
    InspectApp(
        Uuid,
        Uuid,
        Uuid,
        SyncSender<Result<avesra_core::apps::AppRecord, ErrorCode>>,
    ),
}
pub type CatalogAuthorization = Box<dyn FnMut() -> Result<(), ErrorCode> + Send>;
/// Must recheck the original qualified native profile/context at commit. A
/// successful arbitrary closure is not a substitute for that runtime adapter.
pub type ConversationAuthorization = Box<dyn FnMut(&Conversation) -> Result<(), ErrorCode> + Send>;
pub type PlannerAuthorization =
    Box<dyn FnMut(&PlannerAuthority<'_>) -> Result<(), ErrorCode> + Send>;
pub type TaskAuthorization = Box<dyn FnMut(&TaskAuthority<'_>) -> Result<(), ErrorCode> + Send>;
/// Native-owned setup commands. Neither an alias nor registration grants effects.
pub enum CatalogCommand {
    List,
    Remember {
        record: Box<avesra_core::apps::AppRecord>,
        phrase: String,
        authorize: CatalogAuthorization,
    },
    Forget {
        id: Uuid,
        revision: Uuid,
        actor: Uuid,
        authorize: CatalogAuthorization,
    },
}
impl CatalogCommand {
    fn apply(
        self,
        apps: &mut avesra_core::apps::AppCatalog,
        store: &Store,
    ) -> Result<Vec<avesra_core::apps::AppAlias>, ErrorCode> {
        match self {
            Self::List => {}
            Self::Remember {
                record,
                phrase,
                mut authorize,
            } => apps.remember(&record, &phrase, &mut authorize)?,
            Self::Forget {
                id,
                revision,
                actor,
                mut authorize,
            } => apps.forget(id, revision, actor, &mut authorize)?,
        }
        let mut values = apps.aliases()?;
        for value in &mut values {
            value.last_success_ms =
                store.last_app_success(value.target, value.target_revision, value.selected_by)?;
        }
        Ok(values)
    }
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
    read_consumer: Option<ReadConsumer>,
}
/// Runs on the actual worker with the borrowed read and native reservation held.
/// C5 supplies the concrete accepted-task consumer; no webview callback exists.
pub type ReadConsumer = Box<
    dyn for<'a, 'store> FnOnce(
            crate::browser_receive::ReadReply<'a, 'store>,
        ) -> Result<(), ErrorCode>
        + Send,
>;
/// Losing the original native caller withdraws immediately, independently of
/// the worker's Store/Chrome cleanup. This owner never releases worker resources.
pub struct ExecutionWaiter {
    receive: Receiver<Result<ExecutionReceipt, ErrorCode>>,
    cancellation: Cancellation,
}
impl ExecutionWaiter {
    pub fn receive(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Result<ExecutionReceipt, ErrorCode>, mpsc::RecvTimeoutError> {
        self.receive.recv_timeout(timeout)
    }
}
impl Drop for ExecutionWaiter {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
/// Native publication proof with continuous source withdrawal ownership.
/// No Clone/Deserialize; normal output must consume this instead of StoredReply.
pub struct PublishedReply {
    reply: StoredReply,
}
impl PublishedReply {
    pub fn cancellation(&self) -> PlannerCancellation {
        self.reply.cancellation()
    }
    pub fn current(&self) -> bool {
        self.reply.publication_current()
    }
    pub fn context(&self) -> &avesra_contracts::planner::Context {
        self.reply.context()
    }
    pub fn revision(&self) -> Uuid {
        self.reply.revision()
    }
    pub fn response(&self) -> &avesra_contracts::planner::Response {
        self.reply.response()
    }
    pub fn binding(&self) -> &avesra_contracts::actors::Binding {
        self.reply.binding()
    }
}
struct PublishedSource {
    target: CancellationTarget,
    registration: Uuid,
    signal: PlannerCancellation,
}
struct State {
    action_epoch: u64,
    allowed: bool,
    active: Option<Active>,
    pending_cancellations: Vec<CancellationTarget>,
    planners: Vec<(CancellationTarget, PlannerCancellation)>,
    replies: Vec<PublishedSource>,
}
fn planner_current(
    state: &Mutex<State>,
    target: CancellationTarget,
    epoch: u64,
) -> Result<(), ErrorCode> {
    let state = state.lock().map_err(|_| ErrorCode::Unavailable)?;
    if !state.allowed
        || state.action_epoch != epoch
        || state.pending_cancellations.contains(&target)
        || !state
            .planners
            .iter()
            .any(|(t, c)| *t == target && !c.cancelled())
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
struct Active {
    step: Uuid,
    actor: Uuid,
    cancellation: Cancellation,
    conversation: Option<CancellationTarget>,
    registration: Option<(Uuid, Uuid)>,
}
struct BrowserCompletion {
    proof: crate::browser_receive::Settlement,
    reply: SyncSender<Result<avesra_core::browser_jobs::Retirement, ErrorCode>>,
}
// Low bit is blocked; upper bits retire metadata observations across even a
// complete blocked->clear transition between polls. Exhaustion stays closed.
fn browser_resource(state: &AtomicU64, blocked: bool) {
    let mut old = state.load(Ordering::SeqCst);
    loop {
        if old == u64::MAX || (old & 1 != 0) == blocked {
            return;
        }
        let next = old
            .checked_add(2)
            .map(|v| (v & !1) | u64::from(blocked))
            .unwrap_or(u64::MAX);
        match state.compare_exchange(old, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return,
            Err(current) => old = current,
        }
    }
}
struct BrowserWorkerLifetime(Arc<AtomicU64>);
impl Drop for BrowserWorkerLifetime {
    fn drop(&mut self) {
        browser_resource(&self.0, true);
    }
}
fn settle_browser(
    store: &mut Store,
    proof: crate::browser_receive::Settlement,
) -> Result<avesra_core::browser_jobs::Retirement, ErrorCode> {
    // Receipt lookup also rejects an impossible same-dispatch live marker. It
    // never clears a different active dispatch or constructs an execution.
    if let Some(receipt) = store.browser_read_retirement(proof.context())? {
        return Ok(receipt);
    }
    let recovery = store.recover_browser_read()?.ok_or(ErrorCode::Stale)?;
    if recovery.context() != proof.context() {
        return Err(ErrorCode::Stale);
    }
    recovery.retire()
}
fn active_completion(
    execution: &mut avesra_core::browser_execution::ReadExecution<'_>,
    preparation: &crate::browser_read_channel::WorkerPreparation,
    lease: &mut Option<avesra_core::browser_execution::MarkerLease>,
    receive: &Receiver<BrowserCompletion>,
) -> Result<bool, ErrorCode> {
    let Ok(completion) = receive.try_recv() else {
        return Ok(false);
    };
    let own = lease
        .as_ref()
        .is_some_and(|v| v.context() == completion.proof.context());
    let result = if own {
        preparation.close_publication();
        execution.retire_published(lease.take().ok_or(ErrorCode::Stale)?)
    } else {
        execution
            .retirement(completion.proof.context())
            .and_then(|v| v.ok_or(ErrorCode::Stale))
    };
    let succeeded = result.is_ok();
    let _ = completion.reply.try_send(result);
    if own && !succeeded {
        return Err(ErrorCode::Storage);
    }
    Ok(own)
}
fn execute_read(
    controller: &mut ExecutionController,
    apps: &avesra_core::apps::AppCatalog,
    owner: &crate::browser_read_channel::WorkerOwner,
    completions: &Receiver<BrowserCompletion>,
    state: &Mutex<State>,
    job: &mut Job,
) -> Result<ExecutionReceipt, ErrorCode> {
    let mut execution = controller.begin_browser_read(job.step, &job.session, &job.cancellation)?;
    let Some(consumer) = job.read_consumer.take() else {
        let permit = execution.permit();
        let receipt = ExecutionReceipt {
            dispatch_id: permit.dispatch_id,
            target_id: permit.action.target_id,
            action_revision: permit.action.revision,
            outcome: Outcome::Unsupported,
            crossed_commit_boundary: false,
            observation: None,
        };
        execution.finish_unpublished(None, Outcome::Unsupported)?;
        return Ok(receipt);
    };
    let mut preparation = match owner.offer(&mut execution) {
        Ok(value) => value,
        Err(error) => {
            execution.finish_unpublished(None, Outcome::Failed)?;
            return Err(error);
        }
    };
    let mut lease = None;
    let prepared = loop {
        active_completion(&mut execution, &preparation, &mut lease, completions)?;
        if execution.remaining_ms().is_err() {
            break Err(ErrorCode::Stale);
        }
        match preparation.try_prepared() {
            Ok(Some(value)) => break Ok(value),
            Err(error) => break Err(error),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    };
    let prepared = match prepared {
        Ok(value) => value,
        Err(error) => {
            preparation.withdraw();
            // A dropped waiter never releases a still-running native inspection.
            while !preparation.finished() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            execution.finish_unpublished(None, Outcome::Cancelled)?;
            preparation.release()?;
            return Err(error);
        }
    };
    let (request, binding, mut current) = prepared.into_parts();
    let authorize = (|| {
        let record = apps.get(request.context.browser_app.id.uuid())?;
        if record.revision != request.context.browser_app.revision.uuid()
            || record.selected_by != request.context.actor.uuid()
        {
            return Err(ErrorCode::Stale);
        }
        {
            let mut state = state.lock().map_err(|_| ErrorCode::Unavailable)?;
            let active = state
                .active
                .as_mut()
                .filter(|v| v.step == job.step)
                .ok_or(ErrorCode::Stale)?;
            if active.cancellation.is_cancelled() {
                return Err(ErrorCode::Stale);
            }
            active.registration = Some((binding.actor, binding.registration_revision));
        }
        execution.authorize(request, &mut current)
    })();
    let permit = match authorize {
        Ok((marker, permit)) => {
            lease = Some(marker);
            permit
        }
        Err(error) => {
            preparation.withdraw();
            while !preparation.finished() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            drop(current);
            execution.finish_unpublished(None, Outcome::Failed)?;
            preparation.release()?;
            return Err(error);
        }
    };
    let publication = preparation.publish(permit);
    let mut settled = false;
    let mut content = None;
    let result = (|| {
        publication?;
        loop {
            match active_completion(&mut execution, &preparation, &mut lease, completions) {
                Ok(value) => settled |= value,
                Err(error) => break Err(error),
            }
            if content.is_none() {
                match preparation.try_content() {
                    Ok(value) => content = value,
                    Err(error) => break Err(error),
                }
            }
            if settled && content.is_some() {
                let borrowed = content
                    .take()
                    .ok_or(ErrorCode::Stale)?
                    .finalize(&mut execution, &mut current)?;
                consumer(borrowed)?;
                break execution.completed_receipt();
            }
            if execution.remaining_ms().is_err() {
                preparation.close_publication();
                preparation.withdraw();
                if settled || !execution.possibly_published() || preparation.native_lost() {
                    break Err(ErrorCode::Stale);
                }
            }
            if preparation.native_lost() {
                break Err(ErrorCode::Unavailable);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    })();
    preparation.close_publication();
    while !preparation.finished() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    drop(current);
    if !execution.possibly_published() {
        execution.finish_unpublished(lease, Outcome::Cancelled)?;
        preparation.release()?;
    } else {
        // A missing receipt keeps the exact resource word closed and the durable
        // marker uncertain. Idle recovery alone may later retire actual proof.
        drop(execution);
        if settled {
            preparation.release()?;
        }
    }
    result
}
/// Thread lifetime owns admission, including after receiver timeout/drop. Target
/// records are immutable for its lifetime; replacement requires a new worker.
pub struct NativeEffects {
    send: SyncSender<Command>,
    browser_completion: SyncSender<BrowserCompletion>,
    browser_blocked: Arc<AtomicU64>,
    browser_preparation: Mutex<Option<crate::browser_read_channel::PreparationReceiver>>,
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
        let (browser_completion, browser_receive) = mpsc::sync_channel::<BrowserCompletion>(1);
        let browser_blocked = Arc::new(AtomicU64::new(1));
        let (browser_preparation_owner, browser_preparation) =
            crate::browser_read_channel::channel(browser_blocked.clone());
        let owned_browser = browser_blocked.clone();
        let state = Arc::new(Mutex::new(State {
            action_epoch: 0,
            allowed: false,
            active: None,
            pending_cancellations: Vec::new(),
            planners: Vec::new(),
            replies: Vec::with_capacity(16),
        }));
        let owned = state.clone();
        std::thread::Builder::new()
            .name("avesra-effects".into())
            .spawn(move || {
                let _ownership = ownership;
                let _browser_lifetime = BrowserWorkerLifetime(owned_browser.clone());
                // The preparation sender stays on this actual Store owner.
                let Ok(store) = Store::open(&path) else {
                    return;
                };
                let Ok(apps) = avesra_core::apps::AppCatalog::open(&path.with_extension("apps.db"))
                else {
                    return;
                };
                browser_resource(
                    &owned_browser,
                    store
                        .browser_read_ownership()
                        .map(|v| v.is_some())
                        .unwrap_or(true),
                );
                let mut controller = ExecutionController::new(store);
                let mut adapter = NativeAdapter {
                    targets: registry,
                    apps,
                };
                while let Ok(command) = receive.recv() {
                    // This dedicated mailbox is also the completion input for
                    // the active ReadExecution loop. Never queue its
                    // settlement behind a worker waiting for that same browser.
                    if let Ok(completion) = browser_receive.try_recv() {
                        let mut result = settle_browser(controller.management(), completion.proof);
                        match controller.management().browser_read_ownership() {
                            Ok(Some(_)) => browser_resource(&owned_browser, true),
                            Ok(None) if result.is_ok() => browser_resource(&owned_browser, false),
                            // Failed retirement never clears retained uncertainty.
                            // An unsolicited invalid proof also cannot close an
                            // otherwise clear resource without ownership evidence.
                            Ok(None) => {}
                            Err(error) => {
                                browser_resource(&owned_browser, true);
                                result = Err(error);
                            }
                        }
                        let _ = completion.reply.try_send(result);
                    }
                    let mut job = match command {
                        Command::BrowserCompletionWake => continue,
                        Command::RetirePlanner { retirement, reply } => {
                            let target = retirement.target();
                            let result = controller.management().retire_planner(retirement);
                            if let Ok(mut state) = owned.lock() {
                                state.planners.retain(|(t, _)| *t != target);
                            }
                            let _ = reply.try_send(result);
                            continue;
                        }
                        Command::ClaimPlanner {
                            request,
                            mut authorize,
                            reply,
                        } => {
                            let target = request.target();
                            let epoch = request.action_epoch();
                            let result =
                                controller
                                    .management()
                                    .claim_planner(request, &mut |authority| {
                                        planner_current(&owned, target, epoch)?;
                                        authorize(authority)?;
                                        planner_current(&owned, target, epoch)
                                    });
                            if result.is_err()
                                && let Ok(mut state) = owned.lock()
                            {
                                state.planners.retain(|(t, _)| *t != target);
                            }
                            if let Err(
                                mpsc::TrySendError::Full(Ok(claim))
                                | mpsc::TrySendError::Disconnected(Ok(claim)),
                            ) = reply.try_send(result)
                            {
                                let _ = controller.management().retire_planner(claim.retirement());
                                if let Ok(mut state) = owned.lock() {
                                    state.planners.retain(|(t, _)| *t != target);
                                }
                            }
                            continue;
                        }
                        Command::FinishPlanner {
                            claim,
                            result,
                            mut authorize,
                            reply,
                        } => {
                            let target = claim.target();
                            let epoch = claim.context().action_epoch;
                            let result = controller.management().finish_planner(
                                *claim,
                                result,
                                &mut |authority| {
                                    planner_current(&owned, target, epoch)?;
                                    authorize(authority)?;
                                    planner_current(&owned, target, epoch)
                                },
                            );
                            if result.is_err()
                                && let Ok(mut state) = owned.lock()
                            {
                                state.planners.retain(|(t, _)| *t != target);
                            }
                            let _ = reply.try_send(result);
                            continue;
                        }
                        Command::AcceptAppTask {
                            request,
                            mut authorize,
                            reply,
                        } => {
                            let _ = reply.try_send(controller.management().accept_app_task(
                                request,
                                &adapter.apps,
                                &mut authorize,
                            ));
                            continue;
                        }
                        Command::AcceptConversation {
                            conversation,
                            mut authorize,
                            reply,
                        } => {
                            let _ = reply.try_send(
                                controller
                                    .management()
                                    .accept_conversation(conversation, &mut authorize),
                            );
                            continue;
                        }
                        Command::ConversationStatus {
                            actor,
                            source,
                            reply,
                        } => {
                            let _ = reply.try_send(
                                controller.management().conversation_status(actor, source),
                            );
                            continue;
                        }
                        Command::CancelConversation {
                            actor,
                            source,
                            id,
                            revision,
                            mut authorize,
                            reply,
                        } => {
                            let result = controller.management().cancel_conversation(
                                actor,
                                source,
                                id,
                                revision,
                                &mut authorize,
                            );
                            if let Ok(mut state) = owned.lock() {
                                state.pending_cancellations.retain(|value| {
                                    *value
                                        != CancellationTarget {
                                            actor,
                                            source,
                                            id,
                                            revision,
                                        }
                                });
                            }
                            let _ = reply.try_send(result);
                            continue;
                        }
                        Command::Execute(job) => job,
                        Command::Manage(command, reply) => {
                            let _ = reply.try_send(
                                command.apply(controller.management(), &mut adapter.apps),
                            );
                            continue;
                        }
                        Command::Catalog(command, reply) => {
                            let _ = reply.try_send(
                                command.apply(&mut adapter.apps, controller.management()),
                            );
                            continue;
                        }
                        Command::ResolveApp(actor, phrase, reply) => {
                            let _ = reply.try_send(adapter.apps.resolve(actor, &phrase));
                            continue;
                        }
                        Command::WithApp {
                            actor,
                            id,
                            revision,
                            mut operation,
                            reply,
                        } => {
                            let result = adapter.apps.get(id).and_then(|record| {
                                if record.selected_by != actor || record.revision != revision {
                                    return Err(ErrorCode::Stale);
                                }
                                // The same catalog owner retains this exact mapping through
                                // the native operation; queued revoke cannot interleave.
                                operation()
                            });
                            let _ = reply.try_send(result);
                            continue;
                        }
                        Command::InspectApp(actor, id, revision, reply) => {
                            let _ = reply.try_send(adapter.apps.get(id).and_then(|record| {
                                if record.selected_by != actor || record.revision != revision {
                                    return Err(ErrorCode::Stale);
                                }
                                Ok(record)
                            }));
                            continue;
                        }
                    };
                    let binding = controller
                        .management()
                        .conversation_for_step(job.step)
                        .and_then(|conversation| {
                            let mut state = owned.lock().map_err(|_| ErrorCode::Unavailable)?;
                            let cancelled = conversation
                                .is_some_and(|value| state.pending_cancellations.contains(&value));
                            let active = state
                                .active
                                .as_mut()
                                .filter(|active| active.step == job.step)
                                .ok_or(ErrorCode::Stale)?;
                            active.conversation = conversation;
                            if cancelled {
                                active.cancellation.cancel();
                            }
                            Ok(())
                        });
                    let result = binding.and_then(|()| {
                        if controller.management().action_is_browser_read(job.step)? {
                            execute_read(
                                &mut controller,
                                &adapter.apps,
                                &browser_preparation_owner,
                                &browser_receive,
                                &owned,
                                &mut job,
                            )
                        } else {
                            controller.execute(
                                job.step,
                                &job.session,
                                &job.cancellation,
                                &mut adapter,
                            )
                        }
                    });
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
        Ok(Self {
            send,
            state,
            browser_completion,
            browser_blocked,
            browser_preparation: Mutex::new(Some(browser_preparation)),
        })
    }
    /// Native runtime only. No clone/replacement receiver or sender injection.
    /// Taking this endpoint does not admit a read or construct an offer.
    pub fn take_browser_preparation(
        &self,
    ) -> Result<crate::browser_read_channel::PreparationReceiver, ErrorCode> {
        self.browser_preparation
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?
            .take()
            .ok_or(ErrorCode::InvalidTransition)
    }
    /// A resource-exclusion snapshot only, closed through initialization/exit.
    /// It never grants metadata or accepted page access.
    pub fn browser_work_blocked(&self) -> bool {
        self.browser_blocked.load(Ordering::SeqCst) & 1 != 0
    }
    /// Same atomic word as blocked; unavailable/overflow never grants metadata.
    pub fn browser_work_generation(&self) -> Result<u64, ErrorCode> {
        let value = self.browser_blocked.load(Ordering::SeqCst);
        if value & 1 != 0 {
            Err(ErrorCode::Unavailable)
        } else {
            Ok(value >> 1)
        }
    }
    /// Only an actual authenticated pipe owner can supply this non-clonable
    /// proof. Dropping the receiver cannot withdraw or duplicate the SQL job.
    pub fn settle_browser_read(
        &self,
        proof: crate::browser_receive::Settlement,
    ) -> Result<Receiver<Result<avesra_core::browser_jobs::Retirement, ErrorCode>>, ErrorCode> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.browser_completion
            .try_send(BrowserCompletion { proof, reply })
            .map_err(|_| ErrorCode::Unavailable)?;
        // Full means the worker already has a command that will drain this
        // mailbox. Disconnected means the owned receiver will close; neither
        // case makes an already-admitted retirement safe to reissue blindly.
        let _ = self.send.try_send(Command::BrowserCompletionWake);
        Ok(receive)
    }
    /// Accepted work retains its frozen capture provenance. Only current action
    /// authority is observed here; input/output listening controls are independent.
    /// Called synchronously with local state changes, before any storage await.
    pub fn observe(&self, action_epoch: u64, allowed: bool) {
        if let Ok(mut state) = self.state.lock() {
            if (action_epoch != state.action_epoch || !allowed)
                && let Some(active) = &state.active
            {
                active.cancellation.cancel();
            }
            if action_epoch != state.action_epoch || !allowed {
                for (_, planner) in &state.planners {
                    planner.cancel();
                }
                for source in &state.replies {
                    source.signal.cancel();
                }
            }
            state.action_epoch = action_epoch;
            state.allowed = allowed;
        }
    }
    /// Same durable owner; no transcript or accepted-boolean frontend ingress.
    /// Protected native actor-management withdrawal; no durable history mutation.
    pub fn revoke_reply_registration(&self, actor: Uuid, revision: Uuid) {
        if let Ok(state) = self.state.lock() {
            if let Some(active) = &state.active
                && active.actor == actor
                && active.registration.is_none_or(|v| v == (actor, revision))
            {
                active.cancellation.cancel();
            }
            for source in &state.replies {
                if source.target.actor == actor && source.registration == revision {
                    source.signal.cancel();
                }
            }
        }
    }
    pub fn retire_planner(
        &self,
        retirement: PlannerRetirement,
    ) -> Result<Receiver<Result<(), ErrorCode>>, ErrorCode> {
        let target = retirement.target();
        {
            let state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
            for (owned, signal) in &state.planners {
                if *owned == target {
                    signal.cancel();
                }
            }
            for source in &state.replies {
                if source.target == target {
                    source.signal.cancel();
                }
            }
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::RetirePlanner { retirement, reply })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Final native publication linearization; consumes the only stored handle.
    /// Call under current Runtime.local ownership after exact session validation.
    pub fn publish_planner(&self, reply: StoredReply) -> Result<PublishedReply, ErrorCode> {
        let c = reply.context();
        let target = CancellationTarget {
            actor: c.actor,
            source: ConversationSource {
                device: c.device,
                session: c.session,
                utterance: c.utterance,
            },
            id: c.turn,
            revision: c.turn_revision,
        };
        let mut state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
        if !reply.publication_current()
            || !state.allowed
            || state.action_epoch != c.action_epoch
            || state.pending_cancellations.contains(&target)
            || !state
                .planners
                .iter()
                .any(|(t, signal)| *t == target && !signal.cancelled())
        {
            return Err(ErrorCode::Stale);
        }
        state.replies.retain(|source| !source.signal.cancelled());
        if state.replies.len() >= 16 || state.replies.iter().any(|source| source.target == target) {
            return Err(ErrorCode::Unavailable);
        }
        let signal = state
            .planners
            .iter()
            .find(|(t, _)| *t == target)
            .ok_or(ErrorCode::Stale)?
            .1
            .clone();
        // Transfer under one state lock; exact-source cancellation has no gap.
        state.replies.push(PublishedSource {
            target,
            registration: c.registration_revision,
            signal,
        });
        state.planners.retain(|(t, _)| *t != target);
        Ok(PublishedReply { reply })
    }
    pub fn claim_planner(
        &self,
        request: PlannerRequest,
        authorize: PlannerAuthorization,
    ) -> Result<Receiver<Result<PlannerClaim, ErrorCode>>, ErrorCode> {
        let target = request.target();
        let epoch = request.action_epoch();
        let cancellation = request.cancellation();
        let mut state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
        state.planners.retain(|(_, c)| !c.cancelled());
        state.replies.retain(|source| !source.signal.cancelled());
        if !state.allowed
            || state.action_epoch != epoch
            || state.pending_cancellations.contains(&target)
            || state.planners.len() + state.replies.len() >= 16
            || state.planners.iter().any(|(t, _)| *t == target)
            || state.replies.iter().any(|source| source.target == target)
        {
            return Err(ErrorCode::Denied);
        }
        state.planners.push((target, cancellation.clone()));
        let (reply, receive) = mpsc::sync_channel(1);
        if self
            .send
            .try_send(Command::ClaimPlanner {
                request,
                authorize,
                reply,
            })
            .is_err()
        {
            cancellation.cancel();
            state.planners.retain(|(t, _)| *t != target);
            return Err(ErrorCode::Unavailable);
        }
        Ok(receive)
    }
    pub fn finish_planner(
        &self,
        claim: PlannerClaim,
        result: avesra_contracts::planner::Reply,
        authorize: PlannerAuthorization,
    ) -> Result<Receiver<Result<StoredReply, ErrorCode>>, ErrorCode> {
        planner_current(&self.state, claim.target(), claim.context().action_epoch)?;
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::FinishPlanner {
                claim: Box::new(claim),
                result,
                authorize,
                reply,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Dormant until a qualified native producer supplies the opaque value and
    /// exact current-context callback. Queue delay never renews gate freshness.
    /// New conversation admission does not cancel previously accepted actions.
    pub fn accept_conversation(
        &self,
        conversation: Conversation,
        authorize: ConversationAuthorization,
    ) -> Result<Receiver<Result<DurableTurn, ErrorCode>>, ErrorCode> {
        if !conversation.precommit_current() {
            return Err(ErrorCode::Expired);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::AcceptConversation {
                conversation,
                authorize,
                reply,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Actor must be supplied by authenticated native history management, never
    /// by a model. No accepted token or transcript is returned by recovery.
    pub fn conversation_status(
        &self,
        actor: Uuid,
        source: ConversationSource,
    ) -> Result<Receiver<Result<Option<ConversationSummary>, ErrorCode>>, ErrorCode> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::ConversationStatus {
                actor,
                source,
                reply,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    pub fn cancel_conversation(
        &self,
        actor: Uuid,
        source: ConversationSource,
        id: Uuid,
        revision: Uuid,
        mut authorize: CatalogAuthorization,
    ) -> Result<Receiver<Result<Vec<Uuid>, ErrorCode>>, ErrorCode> {
        if [
            actor,
            source.device,
            source.session,
            source.utterance,
            id,
            revision,
        ]
        .iter()
        .any(Uuid::is_nil)
        {
            return Err(ErrorCode::Malformed);
        }
        // Original authenticated native cancellation is rechecked here before
        // signalling and again before durable commit, never consumed anew.
        authorize()?;
        let target = CancellationTarget {
            actor,
            source,
            id,
            revision,
        };
        let mut state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
        if state.pending_cancellations.len() >= 16 || state.pending_cancellations.contains(&target)
        {
            return Err(ErrorCode::Unavailable);
        }
        state.pending_cancellations.push(target);
        for (owned, planner) in &state.planners {
            if *owned == target {
                planner.cancel();
            }
        }
        for source in &state.replies {
            if source.target == target {
                source.signal.cancel();
            }
        }
        if let Some(active) = &state.active
            && active.conversation == Some(target)
        {
            active.cancellation.cancel();
        }
        let (reply, receive) = mpsc::sync_channel(1);
        if self
            .send
            .try_send(Command::CancelConversation {
                actor,
                source,
                id,
                revision,
                authorize,
                reply,
            })
            .is_err()
        {
            state.pending_cancellations.retain(|value| *value != target);
            return Err(ErrorCode::Unavailable);
        }
        Ok(receive)
    }
    /// Exact native resolver only. This never dispatches an effect or grants
    /// permission; the source's stored record and grant decide the one payload.
    pub fn accept_app_task(
        &self,
        request: AppTaskRequest,
        authorize: TaskAuthorization,
    ) -> Result<Receiver<Result<TaskResolution, ErrorCode>>, ErrorCode> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::AcceptAppTask {
                request,
                authorize,
                reply,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Native authenticated controller only; step must already exist in this
    /// worker's durable ledger. Public/network messages cannot create authority.
    pub fn submit(
        &self,
        step: Uuid,
        session: DispatchSession,
        read_consumer: Option<ReadConsumer>,
    ) -> Result<ExecutionWaiter, ErrorCode> {
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
        state.active = Some(Active {
            step,
            actor: session.actor_id,
            cancellation: cancellation.clone(),
            conversation: None,
            registration: None,
        });
        if self
            .send
            .try_send(Command::Execute(Job {
                step,
                session,
                cancellation: cancellation.clone(),
                reply,
                read_consumer,
            }))
            .is_err()
        {
            state.active = None;
            return Err(ErrorCode::Unavailable);
        }
        Ok(ExecutionWaiter {
            receive,
            cancellation,
        })
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
            active.cancellation.cancel();
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::Manage(command, reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Catalog management shares the effect owner and never recovers a second
    /// live ledger. Remember/forget leave frozen accepted tasks and grants intact.
    pub fn catalog(
        &self,
        command: CatalogCommand,
    ) -> Result<Receiver<Result<Vec<avesra_core::apps::AppAlias>, ErrorCode>>, ErrorCode> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::Catalog(command, reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Authenticated native planner only; a resolved mapping is not authority.
    /// Freeze its immutable app UUID in the proposal and recheck accepted scope.
    pub fn resolve_app(
        &self,
        actor: Uuid,
        phrase: String,
    ) -> Result<Receiver<Result<avesra_core::apps::ResolvedApp, ErrorCode>>, ErrorCode> {
        avesra_core::apps::alias_phrase(&phrase)?;
        if actor.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::ResolveApp(actor, phrase, reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    /// Native management only. Callback retains its actual resources if the
    /// caller disappears; no second catalog owner or mutable identity rebinding.
    pub fn with_app(
        &self,
        actor: Uuid,
        id: Uuid,
        revision: Uuid,
        operation: CatalogAuthorization,
    ) -> Result<Receiver<Result<(), ErrorCode>>, ErrorCode> {
        if actor.is_nil() || id.is_nil() || revision.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::WithApp {
                actor,
                id,
                revision,
                operation,
                reply,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    pub fn inspect_app(
        &self,
        actor: Uuid,
        id: Uuid,
        revision: Uuid,
    ) -> Result<Receiver<Result<avesra_core::apps::AppRecord, ErrorCode>>, ErrorCode> {
        if actor.is_nil() || id.is_nil() || revision.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::InspectApp(actor, id, revision, reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
}
impl Drop for NativeEffects {
    fn drop(&mut self) {
        self.observe(0, false);
    }
}
