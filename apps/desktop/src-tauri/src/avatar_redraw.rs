//! Optional presentation transaction. No capture, source selection or voice grant.
use super::ManagementProof;
use crate::{
    Runtime,
    profiles::voice_avatar::{self, FinalizeError, Redraw, View},
};
use avesra_core::state::LocalState;
use serde::Serialize;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;

const CONFIRMATION: Duration = Duration::from_secs(30);
const OPERATION: Duration = Duration::from_secs(12);

#[derive(Default)]
pub(crate) struct State(Mutex<Option<Arc<Ticket>>>);
impl State {
    pub(super) fn invalidate(&self) {
        if let Ok(mut slot) = self.0.lock()
            && let Some(ticket) = slot.take()
        {
            ticket.context.cancelled.store(true, Ordering::SeqCst);
        }
    }
    fn remove(&self, id: Uuid) {
        if let Ok(mut slot) = self.0.lock()
            && slot.as_ref().is_some_and(|value| value.id == id)
        {
            slot.take();
        }
    }
}
struct Context {
    proof: ManagementProof,
    started: Instant,
    action: u64,
    microphone: String,
    controls: (bool, bool, bool),
    cancelled: Arc<AtomicBool>,
}
impl Context {
    fn current_locked(
        &self,
        state: &Runtime,
        local: &LocalState,
        operation: Instant,
    ) -> Result<(), String> {
        if self.cancelled.load(Ordering::SeqCst)
            || self.started.elapsed() >= CONFIRMATION
            || operation.elapsed() >= OPERATION
            || !self.proof.current_locked(state, local)
            || local.action_epoch != self.action
            || local.settings.microphone.as_ref() != Some(&self.microphone)
            || self.controls
                != (
                    local.settings.explicit_mute,
                    local.settings.deafened,
                    local.settings.paused,
                )
        {
            return Err(
                "Redraw expired or its Settings, owner, source or controls changed; prepare again"
                    .into(),
            );
        }
        Ok(())
    }
    fn current(&self, app: &tauri::AppHandle, operation: Instant) -> Result<(), String> {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        self.current_locked(&state, &local, operation)
    }
    fn remaining_ms(&self) -> u64 {
        self.proof.remaining_ms().min(
            CONFIRMATION
                .saturating_sub(self.started.elapsed())
                .as_millis() as u64,
        )
    }
}
struct Ticket {
    id: Uuid,
    proposal: Redraw,
    used: AtomicBool,
    context: Context,
}
#[derive(Serialize)]
pub(crate) struct Prepared {
    version: u16,
    ticket: Uuid,
    remaining_ms: u64,
    preview: View,
}
struct Caller {
    cancelled: Arc<AtomicBool>,
    complete: bool,
}
impl Drop for Caller {
    fn drop(&mut self) {
        if !self.complete {
            self.cancelled.store(true, Ordering::SeqCst);
        }
    }
}
fn visible(window: &tauri::WebviewWindow) -> Result<(), String> {
    super::settings_only(window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings visibility unavailable")?
    {
        return Err("Open Settings to redraw your voice avatar".into());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn prepare_voice_avatar_redraw(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Prepared, String> {
    let started = Instant::now();
    super::settings_only(&window)?;
    let state = app.state::<Runtime>();
    // Capture the original native proof before the synchronous visibility getter.
    let context = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        Context {
            proof: state
                .setup
                .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?,
            started,
            action: local.action_epoch,
            microphone: local
                .settings
                .microphone
                .clone()
                .ok_or("Choose a microphone first")?,
            controls: (
                local.settings.explicit_mute,
                local.settings.deafened,
                local.settings.paused,
            ),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    };
    visible(&window)?;
    context.current(&app, started)?;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let mut caller = Caller {
        cancelled: context.cancelled.clone(),
        complete: false,
    };
    let result = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        let state = app.state::<Runtime>();
        {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            context.current_locked(&state, &local, started)?;
            let mut slot = state
                .setup
                .redraw
                .0
                .lock()
                .map_err(|_| "Redraw state unavailable")?;
            if slot.as_ref().is_some_and(|ticket| {
                ticket
                    .context
                    .current_locked(&state, &local, started)
                    .is_ok()
            }) {
                return Err("Finish or cancel the pending redraw first".into());
            }
            if let Some(previous) = slot.take() {
                previous.context.cancelled.store(true, Ordering::SeqCst);
            }
        }
        let proposal = voice_avatar::prepare_redraw(&directory, &context.microphone, &mut || {
            context.current(&app, started)
        })?;
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        context.current_locked(&state, &local, started)?;
        let ticket = Arc::new(Ticket {
            id: Uuid::new_v4(),
            proposal,
            used: AtomicBool::new(false),
            context,
        });
        let response = Prepared {
            version: 1,
            ticket: ticket.id,
            remaining_ms: ticket.context.remaining_ms(),
            preview: ticket.proposal.view(),
        };
        *state
            .setup
            .redraw
            .0
            .lock()
            .map_err(|_| "Redraw state unavailable")? = Some(ticket);
        Ok(response)
    })
    .await
    .map_err(|_| "Redraw preparation owner stopped")?;
    caller.complete = result.is_ok();
    result
}

#[tauri::command]
pub(crate) async fn confirm_voice_avatar_redraw(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    ticket: Uuid,
) -> Result<View, String> {
    let started = Instant::now();
    visible(&window)?;
    let state = app.state::<Runtime>();
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let ticket = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let slot = state
            .setup
            .redraw
            .0
            .lock()
            .map_err(|_| "Redraw state unavailable")?;
        let value = slot
            .as_ref()
            .filter(|value| value.id == ticket)
            .ok_or("Redraw confirmation ended; prepare again")?;
        value.context.current_locked(&state, &local, started)?;
        value
            .used
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Redraw confirmation was already used; refresh saved state")?;
        value.clone()
    };
    let mut caller = Caller {
        cancelled: ticket.context.cancelled.clone(),
        complete: false,
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Profile directory unavailable")?;
    let result = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        let result = voice_avatar::confirm_redraw(
            &directory,
            &ticket.context.microphone,
            &ticket.proposal,
            &mut || ticket.context.current(&app, started),
            &mut |temporary: &Path, destination: &Path| {
                let state = app.state::<Runtime>();
                let local = state
                    .local
                    .lock()
                    .map_err(|_| FinalizeError::Failed("Local state unavailable".to_owned()))?;
                ticket
                    .context
                    .current_locked(&state, &local, started)
                    .map_err(FinalizeError::Withdrawn)?;
                std::fs::rename(temporary, destination).map_err(|_| {
                    FinalizeError::Failed(
                        "Avatar publication failed; refresh saved state before retrying".to_owned(),
                    )
                })
            },
        );
        app.state::<Runtime>().setup.redraw.remove(ticket.id);
        // Publication may have committed before a later withdrawal/lost response.
        // Never repeat it automatically; the caller must reread saved state.
        result.and_then(|view| {
            ticket.context.current(&app, started)?;
            Ok(view)
        })
    })
    .await
    .map_err(|_| "Redraw publication owner stopped; refresh saved state")?;
    caller.complete = result.is_ok();
    result
}

#[tauri::command]
pub(crate) fn cancel_voice_avatar_redraw(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    ticket: Uuid,
) -> Result<(), String> {
    super::settings_only(&window)?;
    let state = app.state::<Runtime>();
    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut slot = state
        .setup
        .redraw
        .0
        .lock()
        .map_err(|_| "Redraw state unavailable")?;
    if slot.as_ref().is_some_and(|value| value.id == ticket)
        && let Some(value) = slot.take()
    {
        value.context.cancelled.store(true, Ordering::SeqCst);
    }
    Ok(())
}
