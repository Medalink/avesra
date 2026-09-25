use avesra_contracts::{Action, ActionPayload, ErrorCode, Operation};
use uuid::Uuid;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub operations: Vec<Operation>,
    pub revoked: bool,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub target_id: Uuid,
    pub action_revision: Uuid,
    pub intent_revision: Uuid,
    pub payload: ActionPayload,
    pub expires_at_ms: u64,
    pub authenticated_local: bool,
}
/// All fields must come from authenticated session state and the durable ledger,
/// never from the model proposal or webpage.
pub struct PolicyContext<'a> {
    pub actor_id: Uuid,
    pub accepted_task_id: Uuid,
    pub intent_revision: Uuid,
    /// Exact arguments resolved from the accepted owner request, never copied
    /// into authority from an untrusted model proposal.
    pub permitted_payloads: &'a [ActionPayload],
    pub now_ms: u64,
    pub grant: &'a Grant,
    pub approval: Option<&'a Approval>,
    pub explicit_submit: bool,
    pub session_active: bool,
}
impl PolicyContext<'_> {
    pub fn authorize(&self, action: &Action) -> Result<(), ErrorCode> {
        action.validate(self.now_ms)?;
        if !self.session_active || self.actor_id.is_nil() || self.accepted_task_id.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        if self.now_ms >= action.expires_at_ms
            || action.issued_at_ms > self.now_ms
            || action.expires_at_ms.saturating_sub(action.issued_at_ms)
                > avesra_contracts::MAX_ACTION_AGE_MS
        {
            return Err(ErrorCode::Expired);
        }
        if self.actor_id != action.actor_id
            || self.accepted_task_id != action.task_id
            || self.grant.revoked
            || self.grant.id != action.grant_id
            || self.grant.actor_id != action.actor_id
            || self.grant.target_id != action.target_id
            || self.intent_revision.is_nil()
            || self.intent_revision != action.intent_revision
            || !self.permitted_payloads.contains(&action.payload)
            || !self.grant.operations.contains(&action.payload.operation())
        {
            return Err(ErrorCode::Denied);
        }
        if action.payload.operation() == Operation::SubmitPrompt && !self.explicit_submit {
            return Err(ErrorCode::Denied);
        }
        if action.payload.operation().needs_approval() {
            let approval = self.approval.ok_or(ErrorCode::ApprovalRequired)?;
            if !approval.authenticated_local
                || action.approval_id != Some(approval.id)
                || approval.actor_id != action.actor_id
                || approval.task_id != action.task_id
                || approval.step_id != action.step_id
                || approval.target_id != action.target_id
                || approval.id.is_nil()
                || approval.action_revision != action.revision
                || approval.intent_revision != action.intent_revision
                || approval.payload != action.payload
                || self.now_ms >= approval.expires_at_ms
            {
                return Err(ErrorCode::ApprovalRequired);
            }
        }
        Ok(())
    }
}
