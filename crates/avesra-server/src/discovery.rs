use avesra_contracts::discovery::{Advertisement, MAX_PACKET, PORT, Probe, local_address};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub struct PairingWindow {
    deadline: Instant,
    available: AtomicBool,
}
impl PairingWindow {
    pub fn new(open: bool) -> Self {
        Self {
            deadline: Instant::now() + Duration::from_secs(300),
            available: AtomicBool::new(open),
        }
    }
    pub fn open(&self) -> bool {
        Instant::now() < self.deadline && self.available.load(Ordering::SeqCst)
    }
    pub fn take(&self) -> bool {
        Instant::now() < self.deadline && self.available.swap(false, Ordering::SeqCst)
    }
    pub fn close(&self) {
        self.available.store(false, Ordering::SeqCst);
    }
}

pub async fn serve(
    certificate: String,
    pairing: std::sync::Arc<PairingWindow>,
) -> Result<(), String> {
    if certificate.len() > 4096 {
        return Err("Discovery certificate is too large".into());
    }
    let socket = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, PORT))
        .await
        .map_err(|_| "Discovery port unavailable")?;
    let name = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .unwrap_or_else(|| "Avesra Spark".into());
    let name: String = name
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ' '))
        .take(64)
        .collect();
    let name = if name.is_empty() {
        "Avesra Spark".into()
    } else {
        name
    };
    let mut buffer = [0u8; MAX_PACKET + 1];
    let mut budget_started = Instant::now();
    let mut replies = 0;
    loop {
        let (size, sender) = socket
            .recv_from(&mut buffer)
            .await
            .map_err(|_| "Discovery receiver stopped")?;
        let std::net::IpAddr::V4(ip) = sender.ip() else {
            continue;
        };
        if !local_address(ip) || size > 256 {
            continue;
        }
        let Ok(probe) = serde_json::from_slice::<Probe>(&buffer[..size]) else {
            continue;
        };
        if probe.product != "Avesra" || probe.version != 1 || probe.nonce.is_nil() {
            continue;
        }
        if budget_started.elapsed() >= Duration::from_secs(1) {
            budget_started = Instant::now();
            replies = 0;
        }
        if replies >= 8 {
            continue;
        }
        replies += 1;
        let response = Advertisement {
            product: "Avesra".into(),
            version: 1,
            protocol: avesra_contracts::PROTOCOL_VERSION,
            nonce: probe.nonce,
            name: name.clone(),
            certificate: certificate.clone(),
            pairing_open: pairing.open(),
        };
        let bytes = serde_json::to_vec(&response).map_err(|_| "Discovery encoding failed")?;
        if bytes.len() <= MAX_PACKET {
            let _ = socket.send_to(&bytes, sender).await;
        }
    }
}
