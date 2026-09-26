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
    pub read_error: Option<String>,
}
// Native only. Never serialize this return value to the webview.
pub fn read_candidate(directory: &Path, id: Uuid, revision: Uuid) -> Result<Candidate, String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    read_candidate_locked(directory, id, revision)
}
/// Native startup reuses the selection, or an unambiguous sole saved candidate.
pub fn personal_seed(directory: &Path) -> Result<Option<Candidate>, String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    if let Some(value) = selected(directory)? {
        return read_candidate_locked(directory, value.id, value.revision).map(Some);
    }
    let mut found = None;
    for entry in std::fs::read_dir(directory.join("speaker-candidates"))
        .map_err(|_| "Saved voices unavailable")?
    {
        let entry = entry.map_err(|_| "Saved voice unreadable")?;
        if entry.path().extension().is_none_or(|v| v != "dpapi") {
            continue;
        }
        if found.is_some() {
            return Err("Choose which saved voice belongs to you in People".into());
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or("Saved voice filename invalid")?;
        let stem = name
            .strip_suffix(".dpapi")
            .ok_or("Saved voice filename invalid")?;
        if stem.len() != 73 || !stem.is_ascii() || stem.as_bytes()[36] != b'-' {
            return Err("Saved voice filename invalid".into());
        }
        let id = Uuid::parse_str(&stem[..36]).map_err(|_| "Saved voice identity invalid")?;
        let revision = Uuid::parse_str(&stem[37..]).map_err(|_| "Saved voice revision invalid")?;
        found = Some(read_candidate_locked(directory, id, revision)?);
    }
    Ok(found)
}

