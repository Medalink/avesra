//! Native messaging status endpoint. No browser/desktop effect is implemented here.
use serde::Deserialize;
use std::io::{self, Read, Write};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u16,
    #[serde(rename = "type")]
    kind: String,
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let origin = std::env::args().nth(1).ok_or("Missing caller origin")?;
    let executable = std::env::current_exe()?;
    let config = executable
        .parent()
        .ok_or("Missing host directory")?
        .join("avesra-extension-origin.txt");
    let expected = std::fs::read_to_string(config)?;
    let expected = expected.trim();
    let id = expected
        .strip_prefix("chrome-extension://")
        .and_then(|v| v.strip_suffix('/'))
        .ok_or("Invalid configured origin")?;
    if id.len() != 32 || !id.bytes().all(|b| (b'a'..=b'p').contains(&b)) || origin != expected {
        return Err("Caller origin is not allowed".into());
    }
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut prefix = [0u8; 4];
        match input.read_exact(&mut prefix) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let length = u32::from_le_bytes(prefix) as usize;
        if length == 0 || length > avesra_contracts::MAX_CONTROL_BYTES {
            return Err("Invalid message length".into());
        }
        let mut bytes = vec![0u8; length];
        input.read_exact(&mut bytes)?;
        let request: Request = serde_json::from_slice(&bytes).map_err(|_| "Invalid message")?;
        if request.version != 1 || request.kind != "status" {
            return Err("Unsupported message".into());
        }
        let response=br#"{"version":1,"status":"unavailable","reason":"authenticated_browser_control_not_configured"}"#;
        output.write_all(&(response.len() as u32).to_le_bytes())?;
        output.write_all(response)?;
        output.flush()?;
    }
    Ok(())
}
fn main() {
    if run().is_err() {
        eprintln!("Avesra native host rejected the connection or encountered an I/O error.");
        std::process::exit(1);
    }
}
