//! Local management proof stays native and never grants voice/action authority.
use crate::Runtime;
use serde::Serialize;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;

struct Proof {
    challenge: u64,
    generation: u64,
    epoch: u64,
    verified_at: Instant,
}
#[derive(Default)]
pub struct Setup {
    context: Mutex<Option<(u64, u64)>>,
    operation: Mutex<Option<avesra_windows::authentication::Verification>>,
    generation: AtomicU64,
    proof: Mutex<Option<Proof>>,
    pending: tokio::sync::Mutex<()>,
}
#[derive(Serialize)]
pub struct SetupStatus {
    local_authentication: &'static str,
    authentication_seconds_remaining: u64,
    enrollment: &'static str,
    reason: &'static str,
}
impl Setup {
    pub fn observe(&self, epoch: u64, connection: u64) {
        if let Ok(mut context) = self.context.lock()
            && *context != Some((epoch, connection))
        {
            *context = Some((epoch, connection));
            self.invalidate();
        }
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut proof) = self.proof.lock() {
            *proof = None;
        }
        if let Ok(operation) = self.operation.lock()
            && let Some(operation) = operation.as_ref()
        {
            operation.cancel();
        }
    }
}
fn settings_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use the local Settings window".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn setup_status(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
) -> Result<SetupStatus, String> {
    settings_only(&window)?;
    let (epoch, generation) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        (
            local.capture_epoch,
            state.connection_generation.load(Ordering::SeqCst),
        )
    };
    let remaining = {
        let mut stored = state.setup.proof.lock().map_err(|_| "Setup unavailable")?;
        if stored.as_ref().is_some_and(|proof| {
            proof.epoch != epoch
                || proof.challenge != state.setup.generation.load(Ordering::SeqCst)
                || proof.generation != generation
                || proof.verified_at.elapsed() >= Duration::from_secs(60)
        }) {
            *stored = None;
        }
        stored.as_ref().map_or(0, |proof| {
            60u64.saturating_sub(proof.verified_at.elapsed().as_secs())
        })
    };
    let authentication = if remaining > 0 {
        "verified"
    } else if avesra_windows::authentication::available()
        .await
        .map_err(|e| e.to_string())?
    {
        "available"
    } else {
        "unavailable"
    };
    Ok(SetupStatus {
        local_authentication: authentication,
        authentication_seconds_remaining: remaining,
        enrollment: "unavailable",
        reason: "Speaker enrollment capture and held-out quality validation are not ready. Verification does not enable listening.",
    })
}
#[tauri::command]
pub async fn verify_setup(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<SetupStatus, String> {
    settings_only(&window)?;
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Open Settings to verify".into());
    }
    let state = app.state::<Runtime>();
    let _pending = state
        .setup
        .pending
        .try_lock()
        .map_err(|_| "Local verification is already pending")?;
    state.setup.invalidate();
    if state
        .setup
        .operation
        .lock()
        .map_err(|_| "Setup unavailable")?
        .as_ref()
        .is_some_and(|operation| operation.running())
    {
        return Err("Previous Windows verification is still closing".into());
    }
    let challenge = state.setup.generation.load(Ordering::SeqCst);
    let (epoch, generation) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected || local.locked {
            return Err("Connect Spark before voice setup".into());
        }
        (
            local.capture_epoch,
            state.connection_generation.load(Ordering::SeqCst),
        )
    };
    let operation = avesra_windows::authentication::request_for_settings(
        window.hwnd().map_err(|_| "Settings window unavailable")?.0 as isize,
    )
    .map_err(|e| e.to_string())?;
    struct CancelOnDrop(avesra_windows::authentication::Verification);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            self.0.cancel();
        }
    }
    let cancel = CancelOnDrop(operation.clone());
    *state
        .setup
        .operation
        .lock()
        .map_err(|_| "Setup unavailable")? = Some(operation.clone());
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if state.setup.generation.load(Ordering::SeqCst) != challenge
            || state.connection_generation.load(Ordering::SeqCst) != generation
            || local.capture_epoch != epoch
            || !local.connected
            || local.locked
            || !window
                .is_visible()
                .map_err(|_| "Settings window unavailable")?
        {
            operation.cancel();
            return Err("Setup changed while Windows verification opened".into());
        }
    }
    tokio::time::timeout(Duration::from_secs(60), operation.verified())
        .await
        .map_err(|_| "Local verification timed out")?
        .map_err(|_| "Windows did not verify this user")?;
    drop(cancel);
    if !window
        .is_visible()
        .map_err(|_| "Settings window unavailable")?
    {
        return Err("Settings closed during verification".into());
    }
    {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if !local.connected
            || local.locked
            || local.capture_epoch != epoch
            || state.connection_generation.load(Ordering::SeqCst) != generation
            || state.setup.generation.load(Ordering::SeqCst) != challenge
        {
            return Err("Setup changed during verification; verify again when ready".into());
        }
        *state.setup.proof.lock().map_err(|_| "Setup unavailable")? = Some(Proof {
            challenge,
            generation,
            epoch,
            verified_at: Instant::now(),
        });
    }
    setup_status(window, app.state::<Runtime>()).await
}
#[tauri::command]
pub fn cancel_setup(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Runtime>,
) -> Result<(), String> {
    settings_only(&window)?;
    state.setup.invalidate();
    Ok(())
}
