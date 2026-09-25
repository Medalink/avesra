use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub fn now_ms() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .ok_or_else(|| "System clock unavailable".into())
}
fn secret() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
fn digest(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
pub struct AuthStore {
    connection: Connection,
}
impl AuthStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection =
            Connection::open(path).map_err(|_| "Authentication database unavailable")?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|_| "Authentication version unavailable")?;
        if version != 0 && version != 1 {
            return Err("Unsupported authentication schema".into());
        }
        if version == 0 {
            let tables:i64=connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(|_|"Authentication schema unavailable")?;
            if tables != 0 {
                return Err("Unversioned authentication database is not empty".into());
            }
            let tx = connection
                .transaction()
                .map_err(|_| "Authentication schema unavailable")?;
            tx.execute_batch("CREATE TABLE pairing(id INTEGER PRIMARY KEY CHECK(id=1),hash BLOB NOT NULL,expires_ms INTEGER NOT NULL,attempts INTEGER NOT NULL); CREATE TABLE devices(id TEXT PRIMARY KEY,hash BLOB NOT NULL UNIQUE,revoked INTEGER NOT NULL DEFAULT 0,created_ms INTEGER NOT NULL); PRAGMA user_version=1;").map_err(|_|"Authentication schema unavailable")?;
            tx.commit()
                .map_err(|_| "Authentication schema unavailable")?;
        }
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=2000;")
            .map_err(|_| "Authentication settings unavailable")?;
        Ok(Self { connection })
    }
    pub fn new_code(&mut self) -> Result<String, String> {
        let code = secret();
        let expires = now_ms()?.checked_add(300_000).ok_or("Clock overflow")?;
        self.connection.execute("INSERT INTO pairing VALUES(1,?1,?2,0) ON CONFLICT(id) DO UPDATE SET hash=excluded.hash,expires_ms=excluded.expires_ms,attempts=0",params![digest(&code),expires]).map_err(|_|"Unable to create pairing code")?;
        Ok(code)
    }
    pub fn pair(&mut self, code: &str) -> Result<(Uuid, String), String> {
        if code.len() != 64 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Pairing rejected".into());
        }
        let tx = self
            .connection
            .transaction()
            .map_err(|_| "Pairing unavailable")?;
        let row: Option<(Vec<u8>, i64, i64)> = tx
            .query_row(
                "SELECT hash,expires_ms,attempts FROM pairing WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|_| "Pairing unavailable")?;
        let (hash, expires, attempts) = row.ok_or("Pairing is not enabled")?;
        let now = now_ms()?;
        if now >= expires || now < expires.saturating_sub(300_000) || attempts >= 5 {
            return Err("Pairing code expired or locked".into());
        }
        if !bool::from(hash.ct_eq(&digest(code))) {
            tx.execute("UPDATE pairing SET attempts=attempts+1 WHERE id=1", [])
                .map_err(|_| "Pairing unavailable")?;
            tx.commit().map_err(|_| "Pairing unavailable")?;
            return Err("Pairing rejected".into());
        }
        let device = Uuid::new_v4();
        let token = secret();
        tx.execute(
            "INSERT INTO devices(id,hash,created_ms) VALUES(?1,?2,?3)",
            params![device.to_string(), digest(&token), now_ms()?],
        )
        .map_err(|_| "Pairing unavailable")?;
        tx.execute("DELETE FROM pairing", [])
            .map_err(|_| "Pairing unavailable")?;
        tx.commit().map_err(|_| "Pairing unavailable")?;
        Ok((device, token))
    }
    pub fn authenticate(&self, token: &str) -> Result<Uuid, String> {
        if token.len() != 64 {
            return Err("Unauthorized".into());
        }
        let id: Option<String> = self
            .connection
            .query_row(
                "SELECT id FROM devices WHERE hash=?1 AND revoked=0",
                [digest(token)],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "Authentication unavailable")?;
        Uuid::parse_str(&id.ok_or("Unauthorized")?).map_err(|_| "Invalid device record".into())
    }
    pub fn active(&self, device: Uuid) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE id=?1 AND revoked=0)",
                [device.to_string()],
                |r| r.get(0),
            )
            .map_err(|_| "Authentication unavailable".into())
    }
    pub fn revoke(&mut self, device: Uuid) -> Result<(), String> {
        let changed = self
            .connection
            .execute(
                "UPDATE devices SET revoked=1 WHERE id=?1",
                [device.to_string()],
            )
            .map_err(|_| "Revocation unavailable")?;
        if changed == 1 {
            Ok(())
        } else {
            Err("Unknown device".into())
        }
    }
}

pub fn private_write(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
