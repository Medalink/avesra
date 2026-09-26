//! Actual Store worker owns all observation roster and candidate writes.
use super::*;
use avesra_core::demonstration::{Setup, Snapshot};
pub enum Operation {
    Read,
    PassiveRead,
    Passive {
        scope: Uuid,
        revision: Uuid,
        enabled: bool,
    },
    PassiveSave(Box<crate::apps::demonstration::Evidence>),
    Scope {
        alias: Uuid,
        revision: Uuid,
        excluded: bool,
    },
    Exclude {
        scope: Uuid,
        revision: Uuid,
    },
    Prepare {
        scope: Uuid,
        revision: Uuid,
    },
    Save(Box<crate::apps::demonstration::Evidence>),
    Change {
        id: Uuid,
        revision: Uuid,
        name: Option<String>,
    },
}
pub enum ResultValue {
    Snapshot(Snapshot),
    Setup(Box<Setup>),
    Done,
    Passive(Option<Box<avesra_core::demonstration::passive::Prepared>>),
}
pub struct Request {
    pub actor: Uuid,
    pub operation: Operation,
    pub authorize: CatalogAuthorization,
}
pub(super) fn execute(
    store: &mut Store,
    apps: &avesra_core::apps::AppCatalog,
    mut request: Request,
) -> Result<ResultValue, ErrorCode> {
    (request.authorize)()?;
    match request.operation {
        Operation::PassiveRead => {
            let value = store.passive_teaching(request.actor, apps)?;
            (request.authorize)()?;
            Ok(ResultValue::Passive(value.map(Box::new)))
        }
        Operation::Passive {
            scope,
            revision,
            enabled,
        } => {
            store.set_passive_teaching(
                request.actor,
                scope,
                revision,
                enabled,
                &mut request.authorize,
            )?;
            Ok(ResultValue::Done)
        }
        Operation::PassiveSave(evidence) => {
            let setting = evidence.passive().ok_or(ErrorCode::Denied)?;
            let deadline = evidence.passive_deadline()?;
            let value = evidence.candidate()?;
            if value.actor != request.actor {
                return Err(ErrorCode::Stale);
            }
            store.save_passive_demonstration(value, setting, apps, &mut || {
                if std::time::Instant::now() >= deadline {
                    return Err(ErrorCode::Expired);
                }
                (request.authorize)()
            })?;
            Ok(ResultValue::Done)
        }
        Operation::Read => {
            let value = store.teaching_snapshot(request.actor)?;
            (request.authorize)()?;
            Ok(ResultValue::Snapshot(value))
        }
        Operation::Exclude { scope, revision } => {
            store.exclude_teaching_scope(request.actor, scope, revision, &mut request.authorize)?;
            Ok(ResultValue::Done)
        }
        Operation::Prepare { scope, revision } => {
            let value = store.teaching_setup(request.actor, scope, revision, apps)?;
            (request.authorize)()?;
            Ok(ResultValue::Setup(Box::new(value)))
        }
        Operation::Scope {
            alias,
            revision,
            excluded,
        } => {
            store.teaching_scope(
                request.actor,
                alias,
                revision,
                excluded,
                apps,
                &mut request.authorize,
            )?;
            Ok(ResultValue::Done)
        }
        Operation::Save(evidence) => {
            let value = evidence.candidate()?;
            if value.actor != request.actor {
                return Err(ErrorCode::Stale);
            }
            store.save_demonstration(value, apps, &mut request.authorize)?;
            Ok(ResultValue::Done)
        }
        Operation::Change { id, revision, name } => {
            store.change_demonstration(
                request.actor,
                id,
                revision,
                name,
                &mut request.authorize,
            )?;
            Ok(ResultValue::Done)
        }
    }
}
impl NativeEffects {
    pub fn teaching_background(
        &self,
        request: Request,
    ) -> Result<Receiver<Result<ResultValue, ErrorCode>>, ErrorCode> {
        if !matches!(
            request.operation,
            Operation::PassiveRead | Operation::PassiveSave(_)
        ) {
            return Err(ErrorCode::Denied);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.background
            .try_send(Command::Teaching(Box::new(request), reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
    pub fn teaching(
        &self,
        request: Request,
    ) -> Result<Receiver<Result<ResultValue, ErrorCode>>, ErrorCode> {
        if matches!(
            &request.operation,
            Operation::Scope { .. } | Operation::Exclude { .. } | Operation::Change { .. }
        ) {
            let state = self.state.lock().map_err(|_| ErrorCode::Unavailable)?;
            if let Some(active) = state.active.as_ref().filter(|v| v.actor == request.actor) {
                active.cancellation.cancel();
            }
            for (target, signal) in &state.planners {
                if target.actor == request.actor {
                    signal.cancel();
                }
            }
            for source in &state.replies {
                if source.target.actor == request.actor {
                    source.signal.cancel();
                }
            }
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.send
            .try_send(Command::Teaching(Box::new(request), reply))
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(receive)
    }
}
