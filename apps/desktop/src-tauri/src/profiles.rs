//! Sensitive derived candidates are isolated from ordinary settings/history.
use avesra_core::enrollment::Candidate;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};
use uuid::Uuid;
fn lock_directory(directory: &Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(directory).map_err(|_| "Candidate directory unavailable")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".writer.lock"))
        .map_err(|_| "Candidate lock unavailable")?;
    file.try_lock().map_err(|_| "Candidate store is busy")?;
    Ok(file)
}

#[derive(Serialize)]
pub struct CandidateSummary {
    pub id: Uuid,
    pub revision: Uuid,
    pub model_revision: Option<String>,
    pub segments: Option<usize>,
    pub state: &'static str,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    version: u16,
    id: Uuid,
    revision: Uuid,
    model_revision: String,
    microphone: String,
}
fn selected(directory: &Path) -> Result<Option<Selection>, String> {
    use std::io::Read;
    let path = directory.join("speaker-selection.dpapi");
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err("Selected profile unavailable; clear selection after verification".into());
        }
    };
    if !metadata.is_file() || metadata.len() > 8192 {
        return Err("Selected profile invalid; clear selection after verification".into());
    }
    let mut bytes = vec![];
    std::fs::File::open(path)
        .and_then(|file| file.take(8193).read_to_end(&mut bytes))
        .map_err(|_| "Selected profile unavailable")?;
    if bytes.len() > 8192 {
        return Err("Selected profile exceeds limit".into());
    }
    let bytes = avesra_windows::credentials::unprotect(&bytes)
        .map_err(|_| "Selected profile cannot be decrypted")?;
    let value: Selection =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid selected profile")?;
    if value.version != 1
        || value.id.is_nil()
        || value.revision.is_nil()
        || value.model_revision.len() != 40
        || !value.model_revision.bytes().all(|v| v.is_ascii_hexdigit())
        || value.microphone.is_empty()
        || value.microphone.len() > 512
    {
        return Err("Invalid selected profile".into());
    }
    Ok(Some(value))
}
pub fn select_candidate(
    directory: &Path,
    id: Uuid,
    revision: Uuid,
    microphone: &str,
) -> Result<(), String> {
    use std::io::Read;
    if id.is_nil() || revision.is_nil() {
        return Err("Invalid candidate identity".into());
    }
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    if selected(directory)?.is_some() {
        return Err("Clear the current selection before replacing it".into());
    }
    let path = directory
        .join("speaker-candidates")
        .join(format!("{id}-{revision}.dpapi"));
    if !std::fs::symlink_metadata(&path)
        .map_err(|_| "Candidate unavailable")?
        .is_file()
    {
        return Err("Candidate is not a regular record".into());
    }
    let mut bytes = vec![];
    std::fs::File::open(path)
        .and_then(|file| file.take(32769).read_to_end(&mut bytes))
        .map_err(|_| "Candidate unavailable")?;
    if bytes.len() > 32768 {
        return Err("Candidate exceeds limit".into());
    }
    let clear = avesra_windows::credentials::unprotect(&bytes)
        .map_err(|_| "Candidate cannot be decrypted")?;
    let candidate: Candidate = serde_json::from_slice(&clear).map_err(|_| "Candidate invalid")?;
    candidate.validate().map_err(|_| "Candidate invalid")?;
    if candidate.id != id
        || candidate.revision != revision
        || candidate.microphone != microphone
        || candidate.model_revision != "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286"
    {
        return Err("Candidate does not match this microphone and speaker model".into());
    }
    let value = Selection {
        version: 1,
        id,
        revision,
        model_revision: candidate.model_revision,
        microphone: candidate.microphone,
    };
    let encoded = serde_json::to_vec(&value).map_err(|_| "Selection encoding failed")?;
    let protected = avesra_windows::credentials::protect(&encoded)
        .map_err(|_| "Selection protection failed")?;
    let temporary = directory.join(".speaker-selection.pending");
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Interrupted selection unavailable".into()),
    }
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Selection storage unavailable")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Selection storage failed")?;
        drop(file);
        std::fs::hard_link(&temporary, directory.join("speaker-selection.dpapi"))
            .map_err(|_| "Selection publication failed")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
