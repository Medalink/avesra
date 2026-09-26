//! Observer-only frontend ingress and explicit local installation export.
use crate::Runtime;
use avesra_core::app_timing::{self as timing, Operation, Outcome, Stage, Window};
use serde::Deserialize;
use std::{
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;

struct Caller(Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
struct Page {
    id: Uuid,
    period: Instant,
    count: usize,
}
pub struct State {
    pages: Mutex<[Option<Page>; 2]>,
    reader: Arc<tokio::sync::Mutex<()>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            pages: Mutex::new([None, None]),
            reader: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}
fn source(window: &tauri::WebviewWindow) -> Result<(usize, Window), String> {
    match window.label() {
        "settings" => Ok((0, Window::Settings)),
        "overlay" => Ok((1, Window::Overlay)),
        _ => Err("Timing window unavailable".into()),
    }
}
#[tauri::command]
pub fn begin_app_timing(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, State>,
) -> Result<Uuid, String> {
    let (index, _) = source(&window)?;
    let mut pages = state.pages.try_lock().map_err(|_| "Timing observer busy")?;
    let id = Uuid::new_v4();
    if let Some(page) = pages[index].as_mut() {
        page.id = id;
    } else {
        pages[index] = Some(Page {
            id,
            period: Instant::now(),
            count: 0,
        });
    }
    Ok(id)
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    operation: Operation,
    stage: Stage,
    outcome: Outcome,
    duration_us: u64,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    page: Uuid,
    records: Vec<Observation>,
    lost: u32,
}
#[tauri::command]
pub fn observe_app_timings(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, State>,
    batch: Batch,
) -> Result<(), String> {
    let (index, origin) = source(&window)?;
    if batch.records.len() > 32
        || batch.lost > 1_000_000
        || serde_json::to_vec(&batch)
            .map_err(|_| "Timing batch invalid")?
            .len()
            > 32768
    {
        return Err("Timing batch exceeds bounds".into());
    }
    for r in &batch.records {
        let valid = match &r.operation {
            Operation::Command(_) => r.stage == Stage::InvokeRoundTrip,
            Operation::AppMount | Operation::View(_) => {
                matches!(r.stage, Stage::MountCommit | Stage::NextFrame)
            }
            Operation::UiReady => r.stage == Stage::Initialize,
            _ => false,
        };
        if !valid || !r.operation.valid() || r.duration_us > timing::MAX_DURATION_US {
            return Err("Timing observation invalid".into());
        }
    }
    let mut pages = state.pages.try_lock().map_err(|_| "Timing observer busy")?;
    let page = pages[index]
        .as_mut()
        .filter(|v| v.id == batch.page)
        .ok_or("Timing page changed")?;
    if page.period.elapsed() >= Duration::from_secs(1) {
        page.period = Instant::now();
        page.count = 0;
    }
    if page.count + batch.records.len() > 128 {
        return Err("Timing observation rate exceeded".into());
    }
    page.count += batch.records.len();
    drop(pages);
    avesra_core::trace::app_frontend_loss(u64::from(batch.lost));
    for r in batch.records {
        timing::frontend(
            r.operation,
            r.stage,
            r.outcome,
            origin,
            batch.page,
            r.duration_us,
        );
    }
    Ok(())
}
fn current(
    window: &tauri::WebviewWindow,
    app: &tauri::AppHandle,
    started: Instant,
    challenge: u64,
    present: &AtomicBool,
) -> Result<(), String> {
    if !present.load(Ordering::SeqCst)
        || started.elapsed() >= Duration::from_secs(5)
        || window.label() != "settings"
        || !window.is_visible().unwrap_or(false)
    {
        return Err("App timing inspection withdrawn".into());
    }
    let state = app.state::<Runtime>();
    if state.setup.challenge() != challenge
        || state
            .local
            .lock()
            .map_err(|_| "Local state unavailable")?
            .locked
    {
        return Err("App timing context changed".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn app_timing_snapshot(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<timing::Snapshot, String> {
    let started = Instant::now();
    let challenge = app.state::<Runtime>().setup.challenge();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    current(&window, &app, started, challenge, &present)?;
    let permit = app
        .state::<State>()
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Timing reader busy")?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        current(&window, &app, started, challenge, &present)?;
        let snapshot = avesra_core::trace::app_snapshot().map_err(|_| "App timings unavailable")?;
        current(&window, &app, started, challenge, &present)?;
        Ok(snapshot)
    })
    .await
    .map_err(|_| "Timing reader stopped")?
}
#[tauri::command]
pub async fn export_app_timings(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let started = Instant::now();
    let challenge = app.state::<Runtime>().setup.challenge();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    current(&window, &app, started, challenge, &present)?;
    let permit = app
        .state::<State>()
        .reader
        .clone()
        .try_lock_owned()
        .map_err(|_| "Timing writer busy")?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Private directory unavailable")?
        .join("app-timing-exports");
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        current(&window, &app, started, challenge, &present)?;
        let snapshot = avesra_core::trace::app_snapshot().map_err(|_| "App timings unavailable")?;
        let bytes = serde_json::to_vec(&snapshot).map_err(|_| "Timing encoding unavailable")?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("Timing export exceeds bound".into());
        }
        std::fs::create_dir_all(&directory).map_err(|_| "Timing export directory unavailable")?;
        let meta = std::fs::symlink_metadata(&directory)
            .map_err(|_| "Timing export directory unavailable")?;
        if !meta.is_dir() || meta.is_symlink() {
            return Err("Timing export directory invalid".into());
        }
        if std::fs::read_dir(&directory)
            .map_err(|_| "Timing export directory unavailable")?
            .take(33)
            .count()
            >= 32
        {
            return Err(
                "Timing export folder is full; move old exports before exporting again".into(),
            );
        }
        let path = directory.join(format!("app-timings-{}.json", Uuid::new_v4()));
        let temporary = path.with_extension("pending");
        let result = (|| {
            current(&window, &app, started, challenge, &present)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| "Timing export unavailable")?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Timing export failed")?;
            drop(file);
            current(&window, &app, started, challenge, &present)?;
            std::fs::hard_link(&temporary, &path).map_err(|_| "Timing publication failed")?;
            if let Err(error) = current(&window, &app, started, challenge, &present) {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
            Ok(path.to_string_lossy().into_owned())
        })();
        let _ = std::fs::remove_file(temporary);
        result
    })
    .await
    .map_err(|_| "Timing writer stopped")?
}
