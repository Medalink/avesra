//! Explicit protected deletion of a bounded, fully retired ordinary history closure.
use super::*;
use avesra_contracts::planner::retirement;
use avesra_core::conversations::deletion as core_delete;
use tokio::sync::OwnedMutexGuard;

struct Owners {
    _reader: OwnedMutexGuard<()>,
    _owner: OwnedMutexGuard<()>,
    _accepted: Option<OwnedMutexGuard<()>>,
}
fn owners(app: &tauri::AppHandle, commit: bool) -> Result<Arc<Owners>, String> {
    let state = app.state::<Runtime>();
    let reader = state
        .tasks
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "History reader is busy")?;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let accepted = if commit {
        Some(
            state
                .tasks
                .accepted
                .clone()
                .try_lock_owned()
                .map_err(|_| "An accepted turn is still active; retry after it settles")?,
        )
    } else {
        None
    };
    Ok(Arc::new(Owners {
        _reader: reader,
        _owner: owner,
        _accepted: accepted,
    }))
}
struct Bound {
    reader: Arc<Reader>,
    session: connection::SessionIdentity,
    challenge: u64,
    started: Instant,
    path: std::path::PathBuf,
}
impl Bound {
    fn current(&self, app: &tauri::AppHandle, present: &AtomicBool) -> Result<(), ErrorCode> {
        if !present.load(Ordering::SeqCst)
            || self.started.elapsed() >= Duration::from_secs(30)
            || app.state::<Runtime>().setup.challenge() != self.challenge
        {
            return Err(ErrorCode::Stale);
        }
        reader_current(app, &self.reader)?;
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
        let ack = state
            .acknowledged_session
            .lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if !local.connected
            || local.locked
            || local.action_epoch != self.session.action_epoch
            || state.connection_generation.load(Ordering::SeqCst) != self.session.generation
            || !ack.is_some_and(|s| {
                s.id == self.session.id
                    && s.device == self.session.device
                    && s.action_epoch == self.session.action_epoch
                    && s.generation == self.session.generation
                    && s.server_fingerprint == self.session.server_fingerprint
            })
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    fn operation(
        &self,
        app: &tauri::AppHandle,
        present: &AtomicBool,
        started: Instant,
    ) -> Result<(), ErrorCode> {
        if started.elapsed() >= Duration::from_secs(12) {
            return Err(ErrorCode::Expired);
        }
        self.current(app, present)
    }
    fn remaining_ms(&self) -> u64 {
        u64::try_from(
            Duration::from_secs(30)
                .saturating_sub(self.started.elapsed())
                .as_millis(),
        )
        .unwrap_or(0)
        .min(self.reader.proof.remaining_ms())
    }
}
pub(super) struct Ticket {
    id: Uuid,
    bound: Arc<Bound>,
    binding: actors::Binding,
    prepared: core_delete::Prepared,
}
#[derive(Serialize)]
pub struct Preview {
    ticket: Uuid,
    selected: Uuid,
    affected: Vec<core_delete::Affected>,
    remaining_ms: u64,
}
fn selected_reader(app: &tauri::AppHandle, panel: Uuid, id: Uuid) -> Result<Arc<Reader>, String> {
    let reader = app
        .state::<Runtime>()
        .tasks
        .history
        .reader
        .lock()
        .map_err(|_| "History reader unavailable")?
        .clone()
        .filter(|v| v.id == id && v.panel == panel)
        .ok_or("Reopen verified history before deleting")?;
    reader_current(app, &reader).map_err(|_| "History proof expired")?;
    Ok(reader)
}
async fn pairing(
    bound: &Arc<Bound>,
    owners: &Arc<Owners>,
    expected: Option<actors::Binding>,
) -> Result<connection::PairingRecord, String> {
    let bound = bound.clone();
    let owners = owners.clone();
    tokio::task::spawn_blocking(move || {
        let _owners = owners;
        if Binding::read(&bound.path)? != bound.reader.binding {
            return Err("History owner or pairing changed".into());
        }
        if let Some(expected) = expected {
            crate::planner::paired_owner(&bound.path, &expected, bound.session)
        } else {
            let pairing = connection::load(&bound.path)?;
            if pairing.device_id != bound.session.device
                || pairing.server_fingerprint()? != hex::encode(bound.session.server_fingerprint)
            {
                return Err("Original paired session changed".into());
            }
            Ok(pairing)
        }
    })
    .await
    .map_err(|_| "History identity reader stopped")?
}
#[tauri::command]
pub async fn prepare_conversation_deletion(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    reader: Uuid,
    turn: Uuid,
    revision: Uuid,
) -> Result<Preview, String> {
    let started = Instant::now();
    let challenge = app.state::<Runtime>().setup.challenge();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    visible(&window)?;
    let owners = owners(&app, false)?;
    let reader = selected_reader(&app, panel, reader)?;
    let session = app
        .state::<Runtime>()
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?
        .ok_or("Deletion requires the original live connection")?;
    if session.device != reader.binding.device
        || hex::encode(session.server_fingerprint) != reader.binding.server
    {
        return Err("Original paired session changed".into());
    }
    let bound = Arc::new(Bound {
        reader,
        session,
        challenge,
        started,
        path: app
            .path()
            .app_data_dir()
            .map_err(|_| "Private directory unavailable")?,
    });
    bound
        .operation(&app, &present, started)
        .map_err(|_| "Deletion preparation withdrawn")?;
    let work = tauri::async_runtime::spawn(async move {
        let paired = pairing(&bound, &owners, None).await?;
        bound
            .operation(&app, &present, started)
            .map_err(|_| "Deletion preparation withdrawn")?;
        let request = actors::Request {
            version: actors::VERSION,
            request: Uuid::new_v4(),
            attempt: Uuid::new_v4(),
            session: session.id,
            action_epoch: session.action_epoch,
            command: actors::Command::Status,
        };
        let status = tokio::time::timeout(
            Duration::from_millis(bound.remaining_ms().min(5000)),
            connection::actor_operation(&paired, &request),
        )
        .await
        .map_err(|_| "Owner registration inspection expired")??;
        let binding = status
            .binding
            .ok_or("Current owner registration unavailable")?;
        binding
            .validate()
            .map_err(|_| "Invalid owner registration")?;
        if binding.revoked
            || binding.actor != bound.reader.binding.actor
            || binding.owner_revision != bound.reader.binding.revision
            || binding.device != session.device
        {
            return Err("Owner registration changed".into());
        }
        let _ = pairing(&bound, &owners, Some(binding.clone())).await?;
        bound
            .operation(&app, &present, started)
            .map_err(|_| "Deletion preparation withdrawn")?;
        let selection = core_delete::Selection {
            current: retirement::Current {
                actor: binding.actor,
                device: session.device,
                registration_revision: binding.registration_revision,
                session: session.id,
                action_epoch: session.action_epoch,
            },
            owner_revision: binding.owner_revision,
            turn,
            revision,
        };
        let auth_app = app.clone();
        let auth_bound = bound.clone();
        let auth_present = present.clone();
        let auth_owners = owners.clone();
        let receiver = app
            .state::<Runtime>()
            .effects
            .prepare_history_deletion(
                selection,
                Box::new(move || {
                    let _owners = &auth_owners;
                    auth_bound.operation(&auth_app, &auth_present, started)
                }),
            )
            .map_err(|_| "History ledger worker busy")?;
        let prepared = receive(receiver).await?;
        bound
            .operation(&app, &present, started)
            .map_err(|_| "Deletion preparation expired")?;
        app.state::<Runtime>()
            .effects
            .history_sources_retired(prepared.contexts())
            .map_err(|_| "A reply still owns this history; wait for actual retirement")?;
        let ticket = Arc::new(Ticket {
            id: Uuid::new_v4(),
            bound,
            binding,
            prepared,
        });
        let view = Preview {
            ticket: ticket.id,
            selected: ticket.prepared.selected(),
            affected: ticket.prepared.affected().to_vec(),
            remaining_ms: ticket.bound.remaining_ms(),
        };
        if view.remaining_ms == 0 {
            return Err("Deletion confirmation expired".into());
        }
        *app.state::<Runtime>()
            .tasks
            .history
            .pending
            .lock()
            .map_err(|_| "Deletion confirmation unavailable")? = Some(ticket);
        Ok(view)
    });
    let result = work.await.map_err(|_| "Deletion preparation stopped")?;
    drop(caller);
    result
}
#[tauri::command]
pub async fn confirm_conversation_deletion(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    ticket: Uuid,
) -> Result<core_delete::Deleted, String> {
    let started = Instant::now();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    visible(&window)?;
    let owners = owners(&app, true)?;
    let ticket = app
        .state::<Runtime>()
        .tasks
        .history
        .pending
        .lock()
        .map_err(|_| "Deletion confirmation unavailable")?
        .take()
        .filter(|v| v.id == ticket && v.bound.reader.panel == panel)
        .ok_or("Deletion confirmation expired; prepare it again")?;
    ticket
        .bound
        .operation(&app, &present, started)
        .map_err(|_| "Deletion confirmation expired")?;
    let work = tauri::async_runtime::spawn(async move {
        let paired = pairing(&ticket.bound, &owners, Some(ticket.binding.clone())).await?;
        ticket
            .bound
            .operation(&app, &present, started)
            .map_err(|_| "Deletion withdrawn")?;
        app.state::<Runtime>()
            .effects
            .history_sources_retired(ticket.prepared.contexts())
            .map_err(|_| "Native reply ownership has not retired")?;
        let request = retirement::Request {
            version: retirement::VERSION,
            request: Uuid::new_v4(),
            current: retirement::Current {
                device: ticket.binding.device,
                actor: ticket.binding.actor,
                registration_revision: ticket.binding.registration_revision,
                session: ticket.bound.session.id,
                action_epoch: ticket.bound.session.action_epoch,
            },
            contexts: ticket.prepared.contexts().to_vec(),
            remaining_ms: ticket.bound.remaining_ms().min(retirement::MAX_BUDGET_MS),
        };
        let receipt = connection::retirement::inspect(&paired, &request, || {
            ticket
                .bound
                .operation(&app, &present, started)
                .map_err(|_| "Deletion withdrawn".into())
        })
        .await?;
        if receipt.observations.iter().any(|v| {
            !matches!(
                v.status,
                retirement::Status::Retired | retirement::Status::ClosedAndCompacted
            )
        }) {
            return Err(
                "Controller cannot prove every reply has retired; nothing was deleted".into(),
            );
        }
        let _ = pairing(&ticket.bound, &owners, Some(ticket.binding.clone())).await?;
        let auth_app = app.clone();
        let auth_ticket = ticket.clone();
        let auth_present = present.clone();
        let auth_owners = owners.clone();
        let committed_app = app.clone();
        let receiver = app
            .state::<Runtime>()
            .effects
            .delete_history(
                Box::new(move || {
                    committed_app.state::<Runtime>().tasks.history.invalidate();
                    let _ = committed_app.emit("conversation-history-changed", ());
                }),
                ticket.prepared.clone(),
                Box::new(move || {
                    let _owners = &auth_owners;
                    let _receipt = &receipt;
                    auth_ticket
                        .bound
                        .operation(&auth_app, &auth_present, started)?;
                    auth_app
                        .state::<Runtime>()
                        .effects
                        .history_sources_retired(auth_ticket.prepared.contexts())
                }),
            )
            .map_err(|_| "History ledger worker busy")?;
        let result = receive(receiver).await?;
        Ok(result)
    });
    let result = work
        .await
        .map_err(|_| "Deletion coordinator stopped; reopen history to inspect its state")?;
    drop(caller);
    result
}
