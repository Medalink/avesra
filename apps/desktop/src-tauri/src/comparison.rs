//! Private saved observer reports. No caller-supplied measurements or authority.
use crate::{Runtime, connection};
use avesra_core::comparison::{self, CAPACITY, MAX_BYTES, Report, Summary};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub actor: Uuid,
    pub revision: Uuid,
    pub device: Uuid,
    pub server: String,
}
impl Binding {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        let (actor, revision) =
            crate::owner::identity(path).map_err(|_| "Native owner unavailable")?;
        let pairing = connection::load(path)?;
        Ok(Self {
            actor,
            revision,
            device: pairing.device_id,
            server: pairing.server_fingerprint()?,
        })
    }
    pub(crate) fn current(&self, path: &Path) -> Result<(), String> {
        if Self::read(path)? != *self {
            return Err("Saved report owner or pairing changed".into());
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u16,
    binding: Binding,
    report: Report,
}
fn ordinary(path: &Path, directory: bool) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| "Saved report path unavailable")?;
    use std::os::windows::fs::MetadataExt;
    if meta.file_attributes() & 0x400 != 0
        || (if directory {
            !meta.is_dir()
        } else {
            !meta.is_file()
        })
    {
        return Err("Saved report path is not an ordinary private path".into());
    }
    Ok(())
}
fn lock(path: &Path) -> Result<(PathBuf, std::fs::File), String> {
    ordinary(path, true)?;
    let directory = path.join("telemetry-cohorts");
    std::fs::create_dir_all(&directory).map_err(|_| "Saved report directory unavailable")?;
    ordinary(&directory, true)?;
    let lock_path = directory.join(".writer.lock");
    if lock_path.exists() {
        ordinary(&lock_path, false)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|_| "Saved report lock unavailable")?;
    file.try_lock().map_err(|_| "Saved report store is busy")?;
    Ok((directory, file))
}
fn ids(directory: &Path) -> Result<Vec<Uuid>, String> {
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|_| "Saved report directory unreadable")? {
        let entry = entry.map_err(|_| "Saved report entry unavailable")?;
        let name = entry.file_name();
        if name == ".writer.lock" || name == ".pending" || name == ".export-pending" {
            continue;
        }
        let name = name.to_str().ok_or("Unexpected saved report filename")?;
        let id = name
            .strip_suffix(".dpapi")
            .and_then(|v| Uuid::parse_str(v).ok())
            .filter(|v| !v.is_nil())
            .ok_or("Unexpected saved report filename")?;
        if name != format!("{id}.dpapi") {
            return Err("Noncanonical saved report filename".into());
        }
        ordinary(&entry.path(), false)?;
        ids.push(id);
        if ids.len() > CAPACITY {
            return Err("Saved report store exceeds capacity".into());
        }
    }
    Ok(ids)
}
fn read(directory: &Path, id: Uuid) -> Result<Envelope, String> {
    if id.is_nil() {
        return Err("Invalid saved report identifier".into());
    }
    let path = directory.join(format!("{id}.dpapi"));
    ordinary(&path, false)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes))
        .map_err(|_| "Saved report read failed")?;
    if bytes.len() > MAX_BYTES {
        return Err("Saved report exceeds bound".into());
    }
    let clear = avesra_windows::credentials::unprotect(&bytes)
        .map_err(|_| "Saved report protection unavailable")?;
    if clear.len() > MAX_BYTES {
        return Err("Saved report cleartext exceeds bound".into());
    }
    let envelope: Envelope =
        serde_json::from_slice(&clear).map_err(|_| "Saved report malformed")?;
    if envelope.version != 1
        || envelope.report.id != id
        || envelope.binding.revision.is_nil()
        || envelope.binding.server.len() != 64
        || !envelope
            .binding
            .server
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Saved report binding malformed".into());
    }
    envelope
        .report
        .validate(envelope.binding.actor, envelope.binding.device)
        .map_err(|_| "Saved report evidence invalid")?;
    Ok(envelope)
}
pub(crate) fn now_ms() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Clock unavailable")?
        .as_millis();
    u64::try_from(millis).map_err(|_| "Clock exceeds report bound".into())
}
pub(crate) fn save(
    path: &Path,
    binding: Binding,
    report: Report,
    current: &dyn Fn() -> Result<(), String>,
) -> Result<Uuid, String> {
    current()?;
    binding.current(path)?;
    report
        .validate(binding.actor, binding.device)
        .map_err(|_| "Selected evidence unavailable")?;
    let (directory, _writer) = lock(path)?;
    if ids(&directory)?.len() >= CAPACITY {
        return Err(
            "Saved report store is full; delete a current-owner report before saving".into(),
        );
    }
    let id = report.id;
    let envelope = Envelope {
        version: 1,
        binding,
        report,
    };
    let clear = serde_json::to_vec(&envelope).map_err(|_| "Saved report encoding failed")?;
    if clear.len() > MAX_BYTES {
        return Err("Saved report exceeds bound".into());
    }
    let protected = avesra_windows::credentials::protect(&clear)
        .map_err(|_| "Saved report protection failed")?;
    if protected.len() > MAX_BYTES {
        return Err("Protected saved report exceeds bound".into());
    }
    let temporary = directory.join(".pending");
    if temporary.exists() {
        ordinary(&temporary, false)?;
        std::fs::remove_file(&temporary).map_err(|_| "Interrupted report write unavailable")?;
    }
    let result = (|| {
        current()?;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Saved report write unavailable")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Saved report write failed")?;
        drop(file);
        envelope.binding.current(path)?;
        current()?;
        std::fs::hard_link(&temporary, directory.join(format!("{id}.dpapi")))
            .map_err(|_| "Saved report publication failed")?;
        Ok(id)
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    List,
    DeleteInaccessible,
    Compare { baseline: Uuid, candidate: Uuid },
    Delete { id: Uuid },
    Export { id: Uuid },
}
#[derive(Serialize)]
pub struct View {
    reports: Vec<Summary>,
    capacity: usize,
    occupied_slots: usize,
    inaccessible_slots: usize,
    comparison: Option<comparison::Comparison>,
    export_path: Option<String>,
}
struct Caller(Arc<AtomicBool>);
impl Drop for Caller {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
#[tauri::command]
pub async fn trace_cohorts(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    operation: Operation,
) -> Result<View, String> {
    let started = Instant::now();
    let challenge = app.state::<Runtime>().setup.challenge();
    let caller = Caller(Arc::new(AtomicBool::new(true)));
    let present = caller.0.clone();
    crate::traces::current(&window, &app, challenge, started, &present)?;
    let owner = app
        .state::<Runtime>()
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner inspection is busy")?;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "Private directory unavailable")?;
    // The real blocking reader/writer, not its caller future, owns admission.
    let work = tokio::task::spawn_blocking(move || {
        let _owner = owner;
        let current = || crate::traces::current(&window, &app, challenge, started, &present);
        current()?;
        let binding = Binding::read(&path)?;
        let (directory, _writer) = lock(&path)?;
        let saved = ids(&directory)?;
        let mut visible = Vec::new();
        let mut inaccessible = Vec::new();
        for id in &saved {
            current()?;
            let envelope = read(&directory, *id)?;
            if envelope.binding == binding {
                visible.push(envelope.report);
            } else {
                inaccessible.push(*id);
            }
        }
        let mut comparison = None;
        let mut export_path = None;
        match operation {
            Operation::List => {}
            Operation::DeleteInaccessible => {
                for id in &inaccessible {
                    binding.current(&path)?;
                    current()?;
                    std::fs::remove_file(directory.join(format!("{id}.dpapi"))).map_err(
                        |_| "Inaccessible report cleanup incomplete; refresh before retrying",
                    )?;
                }
                inaccessible.clear();
            }
            Operation::Compare {
                baseline,
                candidate,
            } => {
                let a = visible
                    .iter()
                    .find(|v| v.id == baseline)
                    .ok_or("Current-owner baseline unavailable")?;
                let b = visible
                    .iter()
                    .find(|v| v.id == candidate)
                    .ok_or("Current-owner candidate unavailable")?;
                comparison = Some(
                    comparison::compare(a, b).map_err(|_| "Saved cohorts cannot be compared")?,
                );
            }
            Operation::Delete { id } => {
                if !visible.iter().any(|v| v.id == id) {
                    return Err("Current-owner report unavailable".into());
                }
                binding.current(&path)?;
                current()?;
                std::fs::remove_file(directory.join(format!("{id}.dpapi")))
                    .map_err(|_| "Saved report delete failed")?;
                visible.retain(|v| v.id != id);
            }
            Operation::Export { id } => {
                let report = visible
                    .iter()
                    .find(|v| v.id == id)
                    .ok_or("Current-owner report unavailable")?;
                // Redacted source-binding facts are preserved in the export; no
                // current deployment/process identity is stamped onto old records.
                let bytes = serde_json::to_vec(
                    &serde_json::json!({"version":1,"binding":&binding,"report":report}),
                )
                .map_err(|_| "Report export encoding failed")?;
                if bytes.len() > MAX_BYTES {
                    return Err("Report export exceeds bound".into());
                }
                let export_dir = path.join("telemetry-exports");
                std::fs::create_dir_all(&export_dir).map_err(|_| "Export directory unavailable")?;
                ordinary(&export_dir, true)?;
                let target = export_dir.join(format!("saved-cohort-{id}-{}.json", Uuid::new_v4()));
                let temporary = directory.join(".export-pending");
                if temporary.exists() {
                    ordinary(&temporary, false)?;
                    std::fs::remove_file(&temporary)
                        .map_err(|_| "Interrupted export unavailable")?;
                }
                let written = (|| {
                    binding.current(&path)?;
                    current()?;
                    let mut file = std::fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(&temporary)
                        .map_err(|_| "Export file unavailable")?;
                    file.write_all(&bytes)
                        .and_then(|_| file.sync_all())
                        .map_err(|_| "Report export write failed")?;
                    drop(file);
                    binding.current(&path)?;
                    current()?;
                    std::fs::hard_link(&temporary, &target)
                        .map_err(|_| "Report export publication failed")?;
                    Ok::<_, String>(())
                })();
                let _ = std::fs::remove_file(temporary);
                written?;
                export_path = Some(target.to_string_lossy().into_owned());
            }
        }
        if let Err(error) = binding.current(&path).and_then(|_| current()) {
            if let Some(exported) = &export_path
                && std::fs::remove_file(exported).is_err()
            {
                return Err(format!(
                    "Export withdrawn, but file cleanup failed: {exported}"
                ));
            }
            return Err(error);
        }
        visible.sort_by_key(|v| std::cmp::Reverse(v.created_ms));
        Ok(View {
            reports: visible.iter().map(Report::summary).collect(),
            capacity: CAPACITY,
            occupied_slots: visible.len() + inaccessible.len(),
            inaccessible_slots: inaccessible.len(),
            comparison,
            export_path,
        })
    });
    let result = work.await.map_err(|_| "Saved report coordinator stopped")?;
    drop(caller);
    result
}
