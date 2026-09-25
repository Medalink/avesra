use avesra_contracts::discovery::{Advertisement, MAX_PACKET, PORT, Probe, local_address};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Default)]
pub struct Discovery {
    scanning: tokio::sync::Mutex<()>,
    epoch: AtomicU64,
    candidates: Mutex<Vec<Candidate>>,
}
struct Candidate {
    summary: FoundSpark,
    certificate: String,
    expires: Instant,
}
#[derive(Clone, Serialize)]
pub struct FoundSpark {
    id: Uuid,
    name: String,
    address: String,
    pairing_open: bool,
}
impl Discovery {
    pub fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut entries) = self.candidates.lock() {
            entries.clear();
        }
    }
}

#[tauri::command]
pub fn cancel_spark_discovery(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::Runtime>,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Open Settings to manage Spark discovery".into());
    }
    state.discovery.invalidate();
    Ok(())
}

#[tauri::command]
pub async fn discover_sparks(
    window: tauri::WebviewWindow,
    hint: Option<String>,
    state: tauri::State<'_, crate::Runtime>,
) -> Result<Vec<FoundSpark>, String> {
    if window.label() != "settings" {
        return Err("Open Settings to find a Spark".into());
    }
    let _scan = state
        .discovery
        .scanning
        .try_lock()
        .map_err(|_| "A scan is already running")?;
    state.discovery.invalidate();
    let epoch = state.discovery.epoch.load(Ordering::SeqCst);
    let socket = tokio::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|_| "Local discovery unavailable")?;
    socket
        .set_broadcast(true)
        .map_err(|_| "Local broadcast unavailable")?;
    let nonce = Uuid::new_v4();
    let bytes = serde_json::to_vec(&Probe {
        product: "Avesra".into(),
        version: 1,
        nonce,
    })
    .map_err(|_| "Discovery unavailable")?;
    let mut sent = socket
        .send_to(&bytes, SocketAddrV4::new(Ipv4Addr::BROADCAST, PORT))
        .await
        .is_ok();
    if let Some(ip) = hint
        .as_deref()
        .and_then(|value| reqwest::Url::parse(value).ok())
        .and_then(|url| url.host_str()?.parse::<Ipv4Addr>().ok())
        .filter(|ip| local_address(*ip))
    {
        sent |= socket
            .send_to(&bytes, SocketAddrV4::new(ip, PORT))
            .await
            .is_ok();
    }
    if !sent {
        return Err("Unable to scan this network. Check your connection and try again.".into());
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut buffer = [0u8; MAX_PACKET + 1];
    let mut candidates: Vec<Candidate> = Vec::new();
    for _ in 0..128 {
        if state.discovery.epoch.load(Ordering::SeqCst) != epoch {
            return Err("Scan cancelled".into());
        }
        let Ok(received) = tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
        else {
            break;
        };
        let (size, sender) = received.map_err(|_| "Network discovery interrupted")?;
        let std::net::IpAddr::V4(ip) = sender.ip() else {
            continue;
        };
        if !local_address(ip) || sender.port() != PORT || size > MAX_PACKET {
            continue;
        }
        let Ok(ad) = serde_json::from_slice::<Advertisement>(&buffer[..size]) else {
            continue;
        };
        if !ad.valid(nonce) || crate::connection::certificate(&ad.certificate).is_err() {
            continue;
        }
        let address = format!("https://{ip}:9474");
        if candidates
            .iter()
            .any(|c| c.summary.address == address && c.certificate == ad.certificate)
        {
            continue;
        }
        candidates.push(Candidate {
            summary: FoundSpark {
                id: Uuid::new_v4(),
                name: ad.name,
                address,
                pairing_open: ad.pairing_open,
            },
            certificate: ad.certificate,
            expires: Instant::now() + Duration::from_secs(60),
        });
        if candidates.len() == 16 {
            break;
        }
    }
    let mut entries = state
        .discovery
        .candidates
        .lock()
        .map_err(|_| "Discovery unavailable")?;
    if state.discovery.epoch.load(Ordering::SeqCst) != epoch {
        return Err("Scan cancelled".into());
    }
    candidates.sort_by(|a, b| {
        b.summary
            .pairing_open
            .cmp(&a.summary.pairing_open)
            .then(a.summary.name.cmp(&b.summary.name))
    });
    let summaries = candidates.iter().map(|c| c.summary.clone()).collect();
    *entries = candidates;
    Ok(summaries)
}

#[tauri::command]
pub async fn pair_discovered_spark(
    window: tauri::WebviewWindow,
    id: Uuid,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::Runtime>,
) -> Result<(), String> {
    if window.label() != "settings"
        || !window.is_visible().map_err(|_| "Settings unavailable")?
        || state
            .local
            .lock()
            .map_err(|_| "Local state unavailable")?
            .locked
    {
        return Err("Open Settings on the unlocked PC to choose a Spark".into());
    }
    let _guard = state
        .pairing
        .try_lock()
        .map_err(|_| "Pairing is already in progress")?;
    let generation = state.connection_generation.load(Ordering::SeqCst);
    let candidate = {
        let mut entries = state
            .discovery
            .candidates
            .lock()
            .map_err(|_| "Discovery unavailable")?;
        let index = entries
            .iter()
            .position(|c| c.summary.id == id)
            .ok_or("Scan again to find this Spark")?;
        let candidate = entries.remove(index);
        if Instant::now() >= candidate.expires || !candidate.summary.pairing_open {
            return Err("Scan again; this pairing offer is no longer available".into());
        }
        candidate
    };
    let cert = crate::connection::certificate(&candidate.certificate)?;
    let input = crate::connection::PairInput {
        url: candidate.summary.address,
        fingerprint: hex::encode(Sha256::digest(cert)),
        certificate: candidate.certificate,
        code: String::new(),
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Local data directory unavailable")?;
    let record = crate::connection::pair_discovered(input, &directory).await?;
    crate::start_connection(&app, record, generation)
}