pub fn clear_selection(directory: &Path) -> Result<(), String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    match std::fs::remove_file(directory.join("speaker-selection.dpapi")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Selection could not be cleared".into()),
    }
}
pub fn list_candidates(directory: &Path) -> Result<Vec<CandidateSummary>, String> {
    use std::io::Read;
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    let selection = selected(directory);
    let entries = match std::fs::read_dir(directory.join("speaker-candidates")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err("Candidate directory unavailable".into()),
    };
    let mut summaries = vec![];
    for (index, entry) in entries.enumerate() {
        if index >= 4096 {
            return Err("Candidate directory exceeds limit".into());
        }
        let path = entry.map_err(|_| "Candidate entry unavailable")?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("dpapi") {
            continue;
        }
        if summaries.len() >= 128 {
            return Err("Candidate count exceeds limit".into());
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if stem.len() != 73 || !stem.is_ascii() || &stem[36..37] != "-" {
            continue;
        }
        let (Ok(id), Ok(revision)) = (Uuid::parse_str(&stem[..36]), Uuid::parse_str(&stem[37..]))
        else {
            continue;
        };
        if id.is_nil() || revision.is_nil() || stem != format!("{id}-{revision}") {
            continue;
        }
        let decoded = (|| -> Option<Candidate> {
            if !std::fs::symlink_metadata(&path).ok()?.is_file() {
                return None;
            }
            let mut bytes = vec![];
            std::fs::File::open(&path)
                .ok()?
                .take(32_769)
                .read_to_end(&mut bytes)
                .ok()?;
            if bytes.len() > 32_768 {
                return None;
            }
            let clear = avesra_windows::credentials::unprotect(&bytes).ok()?;
            let candidate: Candidate = serde_json::from_slice(&clear).ok()?;
            candidate.validate().ok()?;
            (candidate.id == id && candidate.revision == revision).then_some(candidate)
        })();
        summaries.push(CandidateSummary {
            id,
            revision,
            model_revision: decoded.as_ref().map(|value| value.model_revision.clone()),
            segments: decoded.as_ref().map(|value| value.segments),
            state: if selection.is_err() {
                "selection_unavailable"
            } else if decoded.is_some()
                && selection
                    .as_ref()
                    .ok()
                    .and_then(|value| value.as_ref())
                    .is_some_and(|selected| selected.id == id && selected.revision == revision)
            {
                "selected_quality_unqualified"
            } else if decoded.is_some() {
                "candidate_quality_unqualified"
            } else {
                "unreadable_candidate"
            },
        });
    }
    Ok(summaries)
}
pub fn remove_candidate(directory: &Path, id: Uuid, revision: Uuid) -> Result<(), String> {
    if id.is_nil() || revision.is_nil() {
        return Err("Invalid candidate identity".into());
    }
    let path = directory
        .join("speaker-candidates")
        .join(format!("{id}-{revision}.dpapi"));
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    if selected(directory)?
        .is_some_and(|selected| selected.id == id && selected.revision == revision)
    {
        std::fs::remove_file(directory.join("speaker-selection.dpapi"))
            .map_err(|_| "Selected profile could not be cleared")?;
    }
    std::fs::remove_file(path).map_err(|_| "Candidate could not be removed".into())
}

pub fn save_candidate(directory: &Path, candidate: &Candidate) -> Result<(), String> {
    candidate
        .validate()
        .map_err(|_| "Invalid speaker candidate")?;
    let bytes = serde_json::to_vec(candidate).map_err(|_| "Candidate encoding failed")?;
    if bytes.len() > 16_384 {
        return Err("Candidate exceeds storage limit".into());
    }
    let protected = avesra_windows::credentials::protect(&bytes)
        .map_err(|_| "Windows profile protection failed")?;
    let directory = directory.join("speaker-candidates");
    let _lock = lock_directory(&directory)?;
    let mut count = 0;
    for (index, entry) in std::fs::read_dir(&directory)
        .map_err(|_| "Candidate directory unavailable")?
        .enumerate()
    {
        if index >= 4096 {
            return Err("Candidate directory exceeds limit".into());
        }
        if entry
            .map_err(|_| "Candidate entry unavailable")?
            .path()
            .extension()
            .and_then(|v| v.to_str())
            == Some("dpapi")
        {
            count += 1;
        }
    }
    if count >= 128 {
        return Err("Remove a previous candidate before collecting another".into());
    }
    // The exclusive OS file lock proves no other candidate writer owns this
    // fixed temporary path. An interrupted write cannot accumulate temp files.
    let temporary = directory.join(".candidate.pending");
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Interrupted candidate cannot be cleared".into()),
    }
    let destination = directory.join(format!("{}-{}.dpapi", candidate.id, candidate.revision));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Candidate storage unavailable")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Candidate storage failed")?;
        drop(file);
        std::fs::hard_link(&temporary, &destination).map_err(|_| "Candidate publication failed")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
