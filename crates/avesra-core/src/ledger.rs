//! Durable execution ledger. Management inputs must originate in authenticated
//! controller/local-owner flows, never deserialized model or browser authority.
use crate::{
    policy::{Approval, Grant, PolicyContext},
    store::Store,
};
use avesra_contracts::{Action, ActionPayload, ErrorCode, Outcome, TaskState};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub struct AcceptedIntent {
    pub task_id: Uuid,
    pub actor_id: Uuid,
    pub revision: Uuid,
    pub payloads: Vec<ActionPayload>,
    pub explicit_submit: bool,
}
/// Controller-derived active session, not fields accepted from an action proposal.
pub struct DispatchSession {
    pub actor_id: Uuid,
    pub device_id: Uuid,
    pub session_id: Uuid,
    /// Frozen accepted source provenance, not the current microphone epoch.
    pub capture_epoch: u64,
    /// Current native action authority; pause/stop/lock/disconnect revoke it.
    pub action_epoch: u64,
    pub active: bool,
}
#[derive(Serialize)]
pub struct DispatchPermit {
    pub dispatch_id: Uuid,
    pub action: Action,
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub capture_epoch: u64,
    pub action_epoch: u64,
}
/// The local authenticated management adapter supplies actual postcondition
/// evidence. This is never accepted directly from a model or remote page.
pub struct Reconciliation {
    pub dispatch_id: Uuid,
    pub actor_id: Uuid,
    pub evidence_id: Uuid,
    pub authenticated_local: bool,
    pub outcome: Outcome,
}
fn encode<T: Serialize>(value: &T) -> Result<String, ErrorCode> {
    serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T, ErrorCode> {
    serde_json::from_str(value).map_err(|_| ErrorCode::Malformed)
}
fn sql<T>(value: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    value.map_err(|_| ErrorCode::Storage)
}

impl Store {
    /// Historical evidence only. Never authorizes a new effect or claims that a
    /// process/window is still present. Use the existing ledger owner.
    pub fn last_app_success(
        &self,
        target: Uuid,
        catalog_revision: Uuid,
        actor: Uuid,
    ) -> Result<Option<u64>, ErrorCode> {
        if target.is_nil() || catalog_revision.is_nil() || actor.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let mut query = sql(self.connection.prepare("WITH latest AS MATERIALIZED (SELECT dispatch_id,target_id,action_revision,actor_id,at_ms FROM native_finalizations INDEXED BY native_finalization_lookup WHERE target_id=?1 AND actor_id=?2 AND outcome='\"success\"' ORDER BY rowid DESC LIMIT 1) SELECT substr(o.body,1,4097),length(CAST(o.body AS BLOB)),substr(a.body,1,32769),length(CAST(a.body AS BLOB)),o.action_revision,o.at_ms,s.id,s.task_id,b.actor_id FROM latest f JOIN native_observations o ON o.dispatch_id=f.dispatch_id JOIN action_revisions a ON a.revision=o.action_revision AND a.dispatch_id=o.dispatch_id JOIN steps s ON s.id=a.step_id JOIN dispatch_bindings b ON b.dispatch_id=o.dispatch_id WHERE f.action_revision=o.action_revision AND f.target_id=o.target_id AND f.actor_id=b.actor_id AND f.at_ms=o.at_ms AND o.target_id=?1 AND s.target_id=?1 AND b.actor_id=?2"))?;
        let mut rows = sql(query.query(params![target.to_string(), actor.to_string()]))?;
        let Some(row) = sql(rows.next())? else {
            return Ok(None);
        };
        let length: i64 = sql(row.get(1))?;
        let action_length: i64 = sql(row.get(3))?;
        if !(1..=4096).contains(&length) || !(1..=32768).contains(&action_length) {
            return Err(ErrorCode::Malformed);
        }
        let observation: crate::execution::EffectObservation = decode(&sql::<String>(row.get(0))?)?;
        let action: Action = decode(&sql::<String>(row.get(2))?)?;
        action.validate(action.issued_at_ms)?;
        if sql::<String>(row.get(4))? != action.revision.to_string()
            || sql::<String>(row.get(6))? != action.step_id.to_string()
            || sql::<String>(row.get(7))? != action.task_id.to_string()
            || sql::<String>(row.get(8))? != action.actor_id.to_string()
            || action.target_id != target
            || action.actor_id != actor
        {
            return Err(ErrorCode::Malformed);
        }
        observation.validate(&action, Outcome::Success)?;
        if !matches!(observation, crate::execution::EffectObservation::Application{app_id,catalog_revision: revision,..} | crate::execution::EffectObservation::PackagedApplication{app_id,catalog_revision: revision,..} if app_id==target && revision==catalog_revision)
        {
            return Err(ErrorCode::Malformed);
        }
        let at: i64 = sql(row.get(5))?;
        let at = u64::try_from(at).map_err(|_| ErrorCode::Malformed)?;
        if at == 0 || at < action.issued_at_ms {
            return Err(ErrorCode::Malformed);
        }
        Ok(Some(at))
    }
}
fn clock(value: u64) -> Result<i64, ErrorCode> {
    i64::try_from(value).map_err(|_| ErrorCode::Malformed)
}
fn event(
    tx: &Transaction<'_>,
    task: Uuid,
    step: Option<Uuid>,
    kind: &str,
    now: i64,
) -> Result<(), ErrorCode> {
    sql(tx.execute(
        "INSERT INTO ledger_events(task_id,step_id,kind,at_ms) VALUES(?1,?2,?3,?4)",
        params![task.to_string(), step.map(|id| id.to_string()), kind, now],
    ))?;
    Ok(())
}

impl Store {
    /// Recheck an already claimed exact revision immediately before its native
    /// commit boundary. Consumed approval is valid only for this bound dispatch.
    pub fn validate_dispatch(
        &self,
        permit: &DispatchPermit,
        session: &DispatchSession,
        now_ms: u64,
    ) -> Result<(), ErrorCode> {
        if let Some(owner) = crate::browser_jobs::current(&self.connection)?
            && owner.context.dispatch.uuid() != permit.dispatch_id
        {
            return Err(ErrorCode::InvalidTransition);
        }
        crate::conversations::validate_linked_dispatch(&self.connection, &permit.action, session)?;
        if !session.active
            || session.actor_id != permit.action.actor_id
            || session.device_id != permit.device_id
            || session.session_id != permit.session_id
            || session.capture_epoch != permit.capture_epoch
            || session.action_epoch != permit.action_epoch
            || permit.dispatch_id.is_nil()
        {
            return Err(ErrorCode::Stale);
        }
        let (body, state, cancelled): (String,String,bool) = sql(self.connection.query_row(
            "SELECT a.body,s.state,a.cancel_requested OR i.cancel_requested FROM action_revisions a JOIN action_heads h ON h.revision=a.revision JOIN steps s ON s.id=a.step_id JOIN accepted_intents i ON i.task_id=s.task_id JOIN dispatch_bindings d ON d.dispatch_id=a.dispatch_id WHERE a.dispatch_id=?1 AND d.actor_id=?2 AND d.device_id=?3 AND d.session_id=?4 AND d.capture_epoch=?5 AND d.action_epoch=?6",
            params![permit.dispatch_id.to_string(),session.actor_id.to_string(),session.device_id.to_string(),session.session_id.to_string(),session.capture_epoch.to_string(),session.action_epoch.to_string()],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))))?;
        if cancelled
            || decode::<TaskState>(&state)? != TaskState::Running
            || decode::<Action>(&body)? != permit.action
        {
            return Err(ErrorCode::Stale);
        }
        let (payloads,explicit_submit):(String,bool)=sql(self.connection.query_row("SELECT payloads,explicit_submit FROM accepted_intents WHERE task_id=?1 AND actor_id=?2 AND revision=?3 AND sealed=1",params![permit.action.task_id.to_string(),session.actor_id.to_string(),permit.action.intent_revision.to_string()],|r|Ok((r.get(0)?,r.get(1)?))))?;
        let (body, revoked): (String, bool) = sql(self.connection.query_row(
            "SELECT body,revoked FROM ledger_grants WHERE id=?1",
            [permit.action.grant_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ))?;
        let mut grant: Grant = decode(&body)?;
        grant.revoked |= revoked;
        let approval = if let Some(id) = permit.action.approval_id {
            let body:Option<String>=sql(self.connection.query_row("SELECT body FROM ledger_approvals WHERE id=?1 AND action_revision=?2 AND consumed=1 AND revoked=0",params![id.to_string(),permit.action.revision.to_string()],|r|r.get(0)).optional())?;
            body.map(|body| decode::<Approval>(&body)).transpose()?
        } else {
            None
        };
        PolicyContext {
            actor_id: session.actor_id,
            accepted_task_id: permit.action.task_id,
            intent_revision: permit.action.intent_revision,
            permitted_payloads: &decode::<Vec<ActionPayload>>(&payloads)?,
            now_ms,
            grant: &grant,
            approval: approval.as_ref(),
            explicit_submit,
            session_active: session.active,
        }
        .authorize(&permit.action)
    }
    /// Resolve uncertainty without issuing another effect. Remaining work stays
    /// suspended and requires a new accepted task/intent, not mutation replay.
    pub fn reconcile_action(
        &mut self,
        proof: &Reconciliation,
        now_ms: u64,
    ) -> Result<(), ErrorCode> {
        if !proof.authenticated_local
            || [proof.dispatch_id, proof.actor_id, proof.evidence_id]
                .iter()
                .any(Uuid::is_nil)
            || !matches!(
                proof.outcome,
                Outcome::Success | Outcome::Failed | Outcome::Cancelled
            )
        {
            return Err(ErrorCode::Denied);
        }
        let tx = sql(self.connection.transaction())?;
        let (body,state):(String,String)=sql(tx.query_row("SELECT a.body,s.state FROM action_revisions a JOIN steps s ON s.id=a.step_id WHERE a.dispatch_id=?1",[proof.dispatch_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))))?;
        let action: Action = decode(&body)?;
        if action.actor_id != proof.actor_id
            || !matches!(
                decode::<TaskState>(&state)?,
                TaskState::WaitingForUser | TaskState::UnknownEffect | TaskState::Suspended
            )
        {
            return Err(ErrorCode::InvalidTransition);
        }
        let next = match proof.outcome {
            Outcome::Success => TaskState::Succeeded,
            Outcome::Cancelled => TaskState::Cancelled,
            _ => TaskState::Failed,
        };
        sql(tx.execute(
            "INSERT INTO reconciliation_evidence VALUES(?1,?2,?3,?4,?5)",
            params![
                proof.dispatch_id.to_string(),
                proof.evidence_id.to_string(),
                proof.actor_id.to_string(),
                encode(&proof.outcome)?,
                clock(now_ms)?
            ],
        ))?;
        sql(tx.execute(
            "UPDATE steps SET state=?2,updated_ms=?3 WHERE id=?1",
            params![action.step_id.to_string(), encode(&next)?, clock(now_ms)?],
        ))?;
        let remaining: bool = sql(tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM steps WHERE task_id=?1 AND state!=?2)",
            params![action.task_id.to_string(), encode(&TaskState::Succeeded)?],
            |r| r.get(0),
        ))?;
        let cancelled: bool = sql(tx.query_row(
            "SELECT cancel_requested FROM accepted_intents WHERE task_id=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        ))?;
        let task_next = if cancelled {
            TaskState::Cancelled
        } else if next != TaskState::Succeeded {
            next
        } else if remaining {
            TaskState::Suspended
        } else {
            next
        };
        sql(tx.execute(
            "UPDATE tasks SET state=?2,updated_ms=?3 WHERE id=?1",
            params![
                action.task_id.to_string(),
                encode(&task_next)?,
                clock(now_ms)?
            ],
        ))?;
        event(
            &tx,
            action.task_id,
            Some(action.step_id),
            "reconciled_without_replay",
            clock(now_ms)?,
        )?;
        sql(tx.commit())
    }
    /// Call only after final identity, directed intent, permissions and epoch gates.
    pub fn accept_intent(&mut self, intent: &AcceptedIntent, now_ms: u64) -> Result<(), ErrorCode> {
        if [intent.task_id, intent.actor_id, intent.revision]
            .iter()
            .any(Uuid::is_nil)
            || intent.payloads.is_empty()
            || intent.payloads.len() > 32
        {
            return Err(ErrorCode::Malformed);
        }
        for payload in &intent.payloads {
            payload.validate()?;
        }
        let now = clock(now_ms)?;
        let body = encode(&intent.payloads)?;
        if body.len() > 65_536 {
            return Err(ErrorCode::TooLarge);
        }
        let tx = sql(self.connection.transaction())?;
        sql(tx.execute(
            "INSERT INTO tasks(id,actor_id,state,updated_ms) VALUES(?1,?2,?3,?4)",
            params![
                intent.task_id.to_string(),
                intent.actor_id.to_string(),
                encode(&TaskState::Proposed)?,
                now
            ],
        ))?;
        sql(tx.execute(
            "INSERT INTO accepted_intents(task_id,actor_id,revision,payloads,explicit_submit) VALUES(?1,?2,?3,?4,?5)",
            params![
                intent.task_id.to_string(),
                intent.actor_id.to_string(),
                intent.revision.to_string(),
                body,
                intent.explicit_submit
            ],
        ))?;
        event(&tx, intent.task_id, None, "accepted", now)?;
        sql(tx.commit())
    }

    /// Owner-management adapter must authenticate the local owner before calling.
    /// IDs are immutable; changing scope requires revoking and issuing a new grant.
    pub fn record_grant(&mut self, grant: &Grant) -> Result<(), ErrorCode> {
        if [grant.id, grant.actor_id, grant.target_id]
            .iter()
            .any(Uuid::is_nil)
            || grant.operations.is_empty()
            || grant.operations.len() > 15
        {
            return Err(ErrorCode::Malformed);
        }
        sql(self.connection.execute(
            "INSERT INTO ledger_grants(id,actor_id,body,revoked) VALUES(?1,?2,?3,?4)",
            params![
                grant.id.to_string(),
                grant.actor_id.to_string(),
                encode(grant)?,
                grant.revoked
            ],
        ))?;
        Ok(())
    }
    pub fn revoke_grant(&mut self, id: Uuid) -> Result<(), ErrorCode> {
        if id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        if sql(self.connection.execute(
            "UPDATE ledger_grants SET revoked=1 WHERE id=?1",
            [id.to_string()],
        ))? != 1
        {
            return Err(ErrorCode::Denied);
        }
        Ok(())
    }

    /// Persist a bounded proposal. This is not authorization to dispatch it.
    pub fn propose_action(&mut self, action: &Action, now_ms: u64) -> Result<(), ErrorCode> {
        action.validate(now_ms)?;
        let tx = sql(self.connection.transaction())?;
        let (actor, revision, payloads): (String, String, String) = sql(tx.query_row(
            "SELECT actor_id,revision,payloads FROM accepted_intents WHERE task_id=?1",
            [action.task_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ))?;
        if actor != action.actor_id.to_string()
            || revision != action.intent_revision.to_string()
            || !decode::<Vec<ActionPayload>>(&payloads)?.contains(&action.payload)
        {
            return Err(ErrorCode::Denied);
        }
        let body = encode(action)?;
        let prior: Option<String> = sql(tx
            .query_row(
                "SELECT body FROM action_revisions WHERE revision=?1",
                [action.revision.to_string()],
                |r| r.get(0),
            )
            .optional())?;
        if let Some(prior) = prior {
            return if prior == body {
                Ok(())
            } else {
                Err(ErrorCode::Denied)
            };
        }
        let sealed: bool = sql(tx.query_row(
            "SELECT sealed OR cancel_requested FROM accepted_intents WHERE task_id=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        ))?;
        if sealed {
            return Err(ErrorCode::InvalidTransition);
        }
        let position:i64=sql(tx.query_row("SELECT COUNT(*) FROM steps WHERE task_id=?1 AND rowid<COALESCE((SELECT rowid FROM steps WHERE id=?2),9223372036854775807)",params![action.task_id.to_string(),action.step_id.to_string()],|r|r.get(0)))?;
        if decode::<Vec<ActionPayload>>(&payloads)?
            .get(usize::try_from(position).map_err(|_| ErrorCode::Malformed)?)
            != Some(&action.payload)
        {
            return Err(ErrorCode::Denied);
        }
        let task_state: String = sql(tx.query_row(
            "SELECT state FROM tasks WHERE id=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        ))?;
        if !matches!(
            decode::<TaskState>(&task_state)?,
            TaskState::Proposed | TaskState::Queued | TaskState::AwaitingApproval
        ) {
            return Err(ErrorCode::InvalidTransition);
        }
        let previous: Option<(String, String)> = sql(tx
            .query_row(
                "SELECT task_id,state FROM steps WHERE id=?1",
                [action.step_id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional())?;
        if let Some((task, state)) = previous
            && (task != action.task_id.to_string()
                || !matches!(
                    decode::<TaskState>(&state)?,
                    TaskState::Proposed | TaskState::Queued | TaskState::AwaitingApproval
                ))
        {
            return Err(ErrorCode::InvalidTransition);
        }
        let state = if action.payload.operation().needs_approval() {
            TaskState::AwaitingApproval
        } else {
            TaskState::Queued
        };
        sql(tx.execute("INSERT INTO steps(id,task_id,state,target_id,operation,updated_ms) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET state=excluded.state,target_id=excluded.target_id,operation=excluded.operation,updated_ms=excluded.updated_ms",params![action.step_id.to_string(),action.task_id.to_string(),encode(&state)?,action.target_id.to_string(),encode(&action.payload.operation())?,clock(now_ms)?]))?;
        sql(tx.execute(
            "INSERT INTO action_revisions(revision,step_id,body) VALUES(?1,?2,?3)",
            params![
                action.revision.to_string(),
                action.step_id.to_string(),
                body
            ],
        ))?;
        sql(tx.execute("INSERT INTO action_heads VALUES(?1,?2) ON CONFLICT(step_id) DO UPDATE SET revision=excluded.revision",params![action.step_id.to_string(),action.revision.to_string()]))?;
        sql(tx.execute(
            "UPDATE tasks SET state=?2,updated_ms=?3 WHERE id=?1",
            params![action.task_id.to_string(), encode(&state)?, clock(now_ms)?],
        ))?;
        event(
            &tx,
            action.task_id,
            Some(action.step_id),
            "proposed",
            clock(now_ms)?,
        )?;
        sql(tx.commit())
    }

    /// Exact local approval only; never inferred from speech or model output.
    pub fn record_approval(&mut self, approval: &Approval, now_ms: u64) -> Result<(), ErrorCode> {
        if !approval.authenticated_local || approval.id.is_nil() || approval.expires_at_ms <= now_ms
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        let tx = sql(self.connection.transaction())?;
        let body:String=sql(tx.query_row("SELECT a.body FROM action_revisions a JOIN action_heads h ON h.revision=a.revision WHERE a.revision=?1",[approval.action_revision.to_string()],|r|r.get(0)))?;
        let action: Action = decode(&body)?;
        action.validate(now_ms)?;
        if action.approval_id != Some(approval.id)
            || action.actor_id != approval.actor_id
            || action.task_id != approval.task_id
            || action.step_id != approval.step_id
            || action.target_id != approval.target_id
            || action.intent_revision != approval.intent_revision
            || action.payload != approval.payload
            || approval.expires_at_ms > action.expires_at_ms
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        sql(tx.execute(
            "INSERT INTO ledger_approvals(id,action_revision,body) VALUES(?1,?2,?3)",
            params![
                approval.id.to_string(),
                approval.action_revision.to_string(),
                encode(approval)?
            ],
        ))?;
        sql(tx.execute(
            "UPDATE steps SET state=?2,updated_ms=?3 WHERE id=?1 AND state=?4",
            params![
                action.step_id.to_string(),
                encode(&TaskState::Queued)?,
                clock(now_ms)?,
                encode(&TaskState::AwaitingApproval)?
            ],
        ))?;
        event(
            &tx,
            action.task_id,
            Some(action.step_id),
            "approved",
            clock(now_ms)?,
        )?;
        sql(tx.commit())
    }

    pub fn revoke_approval(&mut self, id: Uuid) -> Result<(), ErrorCode> {
        if id.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        if sql(self.connection.execute(
            "UPDATE ledger_approvals SET revoked=1 WHERE id=?1",
            [id.to_string()],
        ))? != 1
        {
            return Err(ErrorCode::Denied);
        }
        Ok(())
    }

    /// Freeze the complete ordered step list before any effect is claimed.
    pub fn seal_task(&mut self, task: Uuid, actor: Uuid) -> Result<(), ErrorCode> {
        let tx = sql(self.connection.transaction())?;
        let (owner, payloads, cancelled): (String, String, bool) = sql(tx.query_row(
            "SELECT actor_id,payloads,cancel_requested FROM accepted_intents WHERE task_id=?1",
            [task.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ))?;
        if actor.is_nil() || owner != actor.to_string() || cancelled {
            return Err(ErrorCode::Denied);
        }
        let count: i64 = sql(tx.query_row(
            "SELECT COUNT(*) FROM steps WHERE task_id=?1",
            [task.to_string()],
            |r| r.get(0),
        ))?;
        if usize::try_from(count).map_err(|_| ErrorCode::Malformed)?
            != decode::<Vec<ActionPayload>>(&payloads)?.len()
        {
            return Err(ErrorCode::InvalidTransition);
        }
        let actual = {
            let mut statement=sql(tx.prepare("SELECT a.body FROM steps s JOIN action_heads h ON h.step_id=s.id JOIN action_revisions a ON a.revision=h.revision WHERE s.task_id=?1 ORDER BY s.rowid"))?;
            let rows = sql(statement.query_map([task.to_string()], |r| r.get::<_, String>(0)))?;
            let mut values = Vec::new();
            for row in rows {
                values.push(decode::<Action>(&sql(row)?)?.payload);
            }
            values
        };
        if actual != decode::<Vec<ActionPayload>>(&payloads)? {
            return Err(ErrorCode::Denied);
        }
        sql(tx.execute(
            "UPDATE accepted_intents SET sealed=1 WHERE task_id=?1",
            [task.to_string()],
        ))?;
        sql(tx.commit())
    }

    /// Commit before sending the effect. A duplicate claim cannot produce a new
    /// permit, and a crash after this transaction is an unknown effect.
    pub fn claim_action(
        &mut self,
        step: Uuid,
        session: &DispatchSession,
        now_ms: u64,
    ) -> Result<DispatchPermit, ErrorCode> {
        if !session.active
            || [
                session.actor_id,
                session.device_id,
                session.session_id,
                step,
            ]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(ErrorCode::Unauthenticated);
        }
        let tx = sql(self.connection.transaction())?;
        if crate::browser_jobs::current(&tx)?.is_some() {
            return Err(ErrorCode::InvalidTransition);
        }
        let (body,state):(String,String)=sql(tx.query_row("SELECT a.body,s.state FROM action_heads h JOIN action_revisions a ON a.revision=h.revision JOIN steps s ON s.id=h.step_id WHERE h.step_id=?1 AND a.dispatch_id IS NULL AND a.cancel_requested=0",[step.to_string()],|r|Ok((r.get(0)?,r.get(1)?))))?;
        if decode::<TaskState>(&state)? != TaskState::Queued {
            return Err(ErrorCode::InvalidTransition);
        }
        let action: Action = decode(&body)?;
        crate::conversations::validate_linked_dispatch(&tx, &action, session)?;
        let task_state: String = sql(tx.query_row(
            "SELECT state FROM tasks WHERE id=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        ))?;
        let allowed: bool = sql(tx.query_row(
            "SELECT sealed AND NOT cancel_requested FROM accepted_intents WHERE task_id=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        ))?;
        if !allowed {
            return Err(ErrorCode::InvalidTransition);
        }
        if !matches!(
            decode::<TaskState>(&task_state)?,
            TaskState::Proposed | TaskState::Queued | TaskState::AwaitingApproval
        ) {
            return Err(ErrorCode::InvalidTransition);
        }
        let (actor,revision,payloads,explicit_submit):(String,String,String,bool)=sql(tx.query_row("SELECT actor_id,revision,payloads,explicit_submit FROM accepted_intents WHERE task_id=?1",[action.task_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))))?;
        if actor != session.actor_id.to_string() || revision != action.intent_revision.to_string() {
            return Err(ErrorCode::Denied);
        }
        let (grant, revoked): (String, bool) = sql(tx.query_row(
            "SELECT body,revoked FROM ledger_grants WHERE id=?1",
            [action.grant_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ))?;
        let mut grant: Grant = decode(&grant)?;
        grant.revoked |= revoked;
        let approval = if let Some(id) = action.approval_id {
            let body: Option<String> = sql(tx
                .query_row(
                    "SELECT body FROM ledger_approvals WHERE id=?1 AND consumed=0 AND revoked=0",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .optional())?;
            body.map(|body| decode::<Approval>(&body)).transpose()?
        } else {
            None
        };
        let payloads = decode::<Vec<ActionPayload>>(&payloads)?;
        PolicyContext {
            actor_id: session.actor_id,
            accepted_task_id: action.task_id,
            intent_revision: action.intent_revision,
            permitted_payloads: &payloads,
            now_ms,
            grant: &grant,
            approval: approval.as_ref(),
            explicit_submit,
            session_active: session.active,
        }
        .authorize(&action)?;
        let other_running: bool = sql(tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM steps WHERE task_id=?1 AND state=?2)",
            params![action.task_id.to_string(), encode(&TaskState::Running)?],
            |r| r.get(0),
        ))?;
        if other_running {
            return Err(ErrorCode::InvalidTransition);
        }
        let predecessor:bool=sql(tx.query_row("SELECT EXISTS(SELECT 1 FROM steps WHERE task_id=?1 AND rowid<(SELECT rowid FROM steps WHERE id=?2) AND state!=?3)",params![action.task_id.to_string(),step.to_string(),encode(&TaskState::Succeeded)?],|r|r.get(0)))?;
        if predecessor {
            return Err(ErrorCode::InvalidTransition);
        }
        let dispatch_id = Uuid::new_v4();
        sql(tx.execute(
            "INSERT INTO dispatch_bindings VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                dispatch_id.to_string(),
                session.actor_id.to_string(),
                session.device_id.to_string(),
                session.session_id.to_string(),
                session.capture_epoch.to_string(),
                session.action_epoch.to_string()
            ],
        ))?;
        sql(tx.execute(
            "UPDATE action_revisions SET dispatch_id=?2 WHERE revision=?1 AND dispatch_id IS NULL",
            params![action.revision.to_string(), dispatch_id.to_string()],
        ))?;
        sql(tx.execute(
            "UPDATE steps SET state=?2,updated_ms=?3 WHERE id=?1",
            params![
                step.to_string(),
                encode(&TaskState::Running)?,
                clock(now_ms)?
            ],
        ))?;
        sql(tx.execute(
            "UPDATE tasks SET state=?2,updated_ms=?3 WHERE id=?1",
            params![
                action.task_id.to_string(),
                encode(&TaskState::Running)?,
                clock(now_ms)?
            ],
        ))?;
        if let Some(approval) = approval {
            sql(tx.execute(
                "UPDATE ledger_approvals SET consumed=1 WHERE id=?1",
                [approval.id.to_string()],
            ))?;
        }
        event(
            &tx,
            action.task_id,
            Some(step),
            "dispatched",
            clock(now_ms)?,
        )?;
        sql(tx.commit())?;
        Ok(DispatchPermit {
            dispatch_id,
            action,
            device_id: session.device_id,
            session_id: session.session_id,
            capture_epoch: session.capture_epoch,
            action_epoch: session.action_epoch,
        })
    }

    /// An adapter supplies a verified outcome for this exact dispatch. Unknown
    /// effect never grants retry; late replies after recovery are rejected.
    pub fn finish_action(
        &mut self,
        dispatch: Uuid,
        session: &DispatchSession,
        outcome: Outcome,
        now_ms: u64,
    ) -> Result<(), ErrorCode> {
        self.finish_observed_action(dispatch, session, outcome, None, now_ms)
    }
    pub fn finish_observed_action(
        &mut self,
        dispatch: Uuid,
        session: &DispatchSession,
        outcome: Outcome,
        observation: Option<&crate::execution::EffectObservation>,
        now_ms: u64,
    ) -> Result<(), ErrorCode> {
        if dispatch.is_nil()
            || !session.active
            || [session.actor_id, session.device_id, session.session_id]
                .iter()
                .any(Uuid::is_nil)
        {
            return Err(ErrorCode::Unauthenticated);
        }
        let tx = sql(self.connection.transaction())?;
        let matches:bool=sql(tx.query_row("SELECT EXISTS(SELECT 1 FROM dispatch_bindings WHERE dispatch_id=?1 AND actor_id=?2 AND device_id=?3 AND session_id=?4 AND capture_epoch=?5 AND action_epoch=?6)",params![dispatch.to_string(),session.actor_id.to_string(),session.device_id.to_string(),session.session_id.to_string(),session.capture_epoch.to_string(),session.action_epoch.to_string()],|r|r.get(0)))?;
        if !matches {
            return Err(ErrorCode::Stale);
        }
        let (body,state):(String,String)=sql(tx.query_row("SELECT a.body,s.state FROM action_revisions a JOIN steps s ON s.id=a.step_id WHERE a.dispatch_id=?1",[dispatch.to_string()],|r|Ok((r.get(0)?,r.get(1)?))))?;
        let action: Action = decode(&body)?;
        if action.actor_id != session.actor_id || decode::<TaskState>(&state)? != TaskState::Running
        {
            return Err(ErrorCode::InvalidTransition);
        }
        if let Some(observation) = observation {
            observation.validate(&action, outcome)?;
            sql(tx.execute(
                "INSERT INTO native_observations VALUES(?1,?2,?3,?4,?5)",
                params![
                    dispatch.to_string(),
                    action.target_id.to_string(),
                    action.revision.to_string(),
                    encode(observation)?,
                    clock(now_ms)?
                ],
            ))?;
        }
        // Immutable original outcome: later reconciliation cannot rewrite this
        // into invented observed success. Legacy observations have no row.
        sql(tx.execute(
            "INSERT INTO native_finalizations VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                dispatch.to_string(),
                action.target_id.to_string(),
                action.revision.to_string(),
                action.actor_id.to_string(),
                encode(&outcome)?,
                clock(now_ms)?
            ],
        ))?;
        let next = match outcome {
            Outcome::Success => TaskState::Succeeded,
            Outcome::Failed | Outcome::Unsupported => TaskState::Failed,
            Outcome::NeedsInput => TaskState::WaitingForUser,
            Outcome::Cancelled => TaskState::Cancelled,
            Outcome::UnknownEffect => TaskState::UnknownEffect,
        };
        sql(tx.execute(
            "UPDATE steps SET state=?2,updated_ms=?3 WHERE id=?1",
            params![action.step_id.to_string(), encode(&next)?, clock(now_ms)?],
        ))?;
        let remaining: bool = sql(tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM steps WHERE task_id=?1 AND state!=?2)",
            params![action.task_id.to_string(), encode(&TaskState::Succeeded)?],
            |r| r.get(0),
        ))?;
        let cancelled: bool = sql(tx.query_row(
            "SELECT cancel_requested FROM accepted_intents WHERE task_id=?1",
            [action.task_id.to_string()],
            |r| r.get(0),
        ))?;
        let task_next =
            if cancelled && !matches!(next, TaskState::UnknownEffect | TaskState::WaitingForUser) {
                TaskState::Cancelled
            } else if next == TaskState::Succeeded && remaining {
                TaskState::Queued
            } else {
                next
            };
        sql(tx.execute(
            "UPDATE tasks SET state=?2,updated_ms=?3 WHERE id=?1",
            params![
                action.task_id.to_string(),
                encode(&task_next)?,
                clock(now_ms)?
            ],
        ))?;
        event(
            &tx,
            action.task_id,
            Some(action.step_id),
            match outcome {
                Outcome::Success => "verified_success",
                Outcome::Failed => "failed",
                Outcome::NeedsInput => "needs_input",
                Outcome::Cancelled => "cancelled",
                Outcome::Unsupported => "unsupported",
                Outcome::UnknownEffect => "unknown_effect",
            },
            clock(now_ms)?,
        )?;
        sql(tx.commit())
    }

    /// Returns dispatch IDs requiring cooperative cancellation. Running effects
    /// remain unresolved until the adapter acknowledges or reconciliation occurs.
    pub fn cancel_task(
        &mut self,
        task: Uuid,
        actor: Uuid,
        now_ms: u64,
    ) -> Result<Vec<Uuid>, ErrorCode> {
        let tx = sql(self.connection.transaction())?;
        let dispatches = cancel_task_in(&tx, task, actor, now_ms)?;
        sql(tx.commit())?;
        Ok(dispatches)
    }
}

/// Shared transaction primitive for source-turn cancellation; never commits independently.
pub(crate) fn cancel_task_in(
    tx: &Transaction<'_>,
    task: Uuid,
    actor: Uuid,
    now_ms: u64,
) -> Result<Vec<Uuid>, ErrorCode> {
    let owner: String = sql(tx.query_row(
        "SELECT actor_id FROM tasks WHERE id=?1",
        [task.to_string()],
        |r| r.get(0),
    ))?;
    if actor.is_nil() || owner != actor.to_string() {
        return Err(ErrorCode::Denied);
    }
    sql(tx.execute(
        "UPDATE accepted_intents SET cancel_requested=1 WHERE task_id=?1",
        [task.to_string()],
    ))?;
    let dispatches = {
        let mut statement=sql(tx.prepare("SELECT a.dispatch_id FROM action_revisions a JOIN action_heads h ON h.revision=a.revision JOIN steps s ON s.id=h.step_id WHERE s.task_id=?1 AND s.state=?2"))?;
        let rows = sql(statement.query_map(
            params![task.to_string(), encode(&TaskState::Running)?],
            |r| r.get::<_, String>(0),
        ))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(Uuid::parse_str(&sql(row)?).map_err(|_| ErrorCode::Malformed)?);
        }
        ids
    };
    sql(tx.execute("UPDATE action_revisions SET cancel_requested=1 WHERE step_id IN(SELECT id FROM steps WHERE task_id=?1)",[task.to_string()]))?;
    sql(tx.execute(
        "UPDATE steps SET state=?2,updated_ms=?3 WHERE task_id=?1 AND state IN(?4,?5,?6,?7)",
        params![
            task.to_string(),
            encode(&TaskState::Cancelled)?,
            clock(now_ms)?,
            encode(&TaskState::Proposed)?,
            encode(&TaskState::Queued)?,
            encode(&TaskState::AwaitingApproval)?,
            encode(&TaskState::Suspended)?
        ],
    ))?;
    if dispatches.is_empty() {
        sql(tx.execute(
            "UPDATE tasks SET state=?2,updated_ms=?3 WHERE id=?1 AND state NOT IN(?4,?5,?6)",
            params![
                task.to_string(),
                encode(&TaskState::Cancelled)?,
                clock(now_ms)?,
                encode(&TaskState::Succeeded)?,
                encode(&TaskState::Failed)?,
                encode(&TaskState::UnknownEffect)?
            ],
        ))?;
    }
    event(tx, task, None, "cancel_requested", clock(now_ms)?)?;
    Ok(dispatches)
}
