//! Protected, bounded history inspection. Stored content never creates a live turn.
use super::*;
use avesra_core::conversations::{history as core, search};
use serde::Deserialize;
use std::sync::atomic::AtomicU64;
#[path = "tasks_history_deletion.rs"]
pub mod deletion;

#[derive(Default)]
pub struct State {
    reader: Mutex<Option<Arc<Reader>>>,
    generation: AtomicU64,
    pending: Mutex<Option<Arc<deletion::Ticket>>>,
}
impl State {
    pub(super) fn invalidate(&self) {
        let _ = self
            .generation
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_add(1));
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
        if let Ok(mut value) = self.reader.lock() {
            *value = None;
        }
    }
}
#[derive(PartialEq, Eq)]
struct Binding {
    actor: Uuid,
    revision: Uuid,
    device: Uuid,
    server: String,
}
impl Binding {
    fn read(directory: &std::path::Path) -> Result<Self, String> {
        let (actor, revision) =
            crate::owner::identity(directory).map_err(|_| "History owner unavailable")?;
        let pairing = connection::load(directory)?;
        Ok(Self {
            actor,
            revision,
            device: pairing.device_id,
            server: pairing.server_fingerprint()?,
        })
    }
}
struct Reader {
    generation: u64,
    id: Uuid,
    panel: Uuid,
    binding: Binding,
    proof: ManagementProof,
    action_epoch: u64,
    cursor: Mutex<Option<(Uuid, core::Cursor)>>,
    anchor: Mutex<Option<core::Cursor>>,
    search: Mutex<Option<(Uuid, search::Cursor)>>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Search { query: search::Query },
    SearchMore { cursor: Uuid },
    Index,
    Page { cursor: Option<Uuid> },
    MemorySource { memory: Uuid, revision: Uuid },
}
#[derive(Serialize)]
pub struct View {
    search_cursor: Option<Uuid>,
    coverage: Option<search::Coverage>,
    reader: Uuid,
    cursor: Option<Uuid>,
    remaining_ms: u64,
    page: Option<core::Page>,
    source: Option<core::SourceView>,
}
struct Caller(Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn eligible(
    app: &tauri::AppHandle,
    panel: Uuid,
    challenge: u64,
    started: Instant,
    present: &AtomicBool,
) -> Result<(), ErrorCode> {
    if !present.load(Ordering::SeqCst)
        || started.elapsed() >= Duration::from_secs(12)
        || app.state::<Runtime>().setup.challenge() != challenge
        || current(app, panel).is_err()
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
fn reader_current(app: &tauri::AppHandle, reader: &Reader) -> Result<(), ErrorCode> {
    if reader.generation == u64::MAX
        || app
            .state::<Runtime>()
            .tasks
            .history
            .generation
            .load(Ordering::SeqCst)
            != reader.generation
        || current(app, reader.panel).is_err()
        || !reader.proof.current(&app.state::<Runtime>())
    {
        return Err(ErrorCode::Stale);
    }
    let state = app.state::<Runtime>();
    let local = state.local.lock().map_err(|_| ErrorCode::Unavailable)?;
    if local.action_epoch != reader.action_epoch {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
#[tauri::command]
pub async fn inspect_conversation_history(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    panel: Uuid,
    reader: Option<Uuid>,
    query: Query,
) -> Result<View, String> {
    let started = Instant::now();
    let challenge = app.state::<Runtime>().setup.challenge();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    visible(&window)?;
    eligible(&app, panel, challenge, started, &present)
        .map_err(|_| "History inspection withdrawn")?;
    let state = app.state::<Runtime>();
    let admission = Arc::new(
        state
            .tasks
            .reader
            .clone()
            .try_lock_owned()
            .map_err(|_| "History reader is busy")?,
    );
    let owner = Arc::new(
        state
            .owner_setup
            .clone()
            .try_lock_owned()
            .map_err(|_| "Owner inspection is busy")?,
    );
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "Private directory unavailable")?;
    // One retained coordinator and actual worker both retain admission. A dropped
    // caller only withdraws publication; it never releases an in-flight reader.
    let work = tauri::async_runtime::spawn(async move {
        let read_path = path.clone();
        let read_owner = owner.clone();
        let read_admission = admission.clone();
        let binding = tokio::task::spawn_blocking(move || {
            let _owner = read_owner;
            let _admission = read_admission;
            Binding::read(&read_path)
        })
        .await
        .map_err(|_| "History identity reader stopped")??;
        eligible(&app, panel, challenge, started, &present)
            .map_err(|_| "History inspection withdrawn")?;
        let session = if let Some(id) = reader {
            let existing = app
                .state::<Runtime>()
                .tasks
                .history
                .reader
                .lock()
                .map_err(|_| "History session unavailable")?
                .clone()
                .filter(|value| value.id == id && value.panel == panel)
                .ok_or("History reader expired; verify Windows Hello and reopen history")?;
            if existing.binding != binding {
                return Err("History owner or pairing changed".into());
            }
            reader_current(&app, &existing)
                .map_err(|_| "History reader expired; verify Windows Hello and reopen history")?;
            existing
        } else {
            if matches!(&query, Query::Page { cursor: Some(_) }) {
                return Err("Reopen history before paging".into());
            }
            let proof = proof(&app)?;
            let action_epoch = app
                .state::<Runtime>()
                .local
                .lock()
                .map_err(|_| "Local state unavailable")?
                .action_epoch;
            app.state::<Runtime>().tasks.history.invalidate();
            let value = Arc::new(Reader {
                generation: app
                    .state::<Runtime>()
                    .tasks
                    .history
                    .generation
                    .load(Ordering::SeqCst),
                id: Uuid::new_v4(),
                panel,
                binding,
                proof,
                action_epoch,
                cursor: Mutex::new(None),
                anchor: Mutex::new(None),
                search: Mutex::new(None),
            });
            reader_current(&app, &value).map_err(|_| "History proof withdrawn")?;
            *app.state::<Runtime>()
                .tasks
                .history
                .reader
                .lock()
                .map_err(|_| "History session unavailable")? = Some(value.clone());
            value
        };
        let observed = Instant::now();
        let search_deadline = (started + Duration::from_secs(12))
            .min(observed + Duration::from_millis(session.proof.remaining_ms()));
        let request = match query {
            Query::Search { query } => core::Query::Search {
                query: Some(query),
                cursor: None,
                deadline: search_deadline,
            },
            Query::SearchMore { cursor } => {
                let cursor = session
                    .search
                    .lock()
                    .map_err(|_| "Search cursor unavailable")?
                    .as_ref()
                    .filter(|(id, _)| *id == cursor)
                    .map(|(_, c)| c.clone())
                    .ok_or("Search changed; start a new search")?;
                core::Query::Search {
                    query: None,
                    cursor: Some(cursor),
                    deadline: search_deadline,
                }
            }
            Query::Index => core::Query::Index {
                deadline: search_deadline,
            },
            Query::MemorySource { memory, revision } => {
                core::Query::MemorySource { memory, revision }
            }
            Query::Page { cursor } => {
                let next = match cursor {
                    None => session
                        .anchor
                        .lock()
                        .map_err(|_| "History anchor unavailable")?
                        .clone(),
                    Some(id) => Some(
                        session
                            .cursor
                            .lock()
                            .map_err(|_| "History cursor unavailable")?
                            .as_ref()
                            .filter(|(token, _)| *token == id)
                            .map(|(_, value)| value.clone())
                            .ok_or("History cursor changed; reopen the first page")?,
                    ),
                };
                core::Query::Page(next)
            }
        };
        let author_app = app.clone();
        let author_reader = session.clone();
        let author_present = present.clone();
        let author_owner = owner.clone();
        let author_admission = admission.clone();
        let authorize = Box::new(move || {
            let _owner = &author_owner;
            let _admission = &author_admission;
            eligible(&author_app, panel, challenge, started, &author_present)?;
            reader_current(&author_app, &author_reader)
        });
        let receiver = app
            .state::<Runtime>()
            .effects
            .conversation_history(
                session.binding.actor,
                session.binding.device,
                request,
                authorize,
            )
            .map_err(|_| "History ledger worker busy")?;
        let result = receive(receiver).await?;
        eligible(&app, panel, challenge, started, &present)
            .map_err(|_| "History inspection withdrawn")?;
        reader_current(&app, &session).map_err(|_| "History proof expired")?;
        let verify_path = path;
        let verify_owner = owner.clone();
        let verify_admission = admission.clone();
        let actual = tokio::task::spawn_blocking(move || {
            let _owner = verify_owner;
            let _admission = verify_admission;
            Binding::read(&verify_path)
        })
        .await
        .map_err(|_| "History identity verification stopped")??;
        if actual != session.binding {
            return Err("History owner or pairing changed".into());
        }
        eligible(&app, panel, challenge, started, &present)
            .map_err(|_| "History inspection withdrawn")?;
        reader_current(&app, &session).map_err(|_| "History proof expired")?;
        if let Some(anchor) = result.anchor {
            *session
                .anchor
                .lock()
                .map_err(|_| "History anchor unavailable")? = Some(anchor);
        }
        let cursor = result.next.map(|value| (Uuid::new_v4(), value));
        let token = cursor.as_ref().map(|(id, _)| *id);
        if result.page.is_some() {
            *session
                .cursor
                .lock()
                .map_err(|_| "History cursor unavailable")? = cursor;
        }
        let search_cursor = result.search_next.map(|value| (Uuid::new_v4(), value));
        let search_token = search_cursor.as_ref().map(|(id, _)| *id);
        *session
            .search
            .lock()
            .map_err(|_| "Search cursor unavailable")? = search_cursor;
        let remaining_ms = session.proof.remaining_ms();
        if remaining_ms == 0 {
            return Err("History proof expired".into());
        }
        drop(owner);
        drop(admission);
        Ok(View {
            search_cursor: search_token,
            coverage: result.coverage,
            reader: session.id,
            cursor: token,
            remaining_ms,
            page: result.page,
            source: result.source,
        })
    });
    let result = work.await.map_err(|_| "History coordinator stopped")?;
    drop(caller);
    result
}
