//! Actual shared output ownership, including delayed native construction/drop.
use crate::Runtime;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};
use tokio::sync::OwnedMutexGuard;

pub(crate) struct Caller(pub Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
pub(crate) struct Owner {
    app: tauri::AppHandle,
    slot: Option<OwnedMutexGuard<()>>,
    attempt: Option<(u64, u64)>,
}
impl Owner {
    pub fn reserve(app: &tauri::AppHandle) -> Result<Self, String> {
        let slot = app
            .state::<Runtime>()
            .preview
            .clone()
            .try_lock_owned()
            .map_err(|_| "Another voice operation is still active")?;
        Ok(Self {
            app: app.clone(),
            slot: Some(slot),
            attempt: None,
        })
    }
    /// Set with Runtime.local held before fallible device admission. Owner itself
    /// was created outside that critical section, so unwinding releases local first.
    pub fn bind(&mut self, epoch: u64, generation: u64) {
        self.attempt = Some((epoch, generation));
    }
    pub fn withdraw(&self) -> Result<(), String> {
        if let Some(attempt) = self.attempt {
            stop(&self.app, attempt)?;
        }
        Ok(())
    }
    pub async fn finish(mut self) -> Result<(), String> {
        if let Some(attempt) = self.attempt {
            let ticket = stop(&self.app, attempt)?;
            wait(&self.app, ticket).await;
        }
        self.slot.take();
        Ok(())
    }
}
fn stop(app: &tauri::AppHandle, (epoch, generation): (u64, u64)) -> Result<u64, String> {
    let state = app.state::<Runtime>();
    let mut local = state
        .local
        .lock()
        .map_err(|_| "Output retirement state unavailable")?;
    if local.playback_epoch == epoch
        && state.connection_generation.load(Ordering::SeqCst) == generation
    {
        local.playback_epoch = local.playback_epoch.saturating_add(1);
        local.refresh();
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
    state.media.retirement_ticket()
}
async fn wait(app: &tauri::AppHandle, ticket: u64) {
    while !app.state::<Runtime>().media.retired(ticket) {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let Some(slot) = self.slot.take() else {
            return;
        };
        let Some(attempt) = self.attempt else {
            return;
        };
        let app = self.app.clone();
        // Detached cleanup is still the actual owner. No timeout releases the
        // shared slot while a Windows constructor/destructor may still be live.
        tauri::async_runtime::spawn(async move {
            let _slot = slot;
            match stop(&app, attempt) {
                Ok(ticket) => wait(&app, ticket).await,
                Err(_) => std::future::pending::<()>().await,
            }
        });
    }
}
