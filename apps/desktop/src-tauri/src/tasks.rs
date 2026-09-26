//! Protected action setup and opaque accepted-turn routing. No text/action IPC.
use crate::{Runtime, connection, setup::ManagementProof};
use avesra_contracts::{ErrorCode, actors};
use avesra_core::{
    action_permissions::Selection,
    conversations::{
        CancellationTarget, DurableTurn, ExactTaskRequest, PlannerRequest, TaskResolution,
    },
    ledger::DispatchSession,
};
use avesra_windows::effects::{ActionManagement, ActionSnapshot, PublishedReply};
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;
#[path = "tasks_download.rs"]
pub mod download;
#[path = "tasks_history.rs"]
pub mod history;
#[path = "tasks_memory.rs"]
pub mod memory;

#[path = "tasks_teaching.rs"]
pub mod teaching;

#[derive(Default)]
pub struct State {
    history: history::State,
    teaching: teaching::State,
    download_pending: Mutex<Option<download::Pending>>,
    memory_pending: Mutex<Option<memory::Pending>>,
    vpn_channel: Mutex<Option<(Uuid, connection::SessionIdentity, Instant, VpnChannel)>>,
    vpn: Mutex<Option<(Uuid, Instant, avesra_core::vpn::Profile)>>,
    panel: Mutex<Option<Panel>>,
    reader: Arc<tokio::sync::Mutex<()>>,
    management: Arc<tokio::sync::Mutex<()>>,
    cancellation: Arc<tokio::sync::Mutex<()>>,
    accepted: Arc<tokio::sync::Mutex<()>>,
    page: Mutex<Option<PageResult>>,
    page_timer: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}
