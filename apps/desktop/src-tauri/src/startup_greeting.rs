//! One attempt from native startup, never from a page, reconnect or voice change.
use crate::{Runtime, connection, preview};
use avesra_contracts::preview::Greeting;
use std::{sync::atomic::Ordering, time::Duration};
use tauri::{Emitter, Manager};

pub(crate) async fn run(app: tauri::AppHandle, generation: u64, epoch: u64) {
    // Expected context was captured before saved credential loading.
    let state = app.state::<Runtime>();
    let current = || {
        state.local.lock().is_ok_and(|local| {
            !local.locked
                && !local.settings.deafened
                && !local.settings.paused
                && local.settings.speaker.is_some()
                && local.playback_epoch == epoch
                && state.connection_generation.load(Ordering::SeqCst) == generation
                && matches!(
                    local.connection_phase,
                    avesra_core::state::ConnectionPhase::Connecting
                        | avesra_core::state::ConnectionPhase::Connected
                )
        })
    };
    let prepare = async {
        let session = loop {
            if !current() {
                return Ok(None);
            }
            let ack = *state
                .acknowledged_session
                .lock()
                .map_err(|_| "Session unavailable")?;
            if let Some(session) = ack
                && session.generation == generation
                && session.playback_epoch == epoch
                && state.local.lock().is_ok_and(|local| local.connected)
            {
                break session;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "Pairing unavailable")?;
        let record = tokio::task::spawn_blocking(move || connection::load(&directory))
            .await
            .map_err(|_| "Pairing reader stopped")??;
        if !current() {
            return Ok(None);
        }
        let status = connection::greeting_voice_status(&record, session).await?;
        let connection::VoiceResult::Status(status) = status else {
            return Ok(None);
        };
        if status.selection_state != "available"
            || status.active_state != "available"
            || status.selected != status.active_voice
        {
            return Ok(None);
        }
        let Some(voice) = status.selected else {
            return Ok(None);
        };
        let owner_name = match crate::owner::current_actor(&app).await {
            Ok(actor) => state.local.lock().ok().and_then(|local| {
                local
                    .settings
                    .owner_name
                    .as_ref()
                    .filter(|memory| memory.actor == actor && Greeting::valid_name(&memory.name))
                    .map(|memory| memory.name.clone())
            }),
            Err(_) => None,
        };
        if !current() {
            return Ok(None);
        }
        Ok::<_, String>(Some((session, voice, Greeting { owner_name })))
    };
    // Stop/deafen/disconnect cancels preparation as well as actual playback.
    let cancelled = async {
        loop {
            if !current() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let prepared = tokio::select! {
        biased;
        _ = cancelled => return,
        result = tokio::time::timeout(Duration::from_secs(60), prepare) => result,
    };
    if let Ok(Ok(Some((session, voice, greeting)))) = prepared
        && let Err(error) = preview::greet(app.clone(), session, voice, greeting).await
    {
        let _ = app.emit(
            "runtime-error",
            format!("Startup greeting unavailable: {error}"),
        );
    }
}