fn personal_path(
    directory: &Path,
    voice: &avesra_core::voice::personal::Voice,
) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let binding = format!(
        "{}\n{}\n{}\n{}\n{:?}",
        voice.actor, voice.owner_revision, voice.microphone, voice.model_revision, voice.source
    );
    directory
        .join("personal-voices")
        .join(format!("{:x}.dpapi", Sha256::digest(binding.as_bytes())))
}
pub fn read_personal(
    directory: &Path,
    expected: &avesra_core::voice::personal::Voice,
) -> Result<Option<avesra_core::voice::personal::Voice>, String> {
    use std::io::Read;
    let _lock = lock_directory(&directory.join("personal-voices"))?;
    let path = personal_path(directory, expected);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Personal voice unavailable".into()),
    };
    if !meta.is_file() || meta.is_symlink() || meta.len() > 262144 {
        return Err("Personal voice record invalid".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|v| v.take(262145).read_to_end(&mut bytes))
        .map_err(|_| "Personal voice unavailable")?;
    if bytes.len() > 262144 {
        return Err("Personal voice limit".into());
    }
    let clear = avesra_windows::credentials::unprotect(&bytes)
        .map_err(|_| "Personal voice protection invalid")?;
    let voice: avesra_core::voice::personal::Voice =
        serde_json::from_slice(&clear).map_err(|_| "Personal voice invalid")?;
    voice
        .validate()
        .map_err(|_| "Personal voice measurements invalid")?;
    if voice.actor != expected.actor
        || voice.owner_revision != expected.owner_revision
        || voice.microphone != expected.microphone
        || voice.model_revision != expected.model_revision
        || voice.source != expected.source
        || voice.seed != expected.seed
    {
        return Err("Personal voice binding changed".into());
    }
    Ok(Some(voice))
}
pub fn save_personal(
    directory: &Path,
    voice: &avesra_core::voice::personal::Voice,
    publish: &mut dyn FnMut(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    voice
        .validate()
        .map_err(|_| "Invalid personal measurements")?;
    let _lock = lock_directory(&directory.join("personal-voices"))?;
    if crate::owner::identity(directory).map_err(|_| "Owner unavailable")?
        != (voice.actor, voice.owner_revision)
    {
        return Err("Personal owner changed".into());
    }
    let bytes = serde_json::to_vec(voice).map_err(|_| "Personal voice encoding failed")?;
    if bytes.len() > 200000 {
        return Err("Personal voice limit".into());
    }
    let protected = avesra_windows::credentials::protect(&bytes)
        .map_err(|_| "Personal voice protection failed")?;
    let destination = personal_path(directory, voice);
    let temporary = destination.with_extension("pending");
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Personal voice writer unavailable")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Personal voice write failed")?;
        drop(file);
        publish(&temporary, &destination)
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
fn read_candidate_locked(directory: &Path, id: Uuid, revision: Uuid) -> Result<Candidate, String> {
    use std::io::Read;
    if id.is_nil() || revision.is_nil() {
        return Err("Invalid candidate identity".into());
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
    if candidate.id != id || candidate.revision != revision {
        return Err("Candidate identity changed".into());
    }
    Ok(candidate)
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
    if value.version != 2
        || value.id.is_nil()
        || value.revision.is_nil()
        || value.model_revision.len() != 40
        || !value.model_revision.bytes().all(|v| v.is_ascii_hexdigit())
        || !avesra_core::state::valid_audio_device_id(&value.microphone)
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
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    if selected(directory)?.is_some() {
        return Err("Clear the current selection before replacing it".into());
    }
    let candidate = read_candidate_locked(directory, id, revision)?;
    if candidate.microphone != microphone
        || candidate.model_revision != "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286"
    {
        return Err("Candidate does not match this microphone and speaker model".into());
    }
    let value = Selection {
        version: 2,
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
        authorize()?;
        std::fs::hard_link(&temporary, directory.join("speaker-selection.dpapi"))
            .map_err(|_| "Selection publication failed")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
pub fn clear_selection(
    directory: &Path,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    match std::fs::remove_file(directory.join("speaker-selection.dpapi")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Selection could not be cleared".into()),
    }
}
pub fn list_candidates(directory: &Path) -> Result<Vec<CandidateSummary>, String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    let selection = selected(directory);
    let summarize = |id, revision| {
        let decoded = read_candidate_locked(directory, id, revision);
        CandidateSummary {
            id,
            revision,
            model_revision: decoded
                .as_ref()
                .ok()
                .map(|value| value.model_revision.clone()),
            segments: decoded.as_ref().ok().map(|value| value.segments),
            state: if selection.is_err() {
                "selection_unavailable"
            } else if decoded.is_ok()
                && selection
                    .as_ref()
                    .ok()
                    .and_then(|value| value.as_ref())
                    .is_some_and(|selected| selected.id == id && selected.revision == revision)
            {
                "selected_quality_unqualified"
            } else if decoded.is_ok() {
                "candidate_quality_unqualified"
            } else {
                "unreadable_candidate"
            },
            read_error: decoded.err(),
        }
    };
    let mut summaries = vec![];
    if let Ok(Some(selected)) = &selection {
        summaries.push(summarize(selected.id, selected.revision));
    }
    let entries = match std::fs::read_dir(directory.join("speaker-candidates")) {
        Ok(entries) => entries,
        Err(_) => return Err("Candidate directory unavailable".into()),
    };
    for (index, entry) in entries.enumerate() {
        if index >= 4096 {
            return Err("Candidate directory exceeds limit".into());
        }
        let path = entry.map_err(|_| "Candidate entry unavailable")?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("dpapi") {
            continue;
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
        if summaries
            .iter()
            .any(|value| value.id == id && value.revision == revision)
        {
            continue;
        }
        if summaries.len() >= 128 {
            return Err("Candidate count exceeds limit".into());
        }
        summaries.push(summarize(id, revision));
    }
    if selection.is_err() && summaries.iter().all(|value| value.segments.is_none()) {
        return Err(
            "Saved voice selection could not be read. Your saved files have not been changed."
                .into(),
        );
    }
    Ok(summaries)
}
pub fn remove_candidate(
    directory: &Path,
    id: Uuid,
    revision: Uuid,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    if id.is_nil() || revision.is_nil() {
        return Err("Invalid candidate identity".into());
    }
    let path = directory
        .join("speaker-candidates")
        .join(format!("{id}-{revision}.dpapi"));
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    let is_selected = selected(directory)?
        .is_some_and(|selected| selected.id == id && selected.revision == revision);
    authorize()?;
    if is_selected {
        std::fs::remove_file(directory.join("speaker-selection.dpapi"))
            .map_err(|_| "Selected profile could not be cleared")?;
        authorize().map_err(
            |_| "Selection was cleared, but candidate removal was cancelled; refresh stored status",
        )?;
    }
    std::fs::remove_file(path).map_err(|_| {
        if is_selected {
            "Selection was cleared, but candidate removal failed; refresh stored status"
        } else {
            "Candidate could not be removed; refresh stored status"
        }
        .into()
    })
}

pub fn save_candidate(
    directory: &Path,
    candidate: &Candidate,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
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
        authorize()?;
        std::fs::hard_link(&temporary, &destination).map_err(|_| "Candidate publication failed")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}

/// Native-only reviewed evidence; never serialize this to the webview.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationRecord {
    pub version: u16,
    pub owner_revision: Uuid,
    pub registration: avesra_contracts::actors::Binding,
    pub server: String,
    pub candidate_digest: String,
    pub directed_artifact: String,
    pub directed_incarnation: String,
    #[serde(default)]
    pub directed_quality: Option<String>,
    pub report: avesra_core::voice::qualification::Report,
}
pub fn candidate_digest(candidate: &Candidate) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(candidate).map_err(|_| "Candidate encoding failed")?)
    ))
}
fn validate_qualification(
    directory: &Path,
    value: &QualificationRecord,
) -> Result<Candidate, String> {
    let (id, revision) = value.report.candidate();
    let candidate = read_candidate_locked(directory, id, revision)?;
    let owner = crate::owner::identity(directory).map_err(|_| "Authenticated owner unavailable")?;
    let selected = selected(directory)?.ok_or("Select the reviewed candidate first")?;
    value
        .registration
        .validate()
        .map_err(|_| "Invalid stored registration")?;
    if value.version != 1
        || value.owner_revision.is_nil()
        || value.registration.revoked
        || owner != (value.report.point().actor, value.owner_revision)
        || value.registration.actor != owner.0
        || value.registration.owner_revision != owner.1
        || selected.id != id
        || selected.revision != revision
        || candidate_digest(&candidate)? != value.candidate_digest
        || value.server.len() != 64
        || !value.server.bytes().all(|v| v.is_ascii_hexdigit())
        || value.directed_artifact.is_empty()
        || value.directed_artifact.len() > 256
        || value.directed_incarnation.is_empty()
        || value.directed_incarnation.len() > 256
        || value.directed_quality.as_ref().is_some_and(|value| {
            value.len() != 64
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        })
    {
        return Err("Reviewed qualification identity changed".into());
    }
    Ok(candidate)
}
pub fn read_qualification(
    directory: &Path,
) -> Result<Option<(QualificationRecord, Candidate)>, String> {
    use std::io::Read;
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    let path = directory.join("voice-qualification.dpapi");
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Qualification report unavailable".into()),
    };
    if !meta.is_file() || meta.len() > 131072 {
        return Err("Qualification report exceeds limit".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|v| v.take(131073).read_to_end(&mut bytes))
        .map_err(|_| "Qualification read failed")?;
    if bytes.len() > 131072 {
        return Err("Qualification report exceeds limit".into());
    }
    let clear = avesra_windows::credentials::unprotect(&bytes)
        .map_err(|_| "Qualification protection invalid")?;
    if clear.len() > 98304 {
        return Err("Qualification cleartext exceeds limit".into());
    }
    let record: QualificationRecord =
        serde_json::from_slice(&clear).map_err(|_| "Invalid qualification report")?;
    let candidate = validate_qualification(directory, &record)?;
    Ok(Some((record, candidate)))
}
pub fn save_qualification(
    directory: &Path,
    value: &QualificationRecord,
    publish: &mut dyn FnMut(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    validate_qualification(directory, value)?;
    let clear = serde_json::to_vec(value).map_err(|_| "Qualification encoding failed")?;
    if clear.len() > 98304 {
        return Err("Qualification exceeds storage limit".into());
    }
    let protected = avesra_windows::credentials::protect(&clear)
        .map_err(|_| "Qualification protection failed")?;
    if protected.len() > 131072 {
        return Err("Protected qualification exceeds limit".into());
    }
    let temporary = directory.join(".voice-qualification.pending");
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Interrupted qualification write unavailable".into()),
    }
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Qualification storage unavailable")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Qualification storage failed")?;
        drop(file);
        publish(&temporary, &directory.join("voice-qualification.dpapi"))?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
pub fn revoke_qualification(directory: &Path) -> Result<(), String> {
    let _lock = lock_directory(&directory.join("speaker-candidates"))?;
    match std::fs::remove_file(directory.join("voice-qualification.dpapi")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Protected qualification could not be revoked".into()),
    }
}
