//! Native accepted-fact reply and exact deletion continuation.
use super::*;
pub struct PendingMemory {
    pub claim: PlannerClaim,
    pub deletion: crate::memory::conversation::Deletion,
}
pub enum MemoryAnswer {
    Reply(Box<StoredReply>),
    Pending(Box<PendingMemory>),
}
impl Store {
    pub fn finish_memory_answer(
        &mut self,
        claim: PlannerClaim,
        approved: Option<crate::memory::conversation::Deletion>,
        apps: &crate::apps::AppCatalog,
        authorize: &mut dyn FnMut(&PlannerAuthority<'_>) -> Result<(), ErrorCode>,
    ) -> Result<MemoryAnswer, ErrorCode> {
        let proposal = (|| {
            claim.cancellation.check()?;
            remaining(claim.started)?;
            if approved.is_none() {
                let tx = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| ErrorCode::Storage)?;
                let (record, state) = super::super::tasks::read_record(&tx, claim.context().turn)?;
                let (plan, plan_state) = read_plan(&tx, &record)?.ok_or(ErrorCode::Stale)?;
                if state != "planning"
                    || plan_state != "pending"
                    || plan != claim.plan
                    || super::super::tasks::linked(&tx, &record)?.is_some()
                    || read_reply(&tx, &plan)?.is_some()
                {
                    return Err(ErrorCode::Stale);
                }
                let source = crate::memory::AcceptedSource {
                    turn: record.id,
                    revision: record.revision,
                };
                let pending = crate::memory::conversation::deletion(&tx, record.actor, &source)?;
                authorize(&PlannerAuthority {
                    context: claim.context(),
                    binding: claim.binding(),
                })?;
                claim.cancellation.check()?;
                remaining(claim.started)?;
                tx.commit().map_err(|_| ErrorCode::Storage)?;
                claim.cancellation.check()?;
                remaining(claim.started)?;
                return Ok(pending);
            }
            Ok(None)
        })();
        let proposal = match proposal {
            Ok(value) => value,
            Err(error) => {
                self.retire_planner(claim.retirement())?;
                return Err(error);
            }
        };
        if let Some(deletion) = proposal {
            return Ok(MemoryAnswer::Pending(Box::new(PendingMemory {
                claim,
                deletion,
            })));
        }
        self.finish_reply(claim, ReplyInput::Memory(approved), apps, authorize)
            .map(|reply| MemoryAnswer::Reply(Box::new(reply)))
    }
}
