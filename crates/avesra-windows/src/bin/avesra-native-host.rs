//! Dedicated Chrome/Brave native-message relay; no page/effect authority.
#[cfg(windows)]
mod host {
    use avesra_contracts::{
        ErrorCode,
        browser::{self, Client},
    };
    use std::{
        io::{Read, Write},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    use zeroize::Zeroizing;

    struct Deadlines {
        phase: Instant,
        total: Instant,
    }
    struct Watchdog {
        started: Instant,
        deadlines: Arc<Mutex<Deadlines>>,
    }
    impl Watchdog {
        fn start() -> Self {
            let started = Instant::now();
            let deadlines = Arc::new(Mutex::new(Deadlines {
                phase: started + Duration::from_secs(5),
                total: started + Duration::from_secs(45),
            }));
            let owner = deadlines.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(25));
                    let Ok(deadline) = owner.lock() else {
                        std::process::exit(2)
                    };
                    let now = Instant::now();
                    // Expiry and re-arming share this lock. An expired phase cannot
                    // be revived; exit kills this dedicated host and any blocked
                    // stdio/hash worker, never another native/application process.
                    if now >= deadline.phase || now >= deadline.total {
                        std::process::exit(2)
                    }
                }
            });
            Self { started, deadlines }
        }
        fn phase(&self, seconds: u64, authenticated: bool) -> Result<(), ErrorCode> {
            let mut deadline = self.deadlines.lock().map_err(|_| ErrorCode::Unavailable)?;
            let now = Instant::now();
            if now >= deadline.phase || now >= deadline.total {
                return Err(ErrorCode::Expired);
            }
            if authenticated {
                deadline.total = self.started + Duration::from_secs(300);
            }
            deadline.phase = (now + Duration::from_secs(seconds)).min(deadline.total);
            Ok(())
        }
    }
    fn configured_extension() -> Result<String, ErrorCode> {
        let origin = std::env::args().nth(1).ok_or(ErrorCode::Unauthenticated)?;
        if origin.len() > 256 {
            return Err(ErrorCode::TooLarge);
        }
        let path = std::env::current_exe()
            .map_err(|_| ErrorCode::Unavailable)?
            .parent()
            .ok_or(ErrorCode::Unavailable)?
            .join("avesra-extension-origin.txt");
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| ErrorCode::Unavailable)?
            .take(257)
            .read_to_end(&mut bytes)
            .map_err(|_| ErrorCode::Unavailable)?;
        if bytes.len() > 256 {
            return Err(ErrorCode::TooLarge);
        }
        let expected = std::str::from_utf8(&bytes)
            .map_err(|_| ErrorCode::Malformed)?
            .trim();
        let id = expected
            .strip_prefix("chrome-extension://")
            .and_then(|v| v.strip_suffix('/'))
            .filter(|v| browser::extension_id(v))
            .ok_or(ErrorCode::Malformed)?;
        if origin != expected {
            return Err(ErrorCode::Unauthenticated);
        }
        Ok(id.into())
    }
    fn read(input: &mut impl Read) -> Result<Zeroizing<Vec<u8>>, ErrorCode> {
        let mut prefix = [0; 4];
        input
            .read_exact(&mut prefix)
            .map_err(|_| ErrorCode::Unavailable)?;
        let length = u32::from_le_bytes(prefix) as usize;
        if length == 0 || length > browser::MAX_MESSAGE {
            return Err(ErrorCode::TooLarge);
        }
        let mut bytes = Zeroizing::new(vec![0; length]);
        input
            .read_exact(&mut bytes)
            .map_err(|_| ErrorCode::Unavailable)?;
        std::str::from_utf8(&bytes).map_err(|_| ErrorCode::Malformed)?;
        Ok(bytes)
    }
    fn write(output: &mut impl Write, bytes: &[u8]) -> Result<(), ErrorCode> {
        if bytes.is_empty() || bytes.len() > browser::MAX_MESSAGE {
            return Err(ErrorCode::TooLarge);
        }
        output
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .and_then(|_| output.write_all(bytes))
            .and_then(|_| output.flush())
            .map_err(|_| ErrorCode::Unavailable)
    }
    pub fn run() -> Result<(), ErrorCode> {
        let watchdog = Watchdog::start();
        let extension = configured_extension()?;
        let mut input = std::io::stdin().lock();
        let mut output = std::io::stdout().lock();
        let first = read(&mut input)?;
        let Client::Hello(hello) = browser::decode(&first)? else {
            return Err(ErrorCode::Malformed);
        };
        hello.validate()?;
        if hello.extension != extension {
            return Err(ErrorCode::Unauthenticated);
        }
        // Async transport belongs to this dedicated process. A blocked runtime
        // shutdown cannot outlive the watchdog's fixed budget.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ErrorCode::Unavailable)?;
        watchdog.phase(45, false)?;
        let mut pipe = runtime.block_on(avesra_windows::browser_pipe::connect())?;
        watchdog.phase(5, false)?;
        runtime.block_on(pipe.send(&first))?;
        let response = Zeroizing::new(runtime.block_on(pipe.receive())?);
        watchdog.phase(5, false)?;
        write(&mut output, &response)?;
        drop(response);
        drop(first);
        let mut authenticated = false;
        loop {
            watchdog.phase(5, false)?;
            let request = read(&mut input)?;
            // The shared v6 enum also strictly decodes read result/settlement
            // envelopes. This relay forwards bytes; only ReceiveOwner can mint
            // an authenticated native settlement proof.
            let message: Client = browser::decode(&request)?;
            if !authenticated
                && matches!(
                    message,
                    Client::ReadResult { .. } | Client::ReadSettlement { .. }
                )
            {
                return Err(ErrorCode::Unauthenticated);
            }
            if matches!(message, Client::Hello(_)) {
                return Err(ErrorCode::Malformed);
            }
            let authenticating = matches!(message, Client::Authenticate(_));
            let disconnect = matches!(message, Client::Disconnect { .. });
            if authenticated && authenticating {
                return Err(ErrorCode::Malformed);
            }
            watchdog.phase(5, false)?;
            runtime.block_on(pipe.send(&request))?;
            drop(request);
            drop(message);
            if disconnect {
                return Ok(());
            }
            let response = Zeroizing::new(runtime.block_on(pipe.receive())?);
            // Do not deserialize credential-bearing bodies into extra owned
            // strings. The extension validates the exact response schema.
            #[derive(serde::Deserialize)]
            struct Kind {
                #[serde(rename = "type")]
                kind: String,
            }
            let kind: Kind = browser::decode(&response)?;
            if !matches!(kind.kind.as_str(), "status" | "issued" | "authenticated") {
                return Err(ErrorCode::Malformed);
            }
            if kind.kind == "authenticated" {
                if !authenticating || authenticated {
                    return Err(ErrorCode::Malformed);
                }
                authenticated = true;
            } else if authenticating {
                return Err(ErrorCode::Malformed);
            }
            watchdog.phase(5, authenticated)?;
            write(&mut output, &response)?;
        }
    }
}
fn main() {
    #[cfg(windows)]
    let success = host::run().is_ok();
    #[cfg(not(windows))]
    let success = false;
    if !success {
        eprintln!("Avesra native host rejected the connection or encountered an I/O error.");
        std::process::exit(1);
    }
    std::process::exit(0);
}
