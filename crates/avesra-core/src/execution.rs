//! Synchronous durable effect ownership. Run on the one native effect worker,
//! never on the UI thread. Dropping a reply receiver does not release ownership.
use crate::{
    ledger::{DispatchPermit, DispatchSession},
    store::Store,
};
use avesra_contracts::{ErrorCode, MAX_ACTION_AGE_MS, Outcome};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeLevel {
    pub scalar: f32,
    pub muted: bool,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectObservation {
    Application {
        app_id: Uuid,
        catalog_revision: Uuid,
        process_id: Option<u32>,
        window: Option<u64>,
        image_matched: bool,
    },
    Volume {
        before: VolumeLevel,
        after: Option<VolumeLevel>,
    },
}
impl EffectObservation {
    pub fn validate(
        &self,
        action: &avesra_contracts::Action,
        outcome: Outcome,
    ) -> Result<(), ErrorCode> {
        match self {
            Self::Application {
                app_id,
                catalog_revision,
                process_id,
                window,
                image_matched,
            } => {
                if action.payload
                    != (avesra_contracts::ActionPayload::LaunchApp { app_id: *app_id })
                    || action.target_id != *app_id
                    || catalog_revision.is_nil()
                    || process_id == &Some(0)
                    || window == &Some(0)
                    || (window.is_some() && (!image_matched || process_id.is_none()))
                    || (outcome == Outcome::Success
                        && (window.is_none() || process_id.is_none() || !image_matched))
                {
                    return Err(ErrorCode::Malformed);
                }
                Ok(())
            }
            Self::Volume { before, after } => {
                let avesra_contracts::ActionPayload::SetVolume { percent } = action.payload else {
                    return Err(ErrorCode::Denied);
                };
                if !before.scalar.is_finite()
                    || !(0.0..=1.0).contains(&before.scalar)
                    || after
                        .as_ref()
                        .is_some_and(|v| !v.scalar.is_finite() || !(0.0..=1.0).contains(&v.scalar))
                {
                    return Err(ErrorCode::Malformed);
                }
                if outcome == Outcome::Success
                    && !after.as_ref().is_some_and(|v| {
                        (v.scalar - f32::from(percent) / 100.0).abs() <= 0.0001
                            && v.muted == before.muted
                    })
                {
                    return Err(ErrorCode::Malformed);
                }
                Ok(())
            }
        }
    }
}
pub struct EffectResult {
    pub outcome: Outcome,
    pub observation: Option<EffectObservation>,
}

#[derive(Clone)]
pub struct Cancellation(Arc<AtomicBool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Cancellation {
    /// The control path invalidates this synchronously, without waiting for COM.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
pub trait EffectAdapter {
    /// Must call authorize_commit exactly once immediately before the first OS
    /// write. An error after that call is an unknown effect, never a retry.
    fn execute(
        &mut self,
        permit: &DispatchPermit,
        authorize_commit: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<EffectResult, ErrorCode>;
}
pub struct ExecutionReceipt {
    pub dispatch_id: Uuid,
    pub target_id: Uuid,
    pub action_revision: Uuid,
    pub outcome: Outcome,
    pub crossed_commit_boundary: bool,
    pub observation: Option<EffectObservation>,
}
fn now_ms() -> Result<u64, ErrorCode> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrorCode::Expired)?
            .as_millis(),
    )
    .map_err(|_| ErrorCode::Expired)
}
/// Owns its ledger throughout claim, native work and outcome persistence. The
/// worker must retain this value while an OS call is blocked; there is no timeout
/// mechanism that detaches native work or releases its input slot early.
pub struct ExecutionController {
    store: Store,
}
impl ExecutionController {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
    /// Native authenticated management only. Never deserialize management
    /// authority from model proposals or expose this store through a webview.
    pub fn management(&mut self) -> &mut Store {
        &mut self.store
    }
    pub fn execute(
        &mut self,
        step: Uuid,
        session: &DispatchSession,
        cancellation: &Cancellation,
        adapter: &mut dyn EffectAdapter,
    ) -> Result<ExecutionReceipt, ErrorCode> {
        if cancellation.is_cancelled() {
            return Err(ErrorCode::Stale);
        }
        let admitted_at = now_ms()?;
        let started = Instant::now();
        let permit = self.store.claim_action(step, session, admitted_at)?;
        let lifetime = Duration::from_millis(
            permit
                .action
                .expires_at_ms
                .saturating_sub(admitted_at)
                .min(MAX_ACTION_AGE_MS),
        );
        let mut committed = false;
        let mut authorize = || {
            let now = now_ms()?;
            let elapsed_ms =
                u64::try_from(started.elapsed().as_millis()).map_err(|_| ErrorCode::Expired)?;
            if committed
                || cancellation.is_cancelled()
                || started.elapsed() >= lifetime
                || now.abs_diff(admitted_at.saturating_add(elapsed_ms)) > 1000
            {
                return Err(ErrorCode::Stale);
            }
            self.store.validate_dispatch(&permit, session, now)?;
            // Recheck after bounded-but-blocking storage work. No runtime/UI lock
            // is held while the adapter calls COM.
            if cancellation.is_cancelled() || started.elapsed() >= lifetime {
                return Err(ErrorCode::Stale);
            }
            committed = true;
            Ok(())
        };
        let result = adapter.execute(&permit, &mut authorize);
        let observation = result
            .as_ref()
            .ok()
            .and_then(|value| value.observation.clone());
        let result = result.and_then(|value| {
            if let Some(observation) = &value.observation {
                observation.validate(&permit.action, value.outcome)?;
            }
            if value.outcome == Outcome::Success && value.observation.is_none() {
                return Err(ErrorCode::Malformed);
            }
            Ok(value.outcome)
        });
        let outcome = match (committed, result) {
            (true, Ok(Outcome::Success)) => Outcome::Success,
            (true, _) => Outcome::UnknownEffect,
            (false, _) if cancellation.is_cancelled() => Outcome::Cancelled,
            (false, Ok(Outcome::Unsupported)) => Outcome::Unsupported,
            (false, _) => Outcome::Failed,
        };
        // This records the original dispatch's observed result even if its
        // lease was cancelled after commit. It does not authorize another effect.
        let observation =
            observation.filter(|value| value.validate(&permit.action, outcome).is_ok());
        self.store.finish_observed_action(
            permit.dispatch_id,
            session,
            outcome,
            observation.as_ref(),
            now_ms()?,
        )?;
        Ok(ExecutionReceipt {
            dispatch_id: permit.dispatch_id,
            target_id: permit.action.target_id,
            action_revision: permit.action.revision,
            outcome,
            crossed_commit_boundary: committed,
            observation,
        })
    }
}
