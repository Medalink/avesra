//! Explicit native setup of immutable app mappings; never action authority.
use crate::{Runtime, setup::ManagementProof};
use avesra_contracts::ErrorCode;
use avesra_core::apps::AppAlias;
use avesra_windows::effects::CatalogCommand;
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
    native: NativeCandidate,
    cwd: Option<PathBuf>,
    discovered: Instant,
    hints: Vec<(Uuid, avesra_windows::apps::WindowHint)>,
    selected_hint: Option<Uuid>,
    hints_complete: bool,
}
#[derive(Clone)]
enum NativeCandidate {
    Executable(avesra_windows::discovery::Candidate),
    Packaged(avesra_windows::packages::Candidate),
}
impl NativeCandidate {
    fn select(
        &self,
        actor: Uuid,
        cwd: Option<&std::path::Path>,
    ) -> Result<avesra_core::apps::AppRecord, ErrorCode> {
        match self {
            Self::Executable(value) => value.select(actor, cwd.ok_or(ErrorCode::Malformed)?),
            Self::Packaged(value) if cwd.is_none() => value.select(actor),
            Self::Packaged(_) => Err(ErrorCode::Malformed),
        }
    }
    fn packaged(&self) -> bool {
        matches!(self, Self::Packaged(_))
    }
}
#[derive(Serialize)]
pub struct WindowHintView {
    id: Uuid,
    class: String,
    title: String,
    process_id: u32,
}
#[derive(Serialize)]
pub struct CandidateView {
    id: Uuid,
    name: String,
    source: avesra_core::apps::AppSource,
    detail: String,
    arguments: String,
    packaged: bool,
    working_directory: Option<String>,
    selectable: bool,
    window_hints: Vec<WindowHintView>,
    selected_hint: Option<Uuid>,
    hints_complete: bool,
}
impl Choice {
    fn view(&self) -> CandidateView {
        let (name, source, detail, arguments, selectable) = match &self.native {
            NativeCandidate::Executable(value) => (
                value.name.clone(),
                value.source.clone(),
                value.executable.clone(),
                value.arguments.clone(),
                value.selectable,
            ),
            NativeCandidate::Packaged(value) => (
                value.name.clone(),
                avesra_core::apps::AppSource::PackageRegistration,
                format!("{} · {}", value.app_id, value.package_full_name),
                String::new(),
                true,
            ),
        };
        CandidateView {
            id: self.id,
            name,
            source,
            detail,
            arguments,
            packaged: self.native.packaged(),
            working_directory: self.cwd.as_ref().map(|v| v.to_string_lossy().into_owned()),
            selectable,
            window_hints: self
                .hints
                .iter()
                .map(|(id, hint)| WindowHintView {
                    id: *id,
                    class: hint.class().into(),
                    title: hint.title().into(),
                    process_id: hint.pid(),
                })
                .collect(),
            selected_hint: self.selected_hint,
            hints_complete: self.hints_complete,
        }
    }
}
#[derive(Serialize)]
pub struct Scan {
    candidates: Vec<CandidateView>,
    skipped: u32,
    truncated: bool,
    unavailable_sources: Vec<&'static str>,
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
            let first = avesra_windows::discovery::start_menu();
            let second = avesra_windows::discovery::registered_apps();
            let third = avesra_windows::packages::discover();
            (first, second, third)
        })
        .await
        .map_err(|_| "Discovery worker stopped")?;
        with_panel(&app, panel, |p| {
            let mut unavailable_sources = Vec::new();
            let first = scans.0.unwrap_or_else(|_| {
                unavailable_sources.push("Start menu");
                Default::default()
            });
            let second = scans.1.unwrap_or_else(|_| {
                unavailable_sources.push("App Paths");
                Default::default()
            });
            let third = scans.2.unwrap_or_else(|_| {
                unavailable_sources.push("Packaged applications");
                Default::default()
            });
            let scans = (first, second, third);
            let mut choices = Vec::new();
            for value in scans.0.candidates.into_iter().chain(scans.1.candidates) {
                choices.push(Choice {
                    id: Uuid::new_v4(),
                    cwd: value.working_directory.as_ref().map(PathBuf::from),
                    native: NativeCandidate::Executable(value),
                    discovered: Instant::now(),
                    hints: vec![],
                    selected_hint: None,
                    hints_complete: false,
                });
            }
            let mut truncated = scans.0.truncated || scans.1.truncated || scans.2.truncated;
            for value in scans.2.candidates {
                if choices.len() >= 768 {
                    truncated = true;
                    break;
                }
                choices.push(Choice {
                    id: Uuid::new_v4(),
                    cwd: None,
                    native: NativeCandidate::Packaged(value),
                    discovered: Instant::now(),
                    hints: vec![],
                    selected_hint: None,
                    hints_complete: false,
                });
            }
            let candidates = choices.iter().map(Choice::view).collect();
            p.candidates = choices;
            Ok(Scan {
                candidates,
                skipped: scans
                    .0
                    .skipped
                    .saturating_add(scans.1.skipped)
                    .saturating_add(scans.2.skipped),
                truncated,
                unavailable_sources,
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
            || !matches!(&choice.native, NativeCandidate::Executable(value) if value.working_directory.is_none() && value.selectable)
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
            choice.hints.clear();
            choice.selected_hint = None;
            choice.hints_complete = false;
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
pub async fn observe_app_windows(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    candidate: Uuid,
) -> Result<CandidateView, String> {
    visible(&window)?;
    let owner = app
        .state::<Runtime>()
        .catalog
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Application setup is busy")?;
    let (native, cwd, discovered) = with_panel(&app, panel, |p| {
        let choice = p
            .candidates
            .iter_mut()
            .find(|v| v.id == candidate)
            .ok_or("Candidate unavailable")?;
        if choice.discovered.elapsed() >= Duration::from_secs(120) {
            return Err("Candidate expired; scan again".into());
        }
        choice.hints.clear();
        choice.selected_hint = None;
        choice.hints_complete = false;
        Ok((
            choice.native.clone(),
            if choice.native.packaged() {
                None
            } else {
                Some(choice.cwd.clone().ok_or("Choose a working folder first")?)
            },
            choice.discovered,
        ))
    })?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let actor = crate::owner::current_actor(&app).await?;
        with_panel(&app, panel, |_| Ok(()))?;
        let hints = tokio::task::spawn_blocking(move || {
            let record = native.select(actor, cwd.as_deref())?;
            avesra_windows::apps::window_hints(&record)
        })
        .await
        .map_err(|_| "Window observer stopped")?
        .map_err(|_| "Application window identity unavailable")?;
        with_panel(&app, panel, |p| {
            let choice = p
                .candidates
                .iter_mut()
                .find(|v| v.id == candidate && v.discovered == discovered)
                .ok_or("Candidate changed")?;
            if discovered.elapsed() >= Duration::from_secs(120) {
                return Err("Candidate expired; scan again".into());
            }
            choice.hints = hints
                .windows
                .into_iter()
                .map(|v| (Uuid::new_v4(), v))
                .collect();
            choice.hints_complete = hints.complete;
            Ok(choice.view())
        })
    })
    .await
    .map_err(|_| "Window observer coordinator stopped")?
}
#[tauri::command]
pub fn choose_app_window(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    candidate: Uuid,
    hint: Option<Uuid>,
) -> Result<CandidateView, String> {
    visible(&window)?;
    let state = app.state::<Runtime>();
    let _owner = state
        .catalog
        .work
        .try_lock()
        .map_err(|_| "Application setup is busy")?;
    with_panel(&app, panel, |p| {
        let choice = p
            .candidates
            .iter_mut()
            .find(|v| v.id == candidate)
            .ok_or("Candidate unavailable")?;
        if choice.discovered.elapsed() >= Duration::from_secs(120)
            || hint.is_some_and(|id| !choice.hints.iter().any(|v| v.0 == id))
        {
            return Err("Window hint changed; observe again".into());
        }
        choice.selected_hint = hint;
        Ok(choice.view())
    })
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
    let (native, cwd, discovered, hint) = with_panel(&app, panel, |p| {
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
            if choice.native.packaged() {
                None
            } else {
                Some(choice.cwd.clone().ok_or("Choose a working folder first")?)
            },
            choice.discovered,
            choice
                .selected_hint
                .map(|id| {
                    choice
                        .hints
                        .iter()
                        .find(|v| v.0 == id)
                        .map(|v| v.1.clone())
                        .ok_or("Window hint unavailable")
                })
                .transpose()?,
        ))
    })?;
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let actor = crate::owner::current_actor(&app).await?;
        if !proof.current(&app.state::<Runtime>()) {
            return Err("Management verification expired".into());
        }
        with_panel(&app, panel, |_| Ok(()))?;
        let selected_hint = hint.clone();
        let final_candidate = native.clone();
        let record = tokio::task::spawn_blocking(move || {
            let mut record = native.select(actor, cwd.as_deref())?;
            if let Some(hint) = selected_hint {
                avesra_windows::apps::bind_window_hint(&mut record, &hint)?;
            }
            Ok::<_, ErrorCode>(record)
        })
        .await
        .map_err(|_| "Selection worker stopped")?
        .map_err(|_| "Application metadata changed or cannot be selected; scan again")?;
        let mut authorize = authorization(app.clone(), panel, actor, proof)?;
        let mut confirmed_record = record.clone();
        let result = receive(
            &app,
            CatalogCommand::Remember {
                record: Box::new(record),
                phrase,
                authorize: Box::new(move || {
                    if discovered.elapsed() >= Duration::from_secs(120) {
                        return Err(ErrorCode::Expired);
                    }
                    if let Some(hint) = &hint {
                        avesra_windows::apps::bind_window_hint(&mut confirmed_record, hint)?;
                    }
                    if let NativeCandidate::Packaged(value) = &final_candidate {
                        value.revalidate()?;
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
