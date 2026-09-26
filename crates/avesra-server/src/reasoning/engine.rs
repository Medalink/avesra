//! Fixed exact-container observer and transport. No lifecycle commands.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const HOST: &str = include_str!("../../../../services/reasoning/host.py");
const ENGINE: &str = include_str!("../../../../services/reasoning/engine.py");
const PROFILES: &str = include_str!("../../../../services/reasoning/profiles.py");

pub(super) fn observer_revision() -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{PROFILES}\n{HOST}\n{ENGINE}").as_bytes())
    )
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
pub(super) enum Profile {
    #[default]
    #[serde(rename = "legacy_35b")]
    Legacy35b,
    #[serde(rename = "owned_27b")]
    Owned27b,
}
impl Profile {
    pub fn image(self) -> &'static str {
        match self {
            Self::Legacy35b => {
                "sha256:d464f3b466fa9c45ddbff8a812e80564503b6879a9fd95c1a47514f3f0df5a4a"
            }
            Self::Owned27b => {
                "sha256:d7c6827ade3234f66b365a492ff81e927ed64fc1c73f180aaefab69ebf8c2dd0"
            }
        }
    }
    pub fn revision(self) -> &'static str {
        match self {
            Self::Legacy35b => "95a723d08a9490559dae23d0cff1d9466213d989",
            Self::Owned27b => "017b9c7af6b5689d5dd426a76e0bc077eb5ca20a",
        }
    }
    pub fn model(self) -> &'static str {
        match self {
            Self::Legacy35b => "avesra-fast",
            Self::Owned27b => "avesra-owned-27b",
        }
    }
    pub fn port(self) -> u16 {
        match self {
            Self::Legacy35b => 8888,
            Self::Owned27b => 8000,
        }
    }
    pub fn capacity(self) -> u64 {
        match self {
            Self::Legacy35b => 16384,
            Self::Owned27b => 4096,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    #[serde(default)]
    pub profile: Profile,
    pub image: String,
    pub model_directory: PathBuf,
}
impl Config {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.image != self.profile.image() || !self.model_directory.is_absolute() {
            return Err(ErrorCode::Unsupported);
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    pub identity: String,
    pub artifact: String,
    pub prompt_tokens: u64,
    pub capacity: u64,
    pub quality: Option<String>,
    pub stream: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FailureReason {
    CaptureMissing,
    CaptureInvalid,
    ArtifactChanged,
    ControlledLoadRequired,
    EngineIdentity,
    ContextCapacity,
    StreamDrain,
    HelperUnavailable,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    error: FailureReason,
}
impl Observation {
    fn validate(&self, profile: Profile) -> Result<(), ErrorCode> {
        if [&self.identity, &self.artifact].iter().any(|v| {
            v.len() != 64
                || !v
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        }) || self.prompt_tokens == 0
            || self.capacity != profile.capacity()
            || self.prompt_tokens > self.capacity - 512
            || self.stream.as_ref().is_some_and(|v| v.len() > 1_398_104)
            || self.quality.as_ref().is_some_and(|v| {
                v.len() != 64
                    || !v
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
pub(super) struct Request {
    pub config: Config,
    pub record: PathBuf,
    pub container: String,
    pub instance: String,
    pub key: Zeroizing<String>,
    pub body: serde_json::Value,
    pub expected: Option<String>,
    pub stream: bool,
    pub deadline: Instant,
}
/// The caller awaits this actual blocking owner even after caller withdrawal.
/// Host/helper timeout is not remote job retirement; only parsed complete output
/// can retire the existing durable Jobs lease.
pub(super) async fn run(request: Request) -> Result<Observation, ErrorCode> {
    tokio::task::spawn_blocking(move || {
        let remaining = request
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|v| !v.is_zero())
            .ok_or(ErrorCode::Expired)?;
        let milliseconds = remaining.min(Duration::from_secs(30)).as_millis() as u64;
        let bytes = Zeroizing::new(
            serde_json::to_vec(&serde_json::json!({
                "container": request.container, "record": request.record,
                "instance": request.instance,
                "profile": request.config.profile,
                "revision": request.config.profile.revision(), "image": request.config.image,
                "root": request.config.model_directory,
                "request": {"mode": if request.stream {"stream"} else {"inspect"},
                    "profile":request.config.profile,
                    "model":request.config.profile.model(), "port":request.config.profile.port(), "key": &*request.key,
                    "remaining_ms":milliseconds, "body":request.body, "expected":request.expected}
            }))
            .map_err(|_| ErrorCode::Malformed)?,
        );
        if bytes.len() > 131_072 {
            return Err(ErrorCode::TooLarge);
        }
        let host = format!("{PROFILES}\n{HOST}");
        let engine = format!("{PROFILES}\n{ENGINE}");
        let mut child = Command::new("/usr/bin/python3")
            .args(["-I", "-c", &host, &engine])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ErrorCode::Unavailable)?;
        let written = child.stdin.take().map(|mut input| input.write_all(&bytes));
        drop(bytes);
        let mut output = Vec::new();
        let read = child
            .stdout
            .take()
            .map(|output_pipe| output_pipe.take(1_500_001).read_to_end(&mut output));
        // Do not leave a writer stuck behind a bounded full pipe.
        let io_failed = !matches!(written, Some(Ok(()))) || !matches!(read, Some(Ok(_)));
        if output.len() > 1_500_000 || io_failed {
            let _ = child.kill();
        }
        let ended = child.wait().map_err(|_| ErrorCode::Unavailable)?;
        if io_failed || output.len() > 1_500_000 || Instant::now() >= request.deadline {
            return Err(ErrorCode::Unavailable);
        }
        if !ended.success() {
            if let Ok(failure) = serde_json::from_slice::<Failure>(&output) {
                // Fixed typed diagnostics only; never helper exception strings,
                // accepted text, model output, paths or credentials.
                eprintln!("Reasoning unavailable: {:?}", failure.error);
            }
            return Err(ErrorCode::Unavailable);
        }
        let result: Observation =
            serde_json::from_slice(&output).map_err(|_| ErrorCode::Malformed)?;
        result.validate(request.config.profile)?;
        Ok(result)
    })
    .await
    .map_err(|_| ErrorCode::Unavailable)?
}
