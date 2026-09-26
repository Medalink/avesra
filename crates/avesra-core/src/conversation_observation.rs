//! One live accepted-task continuation; history cannot reconstruct this owner.
use super::*;
use avesra_contracts::{Action, ActionPayload, Outcome};

pub struct ObservationClaim {
    target: super::super::CancellationTarget,
    action: Action,
    session: DispatchSession,
    started: Instant,
}
impl ObservationClaim {
    pub(in crate::conversations) fn from_live_resolution(
        record: &Record,
        action: Action,
        session: DispatchSession,
        started: Instant,
    ) -> Option<Self> {
        if !matches!(
            action.payload,
            ActionPayload::Diagnostic { .. }
                | ActionPayload::DiagnoseDownload { .. }
                | ActionPayload::FlushDownloadDns { .. }
                | ActionPayload::ConnectVpn { .. }
                | ActionPayload::ReadInbox { .. }
        ) {
            return None;
        }
        Some(Self {
            target: super::super::CancellationTarget {
                actor: record.actor,
                source: record.source,
                id: record.id,
                revision: record.revision,
            },
            action,
            session,
            started,
        })
    }
    /// Original native coordinator may only shorten the retained budget.
    pub fn bind(
        self,
        binding: Binding,
        started: Instant,
        withdrawal: Arc<AtomicBool>,
    ) -> Result<ObservationRequest, ErrorCode> {
        binding.validate()?;
        if binding.revoked
            || binding.actor != self.target.actor
            || binding.device != self.target.source.device
            || started > self.started
        {
            return Err(ErrorCode::Stale);
        }
        let request = ObservationRequest {
            claim: Self { started, ..self },
            binding,
            cancellation: PlannerCancellation(withdrawal),
            mailbox: None,
            mailbox_lifetime: None,
            lifetime: Arc::new(()),
        };
        request.current()?;
        Ok(request)
    }
}
pub struct ObservationRequest {
    mailbox: Option<crate::browser_execution::MailboxEvidence>,
    mailbox_lifetime: Option<MailboxLifetime>,
    claim: ObservationClaim,
    binding: Binding,
    cancellation: PlannerCancellation,
    // Last: the weak receipt must outlive every content-bearing field.
    lifetime: Arc<()>,
}
impl ObservationRequest {
    pub fn lifetime(&self) -> PlannerLifetime {
        PlannerLifetime(Arc::downgrade(&self.lifetime))
    }
    pub fn target(&self) -> super::super::CancellationTarget {
        self.claim.target
    }
    pub fn action_epoch(&self) -> u64 {
        self.claim.session.action_epoch
    }
    pub fn cancellation(&self) -> PlannerCancellation {
        self.cancellation.clone()
    }
    pub fn requires_mailbox(&self) -> bool {
        matches!(self.claim.action.payload, ActionPayload::ReadInbox { .. })
    }
    pub fn attach_mailbox(
        mut self,
        evidence: crate::browser_execution::MailboxEvidence,
        current: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<Self, ErrorCode> {
        self.current()?;
        if !self.requires_mailbox() || self.mailbox.is_some() || !current() {
            return Err(ErrorCode::Stale);
        }
        let deadline = evidence.deadline().min(
            self.claim.started
                + Duration::from_millis(avesra_contracts::browser::mailbox::LIFETIME_MS),
        );
        self.mailbox = Some(evidence);
        self.mailbox_lifetime = Some(MailboxLifetime { deadline, current });
        self.current()?;
        Ok(self)
    }
    fn current(&self) -> Result<(), ErrorCode> {
        self.cancellation.check()?;
        if self.requires_mailbox() {
            if self.claim.started.elapsed()
                >= Duration::from_millis(avesra_contracts::browser::mailbox::LIFETIME_MS)
                || self
                    .mailbox_lifetime
                    .as_ref()
                    .is_some_and(|v| Instant::now() >= v.deadline || !(v.current)())
            {
                return Err(ErrorCode::Expired);
            }
        } else {
            remaining(self.claim.started)?;
        }
        self.claim.action.validate(crate::execution::now_ms()?)
    }
}
impl Store {
    pub fn finish_observation_answer(
        &mut self,
        mut request: ObservationRequest,
        dispatch: Uuid,
        authorize: &mut dyn FnMut(&PlannerAuthority<'_>) -> Result<(), ErrorCode>,
    ) -> Result<StoredReply, ErrorCode> {
        request.current()?;
        if dispatch.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let (record, state) = super::super::tasks::read_record(&tx, request.claim.target.id)?;
        let linked = super::super::tasks::linked(&tx, &record)?.ok_or(ErrorCode::Stale)?;
        let session = &request.claim.session;
        let action = &request.claim.action;
        if state != "planning"
            || record.revision != request.claim.target.revision
            || record.actor != request.claim.target.actor
            || record.source != request.claim.target.source
            || record.capture_epoch != session.capture_epoch
            || record.action_epoch != session.action_epoch
            || !session.active
            || session.actor_id != record.actor
            || session.device_id != record.source.device
            || session.session_id != record.source.session
            || linked.action_revision != action.revision
            || linked.step != action.step_id
            || linked.task != action.task_id
            || read_plan(&tx, &record)?.is_some()
        {
            return Err(ErrorCode::Stale);
        }
        let live:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM accepted_intents i JOIN ledger_grants g ON g.id=?2 WHERE i.task_id=?1 AND i.actor_id=?3 AND i.sealed=1 AND i.cancel_requested=0 AND g.revoked=0)",params![action.task_id.to_string(),action.grant_id.to_string(),record.actor.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
        if !live {
            return Err(ErrorCode::Stale);
        }
        let grant_bytes: Vec<u8> = tx
            .query_row(
                "SELECT substr(CAST(body AS BLOB),1,8193) FROM ledger_grants WHERE id=?1",
                [action.grant_id.to_string()],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Storage)?;
        if grant_bytes.len() > 8192 {
            return Err(ErrorCode::Malformed);
        }
        let grant: crate::policy::Grant =
            serde_json::from_slice(&grant_bytes).map_err(|_| ErrorCode::Malformed)?;
        if grant.revoked
            || grant.id != action.grant_id
            || grant.actor_id != record.actor
            || grant.target_id != action.target_id
            || !grant.operations.contains(&action.payload.operation())
        {
            return Err(ErrorCode::Stale);
        }
        let (stored_action,outcome,observation):(Vec<u8>,String,Option<Vec<u8>>)=tx.query_row(
            "SELECT substr(CAST(a.body AS BLOB),1,32769),f.outcome,substr(CAST(o.body AS BLOB),1,65537) FROM native_finalizations f JOIN action_revisions a ON a.revision=f.action_revision AND a.dispatch_id=f.dispatch_id JOIN action_heads h ON h.revision=a.revision AND h.step_id=a.step_id JOIN dispatch_bindings b ON b.dispatch_id=f.dispatch_id AND b.actor_id=f.actor_id LEFT JOIN native_observations o ON o.dispatch_id=f.dispatch_id AND o.action_revision=f.action_revision AND o.target_id=f.target_id AND o.at_ms=f.at_ms WHERE f.dispatch_id=?1 AND f.action_revision=?2 AND f.actor_id=?3 AND f.target_id=?4 AND b.device_id=?5 AND b.session_id=?6 AND b.capture_epoch=?7 AND b.action_epoch=?8",
            params![dispatch.to_string(),action.revision.to_string(),record.actor.to_string(),action.target_id.to_string(),session.device_id.to_string(),session.session_id.to_string(),session.capture_epoch.to_string(),session.action_epoch.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_|ErrorCode::Storage)?.ok_or(ErrorCode::Stale)?;
        if stored_action.len() > 32768
            || outcome.len() > 64
            || observation.as_ref().is_some_and(|v| v.len() > 65536)
        {
            return Err(ErrorCode::TooLarge);
        }
        let stored_action: Action =
            serde_json::from_slice(&stored_action).map_err(|_| ErrorCode::Malformed)?;
        if stored_action != *action {
            return Err(ErrorCode::Stale);
        }
        let outcome: Outcome = serde_json::from_str(&outcome).map_err(|_| ErrorCode::Malformed)?;
        let observation = observation
            .map(|v| {
                serde_json::from_slice::<crate::execution::EffectObservation>(&v)
                    .map_err(|_| ErrorCode::Malformed)
            })
            .transpose()?;
        let (text, provenance) = if request.requires_mailbox() {
            if outcome != Outcome::Success || request.mailbox_lifetime.is_none() {
                return Err(ErrorCode::Stale);
            }
            let evidence = request.mailbox.take().ok_or(ErrorCode::Stale)?;
            let (text, provenance, deadline) = evidence.finish(
                action,
                dispatch,
                observation.as_ref().ok_or(ErrorCode::Stale)?,
            )?;
            if request
                .mailbox_lifetime
                .as_ref()
                .is_none_or(|v| v.deadline > deadline)
            {
                return Err(ErrorCode::Stale);
            }
            (text, provenance)
        } else {
            (
                crate::diagnostic_reply::describe(action, outcome, observation.as_ref())?,
                Provenance::NativeObservation {
                    dispatch,
                    action_revision: action.revision,
                },
            )
        };
        let ordinal:i64=tx.query_row("UPDATE planner_claim_sequence SET value=value+1 WHERE id=1 AND value<9007199254740991 RETURNING value",[],|r|r.get(0)).optional().map_err(|_|ErrorCode::Storage)?.ok_or(ErrorCode::TooLarge)?;
        let plan = Plan {
            request: planner::Request {
                version: planner::VERSION,
                context: planner::Context {
                    ordinal: u64::try_from(ordinal).map_err(|_| ErrorCode::Malformed)?,
                    request: Uuid::new_v4(),
                    turn: record.id,
                    turn_revision: record.revision,
                    device: record.source.device,
                    actor: record.actor,
                    registration_revision: request.binding.registration_revision,
                    session: record.source.session,
                    utterance: record.source.utterance,
                    capture_epoch: record.capture_epoch,
                    action_epoch: record.action_epoch,
                },
                text: record.text.clone(),
                dialogue: Vec::new(),
                remaining_ms: planner::MAX_BUDGET_MS,
            },
            binding: request.binding.clone(),
        };
        plan.validate()?;
        let result = ReplyRecord {
            revision: Uuid::new_v4(),
            reply: planner::Reply {
                version: planner::VERSION,
                context: plan.request.context.clone(),
                terminal: planner::Terminal::Complete,
                response: planner::Response::Answer { text },
            },
            provenance,
        };
        result.reply.validate(&plan.request.context)?;
        result.provenance.validate()?;
        tx.execute("INSERT INTO conversation_plans(turn,request,actor,body,state) VALUES(?1,?2,?3,?4,'replied')",params![record.id.to_string(),plan.request.context.request.to_string(),record.actor.to_string(),encode(&plan)?]).map_err(|_|ErrorCode::Storage)?;
        tx.execute(
            "INSERT INTO conversation_replies(turn,revision,request,body) VALUES(?1,?2,?3,?4)",
            params![
                record.id.to_string(),
                result.revision.to_string(),
                plan.request.context.request.to_string(),
                encode(&result)?
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        if tx.execute("UPDATE accepted_conversations SET state='answered' WHERE id=?1 AND state='planning'",[record.id.to_string()]).map_err(|_|ErrorCode::Storage)?!=1 {return Err(ErrorCode::Stale);}
        let stored = read_reply(&tx, &plan)?.ok_or(ErrorCode::Malformed)?;
        if stored.revision != result.revision
            || stored.reply != result.reply
            || stored.provenance != result.provenance
        {
            return Err(ErrorCode::Malformed);
        }
        request.current()?;
        authorize(&PlannerAuthority {
            context: &plan.request.context,
            binding: &plan.binding,
        })?;
        request.current()?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        request.current()?;
        Ok(StoredReply {
            mailbox_lifetime: request.mailbox_lifetime,
            binding: request.binding,
            publication: request.cancellation,
            revision: result.revision,
            reply: result.reply,
            task: None,
            started: request.claim.started,
            provenance: result.provenance,
            lifetime: request.lifetime,
        })
    }
}
