//! Optional actor-bound presentation. This module never creates voice authority.
#[path = "voice_avatar_shape.rs"]
mod shape;
use avesra_core::voice::personal::Voice;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) use shape::Parameters;
use std::{
    io::{Read, Write},
    path::Path,
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const SPEAKER: &str = "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286";
const MAX_ACTORS: usize = 128;
const MAX_RECORDS: usize = 32;
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Source {
    Candidate { id: Uuid, revision: Uuid },
    Personal { id: Uuid },
}
impl Source {
    fn valid(&self) -> bool {
        match self {
            Self::Candidate { id, revision } => !id.is_nil() && !revision.is_nil(),
            Self::Personal { id } => !id.is_nil(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    owner_revision: Uuid,
    source: Source,
    binding_digest: String,
    source_digest: String,
    parameters: Parameters,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Actor {
    actor: Uuid,
    counter: u32,
    ring: String,
    rotation: u8,
    record: Option<Record>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vault {
    version: u16,
    key: [u8; 32],
    actors: Vec<Actor>,
}
impl Drop for Vault {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
#[derive(Serialize)]
pub(crate) struct View {
    state: &'static str,
    parameters: Option<Parameters>,
    reason: Option<String>,
}
impl View {
    pub(crate) fn unavailable(reason: String) -> Self {
        Self {
            state: "unavailable",
            parameters: None,
            reason: Some(reason),
        }
    }
    fn missing() -> Self {
        Self { state:"source_unavailable", parameters:None, reason:Some("Your avatar will be available after Avesra has saved real voice observations for this owner and microphone. You can keep talking normally.".into()) }
    }
}
fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn bounded(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Avatar storage is unavailable".into()),
    };
    if !meta.is_file() || meta.is_symlink() || meta.len() > limit {
        return Err("Avatar storage is invalid or exceeds its limit".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|_| "Avatar read failed")?;
    if bytes.len() as u64 > limit {
        return Err("Avatar read exceeds its limit".into());
    }
    Ok(Some(bytes))
}
fn validate(vault: &Vault, marker: &[u8]) -> Result<(), String> {
    if vault.version != 1
        || marker != hash(&vault.key).as_bytes()
        || vault.actors.len() > MAX_ACTORS
        || vault.actors.iter().filter(|a| a.record.is_some()).count() > MAX_RECORDS
    {
        return Err("Avatar key or registry is invalid; it was not replaced".into());
    }
    for (index, actor) in vault.actors.iter().enumerate() {
        let (ring, rotation) = shape::signature(&vault.key, actor.actor, actor.counter)?;
        if actor.actor.is_nil()
            || actor.counter > 1023
            || actor.ring != ring
            || actor.rotation != rotation
            || vault.actors[..index]
                .iter()
                .any(|v| v.actor == actor.actor || v.ring == actor.ring)
        {
            return Err("Avatar collision registry is invalid".into());
        }
        if let Some(record) = &actor.record {
            if record.owner_revision.is_nil()
                || !record.source.valid()
                || !digest_valid(&record.source_digest)
                || !digest_valid(&record.binding_digest)
            {
                return Err("Avatar source record is invalid".into());
            }
            record.parameters.validate(&actor.ring, actor.rotation)?;
        }
    }
    Ok(())
}
fn load(directory: &Path) -> Result<Option<Vault>, String> {
    let marker = bounded(&directory.join(".voice-avatar-initialized"), 64)?;
    let bytes = bounded(&directory.join("voice-avatars/vault.dpapi"), 131072)?;
    match (marker, bytes) {
        (None, None) => Ok(None),
        (Some(marker), Some(bytes)) => {
            let clear = Zeroizing::new(
                avesra_windows::credentials::unprotect(&bytes)
                    .map_err(|_| "Avatar key cannot be decrypted; it was not replaced")?,
            );
            if clear.len() > 65536 {
                return Err("Avatar vault exceeds its limit".into());
            }
            let vault: Vault = serde_json::from_slice(&clear)
                .map_err(|_| "Avatar vault is invalid; it was not replaced")?;
            validate(&vault, &marker)?;
            Ok(Some(vault))
        }
        _ => Err(
            "Avatar key or initialization record is missing; automatic redraw is disabled".into(),
        ),
    }
}
fn publish(
    directory: &Path,
    vault: &Vault,
    initialize: bool,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let clear = Zeroizing::new(serde_json::to_vec(vault).map_err(|_| "Avatar encoding failed")?);
    if clear.len() > 65536 {
        return Err("Avatar storage capacity reached".into());
    }
    let protected =
        avesra_windows::credentials::protect(&clear).map_err(|_| "Avatar protection failed")?;
    let temporary = directory.join("voice-avatars/.vault.pending");
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Interrupted avatar write cannot be retired".into()),
    }
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Avatar writer unavailable")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Avatar write failed")?;
        drop(file);
        authorize()?;
        if initialize {
            let mut marker = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(directory.join(".voice-avatar-initialized"))
                .map_err(|_| "Avatar initialization already exists or is unavailable")?;
            marker
                .write_all(hash(&vault.key).as_bytes())
                .and_then(|_| marker.sync_all())
                .map_err(|_| "Avatar initialization could not be confirmed")?;
        }
        authorize()?;
        std::fs::rename(&temporary, directory.join("voice-avatars/vault.dpapi"))
            .map_err(|_| "Avatar publication failed; refresh before retrying")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
struct Resolved {
    actor: Uuid,
    revision: Uuid,
    source: Source,
    binding_digest: String,
    source_digest: String,
    representation: Zeroizing<Vec<f32>>,
}
/// Caller retains native owner coordinator and candidate file lock throughout use.
fn resolve(directory: &Path, microphone: &str) -> Result<Option<Resolved>, String> {
    let (actor, revision) =
        crate::owner::identity(directory).map_err(|_| "Avatar owner is unavailable")?;
    let candidate = super::personal_seed_locked(directory)?
        .filter(|c| c.microphone == microphone && c.model_revision == SPEAKER);
    let expected = Voice {
        version: 1,
        id: Uuid::new_v4(),
        actor,
        owner_revision: revision,
        microphone: microphone.into(),
        model_revision: SPEAKER.into(),
        source: candidate.as_ref().map(|v| (v.id, v.revision)),
        seed: candidate.as_ref().map(|v| v.representation.clone()),
        observations: Vec::new(),
    };
    let Some(voice) = super::read_personal(directory, &expected)? else {
        return Ok(None);
    };
    let Some(representation) = voice.representation() else {
        return Ok(None);
    };
    let source = candidate
        .as_ref()
        .map(|v| Source::Candidate {
            id: v.id,
            revision: v.revision,
        })
        .unwrap_or(Source::Personal { id: voice.id });
    let binding = Zeroizing::new(
        serde_json::to_vec(&(actor, revision, &source, microphone, SPEAKER))
            .map_err(|_| "Avatar binding encoding failed")?,
    );
    let encoded = Zeroizing::new(
        if let Some(candidate) = candidate {
            serde_json::to_vec(&candidate)
        } else {
            serde_json::to_vec(&voice)
        }
        .map_err(|_| "Avatar source encoding failed")?,
    );
    Ok(Some(Resolved {
        actor,
        revision,
        source,
        binding_digest: hash(&binding),
        source_digest: hash(&encoded),
        representation: Zeroizing::new(representation),
    }))
}
/// No frontend actor/source/vector argument. Called only by the current-owner Settings reader.
pub(crate) fn current(
    directory: &Path,
    microphone: &str,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<View, String> {
    let _candidates = super::lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    let Some(source) = resolve(directory, microphone)? else {
        return Ok(View::missing());
    };
    let _avatars = super::lock_directory(&directory.join("voice-avatars"))?;
    let saved = load(directory)?;
    let initialize = saved.is_none();
    let mut vault = if let Some(vault) = saved {
        vault
    } else {
        let mut key = [0; 32];
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| "Avatar random key unavailable")?;
        Vault {
            version: 1,
            key,
            actors: Vec::new(),
        }
    };
    if let Some(actor) = vault.actors.iter().find(|v| v.actor == source.actor)
        && let Some(record) = &actor.record
    {
        if record.owner_revision != source.revision
            || record.source != source.source
            || record.binding_digest != source.binding_digest
            || (matches!(source.source, Source::Candidate { .. })
                && record.source_digest != source.source_digest)
        {
            return Err("The voice avatar source changed; an explicit redraw is required".into());
        }
        authorize()?;
        return Ok(View {
            state: "ready_without_portrait",
            parameters: Some(record.parameters.clone()),
            reason: None,
        });
    }
    if vault.actors.iter().filter(|a| a.record.is_some()).count() >= MAX_RECORDS {
        return Err("Avatar record capacity reached".into());
    }
    let index = if let Some(index) = vault.actors.iter().position(|v| v.actor == source.actor) {
        index
    } else {
        if vault.actors.len() >= MAX_ACTORS {
            return Err("Avatar signature registry capacity reached".into());
        }
        let mut chosen = None;
        for counter in 0..=1023 {
            let (ring, rotation) = shape::signature(&vault.key, source.actor, counter)?;
            if !vault.actors.iter().any(|v| v.ring == ring) {
                chosen = Some((counter, ring, rotation));
                break;
            }
        }
        let (counter, ring, rotation) = chosen.ok_or("Avatar signature collision limit reached")?;
        vault.actors.push(Actor {
            actor: source.actor,
            counter,
            ring,
            rotation,
            record: None,
        });
        vault.actors.len() - 1
    };
    let parameters = shape::build(
        &vault.key,
        &source.representation,
        vault.actors[index].ring.clone(),
        vault.actors[index].rotation,
    )?;
    vault.actors[index].record = Some(Record {
        owner_revision: source.revision,
        source: source.source,
        binding_digest: source.binding_digest,
        source_digest: source.source_digest,
        parameters: parameters.clone(),
    });
    publish(directory, &vault, initialize, &mut || {
        authorize()?;
        if crate::owner::identity(directory).map_err(|_| "Avatar owner unavailable")?
            != (source.actor, source.revision)
        {
            return Err("Avatar owner changed".into());
        }
        Ok(())
    })?;
    authorize()?;
    Ok(View {
        state: "ready_without_portrait",
        parameters: Some(parameters),
        reason: None,
    })
}
/// Candidate file lock already held. Ring metadata survives; parameters do not.
pub(super) fn remove_candidate(
    directory: &Path,
    id: Uuid,
    revision: Uuid,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let _lock = super::lock_directory(&directory.join("voice-avatars"))?;
    let Some(mut vault) = load(directory)? else {
        return Ok(());
    };
    let source = Source::Candidate { id, revision };
    let mut changed = false;
    for actor in &mut vault.actors {
        if actor.record.as_ref().is_some_and(|v| v.source == source) {
            actor.record = None;
            changed = true;
        }
    }
    if changed {
        publish(directory, &vault, false, authorize)?;
    }
    Ok(())
}
