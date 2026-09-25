//! Native owner creation is separate from voice activation and permissions.
use crate::Runtime;
use avesra_contracts::ErrorCode;
use avesra_core::owner::ProtectedOwner;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::atomic::Ordering};
use tauri::Manager;
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u16,
    actor: Uuid,
    revision: Uuid,
    principal: String,
}
#[derive(Serialize)]
pub struct OwnerStatus {
    state: &'static str,
    actor: Option<Uuid>,
    revision: Option<Uuid>,
}
fn decode(value: ProtectedOwner) -> Result<Record, ErrorCode> {
    let bytes = avesra_windows::credentials::unprotect(&value.protected)?;
    if bytes.len() > 4096 {
        return Err(ErrorCode::TooLarge);
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| ErrorCode::Malformed)?;
    if record.version != 1
        || record.actor != value.actor
        || record.revision != value.revision
        || !avesra_windows::principal::valid_sid(&record.principal)
        || record.principal != avesra_windows::principal::current_user()?
    {
        return Err(ErrorCode::Unauthenticated);
    }
    Ok(record)
}
fn read(path: &Path) -> Result<OwnerStatus, ErrorCode> {
    let Some(record) = avesra_core::owner::load(path)? else {
        return Ok(OwnerStatus {
            state: "missing",
            actor: None,
            revision: None,
        });
    };
    let record = decode(record)?;
    Ok(OwnerStatus {
        state: "configured",
        actor: Some(record.actor),
        revision: Some(record.revision),
    })
}
pub fn matches_actor(directory: &Path, actor: Uuid) -> bool {
    read(&directory.join("owner.db")).is_ok_and(|v| v.actor == Some(actor))
}
pub async fn current_actor(app: &tauri::AppHandle) -> Result<Uuid, String> {
    let state = app.state::<Runtime>();
    let _owner = state
        .owner_setup
        .try_lock()
        .map_err(|_| "Owner management is busy")?;
    let context = state.setup.challenge();
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?
        .join("owner.db");
    let status = tokio::task::spawn_blocking(move || read(&path))
        .await
        .map_err(|_| "Owner reader stopped")?
        .map_err(|_| "Owner identity unavailable; no management change was made")?;
    if context != state.setup.challenge() {
        return Err("Setup changed while checking owner identity".into());
    }
    status
        .actor
        .ok_or("Create the authenticated local owner first".into())
}
#[tauri::command]
pub async fn owner_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<OwnerStatus, String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    let state = app.state::<Runtime>();
    let _owner = state
        .owner_setup
        .try_lock()
        .map_err(|_| "Owner management is busy")?;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?
        .join("owner.db");
    tokio::task::spawn_blocking(move || read(&path)).await.map_err(|_| "Owner status unavailable")?.map_err(|_| "Owner record is unavailable, corrupt or belongs to another Windows user; it was not replaced.".into())
}
#[tauri::command]
pub async fn create_owner(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<OwnerStatus, String> {
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use visible Settings".into());
    }
    let state = app.state::<Runtime>();
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let proof = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.locked || !local.connected {
            return Err("Connect Spark and unlock Windows first".to_string());
        }
        state
            .setup
            .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?
    };
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?
        .join("owner.db");
    // The accepted coordinator retains admission until durable work completes.
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let owned_app = app.clone();
        tokio::task::spawn_blocking(move || {
            let principal = avesra_windows::principal::current_user()?;
            if avesra_core::owner::load(&path)?.is_some() { return Err(ErrorCode::Denied); }
            let record = Record { version: 1, actor: Uuid::new_v4(), revision: Uuid::new_v4(), principal };
            let protected = avesra_windows::credentials::protect(&serde_json::to_vec(&record).map_err(|_| ErrorCode::Malformed)?)?;
            avesra_core::owner::create(&path, &ProtectedOwner { actor: record.actor, revision: record.revision, protected }, &mut || {
                if avesra_windows::principal::current_user()? != record.principal || !proof.current(&owned_app.state::<Runtime>()) { return Err(ErrorCode::Stale); }
                Ok(())
            })?;
            let status = read(&path)?;
            if status.actor != Some(record.actor) || status.revision != Some(record.revision) { return Err(ErrorCode::Stale); }
            Ok(status)
        }).await.map_err(|_| "Owner coordinator unavailable")?.map_err(|_| "Owner creation did not return a verified result. Refresh owner status before another explicit attempt.".to_string())
    }).await.map_err(|_| "Owner coordinator stopped")?
}
