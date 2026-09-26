//! Read-only Cisco stats for one observed signed build. No connect or credentials.
use avesra_contracts::ErrorCode;
use avesra_core::diagnostics::{Reading, Report, Unavailable, VpnState};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const PATH: &str = r"C:\Program Files (x86)\Cisco\Cisco Secure Client\vpncli.exe";
const DIGEST: &str = "567ff2854d62b1ac24b661774486917201d2112520f30a3a9b33aaa224e1c4e0";
const LIMIT: usize = 16_384;

fn identity(deadline: Instant) -> Result<File, Unavailable> {
    // Deny write/delete for the complete child lifetime, including replacement.
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(PATH)
        .map_err(|_| Unavailable::Missing)?;
    if file.metadata().map_err(|_| Unavailable::Missing)?.len() != 145_968 {
        return Err(Unavailable::Unsupported);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    let mut total = 0;
    loop {
        if Instant::now() >= deadline {
            return Err(Unavailable::Expired);
        }
        let count = file
            .read(&mut buffer)
            .map_err(|_| Unavailable::Unsupported)?;
        if count == 0 {
            break;
        }
        total += count;
        if total > 145_968 {
            return Err(Unavailable::Unsupported);
        }
        digest.update(&buffer[..count]);
    }
    if total != 145_968 || format!("{:x}", digest.finalize()) != DIGEST {
        return Err(Unavailable::Unsupported);
    }
    Ok(file)
}
fn drain(mut pipe: impl Read, overflow: &AtomicBool) -> Result<Vec<u8>, Unavailable> {
    let mut output = Vec::with_capacity(LIMIT);
    let mut buffer = [0u8; 1024];
    loop {
        let count = pipe.read(&mut buffer).map_err(|_| {
            overflow.store(true, Ordering::SeqCst);
            Unavailable::CommandFailed
        })?;
        if count == 0 {
            break;
        }
        if output.len() + count <= LIMIT && !overflow.load(Ordering::SeqCst) {
            output.extend_from_slice(&buffer[..count]);
        } else {
            overflow.store(true, Ordering::SeqCst);
        }
    }
    if overflow.load(Ordering::SeqCst) {
        return Err(Unavailable::OutputLimit);
    }
    Ok(output)
}
struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Do not detach on exceptional paths. Failed kill/reap retains the
        // actual worker; it must never make a second diagnostic appear free.
        while !self.reaped {
            let _ = self.child.kill();
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                self.reaped = true;
            } else {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
fn parsed(output: &[u8]) -> Result<VpnState, Unavailable> {
    let text = std::str::from_utf8(output).map_err(|_| Unavailable::UnrecognizedOutput)?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
    {
        return Err(Unavailable::UnrecognizedOutput);
    }
    let mut state = None;
    let mut tunnel = false;
    for line in text.lines() {
        let line = line.trim().to_ascii_lowercase();
        if line == "[tunnel information]" {
            tunnel = true;
        }
        let Some(event) = line.strip_prefix(">>").map(str::trim) else {
            continue;
        };
        if event.starts_with("error:") {
            return Err(Unavailable::CommandFailed);
        }
        if let Some(value) = event.strip_prefix("state:") {
            state = Some(match value.trim() {
                "unknown" => VpnState::Unknown,
                "disconnected" => VpnState::Disconnected,
                "connecting" => VpnState::Connecting,
                "connected" => VpnState::Connected,
                "reconnecting" => VpnState::Reconnecting,
                _ => return Err(Unavailable::UnrecognizedOutput),
            });
        }
    }
    match state {
        Some(VpnState::Connected) if !tunnel => Err(Unavailable::UnrecognizedOutput),
        Some(value) => Ok(value),
        None => Err(Unavailable::UnrecognizedOutput),
    }
}
fn stats(
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<VpnState, Unavailable> {
    let _identity = identity(deadline)?;
    if authorize().is_err() {
        return Err(Unavailable::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(Unavailable::Expired);
    }
    let overflow = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let child = Command::new(PATH)
            .arg("stats")
            .creation_flags(0x0800_0000)
            .current_dir(r"C:\Program Files (x86)\Cisco\Cisco Secure Client")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| Unavailable::CommandFailed)?;
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
        let out = scope.spawn(move || drain(stdout, shared));
        let err = scope.spawn(move || drain(stderr, shared));
        let mut failure = None;
        let status = loop {
            if failure.is_none() {
                failure = if authorize().is_err() {
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
                Ok(Some(status)) => {
                    owned.reaped = true;
                    break status;
                }
                Ok(None) => {}
                Err(_) => {
                    failure = Some(Unavailable::CommandFailed);
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        // Readers remain owned until actual EOF, including cancellation paths.
        let output = out.join().map_err(|_| Unavailable::CommandFailed)??;
        let errors = err.join().map_err(|_| Unavailable::CommandFailed)??;
        if let Some(reason) = failure {
            return Err(reason);
        }
        if !status.success() || !errors.iter().all(u8::is_ascii_whitespace) {
            return Err(Unavailable::CommandFailed);
        }
        if Instant::now() >= deadline {
            return Err(Unavailable::Expired);
        }
        if authorize().is_err() {
            return Err(Unavailable::Cancelled);
        }
        parsed(&output)
    })
}
pub(crate) fn observe(
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Report, ErrorCode> {
    let state = match stats(deadline, authorize) {
        Ok(value) => Reading::Available { value },
        Err(reason) => Reading::Unavailable { reason },
    };

    authorize()?;
    let report = Report::CiscoVpnStatus { state };
    report.validate()?;
    Ok(report)
}
