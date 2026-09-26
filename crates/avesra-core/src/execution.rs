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
    Vpn {
        report: Box<crate::vpn::Report>,
    },
    PromptDraft {
        app_id: uuid::Uuid,
        project_id: uuid::Uuid,
        binding_revision: uuid::Uuid,
        text_sha256: [u8; 32],
        fresh_composer: bool,
        submitted: bool,
    },
    Diagnostic {
        report: Box<crate::diagnostics::Report>,
    },
    BrowserRead {
        observation: Box<crate::browser_reading::Observation>,
    },
    PackagedApplication {
        app_id: Uuid,
        catalog_revision: Uuid,
        aumid: String,
        package_full_name: String,
        publisher_id: String,
        process_id: Option<u32>,
        process_created: Option<u64>,
        window: Option<u64>,
        package_matched: bool,
        focus_verified: Option<bool>,
    },
    Application {
        app_id: Uuid,
        catalog_revision: Uuid,
        process_id: Option<u32>,
        window: Option<u64>,
        image_matched: bool,
        #[serde(default)]
        focus_verified: Option<bool>,
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
        if matches!(self, Self::BrowserRead { .. })
            && serde_json::to_vec(self)
                .map_err(|_| ErrorCode::Malformed)?
                .len()
                > 4096
        {
            return Err(ErrorCode::TooLarge);
        }
        match self {
            Self::Vpn { report } => report.validate(action, outcome),
            Self::PromptDraft {
                app_id,
                project_id,
                binding_revision,
                text_sha256,
                fresh_composer,
                submitted,
            } => {
                use sha2::{Digest, Sha256};
                let avesra_contracts::ActionPayload::FillPrompt {
                    app_id: app,
                    project_id: project,
                    text,
                } = &action.payload
                else {
                    return Err(ErrorCode::Malformed);
                };
                let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
                if app_id != app
                    || project_id != project
                    || action.target_id != *project_id
                    || binding_revision.is_nil()
                    || *text_sha256 != digest
                    || !*fresh_composer
                    || *submitted
                    || outcome != Outcome::Success
                {
                    return Err(ErrorCode::Malformed);
                }
                Ok(())
            }
            Self::Diagnostic { report } => {
                report.validate()?;
                if action.payload
                    != (avesra_contracts::ActionPayload::Diagnostic {
                        catalog_entry: report.catalog().id(),
                    })
                    || action.target_id != report.catalog().id()
                    || outcome != Outcome::Success
                {
                    return Err(ErrorCode::Malformed);
                }
                Ok(())
            }
            Self::BrowserRead { observation } => observation.validate(action, outcome),
            Self::PackagedApplication {
                app_id,
                catalog_revision,
                aumid,
                package_full_name,
                publisher_id,
                process_id,
                process_created,
                window,
                package_matched,
                focus_verified,
            } => {
                if action.payload
                    != (avesra_contracts::ActionPayload::LaunchApp { app_id: *app_id })
                    || action.target_id != *app_id
                    || catalog_revision.is_nil()
                    || aumid.len() > 512
                    || package_full_name.len() > 512
                    || publisher_id.len() > 128
                    || !crate::apps::package_identity(aumid, package_full_name, publisher_id)
                    || process_id == &Some(0)
                    || process_created == &Some(0)
                    || window == &Some(0)
                    || (*package_matched && (process_id.is_none() || process_created.is_none()))
                    || (window.is_some() && !package_matched)
                    || (outcome == Outcome::Success
                        && (window.is_none() || !package_matched || *focus_verified == Some(false)))
                {
                    return Err(ErrorCode::Malformed);
                }
                Ok(())
            }
            Self::Application {
                app_id,
                catalog_revision,
                process_id,
                window,
                image_matched,
                focus_verified,
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
                    || (outcome == Outcome::Success && *focus_verified == Some(false))
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
    /// Native lifetime withdrawal only; supplying a signal cannot grant dispatch.
    pub fn from_signal(signal: Arc<AtomicBool>) -> Self {
        Self(signal)
    }
    /// The control path invalidates this synchronously, without waiting for COM.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
/// Created only by the durable execution owner. Repeated checks cannot consume
/// or renew the original action's single first-write boundary.
pub struct EffectAuthority<'a> {
    store: &'a Store,
    permit: &'a DispatchPermit,
    session: &'a DispatchSession,
    cancellation: &'a Cancellation,
    started: Instant,
    admitted_at: u64,
    lifetime: Duration,
    committed: bool,
    read_admitted: bool,
}
impl EffectAuthority<'_> {
    pub fn current(&mut self) -> Result<(), ErrorCode> {
        if matches!(
            self.permit.action.payload,
            avesra_contracts::ActionPayload::ReadPage { .. }
                | avesra_contracts::ActionPayload::InspectBrowserProvider { .. }
        ) {
            return Err(ErrorCode::Unsupported);
        }
        let now = now_ms()?;
        let elapsed_ms =
            u64::try_from(self.started.elapsed().as_millis()).map_err(|_| ErrorCode::Expired)?;
        if self.cancellation.is_cancelled()
            || self.started.elapsed() >= self.lifetime
            || now.abs_diff(self.admitted_at.saturating_add(elapsed_ms)) > 1000
        {
            return Err(ErrorCode::Stale);
        }
        self.store
            .validate_dispatch(self.permit, self.session, now)?;
        if self.cancellation.is_cancelled() || self.started.elapsed() >= self.lifetime {
            return Err(ErrorCode::Stale);
        }
        self.read_admitted = true;
        Ok(())
    }
    pub fn commit(&mut self) -> Result<(), ErrorCode> {
        if self.committed
            || matches!(
                self.permit.action.payload,
                avesra_contracts::ActionPayload::Diagnostic { .. }
            )
        {
            return Err(ErrorCode::Denied);
        }
        self.current()?;
        self.committed = true;
        Ok(())
    }
}
pub trait EffectAdapter {
    /// Writes call authority.commit exactly once immediately before the first
    /// mutation. Read catalog adapters only call current. Every postcommit error
    /// is an unknown effect, never a retry.
    fn execute(
        &mut self,
        permit: &DispatchPermit,
        authority: &mut EffectAuthority<'_>,
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
pub(crate) fn now_ms() -> Result<u64, ErrorCode> {
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
    pub fn begin_browser_read<'a>(
        &'a mut self,
        step: Uuid,
        session: &DispatchSession,
        cancellation: &Cancellation,
    ) -> Result<crate::browser_execution::ReadExecution<'a>, ErrorCode> {
        crate::browser_execution::ReadExecution::begin(&mut self.store, step, session, cancellation)
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
        let mut authority = EffectAuthority {
            store: &self.store,
            permit: &permit,
            session,
            cancellation,
            started,
            admitted_at,
            lifetime,
            committed: false,
            read_admitted: false,
        };
        let mut result = adapter.execute(&permit, &mut authority);
        let read = matches!(
            permit.action.payload,
            avesra_contracts::ActionPayload::Diagnostic { .. }
        );
        // Preserve the evidence that the adapter itself obtained admission. The
        // final publication check must not manufacture a previously missing read.
        let read_admitted = authority.read_admitted;
        if read && result.is_ok() {
            result = result.and_then(|value| {
                authority.current()?;
                Ok(value)
            });
        }
        let committed = authority.committed;
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
            (false, Ok(Outcome::Success)) if read && read_admitted => Outcome::Success,
            (false, Ok(Outcome::Unsupported)) => Outcome::Unsupported,
            (false, Ok(Outcome::NeedsInput)) => Outcome::NeedsInput,
            (false, _) => Outcome::Failed,
        };
        // This records the original dispatch's observed result even if its
        // lease was cancelled after commit. It does not authorize another effect.
        let observation =
            observation.filter(|value| value.validate(&permit.action, outcome).is_ok());
        if read && outcome == Outcome::Success {
            let mut current = || {
                let elapsed =
                    u64::try_from(started.elapsed().as_millis()).map_err(|_| ErrorCode::Expired)?;
                if cancellation.is_cancelled()
                    || started.elapsed() >= lifetime
                    || now_ms()?.abs_diff(admitted_at.saturating_add(elapsed)) > 1000
                {
                    return Err(ErrorCode::Stale);
                }
                Ok(())
            };
            self.store.finish_observed_action_checked(
                permit.dispatch_id,
                session,
                outcome,
                observation.as_ref(),
                now_ms()?,
                Some(crate::ledger::ReadFinalCheck {
                    permit: &permit,
                    current: &mut current,
                }),
            )?;
        } else {
            self.store.finish_observed_action(
                permit.dispatch_id,
                session,
                outcome,
                observation.as_ref(),
                now_ms()?,
            )?;
        }
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