#[derive(Clone, Serialize)]
pub struct VpnChannel {
    step: Uuid,
    dispatch: Uuid,
    authenticated: bool,
    checked_ms: u64,
}
struct PageResult {
    actor: Uuid,
    session: connection::SessionIdentity,
    id: Uuid,
    started: Instant,
    delivery: Arc<AtomicU8>,
    dispatch: Uuid,
    action_revision: Uuid,
    view: PageView,
}
#[derive(Clone, Serialize)]
pub struct PageView {
    task: Uuid,
    origin: String,
    blocks: Vec<String>,
    truncated: bool,
    excluded_content: bool,
    remaining_ms: u64,
    provider: Option<avesra_contracts::browser::provider::Probe>,
    x_ready: Option<avesra_contracts::browser::provider::XReady>,
    x_needs_input: Option<avesra_contracts::browser::provider::XNeedsInput>,
    inbox: Option<crate::browser::mailbox::View>,
}
struct Panel {
    id: Uuid,
    generation: u64,
    started: Instant,
    targets: Vec<(Uuid, CancellationTarget)>,
    sequence: u64,
}
impl State {
    pub fn invalidate(&self) {
        self.history.invalidate();
        self.teaching.invalidate();
        if let Ok(mut pending) = self.memory_pending.lock() {
            *pending = None;
        }
        if let Ok(mut pending) = self.download_pending.lock() {
            *pending = None;
        }
        if let Ok(mut vpn) = self.vpn.lock() {
            *vpn = None;
        }
        if let Ok(mut page) = self.page.lock() {
            *page = None;
        }
        if let Ok(mut timer) = self.page_timer.lock()
            && let Some(timer) = timer.take()
        {
            timer.abort();
        }
        if let Ok(mut panel) = self.panel.lock() {
            *panel = None;
        }
    }
}
fn visible(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use visible Settings for actions".into());
    }
    Ok(())
}
fn current(app: &tauri::AppHandle, id: Uuid) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let panel = state
        .tasks
        .panel
        .lock()
        .map_err(|_| "Action panel unavailable")?;
    if local.locked
        || !local.connected
        || panel.as_ref().is_none_or(|p| {
            p.id != id
                || p.started.elapsed() >= Duration::from_secs(600)
                || p.generation != state.connection_generation.load(Ordering::SeqCst)
        })
    {
        return Err("Action panel expired; reopen it".into());
    }
    if app
        .get_webview_window("settings")
        .is_none_or(|w| !w.is_visible().unwrap_or(false))
    {
        return Err("Settings is hidden".into());
    }
    Ok(())
}
#[tauri::command]
pub fn open_action_panel(
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
        .tasks
        .panel
        .lock()
        .map_err(|_| "Action panel unavailable")? = Some(Panel {
        id,
        generation: state.connection_generation.load(Ordering::SeqCst),
        started: Instant::now(),
        targets: Vec::new(),
        sequence: 0,
    });
    Ok(id)
}
#[tauri::command]
pub fn close_action_panel(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    let state = app.state::<Runtime>();
    let mut slot = state
        .tasks
        .panel
        .lock()
        .map_err(|_| "Action panel unavailable")?;
    if slot.as_ref().is_some_and(|p| p.id == panel) {
        *slot = None;
    }
    Ok(())
}
async fn receive<T: Send + 'static>(
    receiver: std::sync::mpsc::Receiver<Result<T, ErrorCode>>,
) -> Result<T, String> {
    tokio::task::spawn_blocking(move || receiver.recv())
        .await
        .map_err(|_| "Action reader stopped")?
        .map_err(|_| "Action worker stopped")?
        .map_err(|e| format!("Action state unavailable ({e:?}). Refresh before another attempt."))
}
fn publish(
    app: &tauri::AppHandle,
    panel: Uuid,
    actor: Uuid,
    snapshot: ActionSnapshot,
) -> Result<ActionSnapshot, String> {
    current(app, panel)?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    if !crate::owner::matches_actor(&directory, actor) {
        return Err("Owner changed".into());
    }
    let state = app.state::<Runtime>();
    let mut slot = state
        .tasks
        .panel
        .lock()
        .map_err(|_| "Action panel unavailable")?;
    let value = slot
        .as_mut()
        .filter(|p| p.id == panel)
        .ok_or("Action panel changed")?;
    if snapshot.sequence < value.sequence {
        return Err("A newer task snapshot is already available".into());
    }
    value.sequence = snapshot.sequence;
    value.targets = snapshot
        .tasks
        .iter()
        .map(|task| (task.task, task.cancellation))
        .collect();
    Ok(snapshot)
}
#[derive(Serialize)]
pub struct Status {
    vpn_channel: Option<VpnChannel>,
    snapshot: ActionSnapshot,
    outputs: Vec<avesra_windows::AudioDevice>,
    scopes: Vec<avesra_core::browser_scopes::Grant>,
    page: Option<PageView>,
}
/// One accepted coordinator can project alongside the bounded Settings reader.
/// Native cache is updated before publishing the non-authoritative display rows.
async fn project(app: &tauri::AppHandle, actor: Uuid) -> Result<(), String> {
    let panel = app
        .state::<Runtime>()
        .tasks
        .panel
        .lock()
        .map_err(|_| "Action panel unavailable")?
        .as_ref()
        .map(|p| p.id);
    let Some(panel) = panel else {
        return Ok(());
    };
    current(app, panel)?;
    let receiver = app
        .state::<Runtime>()
        .effects
        .actions(ActionManagement::Read { actor })
        .map_err(|_| "Action projection unavailable")?;
    let snapshot = publish(app, panel, actor, receive(receiver).await?)?;
    #[derive(Clone, Serialize)]
    struct Projection<'a> {
        panel: Uuid,
        snapshot: &'a ActionSnapshot,
    }
    app.emit_to(
        "settings",
        "action-task-snapshot",
        Projection {
            panel,
            snapshot: &snapshot,
        },
    )
    .map_err(|_| "Action projection unavailable".to_owned())
}
#[tauri::command]
pub async fn action_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<Status, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Action status is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        current(&app, panel)?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .actions(ActionManagement::Read { actor })
            .map_err(|_| "Action worker busy")?;
        let snapshot = receive(receiver).await?;
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "Owner directory unavailable")?;
        let scopes = tokio::task::spawn_blocking(move || {
            let path = directory.join("browser-scopes.db");
            if !path.exists() {
                return Ok(Vec::new());
            }
            avesra_core::browser_scopes::Store::open_read_only(&path)?
                .list(avesra_contracts::browser::Id::new(actor)?)
        })
        .await
        .map_err(|_| "Scope reader stopped")?
        .map_err(|_: ErrorCode| "Browser scopes unavailable")?;
        let outputs = tokio::task::spawn_blocking(avesra_windows::audio_devices)
            .await
            .map_err(|_| "Device reader stopped")?
            .map_err(|_| "Output devices unavailable")?
            .into_iter()
            .filter(|d| d.direction == "output")
            .take(64)
            .collect();
        Ok(Status {
            vpn_channel: {
                let state = app.state::<Runtime>();
                let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                state
                    .tasks
                    .vpn_channel
                    .lock()
                    .map_err(|_| "VPN observation unavailable")?
                    .as_ref()
                    .filter(|(owner, session, at, _)| {
                        *owner == actor
                            && at.elapsed() < Duration::from_secs(60)
                            && session.action_epoch == local.action_epoch
                            && !local.locked
                    })
                    .map(|(_, _, _, value)| value.clone())
            },
            snapshot: publish(&app, panel, actor, snapshot)?,
            outputs,
            scopes,
            page: {
                let state = app.state::<Runtime>();
                let local = state.local.lock().map_err(|_| "Local state unavailable")?;
                let mut page = state
                    .tasks
                    .page
                    .lock()
                    .map_err(|_| "Page result unavailable")?;
                if page.as_ref().is_some_and(|p| {
                    p.actor != actor
                        || p.started.elapsed() >= Duration::from_secs(60)
                        || p.session.action_epoch != local.action_epoch
                        || p.session.generation
                            != state.connection_generation.load(Ordering::SeqCst)
                        || local.locked
                        || !local.connected
                }) {
                    *page = None;
                }
                page.as_ref()
                    .filter(|p| p.delivery.load(Ordering::SeqCst) == 1)
                    .map(|p| {
                        let mut view = p.view.clone();
                        view.remaining_ms =
                            60_000u64.saturating_sub(p.started.elapsed().as_millis() as u64);
                        view
                    })
            },
        })
    })
    .await
    .map_err(|_| "Action status coordinator stopped")?
}
fn proof(app: &tauri::AppHandle) -> Result<ManagementProof, String> {
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    state
        .setup
        .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))
}
#[tauri::command]
pub async fn inspect_prompt_surface(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    alias: Uuid,
    revision: Uuid,
) -> Result<avesra_core::workflows::PromptDiscovery, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Native inspection is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        let authorization = authorize(app.clone(), panel, actor, None)?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .probe_prompt(actor, alias, revision, authorization)
            .map_err(|_| "Native inspection worker busy")?;
        let observed = receive(receiver).await?;
        let mut final_check = authorize(app, panel, actor, None)?;
        final_check().map_err(|_| "Inspection context changed")?;
        Ok(observed)
    })
    .await
    .map_err(|_| "Inspection coordinator stopped")?
}
fn authorize(
    app: tauri::AppHandle,
    panel: Uuid,
    actor: Uuid,
    proof: Option<ManagementProof>,
) -> Result<avesra_windows::effects::CatalogAuthorization, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    Ok(Box::new(move || {
        let state = app.state::<Runtime>();
        let _owner = state
            .owner_setup
            .try_lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        if current(&app, panel).is_err()
            || !crate::owner::matches_actor(&directory, actor)
            || proof
                .as_ref()
                .is_some_and(|proof| !proof.current(&app.state::<Runtime>()))
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }))
}
#[tauri::command]
pub async fn grant_app_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    alias: Uuid,
    revision: Uuid,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    grant(app, panel, Selection::Application { alias, revision }).await
}
#[tauri::command]
pub async fn grant_browser_read_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    reference: avesra_contracts::browser::ScopeRef,
    x_account: Option<String>,
    gmail_account: Option<String>,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Permission management is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "Owner directory unavailable")?;
        let path = directory.join("browser-scopes.db");
        let read_path = path.clone();
        let scope = tokio::task::spawn_blocking(move || {
            avesra_core::browser_scopes::Store::open_read_only(&read_path)?.get(
                avesra_contracts::browser::Id::new(actor)?,
                reference.id,
                reference.revision,
            )
        })
        .await
        .map_err(|_| "Scope reader stopped")?
        .map_err(|_| "Saved browser scope changed")?;
        let mut authorization = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        let expected = scope.clone();
        let receiver =
            app.state::<Runtime>()
                .effects
                .actions(ActionManagement::Grant {
                    actor,
                    selection: if let Some(account) = gmail_account {
                        if x_account.is_some()
                            || !avesra_contracts::browser::mailbox::account(&account)
                        {
                            return Err("Enter one exact Gmail account".into());
                        }
                        Selection::GmailInbox {
                            scope: Box::new(scope),
                            account,
                        }
                    } else if let Some(account) = x_account {
                        if !avesra_contracts::browser::provider::x_account(&account) {
                            return Err("Enter the exact X handle without @".into());
                        }
                        Selection::XReady {
                            scope: Box::new(scope),
                            account,
                        }
                    } else {
                        Selection::BrowserRead {
                            scope: Box::new(scope),
                        }
                    },
                    authorize: Box::new(move || {
                        authorization()?;
                        let current = avesra_core::browser_scopes::Store::open_read_only(&path)?
                            .get(expected.actor, expected.id, expected.revision)?;
                        if current != expected {
                            return Err(ErrorCode::Stale);
                        }
                        authorization()
                    }),
                })
                .map_err(|_| "Permission worker busy")?;
        publish(&app, panel, actor, receive(receiver).await?)
    })
    .await
    .map_err(|_| "Permission coordinator stopped")?
}
#[tauri::command]
pub async fn grant_volume_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    endpoint: String,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    grant(app, panel, Selection::Volume { endpoint }).await
}
#[tauri::command]
pub async fn grant_diagnostic_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    catalog: avesra_core::diagnostics::Catalog,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    grant(app, panel, Selection::Diagnostic { catalog }).await
}
#[tauri::command]
pub async fn inspect_vpn_profile(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
) -> Result<avesra_core::vpn::Profile, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Action reader is busy")?;
    tauri::async_runtime::spawn(async move{
        let _guard=guard;
        let actor=crate::owner::current_actor(&app).await?;
        current(&app,panel)?;
        let profile=tokio::task::spawn_blocking(move||avesra_windows::vpn::discover(actor)).await.map_err(|_|"VPN reader stopped")?.map_err(|_|"Supported saved Cisco default unavailable; select an existing numeric peer using Cisco first")?;
        current(&app,panel)?;
        *app.state::<Runtime>().tasks.vpn.lock().map_err(|_|"VPN selection unavailable")?=Some((panel,Instant::now(),profile.clone()));
        Ok(profile)
    }).await.map_err(|_|"VPN inspection coordinator stopped")?
}
#[tauri::command]
pub async fn grant_vpn_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    id: Uuid,
    revision: Uuid,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let profile = {
        let state = app.state::<Runtime>();
        let mut saved = state
            .tasks
            .vpn
            .lock()
            .map_err(|_| "VPN selection unavailable")?;
        let (observed, started, profile) = saved
            .as_ref()
            .ok_or("Inspect the current saved Cisco target first")?;
        if *observed != panel
            || started.elapsed() >= Duration::from_secs(30)
            || profile.id != id
            || profile.revision != revision
        {
            return Err("VPN selection expired; inspect it again".into());
        }
        let profile = profile.clone();
        *saved = None;
        profile
    };
    grant(
        app,
        panel,
        Selection::Vpn {
            profile: Box::new(profile),
        },
    )
    .await
}
#[tauri::command]
pub async fn bind_prompt_project(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    alias: Uuid,
    revision: Uuid,
    choices: avesra_core::workflows::BindingChoices,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Project setup is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        let authorize = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .actions(ActionManagement::BindPrompt {
                actor,
                alias,
                revision,
                choices,
                authorize,
            })
            .map_err(|_| "Project setup worker busy")?;
        publish(&app, panel, actor, receive(receiver).await?)
    })
    .await
    .map_err(|_| "Project setup coordinator stopped")?
}
async fn grant(
    app: tauri::AppHandle,
    panel: Uuid,
    selection: Selection,
) -> Result<ActionSnapshot, String> {
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Permission management is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        current(&app, panel)?;
        if let Selection::Application { alias, revision } = &selection {
            let receiver = app
                .state::<Runtime>()
                .effects
                .actions(ActionManagement::Read { actor })
                .map_err(|_| "Application identity reader busy")?;
            let observed = receive(receiver).await?;
            if !observed.aliases.iter().any(|v| {
                v.id == *alias && v.revision == *revision && v.selected_by == actor && v.available
            }) {
                return Err("Saved application changed; refresh permissions".into());
            }
        }
        if let Selection::Volume { endpoint } = &selection {
            let endpoint = endpoint.clone();
            let found = tokio::task::spawn_blocking(move || {
                avesra_windows::audio_devices().map(|devices| {
                    devices
                        .into_iter()
                        .any(|d| d.direction == "output" && d.id == endpoint)
                })
            })
            .await
            .map_err(|_| "Output reader stopped")?
            .map_err(|_| "Output devices unavailable")?;
            if !found {
                return Err("Select a currently available output".into());
            }
        }
        current(&app, panel)?;
        let authorization = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .actions(ActionManagement::Grant {
                actor,
                selection,
                authorize: authorization,
            })
            .map_err(|_| "Permission worker busy")?;
        let snapshot = receive(receiver).await?;
        publish(&app, panel, actor, snapshot)
    })
    .await
    .map_err(|_| "Permission coordinator stopped")?
}
#[tauri::command]
pub async fn change_private_memory(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    change: avesra_core::memory::Change,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Memory management is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        current(&app, panel)?;
        let mut authorize = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        authorize().map_err(|_| "Memory context changed")?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .actions(ActionManagement::Memory {
                actor,
                change,
                authorize,
            })
            .map_err(|_| "Memory worker busy")?;
        publish(&app, panel, actor, receive(receiver).await?)
    })
    .await
    .map_err(|_| "Memory coordinator stopped")?
}
#[tauri::command]
pub async fn revoke_action_permission(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    id: Uuid,
) -> Result<ActionSnapshot, String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .management
        .clone()
        .try_lock_owned()
        .map_err(|_| "Permission management is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        current(&app, panel)?;
        let mut authorization = authorize(app.clone(), panel, actor, Some(proof(&app)?))?;
        authorization().map_err(|_| "Permission context changed")?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .actions(ActionManagement::Revoke {
                actor,
                id,
                authorize: authorization,
            })
            .map_err(|_| "Permission worker busy")?;
        let snapshot = receive(receiver).await?;
        publish(&app, panel, actor, snapshot)
    })
    .await
    .map_err(|_| "Permission coordinator stopped")?
}
#[tauri::command]
pub async fn cancel_action_task(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    task: Uuid,
) -> Result<(), String> {
    visible(&window)?;
    current(&app, panel)?;
    let guard = app
        .state::<Runtime>()
        .tasks
        .cancellation
        .clone()
        .try_lock_owned()
        .map_err(|_| "Action management is busy")?;
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let actor = crate::owner::current_actor(&app).await?;
        current(&app, panel)?;
        let target = {
            let state = app.state::<Runtime>();
            let slot = state
                .tasks
                .panel
                .lock()
                .map_err(|_| "Action panel unavailable")?;
            slot.as_ref()
                .filter(|p| p.id == panel)
                .and_then(|p| {
                    p.targets
                        .iter()
                        .find(|(id, target)| *id == task && target.actor == actor)
                })
                .map(|(_, target)| *target)
                .ok_or("Refresh the exact task before cancelling")?
        };
        if let Ok(pending) = app.state::<Runtime>().tasks.download_pending.lock()
            && let Some(context) = pending.as_ref().and_then(|p| p.context.upgrade())
            && context.target == target
        {
            context.withdrawn.store(true, Ordering::SeqCst);
        }
        let authorization = authorize(app.clone(), panel, actor, None)?;
        let receiver = app
            .state::<Runtime>()
            .effects
            .cancel_conversation(
                actor,
                target.source,
                target.id,
                target.revision,
                authorization,
            )
            .map_err(|_| "Cancellation unavailable; refresh task status")?;
        receive(receiver).await?;
        Ok(())
    })
    .await
    .map_err(|_| "Cancellation coordinator stopped")?
}

