//! Read/export actual current-owner telemetry; no work-producing debug ingress.
use crate::{Runtime, connection};
use avesra_core::{actor_intents, trace};
use serde::Serialize;
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;
#[derive(Serialize)]
pub struct View {
    local: trace::Snapshot,
    controller: Option<trace::Snapshot>,
    controller_error: Option<&'static str>,
    process_gate_totals: Option<std::collections::BTreeMap<crate::performance::GateReason, u64>>,
}
struct Caller(Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn current(
    window: &tauri::WebviewWindow,
    app: &tauri::AppHandle,
    challenge: u64,
    started: Instant,
    present: &AtomicBool,
) -> Result<(), String> {
    if !present.load(Ordering::SeqCst)
        || started.elapsed() > Duration::from_secs(12)
        || window.label() != "settings"
        || !window.is_visible().unwrap_or(false)
    {
        return Err("Trace inspection was withdrawn".into());
    }
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if local.locked || state.setup.challenge() != challenge {
        return Err("Trace inspection context changed".into());
    }
    Ok(())
}
async fn collect(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    turn: Option<Uuid>,
    export: bool,
    days: Option<u8>,
) -> Result<(Option<View>, Option<String>), String> {
    let started = Instant::now();
    let state = app.state::<Runtime>();
    let challenge = state.setup.challenge();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    current(&window, &app, challenge, started, &present)?;
    if turn.is_some_and(|id| id.is_nil()) || days.is_some_and(|d| !(1..=7).contains(&d)) {
        return Err("Invalid trace filter or retention".into());
    }
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner inspection is busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Private directory unavailable")?;
    let session = *state
        .acknowledged_session
        .lock()
        .map_err(|_| "Session unavailable")?;
    let work = tauri::async_runtime::spawn(async move {
        let read_path = directory.clone();
        let (actor, revision, pairing, intent, _owner) = tokio::task::spawn_blocking(move || {
            let (actor, revision) =
                crate::owner::identity(&read_path).map_err(|_| "Native owner unavailable")?;
            let pairing = connection::load(&read_path)?;
            let identity = actor_intents::Identity {
                server: pairing.server_fingerprint()?,
                device: pairing.device_id,
                actor,
                owner_revision: revision,
            };
            let intent = actor_intents::load(&read_path.join("actor-intents.db"), &identity)
                .map_err(|_| "Owner registration unavailable")?;
            Ok::<_, String>((actor, revision, pairing, intent, owner))
        })
        .await
        .map_err(|_| "Owner inspection stopped")??;
        current(&window, &app, challenge, started, &present)?;
        let device = pairing.device_id;
        if let Some(days) = days {
            trace::retention(days).map_err(|_| "Trace retention unavailable")?;
        }
        let local = tokio::task::spawn_blocking(move || trace::snapshot(actor, device, turn))
            .await
            .map_err(|_| "Trace reader stopped")?
            .map_err(|_| "Local traces unavailable")?;
        current(&window, &app, challenge, started, &present)?;
        let (controller, controller_error) =
            if let (Some(session), Some(intent)) = (session, intent) {
                let query = trace::Query {
                    version: 1,
                    request: Uuid::new_v4(),
                    session: session.id,
                    action_epoch: session.action_epoch,
                    actor,
                    owner_revision: revision,
                    registered_by: intent.request,
                    turn,
                };
                if session.device != device
                    || hex::encode(session.server_fingerprint) != pairing.server_fingerprint()?
                {
                    (None, Some("Pairing changed"))
                } else {
                    match connection::trace_query(&pairing, &query).await {
                        Ok(reply) => (Some(reply.snapshot), None),
                        Err(_) => (
                            None,
                            Some("Controller unavailable or current registration changed"),
                        ),
                    }
                }
            } else {
                (
                    None,
                    Some("Controller disconnected or owner registration unavailable"),
                )
            };
        current(&window, &app, challenge, started, &present)?;
        // Owner guard remains held through publication/export; re-read the actual
        // protected identity and pairing, never trust a webview owner UUID.
        let verify_path = directory.clone();
        let expected_server = pairing.server_fingerprint()?;
        tokio::task::spawn_blocking(move || {
            if crate::owner::identity(&verify_path).map_err(|_| "Owner unavailable")?
                != (actor, revision)
                || connection::load(&verify_path)?.device_id != device
                || connection::load(&verify_path)?.server_fingerprint()? != expected_server
            {
                return Err("Owner changed".into());
            }
            Ok::<_, String>(())
        })
        .await
        .map_err(|_| "Owner verification stopped")??;
        current(&window, &app, challenge, started, &present)?;
        let view = View {
            process_gate_totals: app.state::<Runtime>().performance.gate_counts(),
            local,
            controller,
            controller_error,
        };
        if !export {
            return Ok((Some(view), None));
        }
        let path = directory
            .join("telemetry-exports")
            .join(format!("accepted-traces-{}.json", Uuid::new_v4()));
        let final_path = path.clone();
        tokio::task::spawn_blocking(move || {
            let _owner = _owner;
            current(&window, &app, challenge, started, &present)?;
        let mut snapshots = vec![&view.local];
        if let Some(remote) = &view.controller {
            snapshots.push(remote);
        }
        let bytes=serde_json::to_vec_pretty(&serde_json::json!({"version":1,"clock_relation":"host_local_only_offset_unavailable","native":&view,"otlp":trace::otlp(&snapshots)})).map_err(|_|"Trace export encoding failed")?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Trace export exceeds bound".into());
        }

            current(&window,&app,challenge,started,&present)?;
            std::fs::create_dir_all(final_path.parent().ok_or("Export directory unavailable")?)
                .map_err(|_| "Export directory unavailable")?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&final_path)
                .map_err(|_| "Export file unavailable")?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Export write failed")?;
            Ok::<_, String>(())
        })
        .await
        .map_err(|_| "Trace export writer stopped")??;
        Ok((None, Some(path.to_string_lossy().into_owned())))
    });
    let result = work.await.map_err(|_| "Trace coordinator stopped")?;
    drop(caller);
    result
}
#[tauri::command]
pub async fn accepted_traces(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    turn: Option<Uuid>,
) -> Result<View, String> {
    collect(window, app, turn, false, None)
        .await?
        .0
        .ok_or("Trace result unavailable".into())
}
#[tauri::command]
pub async fn export_accepted_traces(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    turn: Option<Uuid>,
) -> Result<String, String> {
    collect(window, app, turn, true, None)
        .await?
        .1
        .ok_or("Trace export unavailable".into())
}
#[tauri::command]
pub async fn set_trace_retention(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    days: u8,
) -> Result<View, String> {
    collect(window, app, None, false, Some(days))
        .await?
        .0
        .ok_or("Trace result unavailable".into())
}
