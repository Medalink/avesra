//! Native pairing storage. Its caller owns authenticated setup and session revocation.
use avesra_contracts::{
    ErrorCode,
    browser::{self, Hex32, Id, PairingRef},
};
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

const MAX_RECORD: u64 = 16_384;
const MAX_PAIRS: usize = 16;

/// No Debug/Clone/Serialize: this value cannot accidentally enter UI diagnostics.
struct Credential([u8; 32]);
impl Drop for Credential {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl Credential {
    fn generate() -> Result<Self, ErrorCode> {
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self(bytes))
    }
    fn verify(&self, bytes: &[u8], proof: &Hex32) -> Result<(), ErrorCode> {
        hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, &self.0),
            bytes,
            proof.bytes(),
        )
        .map_err(|_| ErrorCode::Unauthenticated)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub pairing: PairingRef,
    pub actor: Id,
    pub browser_app: Id,
    pub browser_revision: Id,
    pub installation: Id,
    pub extension: String,
    pub label: String,
    pub created_at_ms: u64,
}
impl Binding {
    fn validate(&self) -> Result<(), ErrorCode> {
        if !browser::extension_id(&self.extension)
            || self.label.is_empty()
            || self.label.trim() != self.label
            || self.label.len() > 256
            || self.label.chars().count() > 64
            || self.label.chars().any(char::is_control)
            || self.created_at_ms == 0
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u16,
    principal: String,
    binding: Binding,
    credential: [u8; 32],
}
impl Drop for Record {
    fn drop(&mut self) {
        self.credential.zeroize();
    }
}
pub struct Saved {
    binding: Binding,
    credential: Credential,
}
impl Saved {
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}
#[derive(Serialize)]
pub struct Summary {
    pub pairing: PairingRef,
    pub binding: Option<Binding>,
    /// Corrupt/foreign entries remain removable by their exact native filename identity.
    pub available: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selected {
    pub revision: Id,
    pub pairing: PairingRef,
    pub actor: Id,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedRecord {
    version: u16,
    principal: String,
    selected: Selected,
}
#[derive(Serialize)]
pub struct SelectionStatus {
    pub revision: Id,
    pub selected: Option<Selected>,
    pub binding: Option<Binding>,
    pub available: bool,
}

/// Exclusive across processes and retained through every blocking publication/read.
/// Off-thread ownership must outlive a cancelled async waiter.
pub struct Store {
    directory: PathBuf,
    principal: String,
    _lock: File,
}
impl Store {
    pub fn open(directory: &Path) -> Result<Self, ErrorCode> {
        std::fs::create_dir_all(directory).map_err(|_| ErrorCode::Unavailable)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("pairings.lock"))
            .map_err(|_| ErrorCode::Unavailable)?;
        lock.try_lock().map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self {
            directory: directory.into(),
            principal: crate::principal::current_user()?,
            _lock: lock,
        })
    }
    fn path(&self, pairing: PairingRef) -> PathBuf {
        self.directory.join(format!(
            "{}-{}.dpapi",
            pairing.id.uuid(),
            pairing.revision.uuid()
        ))
    }
    fn identities(&self) -> Result<Vec<PairingRef>, ErrorCode> {
        let mut identities = Vec::new();
        for (index, entry) in std::fs::read_dir(&self.directory)
            .map_err(|_| ErrorCode::Unavailable)?
            .enumerate()
        {
            if index >= 256 {
                return Err(ErrorCode::TooLarge);
            }
            let entry = entry.map_err(|_| ErrorCode::Unavailable)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(ErrorCode::Malformed);
            };
            if name == "pairings.lock" || (name.starts_with("pending-") && name.ends_with(".tmp")) {
                continue;
            }
            if name.starts_with("selected-") {
                selected_revision(name)?;
                continue;
            }
            let Some(stem) = name.strip_suffix(".dpapi") else {
                return Err(ErrorCode::Malformed);
            };
            if !stem.is_ascii() || stem.len() != 73 || stem.as_bytes()[36] != b'-' {
                return Err(ErrorCode::Malformed);
            }
            let id: Id = serde_json::from_value(serde_json::Value::String(stem[..36].into()))
                .map_err(|_| ErrorCode::Malformed)?;
            let revision: Id = serde_json::from_value(serde_json::Value::String(stem[37..].into()))
                .map_err(|_| ErrorCode::Malformed)?;
            identities.push(PairingRef { id, revision });
            if identities.len() > MAX_PAIRS {
                return Err(ErrorCode::TooLarge);
            }
        }
        Ok(identities)
    }
    pub fn list(&self, actor: Id) -> Result<Vec<Summary>, ErrorCode> {
        self.identities()?
            .into_iter()
            .map(|pairing| {
                let binding = self.load(pairing, actor).ok().map(|saved| saved.binding);
                Ok(Summary {
                    pairing,
                    available: binding.is_some(),
                    binding,
                })
            })
            .collect()
    }
    pub fn load(&self, pairing: PairingRef, actor: Id) -> Result<Saved, ErrorCode> {
        let path = self.path(pairing);
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(0x00200000)
            .open(path)
            .map_err(|_| ErrorCode::Unavailable)?;
        let metadata = file.metadata().map_err(|_| ErrorCode::Unavailable)?;
        if !metadata.is_file()
            || metadata.file_attributes() & 0x400 != 0
            || metadata.len() > MAX_RECORD
        {
            return Err(ErrorCode::Malformed);
        }
        let mut protected = Vec::new();
        (&mut file)
            .take(MAX_RECORD + 1)
            .read_to_end(&mut protected)
            .map_err(|_| ErrorCode::Unavailable)?;
        if protected.is_empty() || protected.len() as u64 > MAX_RECORD {
            return Err(ErrorCode::TooLarge);
        }
        let plain = Zeroizing::new(crate::credentials::unprotect(&protected)?);
        if plain.len() as u64 > MAX_RECORD {
            return Err(ErrorCode::TooLarge);
        }
        let record: Record = serde_json::from_slice(&plain).map_err(|_| ErrorCode::Malformed)?;
        record.binding.validate()?;
        if record.version != 1
            || record.principal != self.principal
            || record.binding.actor != actor
            || record.binding.pairing != pairing
            || record.credential == [0; 32]
        {
            return Err(ErrorCode::Unauthenticated);
        }
        Ok(Saved {
            binding: record.binding.clone(),
            credential: Credential(record.credential),
        })
    }
    /// The callback runs after staging/sync, immediately before create-only publication.
    /// An error after publication is uncertain; inspect/revoke, never automatically retry.
    fn create(
        &self,
        binding: Binding,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Saved, ErrorCode> {
        binding.validate()?;
        let identities = self.identities()?;
        if identities.len() >= MAX_PAIRS || identities.iter().any(|p| p.id == binding.pairing.id) {
            return Err(ErrorCode::Denied);
        }
        // A second credential for the same installation requires explicit removal first.
        for pairing in identities {
            let existing = self.load(pairing, binding.actor)?;
            if existing.binding.installation == binding.installation
                && existing.binding.extension == binding.extension
            {
                return Err(ErrorCode::Denied);
            }
        }
        let credential = Credential::generate()?;
        let record = Record {
            version: 1,
            principal: self.principal.clone(),
            binding: binding.clone(),
            credential: credential.0,
        };
        let plain = Zeroizing::new(serde_json::to_vec(&record).map_err(|_| ErrorCode::Malformed)?);
        let protected = crate::credentials::protect(&plain)?;
        if protected.len() as u64 > MAX_RECORD {
            return Err(ErrorCode::TooLarge);
        }
        let temporary = self
            .directory
            .join(format!("pending-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| ErrorCode::Unavailable)?;
            file.write_all(&protected)
                .and_then(|_| file.sync_all())
                .map_err(|_| ErrorCode::Unavailable)?;
            drop(file);
            if crate::principal::current_user()? != self.principal {
                return Err(ErrorCode::Unauthenticated);
            }
            authorize()?;
            std::fs::hard_link(&temporary, self.path(binding.pairing))
                .map_err(|_| ErrorCode::Unavailable)?;
            self.load(binding.pairing, binding.actor)
        })();
        let _ = std::fs::remove_file(&temporary);
        result
    }
    /// Caller revokes in-memory session/challenge ownership before this durable removal.
    pub fn remove(
        &self,
        pairing: PairingRef,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if crate::principal::current_user()? != self.principal {
            return Err(ErrorCode::Unauthenticated);
        }
        authorize()?;
        std::fs::remove_file(self.path(pairing)).map_err(|_| ErrorCode::Unavailable)
    }
    fn selection_path(&self, revision: Id) -> PathBuf {
        self.directory
            .join(format!("selected-{}.dpapi", revision.uuid()))
    }
    fn selection_revisions(&self) -> Result<Vec<Id>, ErrorCode> {
        let mut revisions = Vec::new();
        for (index, entry) in std::fs::read_dir(&self.directory)
            .map_err(|_| ErrorCode::Unavailable)?
            .enumerate()
        {
            if index >= 256 {
                return Err(ErrorCode::TooLarge);
            }
            let entry = entry.map_err(|_| ErrorCode::Unavailable)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(ErrorCode::Malformed)?;
            if name.starts_with("selected-") {
                revisions.push(selected_revision(name)?);
            }
            if revisions.len() > 16 {
                return Err(ErrorCode::TooLarge);
            }
        }
        Ok(revisions)
    }
    fn read_selected(&self, revision: Id, actor: Id) -> Result<(Selected, Binding), ErrorCode> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(0x00200000)
            .open(self.selection_path(revision))
            .map_err(|_| ErrorCode::Unavailable)?;
        let metadata = file.metadata().map_err(|_| ErrorCode::Unavailable)?;
        if !metadata.is_file()
            || metadata.file_attributes() & 0x400 != 0
            || metadata.len() > MAX_RECORD
        {
            return Err(ErrorCode::Malformed);
        }
        let mut protected = Vec::new();
        (&mut file)
            .take(MAX_RECORD + 1)
            .read_to_end(&mut protected)
            .map_err(|_| ErrorCode::Unavailable)?;
        if protected.is_empty() || protected.len() as u64 > MAX_RECORD {
            return Err(ErrorCode::TooLarge);
        }
        let plain = Zeroizing::new(crate::credentials::unprotect(&protected)?);
        if plain.len() as u64 > MAX_RECORD {
            return Err(ErrorCode::TooLarge);
        }
        let record: SelectedRecord =
            serde_json::from_slice(&plain).map_err(|_| ErrorCode::Malformed)?;
        if record.version != 1
            || record.principal != self.principal
            || record.selected.revision != revision
            || record.selected.actor != actor
        {
            return Err(ErrorCode::Unauthenticated);
        }
        let pairing = self.load(record.selected.pairing, actor)?;
        Ok((record.selected, pairing.binding))
    }
    /// Recovery retains each exact filename revision even if its body is corrupt.
    /// Multiple selected records are ambiguous: none is an active selection.
    pub fn selections(&self, actor: Id) -> Result<Vec<SelectionStatus>, ErrorCode> {
        let revisions = self.selection_revisions()?;
        let unambiguous = revisions.len() == 1;
        Ok(revisions
            .into_iter()
            .map(|revision| {
                let decoded = self.read_selected(revision, actor).ok();
                let available = unambiguous && decoded.is_some();
                let (selected, binding) = decoded.map_or((None, None), |(v, b)| (Some(v), Some(b)));
                SelectionStatus {
                    revision,
                    selected,
                    binding,
                    available,
                }
            })
            .collect())
    }
    pub fn selected(&self, actor: Id) -> Result<Option<(Selected, Binding)>, ErrorCode> {
        let revisions = self.selection_revisions()?;
        match revisions.as_slice() {
            [] => Ok(None),
            [revision] => self.read_selected(*revision, actor).map(Some),
            _ => Err(ErrorCode::Denied),
        }
    }
    /// Selection is create-only. Explicitly clear a previous revision first; no
    /// replacement operation can silently retarget already accepted work.
    pub fn select(
        &self,
        pairing: PairingRef,
        actor: Id,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Selected, ErrorCode> {
        if !self.selection_revisions()?.is_empty() {
            return Err(ErrorCode::Denied);
        }
        self.load(pairing, actor)?;
        let selected = Selected {
            revision: Id::new(uuid::Uuid::new_v4())?,
            pairing,
            actor,
        };
        let plain = Zeroizing::new(
            serde_json::to_vec(&SelectedRecord {
                version: 1,
                principal: self.principal.clone(),
                selected: selected.clone(),
            })
            .map_err(|_| ErrorCode::Malformed)?,
        );
        let protected = crate::credentials::protect(&plain)?;
        if protected.len() as u64 > MAX_RECORD {
            return Err(ErrorCode::TooLarge);
        }
        let temporary = self
            .directory
            .join(format!("pending-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| ErrorCode::Unavailable)?;
            file.write_all(&protected)
                .and_then(|_| file.sync_all())
                .map_err(|_| ErrorCode::Unavailable)?;
            drop(file);
            // Refresh the exact credential after potentially blocking staging.
            self.load(pairing, actor)?;
            if crate::principal::current_user()? != self.principal {
                return Err(ErrorCode::Unauthenticated);
            }
            authorize()?;
            std::fs::hard_link(&temporary, self.selection_path(selected.revision))
                .map_err(|_| ErrorCode::Unavailable)?;
            self.read_selected(selected.revision, actor).map(|v| v.0)
        })();
        let _ = std::fs::remove_file(temporary);
        result
    }
    pub fn clear_selection(
        &self,
        revision: Id,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if crate::principal::current_user()? != self.principal {
            return Err(ErrorCode::Unauthenticated);
        }
        authorize()?;
        std::fs::remove_file(self.selection_path(revision)).map_err(|_| ErrorCode::Unavailable)
    }
}
fn selected_revision(name: &str) -> Result<Id, ErrorCode> {
    let raw = name
        .strip_prefix("selected-")
        .and_then(|v| v.strip_suffix(".dpapi"))
        .ok_or(ErrorCode::Malformed)?;
    serde_json::from_value(serde_json::Value::String(raw.into())).map_err(|_| ErrorCode::Malformed)
}

/// Created before pipe acceptance/peer inspection, never reset after blocking work.
pub struct Admission {
    started: Instant,
    generation: u64,
}
impl Admission {
    pub fn new(generation: u64) -> Result<Self, ErrorCode> {
        if generation == 0 {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self {
            started: Instant::now(),
            generation,
        })
    }
    pub fn current(&self, generation: u64) -> Result<(), ErrorCode> {
        if generation != self.generation {
            return Err(ErrorCode::Stale);
        }
        if self.started.elapsed() >= Duration::from_secs(browser::HANDSHAKE_SECONDS) {
            return Err(ErrorCode::Expired);
        }
        Ok(())
    }
    pub fn challenge(
        self,
        hello: browser::Hello,
        configured_extension: &str,
        generation: u64,
    ) -> Result<Pending, ErrorCode> {
        self.current(generation)?;
        hello.validate()?;
        if hello.extension != configured_extension {
            return Err(ErrorCode::Unauthenticated);
        }
        let mut nonce = [0; 32];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| ErrorCode::Unavailable)?;
        let challenge = browser::Challenge {
            version: browser::VERSION,
            installation: hello.installation,
            connection: hello.connection,
            session: Id::new(uuid::Uuid::new_v4())?,
            challenge: Id::new(uuid::Uuid::new_v4())?,
            nonce: Hex32::new(nonce),
            pairing: hello.pairing,
        };
        self.current(generation)?;
        Ok(Pending {
            admission: self,
            hello,
            challenge,
        })
    }
}
pub struct Pending {
    admission: Admission,
    hello: browser::Hello,
    challenge: browser::Challenge,
}
/// Owner-selected native catalog identity; neither a browser account nor a grant.
pub struct Selection {
    pub actor: Id,
    pub browser_app: Id,
    pub browser_revision: Id,
    pub label: String,
    pub created_at_ms: u64,
}
#[derive(Clone, Serialize)]
pub struct Confirmation {
    pub installation: Id,
    pub connection: Id,
    pub session: Id,
    pub challenge: Id,
    pub extension: String,
    pub comparison: String,
}
/// Constructible only by original create-only publication; never by load/reconnect.
pub struct Issuance {
    pending: Pending,
    saved: Saved,
}
impl Issuance {
    /// Consumed exactly once by the native transport owner; never send to the webview.
    pub fn into_frame(
        self,
        generation: u64,
    ) -> Result<(Pending, Saved, Zeroizing<Vec<u8>>), ErrorCode> {
        self.pending.admission.current(generation)?;
        #[derive(Serialize)]
        struct Wire<'a> {
            #[serde(rename = "type")]
            kind: &'static str,
            version: u16,
            challenge: &'a browser::Challenge,
            pairing: PairingRef,
            credential: &'a str,
        }
        let mut hex = Zeroizing::new(String::with_capacity(64));
        for byte in &self.saved.credential.0 {
            hex.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
            hex.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
        }
        let frame = Zeroizing::new(
            serde_json::to_vec(&Wire {
                kind: "issued",
                version: browser::VERSION,
                challenge: &self.pending.challenge,
                pairing: self.saved.binding.pairing,
                credential: &hex,
            })
            .map_err(|_| ErrorCode::Malformed)?,
        );
        Ok((self.pending, self.saved, frame))
    }
}
impl Pending {
    pub fn challenge(&self) -> &browser::Challenge {
        &self.challenge
    }
    pub fn confirmation(&self, generation: u64) -> Result<Confirmation, ErrorCode> {
        self.admission.current(generation)?;
        let digest = ring::digest::digest(
            &ring::digest::SHA256,
            &browser::comparison_transcript(&self.challenge)?,
        );
        let first: [u8; 4] = digest.as_ref()[..4]
            .try_into()
            .map_err(|_| ErrorCode::Malformed)?;
        Ok(Confirmation {
            installation: self.hello.installation,
            connection: self.hello.connection,
            session: self.challenge.session,
            challenge: self.challenge.challenge,
            extension: self.hello.extension.clone(),
            comparison: format!("{:06}", u32::from_be_bytes(first) % 1_000_000),
        })
    }
    /// The coordinator consumed its original one-use proof BEFORE awaiting owner/catalog
    /// reads. This callback must revalidate that exact proof, pending challenge, owner and
    /// catalog under native ownership; a new proof cannot revive this pending operation.
    pub fn approve(
        mut self,
        store: &Store,
        selection: Selection,
        generation: u64,
        authorize: &mut dyn FnMut(&Binding, &browser::Challenge) -> Result<(), ErrorCode>,
    ) -> Result<Issuance, ErrorCode> {
        self.admission.current(generation)?;
        if self.hello.pairing.is_some() {
            return Err(ErrorCode::Denied);
        }
        let pairing = PairingRef {
            id: Id::new(uuid::Uuid::new_v4())?,
            revision: Id::new(uuid::Uuid::new_v4())?,
        };
        let binding = Binding {
            pairing,
            actor: selection.actor,
            browser_app: selection.browser_app,
            browser_revision: selection.browser_revision,
            installation: self.hello.installation,
            extension: self.hello.extension.clone(),
            label: selection.label,
            created_at_ms: selection.created_at_ms,
        };
        let saved = store.create(binding.clone(), &mut || {
            self.admission.current(generation)?;
            authorize(&binding, &self.challenge)?;
            self.admission.current(generation)
        })?;
        // The issued native pairing is now the binding for the original session's
        // proof-of-persistence response. A lost reply is not a reason to issue again.
        self.hello.pairing = Some(pairing);
        self.challenge.pairing = Some(pairing);
        self.admission.current(generation)?;
        Ok(Issuance {
            pending: self,
            saved,
        })
    }
    /// Consuming self makes failed verification non-retryable on this challenge.
    pub fn authenticate(
        self,
        saved: &Saved,
        reply: browser::Authenticate,
        generation: u64,
    ) -> Result<browser::Authenticated, ErrorCode> {
        self.admission.current(generation)?;
        if reply.version != browser::VERSION
            || reply.session != self.challenge.session
            || reply.challenge != self.challenge.challenge
            || reply.pairing != saved.binding.pairing
            || saved.binding.installation != self.hello.installation
            || saved.binding.extension != self.hello.extension
        {
            return Err(ErrorCode::Unauthenticated);
        }
        let transcript = browser::transcript(&self.hello, &self.challenge, reply.pairing)?;
        saved.credential.verify(&transcript, &reply.proof)?;
        self.admission.current(generation)?;
        Ok(browser::Authenticated {
            version: browser::VERSION,
            session: self.challenge.session,
            installation: self.hello.installation,
            connection: self.hello.connection,
            pairing: reply.pairing,
            generation,
        })
    }
}