/// A real accepted producer transfers its one handle here, before planner claim.
/// Neither history lookup nor IPC can construct this input.
pub enum AcceptedResult {
    Action(avesra_core::execution::ExecutionReceipt),
    Reply(Box<PublishedReply>),
    NeedsInput { message: String },
}
struct AcceptedContext {
    target: CancellationTarget,
    binding: actors::Binding,
    session: connection::SessionIdentity,
    capture_epoch: u64,
    withdrawn: Arc<AtomicBool>,
    delivery: Arc<AtomicU8>,
    started: Instant,
}
impl AcceptedContext {
    fn dispatch(&self) -> DispatchSession {
        DispatchSession {
            actor_id: self.target.actor,
            device_id: self.target.source.device,
            session_id: self.target.source.session,
            capture_epoch: self.capture_epoch,
            action_epoch: self.session.action_epoch,
            active: true,
        }
    }
    fn check(&self, app: &tauri::AppHandle) -> Result<(), ErrorCode> {
        let state = app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
        if self.withdrawn.load(Ordering::SeqCst)
            || self.target.source.device != self.session.device
            || self.target.source.session != self.session.id
            || local.locked
            || !local.connected
            || !local.enrolled
            || local.settings.paused
            || local.action_epoch != self.session.action_epoch
            || state.connection_generation.load(Ordering::SeqCst) != self.session.generation
            || state
                .acknowledged_session
                .lock()
                .map_err(|_| ErrorCode::Unavailable)?
                .is_none_or(|s| {
                    s.id != self.session.id
                        || s.device != self.session.device
                        || s.action_epoch != self.session.action_epoch
                        || s.generation != self.session.generation
                        || s.server_fingerprint != self.session.server_fingerprint
                })
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    fn prepare(&self, app: &tauri::AppHandle) -> Result<(), ErrorCode> {
        if self.started.elapsed() >= Duration::from_secs(5) {
            return Err(ErrorCode::Expired);
        }
        self.current_owner(app)
    }
    fn current_owner(&self, app: &tauri::AppHandle) -> Result<(), ErrorCode> {
        self.check(app)?;
        let state = app.state::<Runtime>();
        let _owner = state
            .owner_setup
            .try_lock()
            .map_err(|_| ErrorCode::Unavailable)?;
        let path = app
            .path()
            .app_data_dir()
            .map_err(|_| ErrorCode::Unavailable)?;
        crate::planner::paired_owner(&path, &self.binding, self.session)
            .map_err(|_| ErrorCode::Stale)?;
        self.check(app)
    }
}
async fn execute_accepted(
    app: &tauri::AppHandle,
    context: Arc<AcceptedContext>,
    step: Uuid,
    target: avesra_core::action_permissions::TaskTarget,
    proposal: Option<PublishedReply>,
    approved: Option<avesra_contracts::Action>,
    observation: Option<avesra_core::conversations::ObservationRequest>,
) -> Result<AcceptedResult, String> {
    let check = || {
        if let Some(reply) = &proposal {
            reply.remaining_ms()?;
            context.current_owner(app)
        } else if let Some(action) = &approved {
            if context.started.elapsed() >= Duration::from_secs(30) {
                return Err(ErrorCode::Expired);
            }
            action.validate(
                u64::try_from(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|_| ErrorCode::Expired)?
                        .as_millis(),
                )
                .map_err(|_| ErrorCode::Expired)?,
            )?;
            context.current_owner(app)
        } else {
            context.prepare(app)
        }
    };
    check().map_err(|_| "Task context changed; inspect durable task status")?;
    let _ = project(app, context.target.actor).await;
    check().map_err(|_| "Task queued but context changed; inspect task status")?;
    let state = app.state::<Runtime>();
    let vpn = matches!(
        &target,
        avesra_core::action_permissions::TaskTarget::Vpn { .. }
    );
    let mailbox_result = Arc::new(Mutex::new(None));
    let mailbox_publication = mailbox_result.clone();
    let consumer =
        if let avesra_core::action_permissions::TaskTarget::BrowserRead { scope }
        | avesra_core::action_permissions::TaskTarget::XReady { scope, .. }
        | avesra_core::action_permissions::TaskTarget::GmailInbox { scope, .. } = target
        {
            let expected = context.clone();
            let owned_app = app.clone();
            let origin = scope.origin.as_str().to_owned();
            Some(avesra_windows::effects::ReadConsumer {
                scope: *scope,
                consume: Box::new(move |borrowed| {
                    expected.current_owner(&owned_app)?;
                    let (reply, mailbox) = borrowed.consume_inbox()?;
                    let inbox = crate::browser::mailbox::consume(&reply, mailbox.as_ref())?;
                    let successor = mailbox
                        .as_ref()
                        .map(|e| crate::browser::mailbox::Successor::new(&owned_app, e))
                        .transpose()?;
                    retain_page(&owned_app, &expected, step, origin, reply, inbox)
                        .map_err(|_| ErrorCode::Stale)?;
                    if let (Some(evidence), Some(successor)) = (mailbox, successor) {
                        if !successor.current() {
                            return Err(ErrorCode::Stale);
                        }
                        let mut slot = mailbox_publication
                            .lock()
                            .map_err(|_| ErrorCode::Unavailable)?;
                        if slot.is_some() {
                            return Err(ErrorCode::Stale);
                        }
                        *slot = Some((evidence, successor));
                    }
                    Ok(())
                }),
            })
        } else {
            None
        };
    let waiter = state
        .effects
        .submit_with_withdrawal(
            step,
            context.dispatch(),
            consumer,
            context.withdrawn.clone(),
        )
        .map_err(|_| "Task queued but execution unavailable; inspect task status")?;
    let cancellation = waiter.cancellation();
    {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        local.active_task = true;
        state.publish(&local);
        let _ = app.emit("runtime-state", local.clone());
    }
    let _working = Working {
        app: app.clone(),
        epoch: context.session.action_epoch,
    };
    let mut result = tokio::task::spawn_blocking(move || {
        loop {
            match waiter.receive(Duration::from_secs(1)) {
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                value => return value,
            }
        }
    });
    loop {
        tokio::select! {
            value = &mut result => {
                let _ = project(app, context.target.actor).await;
                let receipt=value.map_err(|_| "Effect reader stopped")?
                    .map_err(|_| "Effect owner stopped")?
                    .map_err(|e| format!("Effect unresolved ({e:?}); inspect task status"))?;
                if vpn {
                    let directory=app.path().app_data_dir().map_err(|_|"Owner directory unavailable")?;
                    let paired=crate::planner::paired_owner(&directory,&context.binding,context.session);
                    let authenticated=if let Ok(pairing)=paired {
                        let request=actors::Request{version:actors::VERSION,request:Uuid::new_v4(),attempt:Uuid::new_v4(),session:context.session.id,action_epoch:context.session.action_epoch,command:actors::Command::Status};
                        matches!(tokio::time::timeout(Duration::from_secs(3),connection::actor_operation(&pairing,&request)).await,Ok(Ok(status)) if status.binding.as_ref()==Some(&context.binding))
                    }else{false};
                    let checked_ms=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"Clock unavailable")?.as_millis() as u64;
                    let view=VpnChannel{step,dispatch:receipt.dispatch_id,authenticated,checked_ms};
                    let state=app.state::<Runtime>();
                    let local=state.local.lock().map_err(|_|"Local state unavailable")?;
                    if !local.locked && local.action_epoch==context.session.action_epoch {
                        *state.tasks.vpn_channel.lock().map_err(|_|"VPN observation unavailable")?=Some((context.target.actor,context.session,Instant::now(),view.clone()));
                        let _=app.emit("vpn-channel-observation",view);
                        if !authenticated {let _=app.emit("runtime-error","VPN observation finished, but the paired Spark could not be authenticated within 3 seconds. The connection is not retried; inspect Cisco and network routing without changing corporate policy.");}
                    }
                }
                if let Some(mut request)=observation {
                    if request.requires_mailbox(){
                        let evidence=mailbox_result.lock().map_err(|_|"Mailbox source unavailable")?.take();
                        let Some((evidence,successor))=evidence else{return Ok(AcceptedResult::Action(receipt));};
                        request=request.attach_mailbox(evidence,Box::new(move||successor.current())).map_err(|_|"Original mailbox source expired or changed")?;
                    }
                    context.current_owner(app).map_err(|_|"Diagnostic output context changed")?;
                    let expected=context.clone();let owner_app=app.clone();
                    let receiver=state.effects.observation_answer(request,receipt.dispatch_id,Box::new(move|authority|{
                        if authority.context.turn!=expected.target.id || authority.context.turn_revision!=expected.target.revision || authority.binding!=&expected.binding {return Err(ErrorCode::Stale);}
                        expected.current_owner(&owner_app)
                    })).map_err(|_|"Diagnostic reply owner unavailable; inspect task status")?;
                    let stored=receive(receiver).await?;
                    context.current_owner(app).map_err(|_|"Diagnostic output withdrawn")?;
                    let local=state.local.lock().map_err(|_|"Local state unavailable")?;
                    if local.action_epoch!=context.session.action_epoch || local.locked || !local.connected {return Err("Diagnostic output context changed".into());}
                    let reply=state.effects.publish_planner(stored).map_err(|_|"Diagnostic publication withdrawn")?;
                    return Ok(AcceptedResult::Reply(Box::new(reply)));
                }
                return Ok(AcceptedResult::Action(receipt));
            }
            _ = tokio::time::sleep(Duration::from_millis(25)) => {
                if context.check(app).is_err() || proposal.as_ref().is_some_and(|reply| reply.remaining_ms().is_err()) {
                    cancellation.cancel();
                }
            }
        }
    }
}
fn retain_page(
    app: &tauri::AppHandle,
    context: &AcceptedContext,
    step: Uuid,
    origin: String,
    reply: avesra_contracts::browser::reading::Reply,
    inbox: Option<crate::browser::mailbox::View>,
) -> Result<(), String> {
    use avesra_contracts::browser::reading::Outcome;
    let source = &reply.context;
    if source.actor.uuid() != context.target.actor
        || source.source.device.uuid() != context.session.device
        || source.source.session.uuid() != context.session.id
        || source.source.action_epoch != context.session.action_epoch
        || source.source.capture_epoch != context.capture_epoch
        || source.step.uuid() != step
    {
        return Err("Page result source changed".into());
    }
    let x_ready = match &reply.outcome {
        Outcome::XReady { ready } => Some(ready.clone()),
        _ => None,
    };
    let x_needs_input = match &reply.outcome {
        Outcome::XNeedsInput { evidence } => Some(evidence.clone()),
        _ => None,
    };
    let (blocks, truncated, excluded_content, provider) = match reply.outcome {
        Outcome::Inbox { .. } | Outcome::XReady { .. } | Outcome::XNeedsInput { .. } => {
            (Vec::new(), false, false, None)
        }
        Outcome::Excerpt { excerpt } => (
            excerpt.blocks,
            excerpt.truncated,
            excerpt.excluded_content,
            None,
        ),
        Outcome::Empty => (Vec::new(), false, false, None),
        Outcome::ProviderInspection { probe } => (Vec::new(), !probe.complete, false, Some(probe)),
        _ => return Err("Page observation incomplete".into()),
    };
    // Only the synchronous actual borrowed consumer calls this function. Its
    // approved scope and validated current read supply exact origin provenance.
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    if context.withdrawn.load(Ordering::SeqCst)
        || context.delivery.load(Ordering::SeqCst) != 0
        || local.action_epoch != context.session.action_epoch
        || local.locked
        || !local.connected
        || state.connection_generation.load(Ordering::SeqCst) != context.session.generation
    {
        return Err("Page publication withdrawn".into());
    }
    let id = Uuid::new_v4();
    let mut page = state
        .tasks
        .page
        .lock()
        .map_err(|_| "Page result unavailable")?;
    if context.delivery.load(Ordering::SeqCst) != 0 {
        return Err("Page caller was lost".into());
    }
    *page = Some(PageResult {
        actor: context.target.actor,
        session: context.session,
        id,
        started: Instant::now(),
        delivery: context.delivery.clone(),
        dispatch: source.dispatch.uuid(),
        action_revision: source.action_revision.uuid(),
        view: PageView {
            task: source.task.uuid(),
            origin,
            blocks,
            truncated,
            excluded_content,
            remaining_ms: 60_000,
            provider,
            x_ready,
            x_needs_input,
            inbox,
        },
    });
    drop(page);
    let mut timer = state
        .tasks
        .page_timer
        .lock()
        .map_err(|_| "Page timer unavailable")?;
    if let Some(previous) = timer.take() {
        previous.abort();
    }
    let owned_app = app.clone();
    *timer = Some(tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let state = owned_app.state::<Runtime>();
        if let Ok(mut page) = state.tasks.page.lock()
            && page.as_ref().is_some_and(|p| p.id == id)
        {
            *page = None;
        }
    }));
    Ok(())
}
struct Withdraw(Option<Arc<AtomicBool>>);
/// Distinct from worker cancellation: only polling the original accepted future
/// to its successful receipt may make its pending display result visible.
struct PageDelivery {
    app: tauri::AppHandle,
    marker: Arc<AtomicU8>,
}
impl Drop for PageDelivery {
    fn drop(&mut self) {
        if self
            .marker
            .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let state = self.app.state::<Runtime>();
            if let Ok(mut page) = state.tasks.page.lock()
                && page
                    .as_ref()
                    .is_some_and(|p| Arc::ptr_eq(&p.delivery, &self.marker))
            {
                *page = None;
            }
        }
    }
}
impl Drop for Withdraw {
    fn drop(&mut self) {
        if let Some(signal) = &self.0 {
            signal.store(true, Ordering::SeqCst);
        }
    }
}
struct Working {
    app: tauri::AppHandle,
    epoch: u64,
}
impl Drop for Working {
    fn drop(&mut self) {
        let state = self.app.state::<Runtime>();
        if let Ok(mut local) = state.local.lock()
            && local.action_epoch == self.epoch
        {
            local.active_task = false;
            state.publish(&local);
            let _ = self.app.emit("runtime-state", local.clone());
        }
    }
}
pub async fn accepted(
    app: tauri::AppHandle,
    turn: DurableTurn,
    dispatch: DispatchSession,
    binding: actors::Binding,
) -> Result<AcceptedResult, String> {
    let started = Instant::now();
    let trace_link = avesra_core::trace::Link {
        turn: turn.id(),
        actor: turn.actor(),
        device: turn.source().device,
        operation: turn.id(),
        parent: None,
    };
    let withdrawn = Arc::new(AtomicBool::new(false));
    let delivery = Arc::new(AtomicU8::new(0));
    let delivered = PageDelivery {
        app: app.clone(),
        marker: delivery.clone(),
    };
    let mut caller = Withdraw(Some(withdrawn.clone()));
    let guard = app
        .state::<Runtime>()
        .tasks
        .accepted
        .clone()
        .try_lock_owned()
        .map_err(|_| "Accepted action coordinator is busy")?;
    let result = tauri::async_runtime::spawn(async move {
        let mut span=avesra_core::trace::begin(trace_link,avesra_core::trace::Stage::Accepted);
        span.queued(started);
        let result=async {
        let _guard=guard;
        let state=app.state::<Runtime>();
        let session=state.acknowledged_session.lock().map_err(|_|"Session unavailable")?.ok_or("Session unavailable")?;
        if dispatch.actor_id!=turn.actor()||dispatch.device_id!=turn.source().device||dispatch.session_id!=turn.source().session||dispatch.device_id!=session.device||dispatch.session_id!=session.id||dispatch.action_epoch!=session.action_epoch||dispatch.capture_epoch==0||!dispatch.active||binding.actor!=turn.actor()||binding.device!=session.device||binding.revoked { return Err("Accepted action provenance changed".into()); }
        binding.validate().map_err(|_|"Invalid registration")?;
        let context=Arc::new(AcceptedContext {target:CancellationTarget {actor:turn.actor(),source:turn.source(),id:turn.id(),revision:turn.revision()},binding,session,capture_epoch:dispatch.capture_epoch,withdrawn,delivery,started});
        context.prepare(&app).map_err(|_|"Accepted action context changed")?;
        let path=app.path().app_data_dir().map_err(|_|"Owner directory unavailable")?;
        let pairing=crate::planner::paired_owner(&path,&context.binding,session)?;
        let status_request=actors::Request { version:actors::VERSION,request:Uuid::new_v4(),attempt:Uuid::new_v4(),session:session.id,action_epoch:session.action_epoch,command:actors::Command::Status };
        let remaining=Duration::from_secs(5).saturating_sub(started.elapsed());
        let status=tokio::time::timeout(remaining,connection::actor_operation(&pairing,&status_request)).await.map_err(|_|"Accepted action registration check expired")??;
        if status.binding.as_ref()!=Some(&context.binding) { return Err("Original actor registration changed".into()); }
        context.prepare(&app).map_err(|_|"Accepted action context changed")?;
        let expected=context.clone(); let owner_app=app.clone();
        let request=ExactTaskRequest::new(turn,dispatch).map_err(|_|"Accepted action request invalid")?;
        let receiver=state.effects.accept_action_task(request,Box::new(move |authority| {
            if authority.turn!=expected.target.id||authority.turn_revision!=expected.target.revision||authority.session.actor_id!=expected.target.actor||authority.session.capture_epoch!=expected.capture_epoch||authority.session.action_epoch!=expected.session.action_epoch { return Err(ErrorCode::Stale); }
            expected.prepare(&owner_app)
        })).map_err(|_|"Action resolver busy")?;
        let resolution=receive(receiver).await?;
        context.prepare(&app).map_err(|_|"Accepted action context changed; inspect durable task status")?;
        match resolution {
            TaskResolution::Linked(task) => {
                let task=*task;
                if let Some(action)=&task.pending_approval {download::wait(&app,&context,action).await?;}
                let observation=task.observation_reply.map(|claim|claim.bind(context.binding.clone(),context.started,context.withdrawn.clone())).transpose().map_err(|_|"Diagnostic source expired")?;
                execute_accepted(&app, context, task.step, task.target, None, task.pending_approval,observation).await
            },
            TaskResolution::NeedsInput(turn) => {
                let expected=context.clone(); let owner_app=app.clone();
                let request=PlannerRequest::new(turn,context.dispatch(),context.binding.clone()).map_err(|_|"Planner request unavailable")?.with_withdrawal(context.withdrawn.clone());
                let receiver=state.effects.claim_planner(request,Box::new(move |authority| {
                    if authority.context.turn!=expected.target.id||authority.context.turn_revision!=expected.target.revision||authority.binding!=&expected.binding { return Err(ErrorCode::Stale); }
                    expected.prepare(&owner_app)
                })).map_err(|_|"Planner owner busy")?;
                let claim=receive(receiver).await?;
                let request=claim.transport().map_err(|_|"Accepted native question expired")?;
                if avesra_core::memory::conversation::parse(&request.text).is_some(){
                    return memory::answer(&app,context.clone(),claim).await;
                }
                let clock=avesra_core::clock::question_kind(&request.text).is_some();
                if clock || avesra_core::notifications::question_kind(&request.text).is_some() {
                    let expected=context.clone();let owner_app=app.clone();
                    let authorize: avesra_windows::effects::PlannerAuthorization=Box::new(move |authority|{
                        if authority.context.turn!=expected.target.id||authority.context.turn_revision!=expected.target.revision||authority.binding!=&expected.binding{return Err(ErrorCode::Stale);}
                        expected.prepare(&owner_app)
                    });
                    let receiver=if clock {state.effects.clock_answer(claim,authorize)}else{state.effects.event_answer(claim,authorize)}.map_err(|_|"Native answer writer busy")?;
                    let stored=receive(receiver).await?;
                    context.check(&app).map_err(|_|"Native answer withdrawn")?;
                    let local=state.local.lock().map_err(|_|"Local state unavailable")?;
                    if local.action_epoch!=context.session.action_epoch||local.locked||!local.connected{return Err("Native answer context changed".into());}
                    let reply=state.effects.publish_planner(stored).map_err(|_|"Native answer publication withdrawn")?;
                    return Ok(AcceptedResult::Reply(Box::new(reply)));
                }
                let future=crate::planner::answer(app.clone(),claim);
                tokio::pin!(future);
                loop {
                    tokio::select! {
                        result=&mut future=>{
                            let reply=result?;
                            if matches!(reply.response(), avesra_contracts::planner::Response::Proposal { .. }) {
                                let Some(task)=reply.proposed_task() else {
                                    return Ok(AcceptedResult::NeedsInput { message: "Choose one current application alias or speaker output and grant its action in Settings before asking again.".into() });
                                };
                                let step=task.step;
                                let target=task.target.clone();
                                return execute_accepted(&app, context, step, target, Some(reply),None,None).await;
                            }
                            return Ok(AcceptedResult::Reply(Box::new(reply)));
                        },
                        _=tokio::time::sleep(Duration::from_millis(25))=>context.check(&app).map_err(|_|"Accepted planning withdrawn")?,
                    }
                }
            }
        }
        }.await;
        match &result {Ok(AcceptedResult::Action(receipt))=>span.effect(receipt.outcome),Ok(AcceptedResult::NeedsInput{..})=>span.finish(avesra_core::trace::Outcome::NeedsInput,None),Ok(AcceptedResult::Reply(_))=>span.finish(avesra_core::trace::Outcome::Complete,None),Err(_)=>span.finish(avesra_core::trace::Outcome::Failed,None)}
        result
    }).await.map_err(|_|"Accepted task coordinator stopped; inspect durable status")?;
    // A successful normal reply transfers its source owner to the caller. All
    // failure/action/clarification paths keep ordinary withdrawal on return.
    if matches!(&result, Ok(AcceptedResult::Reply(_))) {
        caller.0.take();
    }
    if let Ok(AcceptedResult::Action(receipt)) = &result
        && matches!(
            receipt.outcome,
            avesra_contracts::Outcome::Success | avesra_contracts::Outcome::NeedsInput
        )
    {
        let state = delivered.app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let page = state
            .tasks
            .page
            .lock()
            .map_err(|_| "Page result unavailable")?;
        if page.as_ref().is_some_and(|p| {
            Arc::ptr_eq(&p.delivery, &delivered.marker)
                && p.dispatch == receipt.dispatch_id
                && p.action_revision == receipt.action_revision
                && p.session.action_epoch == local.action_epoch
                && p.session.generation == state.connection_generation.load(Ordering::SeqCst)
                && local.connected
                && !local.locked
        }) {
            let _ = delivered
                .marker
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
        }
    }
    result
}
