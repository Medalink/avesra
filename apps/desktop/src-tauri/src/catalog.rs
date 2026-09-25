//! Explicit native setup of immutable app mappings; never action authority.
use crate::{Runtime, setup::ManagementProof};
use avesra_contracts::ErrorCode;
use avesra_core::apps::AppAlias;
use avesra_windows::{discovery::Candidate, effects::CatalogCommand};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Default)]
pub struct CatalogSetup {
    work: Arc<tokio::sync::Mutex<()>>,
    panel: Mutex<Option<Panel>>,
}
struct Panel {
    id: Uuid,
    connection: u64,
    started: Instant,
    candidates: Vec<Choice>,
}
struct Choice {
    id: Uuid,
    native: Candidate,
    cwd: Option<PathBuf>,
    discovered: Instant,
}
#[derive(Serialize)]
pub struct CandidateView {
    id: Uuid,
    name: String,
    source: avesra_core::apps::AppSource,
    detail: String,
    arguments: String,
    working_directory: Option<String>,
    selectable: bool,
}
impl Choice {
    fn view(&self) -> CandidateView {
        CandidateView {
            id: self.id,
            name: self.native.name.clone(),
            source: self.native.source.clone(),
            detail: self.native.executable.clone(),
            arguments: self.native.arguments.clone(),
            working_directory: self.cwd.as_ref().map(|v| v.to_string_lossy().into_owned()),
            selectable: self.native.selectable,
        }
    }
}
#[derive(Serialize)]
pub struct Scan {
    candidates: Vec<CandidateView>,
    skipped: u32,
    truncated: bool,
}
impl CatalogSetup {
    pub fn invalidate(&self) {
        if let Ok(mut panel) = self.panel.lock() {
            *panel = None;
        }
    }
}
fn visible(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use visible Settings to manage application mappings".into());
    }
    Ok(())
}
fn with_panel<T>(
    app: &tauri::AppHandle,
    id: Uuid,
    f: impl FnOnce(&mut Panel) -> Result<T, String>,
) -> Result<T, String> {
    with_panel_state(app, id, |panel, _, _| f(panel))
}
fn with_panel_state<T>(
    app: &tauri::AppHandle,
    id: Uuid,
    f: impl FnOnce(&mut Panel, &Runtime, &avesra_core::state::LocalState) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut slot = state
        .catalog
        .panel
        .lock()
        .map_err(|_| "Application setup unavailable")?;
    let panel = slot.as_mut().ok_or("Reopen application setup")?;
    if local.locked
        || !local.connected
        || id.is_nil()
        || panel.id != id
        || panel.connection != state.connection_generation.load(Ordering::SeqCst)
        || panel.started.elapsed() >= Duration::from_secs(600)
    {
        return Err("Application setup expired; refresh to reopen".into());
    }
    f(panel, &state, &local)
}
fn proof(app: &tauri::AppHandle, panel: Uuid) -> Result<ManagementProof, String> {
    with_panel_state(app, panel, |_, state, local| {
        state
            .setup
            .management_proof(local, state.connection_generation.load(Ordering::SeqCst))
    })
}
fn authorization(
    app: tauri::AppHandle,
    panel: Uuid,
    actor: Uuid,
    proof: ManagementProof,
) -> Result<avesra_windows::effects::CatalogAuthorization, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    Ok(Box::new(move || {
        if !crate::owner::matches_actor(&directory, actor)
            || with_panel_state(&app, panel, |_, state, local| {
                if proof.current_locked(state, local) {
                    Ok(())
                } else {
                    Err("Management verification expired".into())
                }
            })
            .is_err()
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }))
}
async fn receive(app: &tauri::AppHandle, command: CatalogCommand) -> Result<Vec<AppAlias>, String> {
    let reply = app
        .state::<Runtime>()
        .effects
        .catalog(command)
        .map_err(|_| "Application catalog busy or unavailable")?;
    tokio::task::spawn_blocking(move || reply.recv()).await.map_err(|_|"Catalog coordinator stopped")?
        .map_err(|_|"Catalog worker stopped")?.map_err(|_|"Catalog did not return a verified result. Refresh saved mappings before another explicit attempt.".into())
}
#[tauri::command]
pub fn open_app_catalog(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Uuid, String> {
    visible(&window)?;
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if local.locked || !local.connected {
        return Err("Connect Spark and unlock Windows first".into());
    }
    let id = Uuid::new_v4();
    *state
        .catalog
        .panel
        .lock()
        .map_err(|_| "Application setup unavailable")? = Some(Panel {
        id,
        connection: state.connection_generation.load(Ordering::SeqCst),
        started: Instant::now(),
        candidates: vec![],
    });
    Ok(id)
}
#[tauri::command]
pub fn close_app_catalog(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    let state = app.state::<Runtime>();
    let mut slot = state
        .catalog
        .panel
        .lock()
        .map_err(|_| "Application setup unavailable")?;
    if slot.as_ref().is_some_and(|v| v.id == panel) {
        *slot = None;
    }
    Ok(())
}
#[tauri::command]
pub async fn scan_app_catalog(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<Scan, String> {
    visible(&window)?;
    let owner = app
        .state::<Runtime>()
        .catalog
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Application setup is busy")?;
    with_panel(&app, panel, |p| {
        p.candidates.clear();
        Ok(())
    })?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let scans = tokio::task::spawn_blocking(|| {
            let first = avesra_windows::discovery::start_menu()?;
            let second = avesra_windows::discovery::registered_apps()?;
            Ok::<_, ErrorCode>((first, second))
        })
        .await
        .map_err(|_| "Discovery worker stopped")?
        .map_err(|_| "Native application discovery unavailable")?;
        with_panel(&app, panel, |p| {
            let mut choices = Vec::new();
            for value in scans.0.candidates.into_iter().chain(scans.1.candidates) {
                choices.push(Choice {
                    id: Uuid::new_v4(),
                    cwd: value.working_directory.as_ref().map(PathBuf::from),
                    native: value,
                    discovered: Instant::now(),
                });
            }
            let candidates = choices.iter().map(Choice::view).collect();
            p.candidates = choices;
            Ok(Scan {
                candidates,
                skipped: scans.0.skipped.saturating_add(scans.1.skipped),
                truncated: scans.0.truncated || scans.1.truncated,
            })
        })
    })
    .await
    .map_err(|_| "Discovery coordinator stopped")?
}
#[tauri::command]
pub async fn choose_app_folder(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    candidate: Uuid,
) -> Result<CandidateView, String> {
    visible(&window)?;
    let hwnd = window.hwnd().map_err(|_| "Settings handle unavailable")?.0 as isize;
    with_panel(&app, panel, |p| {
        let choice = p
            .candidates
            .iter()
            .find(|v| v.id == candidate)
            .ok_or("Candidate unavailable")?;
        if choice.discovered.elapsed() >= Duration::from_secs(120)
            || choice.native.working_directory.is_some()
            || !choice.native.selectable
        {
            return Err("Candidate changed or does not need a folder".into());
        }
        Ok(())
    })?;
    let owner = app
        .state::<Runtime>()
        .catalog
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Application setup is busy")?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let picker_app = app.clone();
        let cwd = tokio::task::spawn_blocking(move || {
            avesra_windows::discovery::choose_working_directory(hwnd, &mut || {
                with_panel(&picker_app, panel, |p| {
                    if !p.candidates.iter().any(|v| {
                        v.id == candidate && v.discovered.elapsed() < Duration::from_secs(120)
                    }) {
                        return Err("Candidate expired".into());
                    }
                    Ok(())
                })
                .map_err(|_| ErrorCode::Stale)
            })
        })
        .await
        .map_err(|_| "Folder picker stopped")?
        .map_err(|_| "Folder choice cancelled or unavailable")?;
        with_panel(&app, panel, |p| {
            let choice = p
                .candidates
                .iter_mut()
                .find(|v| v.id == candidate)
                .ok_or("Candidate unavailable")?;
            if choice.discovered.elapsed() >= Duration::from_secs(120) {
                return Err("Candidate expired; scan again".into());
            }
            choice.cwd = Some(cwd);
            Ok(choice.view())
        })
    })
    .await
    .map_err(|_| "Folder coordinator stopped")?
}
#[tauri::command]
pub async fn app_aliases(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<Vec<AppAlias>, String> {
    visible(&window)?;
    with_panel(&app, panel, |_| Ok(()))?;
    let owner = app
        .state::<Runtime>()
        .catalog
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Application setup is busy")?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let result = receive(&app, CatalogCommand::List).await?;
        with_panel(&app, panel, |_| Ok(result))
    })
    .await
    .map_err(|_| "Catalog reader stopped")?
}
#[tauri::command]
pub async fn remember_app(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    candidate: Uuid,
    phrase: String,
) -> Result<Vec<AppAlias>, String> {
    visible(&window)?;
    let phrase = avesra_core::apps::alias_phrase(&phrase)
        .map_err(|_| "Use a name of up to 64 letters, digits, spaces, apostrophes or hyphens")?;
    let owner = app
        .state::<Runtime>()
        .catalog
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Application setup is busy")?;
    let proof = proof(&app, panel)?;
    let (native, cwd, discovered) = with_panel(&app, panel, |p| {
        let choice = p
            .candidates
            .iter()
            .find(|v| v.id == candidate)
            .ok_or("Candidate unavailable")?;
        if choice.discovered.elapsed() >= Duration::from_secs(120) {
            return Err("Candidate expired; scan again".into());
        }
        Ok((
            choice.native.clone(),
            choice.cwd.clone().ok_or("Choose a working folder first")?,
            choice.discovered,
        ))
    })?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let actor = crate::owner::current_actor(&app).await?;
        if !proof.current(&app.state::<Runtime>()) {
            return Err("Management verification expired".into());
        }
        with_panel(&app, panel, |_| Ok(()))?;
        let record = tokio::task::spawn_blocking(move || native.select(actor, &cwd))
            .await
            .map_err(|_| "Selection worker stopped")?
            .map_err(|_| "Application metadata changed or cannot be selected; scan again")?;
        let mut authorize = authorization(app.clone(), panel, actor, proof)?;
        let result = receive(
            &app,
            CatalogCommand::Remember {
                record: Box::new(record),
                phrase,
                authorize: Box::new(move || {
                    if discovered.elapsed() >= Duration::from_secs(120) {
                        return Err(ErrorCode::Expired);
                    }
                    authorize()
                }),
            },
        )
        .await?;
        with_panel(&app, panel, |_| Ok(result))
    })
    .await
    .map_err(|_| "Application selection coordinator stopped")?
}
#[tauri::command]
pub async fn forget_app_alias(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    id: Uuid,
    revision: Uuid,
) -> Result<Vec<AppAlias>, String> {
    visible(&window)?;
    let owner = app
        .state::<Runtime>()
        .catalog
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Application setup is busy")?;
    let proof = proof(&app, panel)?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let actor = crate::owner::current_actor(&app).await?;
        let authorize = authorization(app.clone(), panel, actor, proof)?;
        let result = receive(
            &app,
            CatalogCommand::Forget {
                id,
                revision,
                actor,
                authorize,
            },
        )
        .await?;
        with_panel(&app, panel, |_| Ok(result))
    })
    .await
    .map_err(|_| "Mapping coordinator stopped")?
}
