//! Main-thread owner for preference edit identities and voluntary window close.
use crate::{Runtime, invalidate_settings, window_positions};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Hide,
    Quit,
}
#[derive(Clone, Serialize)]
pub struct CloseRequest {
    sequence: u64,
    editor: Uuid,
    request: Uuid,
    intent: Intent,
}
struct Editor {
    id: Uuid,
    generation: u64,
    page: Uuid,
}
#[derive(Default)]
struct Inner {
    sequence: u64,
    editor: Option<Editor>,
    close: Option<CloseRequest>,
}
#[derive(Default)]
pub struct State(Mutex<Inner>, AtomicU64);

struct Caller(Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// All commit, close and editor-identity changes share the UI thread. No window
/// getter runs under Runtime.local, and no coordinator mutex crosses a window call.
pub async fn on_main<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(tauri::AppHandle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = if present.load(Ordering::SeqCst) {
            operation(handle)
        } else {
            Err("Settings request withdrawn".into())
        };
        let _ = send.send(result);
    })
    .map_err(|_| "Settings event thread unavailable")?;
    receive.await.map_err(|_| "Settings event thread stopped")?
}

pub fn require_editor(app: &tauri::AppHandle, editor: Uuid) -> Result<(), String> {
    let state = app.state::<State>();
    let inner = state.0.lock().map_err(|_| "Settings editor unavailable")?;
    if !inner.editor.as_ref().is_some_and(|current| {
        current.id == editor && current.generation == state.1.load(Ordering::SeqCst)
    }) || inner.close.is_some()
    {
        return Err("Settings editor changed or a close decision is pending".into());
    }
    Ok(())
}

/// Called on the event thread after releasing Runtime locks. It cannot postpone
/// OS lock/logoff or the existing withdrawal of safety/protected operations.
pub fn revoke(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<State>() {
        state.1.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn withdraw(app: &tauri::AppHandle) {
    revoke(app);
    if let Some(state) = app.try_state::<State>()
        && let Ok(mut inner) = state.0.lock()
    {
        inner.editor = None;
        inner.close = None;
    }
}

#[derive(Serialize)]
pub struct Registration {
    sequence: u64,
    editor: Uuid,
    close: Option<CloseRequest>,
}

#[tauri::command]
pub async fn begin_preferences_editor(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    page: Uuid,
) -> Result<Registration, String> {
    if window.label() != "settings" {
        return Err("Use the Settings editor".into());
    }
    on_main(app, move |app| {
        let window = app
            .get_webview_window("settings")
            .ok_or("Settings unavailable")?;
        if !window.is_visible().unwrap_or(false) {
            return Err("Open Settings to edit preferences".into());
        }
        {
            let state = app.state::<Runtime>();
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.locked {
                return Err("Unlock Windows before editing preferences".into());
            }
        }
        let state = app.state::<State>();
        let mut inner = state.0.lock().map_err(|_| "Settings editor unavailable")?;
        if let Some(current) = inner.editor.as_ref().filter(|current| {
            current.page == page && current.generation == state.1.load(Ordering::SeqCst)
        }) {
            return Ok(Registration {
                sequence: inner.sequence,
                editor: current.id,
                close: inner.close.clone(),
            });
        }
        // A new page/context supersedes only presentation ownership. It cannot
        // acknowledge the old page's close decision or discard a newer draft.
        inner.close = None;
        let id = Uuid::new_v4();
        inner.editor = Some(Editor {
            id,
            generation: state.1.load(Ordering::SeqCst),
            page,
        });
        Ok(Registration {
            sequence: inner.sequence,
            editor: id,
            close: None,
        })
    })
    .await
}

#[tauri::command]
pub async fn retire_preferences_editor(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    editor: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use the Settings editor".into());
    }
    on_main(app, move |app| {
        let state = app.state::<State>();
        let mut inner = state.0.lock().map_err(|_| "Settings editor unavailable")?;
        if inner
            .editor
            .as_ref()
            .is_some_and(|current| current.id == editor)
        {
            inner.editor = None;
            inner.close = None;
        }
        Ok(())
    })
    .await
}

fn finish(app: &tauri::AppHandle, intent: Intent) -> Result<(), String> {
    match intent {
        Intent::Hide => {
            let window = app
                .get_webview_window("settings")
                .ok_or("Settings unavailable")?;
            invalidate_settings(app);
            window_positions::save(app);
            window
                .hide()
                .map_err(|_| "Settings could not be hidden".into())
        }
        Intent::Quit => {
            withdraw(app);
            crate::quit_app(app);
            Ok(())
        }
    }
}

/// Used directly by native close/tray events and on_main for IPC title-hide.
pub fn request_close(app: &tauri::AppHandle, intent: Intent) -> Result<(), String> {
    let request = {
        let state = app.state::<State>();
        let mut inner = state.0.lock().map_err(|_| "Settings editor unavailable")?;
        if let Some(editor) = inner
            .editor
            .as_ref()
            .filter(|editor| editor.generation == state.1.load(Ordering::SeqCst))
        {
            let editor = editor.id;
            inner.sequence = inner.sequence.saturating_add(1);
            let request = CloseRequest {
                sequence: inner.sequence,
                editor,
                request: Uuid::new_v4(),
                intent,
            };
            inner.close = Some(request.clone());
            Some(request)
        } else {
            None
        }
    };
    let Some(request) = request else {
        return finish(app, intent);
    };
    if let Err(error) = show(app) {
        cancel_request(app, request.request);
        return Err(error);
    }
    let window = app
        .get_webview_window("settings")
        .ok_or("Settings unavailable")?;
    if window.emit("settings-close-requested", &request).is_err() {
        cancel_request(app, request.request);
        return Err("Settings close confirmation unavailable".into());
    }
    Ok(())
}

fn cancel_request(app: &tauri::AppHandle, request: Uuid) {
    let state = app.state::<State>();
    if let Ok(mut inner) = state.0.lock()
        && inner
            .close
            .as_ref()
            .is_some_and(|current| current.request == request)
    {
        inner.sequence = inner.sequence.saturating_add(1);
        inner.close = None;
    }
}

pub fn show(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or("Settings unavailable")?;
    window.show().map_err(|_| "Settings could not be shown")?;
    let shown = window
        .emit("settings-shown", ())
        .map_err(|_| "Settings visibility notification failed".to_string());
    if window.set_focus().is_err() {
        let _ = app.emit(
            "runtime-error",
            "Settings is visible but could not be focused",
        );
    }
    shown
}

#[tauri::command]
pub async fn show_settings(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if !matches!(window.label(), "settings" | "overlay") {
        return Err("Unknown Avesra window".into());
    }
    on_main(app, |app| show(&app)).await
}

#[tauri::command]
pub async fn answer_preferences_close(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    editor: Uuid,
    request: Uuid,
    close: bool,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use the Settings editor".into());
    }
    on_main(app, move |app| {
        let intent = {
            let state = app.state::<State>();
            let mut inner = state.0.lock().map_err(|_| "Settings editor unavailable")?;
            let pending = inner
                .close
                .as_ref()
                .filter(|value| value.editor == editor && value.request == request)
                .ok_or("Settings close request changed")?;
            if !inner.editor.as_ref().is_some_and(|current| {
                current.id == editor && current.generation == state.1.load(Ordering::SeqCst)
            }) {
                return Err("Settings editor changed".into());
            }
            let intent = pending.intent;
            inner.sequence = inner.sequence.saturating_add(1);
            inner.close = None;
            if close {
                inner.editor = None;
            }
            intent
        };
        if close {
            finish(&app, intent)?;
        }
        Ok(())
    })
    .await
}
