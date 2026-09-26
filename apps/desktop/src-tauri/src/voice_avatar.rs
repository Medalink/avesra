//! Optional actor-bound presentation. This module never creates voice authority.
#[path = "portrait_dsp.rs"]
pub(crate) mod portrait_dsp;
#[path = "voice_avatar_shape.rs"]
mod shape;
use avesra_core::{
    app_timing::{Operation, Outcome, Portrait, Span, Stage},
    voice::personal::Voice,
};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) use shape::Parameters;
use std::{
    cell::Cell,
    io::{Read, Write},
    path::Path,
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const SPEAKER: &str = "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286";
const MAX_ACTORS: usize = 128;
const MAX_RECORDS: usize = 32;

/// Per-call observer identity only; never reuse an actor, source or record UUID.
struct Timings {
    operation: Uuid,
    withdrawn: Cell<bool>,
}
impl Timings {
    fn withdrawal(&self, message: &str) -> String {
        self.withdrawn.set(true);
        message.to_owned()
    }
    fn phase<T>(
        &self,
        phase: Portrait,
        work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let span = Span::with_operation(
            Operation::Portrait(phase),
            Stage::Work,
            Some(self.operation),
        );
        let result = work();
        span.finish(match &result {
            Ok(_) => Outcome::Complete,
            Err(_) if self.withdrawn.get() => Outcome::Withdrawn,
            Err(_) => Outcome::Failed,
        });
        result
    }
}
fn observed<T>(
    phase: Portrait,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    work: impl FnOnce(&Timings, &mut dyn FnMut() -> Result<(), String>) -> Result<T, String>,
) -> Result<T, String> {
    let timings = Timings {
        operation: Uuid::new_v4(),
        withdrawn: Cell::new(false),
    };
    let mut checked = || {
        let result = authorize();
        if result.is_err() {
            timings.withdrawn.set(true);
        }
        result
    };
    timings.phase(phase, || work(&timings, &mut checked))
}
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    portrait: Option<Vec<Option<portrait_dsp::Feature>>>,
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
struct CandidateReference {
    id: Uuid,
    revision: Uuid,
}
impl Source {
    fn candidate(&self) -> Option<CandidateReference> {
        match self {
            Self::Candidate { id, revision } => Some(CandidateReference {
                id: *id,
                revision: *revision,
            }),
            Self::Personal { .. } => None,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct View {
    version: u16,
    candidate: Option<CandidateReference>,
    state: &'static str,
    parameters: Option<Parameters>,
    reason: Option<String>,
}
impl View {
    pub(crate) fn unavailable(reason: String) -> Self {
        Self {
            version: 1,
            candidate: None,
            state: "unavailable",
            parameters: None,
            reason: Some(reason),
        }
    }
    fn missing() -> Self {
        Self { version: 1, candidate: None, state:"source_unavailable", parameters:None, reason:Some("Your avatar will be available after Avesra has saved real voice observations for this owner and microphone. You can keep talking normally.".into()) }
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
    if !matches!(vault.version, 1 | 2)
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
            match &record.portrait {
                None if record.parameters.version == 1 => {}
                Some(features) if vault.version == 2 && record.parameters.version == 2 => {
                    if !features.iter().any(Option::is_some)
                        || portrait_dsp::render(features)? != record.parameters.petals
                    {
                        return Err("Portrait features and parameters disagree".into());
                    }
                }
                _ => return Err("Portrait storage version is inconsistent".into()),
            }
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
    observed(Portrait::Prepare, authorize, |timings, authorize| {
        current_observed(directory, microphone, authorize, timings)
    })
}
fn current_observed(
    directory: &Path,
    microphone: &str,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    timings: &Timings,
) -> Result<View, String> {
    let _candidates = super::lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    let Some(source) = timings.phase(Portrait::SourceLoad, || resolve(directory, microphone))?
    else {
        return Ok(View::missing());
    };
    let _avatars = super::lock_directory(&directory.join("voice-avatars"))?;
    let saved = timings.phase(Portrait::VaultLoad, || load(directory))?;
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
            version: record.parameters.version,
            candidate: record.source.candidate(),
            state: if record.portrait.is_some() {
                "ready_with_portrait"
            } else {
                "ready_without_portrait"
            },
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
    let parameters = timings.phase(Portrait::Derive, || {
        shape::build(
            &vault.key,
            &source.representation,
            vault.actors[index].ring.clone(),
            vault.actors[index].rotation,
        )
    })?;
    let candidate = source.source.candidate();
    vault.actors[index].record = Some(Record {
        owner_revision: source.revision,
        source: source.source,
        binding_digest: source.binding_digest,
        source_digest: source.source_digest,
        parameters: parameters.clone(),
        portrait: None,
    });
    timings.phase(Portrait::Publish, || {
        publish(directory, &vault, initialize, &mut || {
            authorize()?;
            if crate::owner::identity(directory).map_err(|_| "Avatar owner unavailable")?
                != (source.actor, source.revision)
            {
                timings.withdrawn.set(true);
                return Err("Avatar owner changed".into());
            }
            Ok(())
        })
    })?;
    authorize()?;
    Ok(View {
        version: 1,
        candidate,
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
    observed(Portrait::Remove, authorize, |timings, authorize| {
        remove_candidate_observed(directory, id, revision, authorize, timings)
    })
}
fn remove_candidate_observed(
    directory: &Path,
    id: Uuid,
    revision: Uuid,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    timings: &Timings,
) -> Result<(), String> {
    let _lock = super::lock_directory(&directory.join("voice-avatars"))?;
    let Some(mut vault) = timings.phase(Portrait::VaultLoad, || load(directory))? else {
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
        timings.phase(Portrait::Publish, || {
            publish(directory, &vault, false, authorize)
        })?;
    }
    Ok(())
}

/// Native-only proposal. Neither a frontend source nor a fresh authority grant.
pub(crate) struct Redraw {
    actor: Uuid,
    owner_revision: Uuid,
    source: Source,
    binding_digest: String,
    source_digest: String,
    previous: String,
    key_digest: String,
    parameters: Parameters,
}
impl Redraw {
    pub(crate) fn view(&self) -> View {
        View {
            version: 1,
            candidate: self.source.candidate(),
            state: "ready_without_portrait",
            parameters: Some(self.parameters.clone()),
            reason: None,
        }
    }
}
fn actor_digest(actor: &Actor) -> Result<String, String> {
    let bytes = Zeroizing::new(serde_json::to_vec(actor).map_err(|_| "Avatar encoding failed")?);
    Ok(hash(&bytes))
}
pub(crate) fn prepare_redraw(
    directory: &Path,
    microphone: &str,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<Redraw, String> {
    observed(Portrait::RedrawPrepare, authorize, |timings, authorize| {
        prepare_redraw_observed(directory, microphone, authorize, timings)
    })
}
fn prepare_redraw_observed(
    directory: &Path,
    microphone: &str,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    timings: &Timings,
) -> Result<Redraw, String> {
    let _candidates = super::lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    let source = timings.phase(Portrait::SourceLoad, || {
        resolve(directory, microphone)?
            .ok_or_else(|| "No current owner-bound voice is available".to_owned())
    })?;
    let _avatars = super::lock_directory(&directory.join("voice-avatars"))?;
    let vault = timings.phase(Portrait::VaultLoad, || {
        load(directory)?.ok_or_else(|| "View your voice avatar before redrawing it".to_owned())
    })?;
    let actor = vault
        .actors
        .iter()
        .find(|value| value.actor == source.actor)
        .ok_or("This owner has no saved avatar signature to redraw")?;
    if actor.record.is_none()
        && vault
            .actors
            .iter()
            .filter(|value| value.record.is_some())
            .count()
            >= MAX_RECORDS
    {
        return Err("Avatar record capacity reached".into());
    }
    let parameters = timings.phase(Portrait::Derive, || {
        shape::build(
            &vault.key,
            &source.representation,
            actor.ring.clone(),
            actor.rotation,
        )
    })?;
    authorize()?;
    Ok(Redraw {
        actor: source.actor,
        owner_revision: source.revision,
        source: source.source,
        binding_digest: source.binding_digest,
        source_digest: source.source_digest,
        previous: actor_digest(actor)?,
        key_digest: hash(&vault.key),
        parameters,
    })
}
/// Typed observer classification only; the original error remains caller-facing.
pub(crate) enum FinalizeError {
    Withdrawn(String),
    Failed(String),
}
/// The caller holds owner management admission. finalize retains native current
/// context across the rename; file locks retain exact source/previous-record state.
pub(crate) fn confirm_redraw(
    directory: &Path,
    microphone: &str,
    expected: &Redraw,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    finalize: &mut dyn FnMut(&Path, &Path) -> Result<(), FinalizeError>,
) -> Result<View, String> {
    observed(Portrait::RedrawConfirm, authorize, |timings, authorize| {
        confirm_redraw_observed(
            directory, microphone, expected, authorize, finalize, timings,
        )
    })
}
fn confirm_redraw_observed(
    directory: &Path,
    microphone: &str,
    expected: &Redraw,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    finalize: &mut dyn FnMut(&Path, &Path) -> Result<(), FinalizeError>,
    timings: &Timings,
) -> Result<View, String> {
    let _candidates = super::lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    timings.phase(Portrait::SourceLoad, || {
        let source = resolve(directory, microphone)?
            .ok_or_else(|| timings.withdrawal("The proposed avatar source is unavailable"))?;
        if source.actor != expected.actor
            || source.revision != expected.owner_revision
            || source.source != expected.source
            || source.binding_digest != expected.binding_digest
            || source.source_digest != expected.source_digest
        {
            return Err(timings.withdrawal("Your voice source changed; prepare a new redraw"));
        }
        Ok(())
    })?;
    let _avatars = super::lock_directory(&directory.join("voice-avatars"))?;
    let (mut vault, index) = timings.phase(Portrait::VaultLoad, || {
        let vault = load(directory)?.ok_or_else(|| {
            timings.withdrawal("The avatar registry is unavailable; it was not replaced")
        })?;
        let index = vault
            .actors
            .iter()
            .position(|value| value.actor == expected.actor)
            .ok_or_else(|| timings.withdrawal("The avatar signature was removed"))?;
        if hash(&vault.key) != expected.key_digest
            || actor_digest(&vault.actors[index])? != expected.previous
        {
            return Err(timings.withdrawal("The saved avatar changed; prepare a new redraw"));
        }
        Ok((vault, index))
    })?;
    if vault.actors[index].record.is_none()
        && vault
            .actors
            .iter()
            .filter(|value| value.record.is_some())
            .count()
            >= MAX_RECORDS
    {
        return Err("Avatar record capacity reached".into());
    }
    expected
        .parameters
        .validate(&vault.actors[index].ring, vault.actors[index].rotation)?;
    vault.actors[index].record = Some(Record {
        owner_revision: expected.owner_revision,
        source: expected.source.clone(),
        binding_digest: expected.binding_digest.clone(),
        source_digest: expected.source_digest.clone(),
        parameters: expected.parameters.clone(),
        portrait: None,
    });
    timings.phase(Portrait::Publish, || {
        // No key/marker initialization or ring mutation is permitted by redraw.
        let clear =
            Zeroizing::new(serde_json::to_vec(&vault).map_err(|_| "Avatar encoding failed")?);
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
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| "Avatar writer unavailable")?;
            file.write_all(&protected)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Avatar write failed")?;
            drop(file);
            authorize()?;
            if crate::owner::identity(directory).map_err(|_| "Avatar owner unavailable")?
                != (expected.actor, expected.owner_revision)
            {
                return Err(timings.withdrawal("Avatar owner changed"));
            }
            finalize(&temporary, &directory.join("voice-avatars/vault.dpapi")).map_err(
                |error| match error {
                    FinalizeError::Withdrawn(message) => {
                        timings.withdrawn.set(true);
                        message
                    }
                    FinalizeError::Failed(message) => message,
                },
            )?;
            Ok(expected.view())
        })();
        let _ = std::fs::remove_file(temporary);
        result
    })
}

pub(crate) struct PortraitSource {
    actor: Uuid,
    revision: Uuid,
    source: Source,
    binding: String,
    record: String,
    key: String,
}
impl PortraitSource {
    pub(crate) fn actor(&self) -> Uuid {
        self.actor
    }
}
pub(crate) fn portrait_source(
    directory: &Path,
    microphone: &str,
    authorize: &mut dyn FnMut() -> Result<(), String>,
) -> Result<PortraitSource, String> {
    // Existing source-change refusal deliberately requires explicit redraw first.
    current(directory, microphone, authorize)?;
    let _candidates = super::lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    let source = resolve(directory, microphone)?.ok_or("No current owner-bound avatar source")?;
    let _avatars = super::lock_directory(&directory.join("voice-avatars"))?;
    let vault = load(directory)?.ok_or("Avatar source unavailable")?;
    let actor = vault
        .actors
        .iter()
        .find(|v| v.actor == source.actor)
        .ok_or("Avatar owner unavailable")?;
    let record = actor.record.as_ref().ok_or("Avatar record unavailable")?;
    if record.owner_revision != source.revision
        || record.source != source.source
        || record.binding_digest != source.binding_digest
        || (matches!(source.source, Source::Candidate { .. })
            && record.source_digest != source.source_digest)
    {
        return Err("Avatar source changed; redraw before observing acoustic prompts".into());
    }
    authorize()?;
    Ok(PortraitSource {
        actor: source.actor,
        revision: source.revision,
        source: source.source,
        binding: source.binding_digest,
        record: actor_digest(actor)?,
        key: hash(&vault.key),
    })
}
pub(crate) fn save_portrait(
    directory: &Path,
    microphone: &str,
    expected: &PortraitSource,
    features: Vec<Option<portrait_dsp::Feature>>,
    authorize: &mut dyn FnMut() -> Result<(), String>,
    finalize: &mut dyn FnMut(&Path, &Path) -> Result<(), String>,
    withdrawn: &mut dyn FnMut(),
) -> Result<View, String> {
    if !features.iter().any(Option::is_some) {
        return Err("No acoustic prompt was measured; nothing was saved".into());
    }
    let _candidates = super::lock_directory(&directory.join("speaker-candidates"))?;
    authorize()?;
    let source = resolve(directory, microphone)?.ok_or_else(|| {
        withdrawn();
        "Portrait source unavailable".to_owned()
    })?;
    if source.actor != expected.actor
        || source.revision != expected.revision
        || source.source != expected.source
        || source.binding_digest != expected.binding
    {
        withdrawn();
        return Err("Portrait source changed; begin again".into());
    }
    let _avatars = super::lock_directory(&directory.join("voice-avatars"))?;
    let mut vault = load(directory)?.ok_or_else(|| {
        withdrawn();
        "Avatar vault unavailable".to_owned()
    })?;
    let index = vault
        .actors
        .iter()
        .position(|v| v.actor == expected.actor)
        .ok_or_else(|| {
            withdrawn();
            "Avatar owner was removed".to_owned()
        })?;
    if hash(&vault.key) != expected.key || actor_digest(&vault.actors[index])? != expected.record {
        withdrawn();
        return Err("Avatar changed while observing; begin again".into());
    }
    let record = vault.actors[index].record.as_mut().ok_or_else(|| {
        withdrawn();
        "Avatar source was removed".to_owned()
    })?;
    if matches!(source.source, Source::Candidate { .. })
        && record.source_digest != source.source_digest
    {
        withdrawn();
        return Err("Saved candidate changed".into());
    }
    record.parameters.portrait(&features)?;
    let parameters = record.parameters.clone();
    let candidate = record.source.candidate();
    record.portrait = Some(features);
    vault.version = 2;
    let clear = Zeroizing::new(serde_json::to_vec(&vault).map_err(|_| "Avatar encoding failed")?);
    if clear.len() > 65536 {
        return Err("Avatar storage capacity reached".into());
    }
    let protected =
        avesra_windows::credentials::protect(&clear).map_err(|_| "Avatar protection failed")?;
    let temporary = directory.join("voice-avatars/.vault.pending");
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
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
        if crate::owner::identity(directory).map_err(|_| "Avatar owner unavailable")?
            != (expected.actor, expected.revision)
        {
            withdrawn();
            return Err("Avatar owner changed".into());
        }
        finalize(&temporary, &directory.join("voice-avatars/vault.dpapi"))?;
        Ok(View {
            version: 2,
            candidate,
            state: "ready_with_portrait",
            parameters: Some(parameters),
            reason: None,
        })
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
