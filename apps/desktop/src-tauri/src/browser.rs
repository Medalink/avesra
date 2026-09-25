//! Explicit native Settings owns pairing; there is no browser action ingress.
use crate::{Runtime, setup::ManagementProof};
use avesra_contracts::{
    ErrorCode,
    browser::{self, Client, Id},
};
use avesra_core::apps::AppRecord;
use avesra_windows::{
    browser_pairing::{
        Admission, Confirmation, Issuance, Saved, Selection, SelectionStatus, Store, Summary,
    },
    browser_pipe::Listener,
};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Default)]
pub struct BrowserSetup {
    work: Arc<tokio::sync::Mutex<()>>,
    inner: Mutex<Inner>,
}
#[derive(Default)]
struct Inner {
    generation: u64,
    settings_generation: u64,
    action_epoch: u64,
    action_allowed: bool,
    attempt: Option<Attempt>,
}
struct Attempt {
    id: Uuid,
    connection: u64,
    started: Instant,
    selected: Option<AppRecord>,
    pending: Option<Confirmation>,
    approval: Option<ManagementProof>,
    selection: Option<Id>,
    state: &'static str,
}
impl Attempt {
    fn current_time(&self) -> bool {
        let seconds = if self.state == "authenticated_no_scopes" {
            300
        } else {
            browser::HANDSHAKE_SECONDS
        };
        self.started.elapsed() < Duration::from_secs(seconds)
    }
}
#[derive(Serialize)]
pub struct Status {
    attempt: Option<Uuid>,
    state: &'static str,
    browser_app: Option<Uuid>,
    browser_revision: Option<Uuid>,
    browser_label: Option<String>,
    pending: Option<Confirmation>,
    selection: Option<Id>,
    action_epoch: u64,
    mode_allows_actions: bool,
}
impl BrowserSetup {
    /// Caller lock order: Runtime.local -> browser.inner. Never does I/O.
    pub fn invalidate(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.generation = inner.generation.saturating_add(1);
            inner.settings_generation = inner.settings_generation.saturating_add(1);
            inner.action_allowed = false;
            inner.attempt = None;
        }
    }
    pub fn settings_hidden(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.settings_generation = inner.settings_generation.saturating_add(1);
            let established = inner.attempt.as_ref().is_some_and(|v| {
                v.selection.is_some() && v.state == "authenticated_no_scopes" && v.current_time()
            });
            if !established {
                inner.generation = inner.generation.saturating_add(1);
                inner.attempt = None;
            }
        }
    }
    /// Runtime.local owns every publication. Future document ingress must match
    /// this exact epoch even when a transport survives Stop/Pause for status.
    pub fn observe(&self, local: &avesra_core::state::LocalState) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.action_epoch = local.action_epoch;
            inner.action_allowed = local.connected && !local.locked && !local.settings.paused;
        }
    }
}
fn management_current(
    app: &tauri::AppHandle,
    generation: u64,
    proof: Option<&ManagementProof>,
) -> Result<(), ErrorCode> {
    let runtime = app.state::<Runtime>();
    let local = runtime.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    let inner = runtime
        .browser
        .inner
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    if local.locked
        || !local.connected
        || inner.settings_generation != generation
        || proof.is_some_and(|v| !v.current_locked(&runtime, &local))
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
#[tauri::command]
pub async fn saved_browser_pairings(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<Summary>, String> {
    let generation = visible(&window)?;
    let state = app.state::<Runtime>();
    let work = state
        .browser
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Browser connection is busy; close it before managing saved pairings")?;
    management_current(&app, generation, None).map_err(|_| "Unlock and connect first")?;
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, None).map_err(|_| "Settings changed")?;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?
        .join("browser-pairings");
    let result = tokio::task::spawn_blocking(move || {
        let _work = work;
        Store::open(&path)?.list(Id::new(actor)?)
    })
    .await
    .map_err(|_| "Browser reader stopped")?
    .map_err(|_| "Saved browser pairings unavailable")?;
    management_current(&app, generation, None).map_err(|_| "Settings changed")?;
    Ok(result)
}
#[tauri::command]
pub async fn revoke_browser_pairing(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    pairing: browser::PairingRef,
) -> Result<(), String> {
    let admitted = visible(&window)?;
    let state = app.state::<Runtime>();
    // Revoke current/pending connection first, including a credential awaiting its
    // persistence proof. A still-running worker keeps admission until it exits.
    let (work, generation, proof) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| "Browser setup unavailable")?;
        if inner.settings_generation != admitted {
            return Err("Settings changed".into());
        }
        inner.generation = inner.generation.checked_add(1).ok_or("Restart Avesra")?;
        inner.settings_generation = inner
            .settings_generation
            .checked_add(1)
            .ok_or("Restart Avesra")?;
        inner.attempt = None;
        let work = state.browser.work.clone().try_lock_owned().map_err(
            |_| "Connection is closing. Refresh, then explicitly revoke the saved revision.",
        )?;
        let proof = state
            .setup
            .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?;
        (work, inner.settings_generation, proof)
    };
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, Some(&proof))
        .map_err(|_| "Original verification expired")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    tokio::task::spawn_blocking(move || {
        let _work=work;
        if !crate::owner::matches_actor(&directory,actor) { return Err(ErrorCode::Unauthenticated); }
        Store::open(&directory.join("browser-pairings"))?.remove(pairing,&mut ||management_current(&app,generation,Some(&proof)))
    }).await.map_err(|_|"Browser revocation worker stopped")?.map_err(|_|"Revocation did not return a verified result. Refresh saved pairings before another explicit attempt.".into())
}
fn selection_change(
    window: &tauri::WebviewWindow,
    app: &tauri::AppHandle,
) -> Result<(tokio::sync::OwnedMutexGuard<()>, u64, ManagementProof), String> {
    let admitted = visible(window)?;
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser setup unavailable")?;
    if inner.settings_generation != admitted {
        return Err("Settings changed".into());
    }
    // Withdraw old connection admission before a durable selection mutation.
    inner.generation = inner.generation.checked_add(1).ok_or("Restart Avesra")?;
    inner.settings_generation = inner
        .settings_generation
        .checked_add(1)
        .ok_or("Restart Avesra")?;
    inner.attempt = None;
    let work = state
        .browser
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Connection is closing. Refresh before changing its selection.")?;
    let proof = state
        .setup
        .management_proof(&local, state.connection_generation.load(Ordering::SeqCst))?;
    Ok((work, inner.settings_generation, proof))
}
#[tauri::command]
pub async fn browser_selections(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<SelectionStatus>, String> {
    let generation = visible(&window)?;
    let work = app
        .state::<Runtime>()
        .browser
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Close the browser connection before reading selection")?;
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, None).map_err(|_| "Settings changed")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let owned_app = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _work = work;
        let mut values =
            Store::open(&directory.join("browser-pairings"))?.selections(Id::new(actor)?)?;
        for value in &mut values {
            if let Some(binding) = &value.binding {
                let inspected = owned_app
                    .state::<Runtime>()
                    .effects
                    .with_app(
                        actor,
                        binding.browser_app.uuid(),
                        binding.browser_revision.uuid(),
                        Box::new(|| Ok(())),
                    )
                    .and_then(|reply| reply.recv().map_err(|_| ErrorCode::Unavailable)?);
                value.available &= inspected.is_ok();
            }
        }
        Ok::<_, ErrorCode>(values)
    })
    .await
    .map_err(|_| "Browser selection reader stopped")?
    .map_err(|_| "Browser selection is unavailable")?;
    management_current(&app, generation, None).map_err(|_| "Settings changed")?;
    Ok(result)
}
#[tauri::command]
pub async fn select_browser_pairing(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    pairing: browser::PairingRef,
) -> Result<(), String> {
    let (work, generation, proof) = selection_change(&window, &app)?;
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, Some(&proof))
        .map_err(|_| "Original verification expired")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    tokio::task::spawn_blocking(move || {
        if !crate::owner::matches_actor(&directory, actor) {
            return Err(ErrorCode::Unauthenticated);
        }
        let store = Store::open(&directory.join("browser-pairings"))?;
        let actor_id = Id::new(actor)?;
        let binding = store.load(pairing, actor_id)?.binding().clone();
        let owned_app = app.clone();
        // Keep store/work ownership in the actual catalog job, including when the
        // async waiter is cancelled. Revalidation follows any catalog queue delay.
        let reply = app.state::<Runtime>().effects.with_app(
            actor,
            binding.browser_app.uuid(),
            binding.browser_revision.uuid(),
            Box::new(move || {
                let _retained_work = &work;
                store.select(pairing, actor_id, &mut || {
                    management_current(&owned_app, generation, Some(&proof))
                })?;
                Ok(())
            }),
        )?;
        reply.recv().map_err(|_| ErrorCode::Unavailable)?
    })
    .await
    .map_err(|_| "Browser selection worker stopped")?
    .map_err(|_| {
        "Selection did not return a verified result. Refresh to inspect the saved revision.".into()
    })
}
#[tauri::command]
pub async fn clear_browser_selection(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    revision: Id,
) -> Result<(), String> {
    let (work, generation, proof) = selection_change(&window, &app)?;
    let actor = crate::owner::current_actor(&app).await?;
    management_current(&app, generation, Some(&proof))
        .map_err(|_| "Original verification expired")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    tokio::task::spawn_blocking(move || {
        let _work = work;
        if !crate::owner::matches_actor(&directory, actor) {
            return Err(ErrorCode::Unauthenticated);
        }
        Store::open(&directory.join("browser-pairings"))?.clear_selection(revision, &mut || {
            management_current(&app, generation, Some(&proof))
        })
    })
    .await
    .map_err(|_| "Browser selection worker stopped")?
    .map_err(|_| {
        "Clear did not return a verified result. Refresh to inspect the saved revision.".into()
    })
}
fn visible(window: &tauri::WebviewWindow) -> Result<u64, String> {
    // Snapshot before the synchronous main-thread getter; no native lock is held
    // across dispatch, and a close during the query cannot become fresh admission.
    let generation = window
        .app_handle()
        .state::<Runtime>()
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser setup unavailable")?
        .settings_generation;
    if window.label() != "settings" || !window.is_visible().map_err(|_| "Settings unavailable")? {
        return Err("Use visible Settings to pair a browser installation".into());
    }
    management_current(window.app_handle(), generation, None).map_err(|_| "Settings changed")?;
    Ok(generation)
}
fn with_attempt<T>(
    app: &tauri::AppHandle,
    id: Uuid,
    generation: u64,
    f: impl FnOnce(&Runtime, &avesra_core::state::LocalState, &mut Attempt) -> Result<T, ErrorCode>,
) -> Result<T, ErrorCode> {
    let runtime = app.state::<Runtime>();
    let local = runtime.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    let mut inner = runtime
        .browser
        .inner
        .lock()
        .map_err(|_| ErrorCode::Unavailable)?;
    if inner.generation != generation || local.locked || !local.connected {
        return Err(ErrorCode::Stale);
    }
    let attempt = inner.attempt.as_mut().ok_or(ErrorCode::Stale)?;
    if attempt.id != id
        || attempt.connection != runtime.connection_generation.load(Ordering::SeqCst)
    {
        return Err(ErrorCode::Stale);
    }
    if !attempt.current_time() {
        return Err(ErrorCode::Expired);
    }
    f(&runtime, &local, attempt)
}
#[tauri::command]
pub fn browser_pairing_status(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Status, String> {
    let admitted = visible(&window)?;
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser setup unavailable")?;
    let active = inner.attempt.as_ref().filter(|v| {
        inner.settings_generation == admitted
            && v.current_time()
            && !local.locked
            && local.connected
            && v.connection == state.connection_generation.load(Ordering::SeqCst)
    });
    Ok(Status {
        attempt: active.map(|v| v.id),
        state: active.map_or("idle", |v| v.state),
        browser_app: active.and_then(|v| v.selected.as_ref().map(|v| v.id)),
        browser_revision: active.and_then(|v| v.selected.as_ref().map(|v| v.revision)),
        browser_label: active.and_then(|v| v.selected.as_ref().map(|v| v.name.clone())),
        pending: active.and_then(|v| v.pending.clone()),
        selection: active.and_then(|v| v.selection),
        action_epoch: inner.action_epoch,
        mode_allows_actions: inner.action_allowed,
    })
}
#[tauri::command]
pub fn cancel_browser_pairing(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    attempt: Uuid,
) -> Result<(), String> {
    let admitted = visible(&window)?;
    let state = app.state::<Runtime>();
    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser setup unavailable")?;
    if inner.settings_generation == admitted
        && inner.attempt.as_ref().is_some_and(|v| v.id == attempt)
    {
        inner.generation = inner.generation.saturating_add(1);
        inner.settings_generation = inner.settings_generation.saturating_add(1);
        inner.attempt = None;
    }
    Ok(())
}
#[tauri::command]
pub fn release_browser_management(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    attempt: Uuid,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Use Settings".into());
    }
    let state = app.state::<Runtime>();
    let _local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser setup unavailable")?;
    if let Some(current) = inner.attempt.as_ref().filter(|v| v.id == attempt) {
        // Decide using authoritative current state, even if authentication finished
        // after the webview's last status read. Explicit Disconnect is separate.
        let established = current.selection.is_some()
            && current.state == "authenticated_no_scopes"
            && current.current_time();
        inner.settings_generation = inner.settings_generation.saturating_add(1);
        if !established {
            inner.generation = inner.generation.saturating_add(1);
            inner.attempt = None;
        }
    }
    Ok(())
}
#[tauri::command]
pub fn approve_browser_pairing(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    attempt: Uuid,
    challenge: Uuid,
) -> Result<(), String> {
    let admitted = visible(&window)?;
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| "Local state unavailable")?;
    let mut inner = state
        .browser
        .inner
        .lock()
        .map_err(|_| "Browser setup unavailable")?;
    if inner.settings_generation != admitted {
        return Err("Settings changed".into());
    }
    let pending = inner.attempt.as_mut().ok_or("Pairing expired")?;
    if local.locked
        || !local.connected
        || pending.id != attempt
        || pending.connection != state.connection_generation.load(Ordering::SeqCst)
        || pending.started.elapsed() >= Duration::from_secs(browser::HANDSHAKE_SECONDS)
        || pending.state != "awaiting_owner"
        || pending.approval.is_some()
        || !pending
            .pending
            .as_ref()
            .is_some_and(|v| v.challenge.uuid() == challenge)
    {
        return Err("Pairing challenge changed or expired".into());
    }
    // Consume original proof before any asynchronous owner/storage work.
    pending.approval = Some(state.setup.management_proof(&local, pending.connection)?);
    pending.state = "saving";
    Ok(())
}
#[tauri::command]
pub fn begin_browser_pairing(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    phrase: String,
) -> Result<Uuid, String> {
    avesra_core::apps::alias_phrase(&phrase)
        .map_err(|_| "Choose an existing learned application name")?;
    begin(&window, app, ConnectionChoice::Setup(phrase))
}
#[tauri::command]
pub fn connect_selected_browser(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    revision: Id,
) -> Result<Uuid, String> {
    begin(&window, app, ConnectionChoice::Selected(revision))
}
enum ConnectionChoice {
    Setup(String),
    Selected(Id),
}
fn begin(
    window: &tauri::WebviewWindow,
    app: tauri::AppHandle,
    choice: ConnectionChoice,
) -> Result<Uuid, String> {
    let admitted = visible(window)?;
    let state = app.state::<Runtime>();
    let owner = state
        .browser
        .work
        .clone()
        .try_lock_owned()
        .map_err(|_| "Browser pairing is still busy")?;
    let id = Uuid::new_v4();
    let generation = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if local.locked || !local.connected {
            return Err("Connect Spark and unlock Windows first".into());
        }
        let mut inner = state
            .browser
            .inner
            .lock()
            .map_err(|_| "Browser setup unavailable")?;
        if inner.settings_generation != admitted {
            return Err("Settings changed".into());
        }
        inner.generation = inner
            .generation
            .checked_add(1)
            .ok_or("Restart Avesra to reset browser ownership")?;
        inner.attempt = Some(Attempt {
            id,
            connection: state.connection_generation.load(Ordering::SeqCst),
            started: Instant::now(),
            selected: None,
            pending: None,
            approval: None,
            selection: None,
            state: "preparing",
        });
        inner.generation
    };
    tauri::async_runtime::spawn(async move {
        let _owner = owner;
        let result = run(&app, id, generation, choice).await;
        let _ = with_attempt(&app, id, generation, |_, _, attempt| {
            attempt.approval = None;
            attempt.pending = None;
            attempt.state = if result.is_ok() {
                "disconnected"
            } else {
                "unavailable_refresh_saved_pairings"
            };
            Ok(())
        });
    });
    Ok(id)
}
async fn run(
    app: &tauri::AppHandle,
    id: Uuid,
    generation: u64,
    choice: ConnectionChoice,
) -> Result<(), ErrorCode> {
    let admission = Admission::new(generation)?;
    let actor = crate::owner::current_actor(app)
        .await
        .map_err(|_| ErrorCode::Unauthenticated)?;
    with_attempt(app, id, generation, |_, _, _| Ok(()))?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| ErrorCode::Unavailable)?;
    let (selected, expected_pairing, selected_revision) = match choice {
        ConnectionChoice::Setup(phrase) => {
            let receiver = app.state::<Runtime>().effects.resolve_app(actor, phrase)?;
            let record = tokio::task::spawn_blocking(move || receiver.recv())
                .await
                .map_err(|_| ErrorCode::Unavailable)?
                .map_err(|_| ErrorCode::Unavailable)??;
            (record.record, None, None)
        }
        ConnectionChoice::Selected(revision) => {
            let path = directory.join("browser-pairings");
            let (selection, binding) = tokio::task::spawn_blocking(move || {
                Store::open(&path)?
                    .selected(Id::new(actor)?)?
                    .ok_or(ErrorCode::Unavailable)
            })
            .await
            .map_err(|_| ErrorCode::Unavailable)??;
            if selection.revision != revision {
                return Err(ErrorCode::Stale);
            }
            with_attempt(app, id, generation, |_, _, _| Ok(()))?;
            admission.current(generation)?;
            let receiver = app.state::<Runtime>().effects.inspect_app(
                actor,
                binding.browser_app.uuid(),
                binding.browser_revision.uuid(),
            )?;
            let record = tokio::task::spawn_blocking(move || receiver.recv())
                .await
                .map_err(|_| ErrorCode::Unavailable)?
                .map_err(|_| ErrorCode::Unavailable)??;
            (record, Some(selection.pairing), Some(revision))
        }
    };
    admission.current(generation)?;
    with_attempt(app, id, generation, |_, _, _| Ok(()))?;
    // Extension origin is installation configuration, never provided by the webview.
    let (extension, listener) = tokio::task::spawn_blocking(|| {
        use std::io::Read;
        let path = std::env::current_exe()
            .map_err(|_| ErrorCode::Unavailable)?
            .parent()
            .ok_or(ErrorCode::Unavailable)?
            .join("avesra-extension-origin.txt");
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| ErrorCode::Unavailable)?
            .take(257)
            .read_to_end(&mut bytes)
            .map_err(|_| ErrorCode::Unavailable)?;
        if bytes.len() > 256 {
            return Err(ErrorCode::TooLarge);
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| ErrorCode::Malformed)?
            .trim();
        let extension = text
            .strip_prefix("chrome-extension://")
            .and_then(|v| v.strip_suffix('/'))
            .filter(|v| browser::extension_id(v))
            .ok_or(ErrorCode::Malformed)?
            .to_string();
        Ok((extension, Listener::create()?))
    })
    .await
    .map_err(|_| ErrorCode::Unavailable)??;
    admission.current(generation)?;
    with_attempt(app, id, generation, |_, _, attempt| {
        attempt.selected = Some(selected.clone());
        attempt.state = "waiting_for_extension";
        Ok(())
    })?;
    // Do not drop peer inspection on cancellation: it owns blocking native work.
    let mut pipe = listener.accept().await?;
    admission.current(generation)?;
    with_attempt(app, id, generation, |_, _, _| Ok(()))?;
    let Client::Hello(hello) = browser::decode(&pipe.receive().await?)? else {
        return Err(ErrorCode::Malformed);
    };
    if expected_pairing.is_some() && hello.pairing != expected_pairing {
        return Err(ErrorCode::Unauthenticated);
    }
    let mut pending = Some(admission.challenge(hello, &extension, generation)?);
    let challenge = pending
        .as_ref()
        .ok_or(ErrorCode::Stale)?
        .challenge()
        .clone();
    let confirmation = pending
        .as_ref()
        .ok_or(ErrorCode::Stale)?
        .confirmation(generation)?;
    with_attempt(app, id, generation, |_, _, attempt| {
        attempt.pending = Some(confirmation);
        attempt.state = if challenge.pairing.is_some() {
            "authenticating"
        } else {
            "awaiting_owner"
        };
        Ok(())
    })?;
    let mut saved: Option<Saved> = None;
    if let Some(pairing) = challenge.pairing {
        let path = directory.join("browser-pairings");
        saved = Some(
            tokio::task::spawn_blocking(move || Store::open(&path)?.load(pairing, Id::new(actor)?))
                .await
                .map_err(|_| ErrorCode::Unavailable)??,
        );
        if !saved.as_ref().is_some_and(|v| {
            v.binding().browser_app.uuid() == selected.id
                && v.binding().browser_revision.uuid() == selected.revision
        }) {
            return Err(ErrorCode::Denied);
        }
    }
    with_attempt(app, id, generation, |_, _, _| Ok(()))?;
    pipe.send(
        &serde_json::to_vec(&serde_json::json!({"type":"challenge","body":challenge}))
            .map_err(|_| ErrorCode::Malformed)?,
    )
    .await?;
    let mut sequence = 0u64;
    let mut authenticated = false;
    let mut job: Option<tokio::task::JoinHandle<Result<Issuance, ErrorCode>>> = None;
    let result = async {
        loop {
            with_attempt(app,id,generation,|_,_,attempt| {
                let limit=if authenticated {300}else{browser::HANDSHAKE_SECONDS};
                if attempt.started.elapsed()>=Duration::from_secs(limit) { return Err(ErrorCode::Expired); }
                Ok(())
            })?;
            let message: Client = browser::decode(&pipe.receive().await?)?;
            with_attempt(app,id,generation,|_,_,_|Ok(()))?;
            match message {
                Client::Disconnect {session} if session==challenge.session => return Ok(()),
                Client::Poll {session,sequence:next} if session==challenge.session && sequence.checked_add(1)==Some(next) => {
                    sequence=next;
                    if job.as_ref().is_some_and(|v|v.is_finished()) {
                        let issued=job.take().ok_or(ErrorCode::Stale)?.await.map_err(|_|ErrorCode::Unavailable)??;
                        let (next, record, bytes)=issued.into_frame(generation)?;
                        with_attempt(app,id,generation,|_,_,attempt| { attempt.state="awaiting_persistence_proof"; Ok(()) })?;
                        pending=Some(next); saved=Some(record); pipe.send(&bytes).await?; continue;
                    }
                    if job.is_none() && saved.is_none() {
                        let proof=with_attempt(app,id,generation,|_,_,attempt|Ok(attempt.approval.take()))?;
                        if let Some(proof)=proof {
                            let native=pending.take().ok_or(ErrorCode::Stale)?;
                            let app=app.clone(); let directory=directory.clone(); let record=selected.clone();
                            job=Some(tokio::task::spawn_blocking(move || {
                                if !crate::owner::matches_actor(&directory, actor) {return Err(ErrorCode::Unauthenticated);}
                                let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|ErrorCode::Expired)?.as_millis();
                                let selection=Selection { actor:Id::new(actor)?, browser_app:Id::new(record.id)?,browser_revision:Id::new(record.revision)?,label:record.name,created_at_ms:u64::try_from(now).map_err(|_|ErrorCode::Expired)? };
                                native.approve(&Store::open(&directory.join("browser-pairings"))?,selection,generation,&mut |binding, challenge| {
                                    with_attempt(&app,id,generation,|state,local,attempt| {
                                        if !proof.current_locked(state,local) || !attempt.pending.as_ref().is_some_and(|v|v.challenge==challenge.challenge)
                                            || !attempt.selected.as_ref().is_some_and(|v|v.id==binding.browser_app.uuid() && v.revision==binding.browser_revision.uuid()) { return Err(ErrorCode::Stale); }
                                        Ok(())
                                    })
                                })
                            }));
                        }
                    }
                    with_attempt(app,id,generation,|_,_,_|Ok(()))?;
                    pipe.send(&serde_json::to_vec(&serde_json::json!({"type":"status","session":challenge.session,"sequence":sequence,"state":if authenticated{"authenticated_no_scopes"}else{"pending"}})).map_err(|_|ErrorCode::Malformed)?).await?;
                }
                Client::Authenticate(reply) if !authenticated && job.is_none() => {
                    let native=pending.take().ok_or(ErrorCode::Stale)?;
                    let record=saved.as_ref().ok_or(ErrorCode::Unauthenticated)?;
                    let response=with_attempt(app,id,generation,|_,_,attempt| {
                        let response=native.authenticate(record,reply,generation)?;
                        attempt.state="authenticated_no_scopes"; attempt.pending=None;
                        attempt.selection=selected_revision;
                        Ok(response)
                    })?;
                    pipe.send(&serde_json::to_vec(&serde_json::json!({"type":"authenticated","body":response})).map_err(|_|ErrorCode::Malformed)?).await?;
                    authenticated=true;
                }
                _=>return Err(ErrorCode::Malformed),
            }
        }
    }.await;
    // Invalidate publication before waiting for potentially blocked storage. The
    // top-level owned slot is not released until the actual writer exits.
    let _ = with_attempt(app, id, generation, |_, _, attempt| {
        attempt.pending = None;
        attempt.approval = None;
        attempt.state = "closing";
        Ok(())
    });
    drop(pipe);
    if let Some(job) = job {
        let _ = job.await;
    }
    result
}
