//! Numeric saved-peer connection only; no credentials, policy edits or retries.
use super::{LIMIT, OwnedChild, PATH, Status, drain, identity, stats};
use avesra_contracts::{Action, ErrorCode};
use avesra_core::{
    diagnostics::{Reading, Unavailable, VpnState},
    execution::EffectAuthority,
    vpn::{OwnerStep, Profile, Report, State},
};
use quick_xml::{events::Event, reader::Reader};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    net::IpAddr,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use uuid::Uuid;
use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_ProgramData, KF_FLAG_DONT_VERIFY, SHGetKnownFolderPath},
};
use zeroize::Zeroizing;

struct Saved {
    _file: File,
    name: String,
    address: IpAddr,
    digest: [u8; 32],
}
fn saved() -> Result<Saved, ErrorCode> {
    let pointer = unsafe { SHGetKnownFolderPath(&FOLDERID_ProgramData, KF_FLAG_DONT_VERIFY, None) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let folder = unsafe { pointer.to_string() };
    unsafe {
        CoTaskMemFree(Some(pointer.0.cast()));
    }
    let path = PathBuf::from(folder.map_err(|_| ErrorCode::Unavailable)?)
        .join(r"Cisco\Cisco Secure Client\VPN\preferences_global.xml");
    for ancestor in path.ancestors() {
        if std::fs::symlink_metadata(ancestor)
            .map_err(|_| ErrorCode::Unavailable)?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err(ErrorCode::Denied);
        }
    }
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .map_err(|_| ErrorCode::Unavailable)?;
    let metadata = file.metadata().map_err(|_| ErrorCode::Unavailable)?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err(ErrorCode::TooLarge);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    (&mut file)
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| ErrorCode::Unavailable)?;
    if bytes.len() > 65_536 {
        return Err(ErrorCode::TooLarge);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ErrorCode::Malformed)?;
    let mut reader = Reader::from_str(text);
    let mut depth = 0usize;
    let mut root = false;
    let mut closed = false;
    let mut field = None;
    let mut selected = [None::<String>, None::<String>];
    for number in 0..=512 {
        if number == 512 {
            return Err(ErrorCode::TooLarge);
        }
        match reader.read_event().map_err(|_| ErrorCode::Malformed)? {
            Event::Start(event) => {
                if closed || depth >= 16 {
                    return Err(ErrorCode::Malformed);
                }
                if depth == 0 {
                    if root || event.name().as_ref() != "AnyConnectPreferences" {
                        return Err(ErrorCode::Malformed);
                    }
                    root = true;
                }
                depth += 1;
                if depth == 2 {
                    field = match event.name().as_ref() {
                        "DefaultHostName" => Some(0),
                        "DefaultHostAddress" => Some(1),
                        _ => None,
                    };
                    if let Some(index) = field {
                        if selected[index].is_some() {
                            return Err(ErrorCode::Malformed);
                        }
                        selected[index] = Some(String::new());
                    }
                } else if field.is_some() {
                    return Err(ErrorCode::Malformed);
                }
            }
            Event::Empty(event) => {
                if depth == 0 || closed {
                    return Err(ErrorCode::Malformed);
                }
                if depth == 1 {
                    let index = match event.name().as_ref() {
                        "DefaultHostName" => Some(0),
                        "DefaultHostAddress" => Some(1),
                        _ => None,
                    };
                    if let Some(index) = index
                        && selected[index].replace(String::new()).is_some()
                    {
                        return Err(ErrorCode::Malformed);
                    }
                } else if field.is_some() {
                    return Err(ErrorCode::Malformed);
                }
            }
            Event::Text(value) => {
                if let Some(index) = field {
                    let value = value.as_ref();
                    let target = selected[index].as_mut().ok_or(ErrorCode::Malformed)?;
                    if target.len() + value.len() > 256 {
                        return Err(ErrorCode::TooLarge);
                    }
                    target.push_str(value);
                }
            }
            Event::End(_) => {
                if depth == 0 {
                    return Err(ErrorCode::Malformed);
                }
                if depth == 2 {
                    field = None;
                }
                depth -= 1;
                if depth == 0 {
                    closed = true;
                }
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Eof => break,
            // No DTD, entity expansion, CDATA or processing instructions.
            _ => return Err(ErrorCode::Malformed),
        }
    }
    if !root || !closed || depth != 0 {
        return Err(ErrorCode::Malformed);
    }
    let name = selected[0]
        .take()
        .ok_or(ErrorCode::Unavailable)?
        .trim()
        .to_owned();
    let configured = selected[1].take().ok_or(ErrorCode::Unavailable)?;
    let configured = configured.trim();
    let address: IpAddr = if let Ok(value) = configured.parse() {
        value
    } else {
        let socket: std::net::SocketAddr =
            configured.parse().map_err(|_| ErrorCode::Unsupported)?;
        if socket.port() != 443 {
            return Err(ErrorCode::Unsupported);
        }
        socket.ip()
    };
    // Bind only the fields selected for this operation, not unrelated Cisco
    // preferences (which may legitimately change during authentication).
    let mut digest = Sha256::new();
    digest.update(b"avesra-cisco-selected-peer-v1\0");
    digest.update((name.len() as u32).to_le_bytes());
    digest.update(name.as_bytes());
    digest.update(address.to_string().as_bytes());
    Ok(Saved {
        _file: file,
        name,
        address,
        digest: digest.finalize().into(),
    })
}
/// Read-only owner-facing selection. It grants neither dispatch nor approval.
pub fn discover(actor: Uuid) -> Result<Profile, ErrorCode> {
    let value = saved()?;
    let profile = Profile {
        id: Uuid::new_v4(),
        actor,
        revision: Uuid::new_v4(),
        name: value.name,
        address: value.address,
        source_sha256: value.digest,
    };
    profile.validate()?;
    let _identity =
        identity(Instant::now() + Duration::from_secs(5)).map_err(|_| ErrorCode::Unsupported)?;
    Ok(profile)
}
fn matched(profile: &Profile) -> Result<Saved, ErrorCode> {
    profile.validate()?;
    let value = saved()?;
    if value.name != profile.name
        || value.address != profile.address
        || value.digest != profile.source_sha256
    {
        return Err(ErrorCode::Stale);
    }
    Ok(value)
}
pub fn current(profile: &Profile) -> Result<(), ErrorCode> {
    matched(profile).map(|_| ())
}
fn report(
    profile: &Profile,
    state: State,
    issued: bool,
    status: Result<Status, Unavailable>,
    owner: Option<OwnerStep>,
) -> Report {
    let target_matched = status.as_ref().is_ok_and(|value| {
        matches!(value.state, VpnState::Connected) && value.server == Some(profile.address)
    });
    Report {
        profile: profile.id,
        revision: profile.revision,
        state,
        command_issued: issued,
        observed: match status {
            Ok(value) => Reading::Available { value: value.state },
            Err(reason) => Reading::Unavailable { reason },
        },
        target_matched,
        owner_step: owner,
    }
}
struct Completion {
    connected: bool,
    owner: Option<OwnerStep>,
}
fn run(
    profile: &Profile,
    deadline: Instant,
    authority: &mut EffectAuthority<'_>,
    issued: &mut bool,
) -> Result<Completion, Unavailable> {
    let _identity = identity(deadline)?;
    let source = matched(profile).map_err(|_| Unavailable::Changed)?;
    authority.commit().map_err(|_| Unavailable::Cancelled)?;
    *issued = true;
    let overflow = AtomicBool::new(false);
    let retired = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let child = Command::new(PATH)
            .args(["connect", &profile.address.to_string()])
            .creation_flags(0x0800_0000)
            .current_dir(r"C:\Program Files (x86)\Cisco\Cisco Secure Client")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| Unavailable::CommandFailed)?;
        // The selected command is now frozen. Do not block Cisco's own normal
        // preference updates while it performs authentication/connection.
        drop(source);
        let mut owned = OwnedChild {
            child,
            reaped: false,
        };
        let stdout = owned
            .child
            .stdout
            .take()
            .ok_or(Unavailable::CommandFailed)?;
        let stderr = owned
            .child
            .stderr
            .take()
            .ok_or(Unavailable::CommandFailed)?;
        let shared = &overflow;
        let stopped = &retired;
        let out = scope.spawn(move || drain(stdout, shared, stopped));
        let err = scope.spawn(move || drain(stderr, shared, stopped));
        let mut failure = None;
        let status = loop {
            if failure.is_none() {
                failure = if authority.current().is_err() {
                    Some(Unavailable::Cancelled)
                } else if Instant::now() >= deadline {
                    Some(Unavailable::Expired)
                } else if overflow.load(Ordering::SeqCst) {
                    Some(Unavailable::OutputLimit)
                } else {
                    None
                };
            }
            if failure.is_some() {
                let _ = owned.child.kill();
            }
            match owned.child.try_wait() {
                Ok(Some(value)) => {
                    owned.reaped = true;
                    break value;
                }
                Ok(None) => {}
                Err(_) => failure = Some(Unavailable::CommandFailed),
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        retired.store(true, Ordering::SeqCst);
        let output = Zeroizing::new(out.join().map_err(|_| Unavailable::CommandFailed)??);
        let errors = Zeroizing::new(err.join().map_err(|_| Unavailable::CommandFailed)??);
        if let Some(reason) = failure {
            return Err(reason);
        }
        if output.len() > LIMIT || errors.len() > LIMIT {
            return Err(Unavailable::OutputLimit);
        }
        let text = std::str::from_utf8(&output).map_err(|_| Unavailable::UnrecognizedOutput)?;
        if text
            .chars()
            .any(|v| v.is_control() && !matches!(v, '\r' | '\n' | '\t'))
        {
            return Err(Unavailable::UnrecognizedOutput);
        }
        let text = Zeroizing::new(text.to_ascii_lowercase());
        let owner = if text.contains("password")
            || text.contains("username:")
            || text.contains("authentication")
            || text.contains("passcode")
        {
            Some(OwnerStep::Authentication)
        } else if text.contains("accept?")
            || text.contains("certificate")
            || text.contains("banner")
        {
            Some(OwnerStep::BannerOrCertificate)
        } else {
            None
        };
        let mut connected = false;
        let mut error = false;
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix(">>").map(str::trim) {
                if let Some(state) = value.strip_prefix("state:") {
                    connected = state.trim() == "connected";
                }
                error |= value.starts_with("error:");
            }
        }
        Ok(Completion {
            connected: status.success()
                && errors.iter().all(u8::is_ascii_whitespace)
                && !error
                && connected,
            owner,
        })
    })
}
/// Requires the actual durable worker's one-shot authority, never callable from
/// a setup/diagnostic request. CLI retirement is not VPN-agent retirement.
pub fn connect(
    profile: &Profile,
    action: &Action,
    authority: &mut EffectAuthority<'_>,
) -> Result<Report, ErrorCode> {
    profile.action(action)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    authority.current()?;
    current(profile)?;
    let before = stats(
        deadline.min(Instant::now() + Duration::from_secs(5)),
        &mut || authority.current(),
    );
    match before.as_ref() {
        Ok(value) if matches!(value.state, VpnState::Disconnected) => {}
        Ok(value) if matches!(value.state, VpnState::Connected) => {
            let state = if value.server == Some(profile.address) {
                State::AlreadyConnected
            } else {
                State::OtherConnection
            };
            return Ok(report(
                profile,
                state,
                false,
                before,
                if state == State::AlreadyConnected {
                    None
                } else {
                    Some(OwnerStep::ReconcileConnection)
                },
            ));
        }
        _ => {
            return Ok(report(
                profile,
                State::NeedsOwner,
                false,
                before,
                Some(OwnerStep::ReconcileConnection),
            ));
        }
    }
    // From the first possible commit onward, every failure is potentially an
    // agent-side request. No retry/disconnect is issued by this adapter.
    let mut issued = false;
    let completion = run(profile, deadline, authority, &mut issued);
    let observed = if authority.current().is_ok() && Instant::now() < deadline {
        stats(
            deadline.min(Instant::now() + Duration::from_secs(5)),
            &mut || authority.current(),
        )
    } else {
        Err(Unavailable::Cancelled)
    };
    if completion.as_ref().is_ok_and(|v| v.connected)
        && observed.as_ref().is_ok_and(|v| {
            matches!(v.state, VpnState::Connected) && v.server == Some(profile.address)
        })
    {
        return Ok(report(profile, State::Connected, true, observed, None));
    }
    let owner = completion.ok().and_then(|v| v.owner);
    let state = if owner.is_some() {
        State::NeedsOwner
    } else {
        State::Uncertain
    };
    Ok(report(
        profile,
        state,
        issued,
        observed,
        owner.or(Some(OwnerStep::ReconcileConnection)),
    ))
}
