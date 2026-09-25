//! Sensitive derived candidates are isolated from ordinary settings/history.
use avesra_core::enrollment::Candidate;
use serde::Serialize;
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
    pub model_revision: String,
    pub segments: usize,
    pub state: &'static str,
}
pub fn list_candidates(directory: &Path) -> Result<Vec<CandidateSummary>, String> {
    use std::io::Read;
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
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
        if !std::fs::symlink_metadata(&path)
            .map_err(|_| "Candidate metadata unavailable")?
            .is_file()
        {
            return Err("Candidate is not a regular file".into());
        }
        let mut bytes = vec![];
        std::fs::File::open(&path)
            .map_err(|_| "Candidate unavailable")?
            .take(32_769)
            .read_to_end(&mut bytes)
            .map_err(|_| "Candidate unavailable")?;
        if bytes.len() > 32_768 {
            return Err("Candidate exceeds limit".into());
        }
        let clear = avesra_windows::credentials::unprotect(&bytes)
            .map_err(|_| "Candidate cannot be unlocked by this Windows user")?;
        let candidate: Candidate =
            serde_json::from_slice(&clear).map_err(|_| "Invalid candidate")?;
        candidate.validate().map_err(|_| "Invalid candidate")?;
        if path.file_name().and_then(|v| v.to_str())
            != Some(format!("{}-{}.dpapi", candidate.id, candidate.revision).as_str())
        {
            return Err("Candidate identity mismatch".into());
        }
        summaries.push(CandidateSummary {
            id: candidate.id,
            revision: candidate.revision,
            model_revision: candidate.model_revision,
            segments: candidate.segments,
            state: "candidate_quality_unqualified",
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
