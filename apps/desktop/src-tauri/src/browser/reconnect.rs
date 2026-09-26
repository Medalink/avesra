//! Saved-selection retry only; never a pairing or action authority producer.
use super::*;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
const FLAG: &str = "browser-reconnect-disabled.dpapi";
const BODY: &[u8] = b"AVESRA-BROWSER-RECONNECT-DISABLED-1";
#[derive(Default)]
pub(super) struct Reconnect {
    state: Mutex<Preference>,
    writer: Arc<tokio::sync::Mutex<()>>,
}
#[derive(Default)]
struct Preference {
    initialized: bool,
    enabled: bool,
    paused: bool,
    revision: u64,
}
impl Reconnect {
    pub(super) fn pause(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.paused = true;
        }
    }
    pub(super) fn release_management(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.paused = false;
        }
    }
    pub(super) fn enabled(&self) -> bool {
        self.state.lock().is_ok_and(|v| v.initialized && v.enabled)
    }
    pub(super) fn allowed(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|v| v.initialized && v.enabled && !v.paused)
    }
}
fn disabled(directory: &Path) -> Result<bool, ErrorCode> {
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| ErrorCode::Unavailable)?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(ErrorCode::Denied);
    }
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(0x00200000)
        .open(directory.join(FLAG))
    {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(ErrorCode::Unavailable),
    };
    let metadata = file.metadata().map_err(|_| ErrorCode::Unavailable)?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 || metadata.len() > 4096 {
        return Err(ErrorCode::Malformed);
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| ErrorCode::Unavailable)?;
    if bytes.len() > 4096 || avesra_windows::credentials::unprotect(&bytes)?.as_slice() != BODY {
        return Err(ErrorCode::Malformed);
    }
    Ok(true)
}
fn persist(directory: &Path, enabled: bool) -> Result<(), ErrorCode> {
    let present = disabled(directory)?;
    if enabled {
        if present {
            std::fs::remove_file(directory.join(FLAG)).map_err(|_| ErrorCode::Unavailable)?;
        }
    } else if !present {
        let temporary = directory.join(format!(".browser-reconnect-{}.tmp", Uuid::new_v4()));
        let bytes = avesra_windows::credentials::protect(BODY)?;
        let result: Result<(), ErrorCode> = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .custom_flags(0x00200000)
                .open(&temporary)
                .map_err(|_| ErrorCode::Unavailable)?;
            file.write_all(&bytes).map_err(|_| ErrorCode::Unavailable)?;
            file.sync_all().map_err(|_| ErrorCode::Unavailable)?;
            std::fs::hard_link(&temporary, directory.join(FLAG))
                .map_err(|_| ErrorCode::Unavailable)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(temporary);
        result?;
    }
    if disabled(directory)? == enabled {
        return Err(ErrorCode::Unavailable);
    }
    Ok(())
}
/// Preference mutation remains in the actual serialized blocking owner, even if
/// the IPC future is dropped. A newer user choice always disables old publication.
pub(super) async fn set(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let runtime = app.state::<Runtime>();
    let reconnect = &runtime.browser.reconnect;
    let revision = {
        let mut state = reconnect
            .state
            .lock()
            .map_err(|_| "Browser preference unavailable")?;
        state.enabled = false;
        state.initialized = true;
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or("Browser preference revision exhausted")?;
        state.revision
    };
    let writer = reconnect.writer.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let writer = writer.lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let _writer = writer;
            let runtime = app.state::<Runtime>();
            let reconnect = &runtime.browser.reconnect;
            if reconnect
                .state
                .lock()
                .map_err(|_| ErrorCode::Unavailable)?
                .revision
                != revision
            {
                return Err(ErrorCode::Stale);
            }
            let directory = app
                .path()
                .app_data_dir()
                .map_err(|_| ErrorCode::Unavailable)?;
            persist(&directory, enabled)?;
            let mut state = reconnect.state.lock().map_err(|_| ErrorCode::Unavailable)?;
            if state.revision != revision {
                return Err(ErrorCode::Stale);
            }
            state.enabled = enabled;
            Ok(())
        })
        .await
        .map_err(|_| ErrorCode::Unavailable)?
    })
    .await
    .map_err(|_| "Browser preference worker stopped")?
    .map_err(|_| {
        "Browser reconnect preference could not be saved; automatic connection remains disabled"
            .into()
    })
}
pub(super) fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let writer = app
            .state::<Runtime>()
            .browser
            .reconnect
            .writer
            .clone()
            .lock_owned()
            .await;
        let owned = app.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _writer = writer;
            let runtime = owned.state::<Runtime>();
            let reconnect = &runtime.browser.reconnect;
            let allowed = owned
                .path()
                .app_data_dir()
                .ok()
                .is_some_and(|v| disabled(&v).is_ok_and(|v| !v));
            if let Ok(mut state) = reconnect.state.lock()
                && !state.initialized
            {
                state.enabled = allowed;
                state.initialized = true;
            }
        })
        .await;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let state = app.state::<Runtime>();
            if !state.browser.reconnect.allowed() {
                continue;
            }
            let ready = state
                .local
                .lock()
                .is_ok_and(|v| v.connected && !v.locked && !v.settings.paused);
            if ready {
                let _ = launch(app.clone(), ConnectionChoice::Saved, None);
            }
        }
    });
}
