use crate::{ModeSnapshot, Runtime};
use avesra_contracts::{
    ConnectionStatus, ControlMessage, Envelope, MAX_CONTROL_BYTES, PROTOCOL_VERSION, ServerStatus,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::BufReader, path::Path, sync::Arc, time::Duration};
use tauri::{Emitter, Manager};
use tokio_tungstenite::{
    Connector,
    tungstenite::{
        client::IntoClientRequest,
        protocol::{Message, WebSocketConfig},
    },
};
use uuid::Uuid;
#[derive(Clone, Copy)]
pub struct SessionIdentity {
    pub id: Uuid,
    pub epoch: u64,
    pub playback_epoch: u64,
    pub generation: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakerHealth {
    version: u16,
    lane: String,
    model_revision: String,
    state: String,
    streaming: bool,
    cancellation: String,
    permission_authority: bool,
    busy: bool,
    successful_inferences: u64,
    last_inference_ms: Option<f64>,
}

fn speaker_client(record: &PairingRecord) -> Result<reqwest::Client, String> {
    let cert = certificate(&record.certificate)?;
    reqwest::Client::builder()
        .tls_built_in_root_certs(false)
        .add_root_certificate(
            reqwest::Certificate::from_der(&cert).map_err(|_| "Invalid TLS certificate")?,
        )
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "Speaker TLS client unavailable".into())
}
pub async fn speaker_health(record: &PairingRecord) -> Result<SpeakerHealth, String> {
    let response = speaker_client(record)?
        .get(
            endpoint(&record.url)?
                .join("speaker")
                .map_err(|_| "Invalid Spark endpoint")?,
        )
        .bearer_auth(&record.credential)
        .send()
        .await
        .map_err(|_| "Speaker service unavailable")?;
    let value: SpeakerHealth = serde_json::from_value(bounded_response(response).await?)
        .map_err(|_| "Invalid speaker metadata")?;
    if value.version != 1
        || value.lane != "speaker"
        || value.model_revision != "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286"
        || !matches!(
            value.state.as_str(),
            "unavailable" | "loading" | "loaded_unqualified" | "termination_pending"
        )
        || value.permission_authority
        || value.streaming
        || value.cancellation != "terminate_process"
        || value
            .last_inference_ms
            .is_some_and(|v| !v.is_finite() || !(0.0..=30000.0).contains(&v))
    {
        return Err("Configured speaker metadata is incompatible".into());
    }
    Ok(value)
}
pub async fn speaker_available(record: &PairingRecord) -> Result<(), String> {
    if speaker_health(record).await?.state != "loaded_unqualified" {
        return Err("Configured speaker deployment is not available for enrollment".into());
    }
    Ok(())
}
async fn bounded_response(mut response: reqwest::Response) -> Result<serde_json::Value, String> {
    if !response.status().is_success() {
        return Err("Speaker request was rejected or unavailable".into());
    }
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Speaker response interrupted")?
    {
        if bytes.len() + chunk.len() > 16_384 {
            return Err("Speaker response exceeds limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid speaker response".into())
}
pub async fn enrollment_embedding(
    record: &PairingRecord,
    session: SessionIdentity,
    id: Uuid,
    pcm: Vec<u8>,
) -> Result<Vec<f32>, String> {
    use base64::Engine;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Reply {
        version: u16,
        request_id: Uuid,
        session_id: Uuid,
        capture_epoch: u64,
        model_revision: String,
        embedding: Vec<f32>,
    }
    let payload = serde_json::json!({"version":1,"request_id":id,"session_id":session.id,"capture_epoch":session.epoch,"pcm_s16le":base64::engine::general_purpose::STANDARD.encode(pcm)});
    let response = speaker_client(record)?
        .post(
            endpoint(&record.url)?
                .join("speaker")
                .map_err(|_| "Invalid Spark endpoint")?,
        )
        .bearer_auth(&record.credential)
        .json(&payload)
        .send()
        .await
        .map_err(|_| "Enrollment speaker request failed")?;
    drop(payload);
    let value: Reply = serde_json::from_value(bounded_response(response).await?)
        .map_err(|_| "Invalid embedding response")?;
    if value.version != 1
        || value.request_id != id
        || value.session_id != session.id
        || value.capture_epoch != session.epoch
        || value.model_revision != "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286"
        || value.embedding.len() != 192
        || value.embedding.iter().any(|v| !v.is_finite())
    {
        return Err("Embedding correlation failed".into());
    }
    Ok(value.embedding)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingRecord {
    version: u16,
    url: String,
    certificate: String,
    pub(crate) device_id: Uuid,
    credential: String,
}
pub(crate) enum MediaEndpoint {
    Capture,
    Preview,
}
pub(crate) async fn voice_socket(
    record: &PairingRecord,
    endpoint_kind: MediaEndpoint,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    record.validate()?;
    let mut url = endpoint(&record.url)?;
    url.set_scheme("wss").map_err(|_| "Invalid websocket URL")?;
    url.set_path(match endpoint_kind {
        MediaEndpoint::Capture => "/voice-stream",
        MediaEndpoint::Preview => "/voice-preview",
    });
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(certificate(&record.certificate)?)
        .map_err(|_| "Invalid trust root")?;
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| "Invalid voice endpoint")?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", record.credential)
            .parse()
            .map_err(|_| "Invalid credential")?,
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(16_384))
        .max_frame_size(Some(16_384))
        .write_buffer_size(1024)
        .max_write_buffer_size(32_768);
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(3),
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            Some(config),
            false,
            Some(Connector::Rustls(Arc::new(tls))),
        ),
    )
    .await
    .map_err(|_| "Voice connection timed out")?
    .map_err(|_| "Authenticated voice connection failed")?;
    Ok(socket)
}
impl PairingRecord {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.device_id.is_nil()
            || self.credential.len() != 64
            || !self.credential.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid saved pairing".into());
        }
        endpoint(&self.url)?;
        certificate(&self.certificate)?;
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairInput {
    pub url: String,
    pub certificate: String,
    pub fingerprint: String,
    pub code: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairReply {
    version: u16,
    device_id: Uuid,
    credential: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionReply {
    version: u16,
    device_id: Uuid,
    session_id: Uuid,
    status: String,
}
fn certificate(pem: &str) -> Result<rustls::pki_types::CertificateDer<'static>, String> {
    if pem.len() > 16_384 {
        return Err("Certificate exceeds size limit".into());
    }
    let certs = rustls_pemfile::certs(&mut BufReader::new(pem.as_bytes()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Invalid certificate")?;
    if certs.len() != 1 {
        return Err("Supply exactly one server certificate".into());
    }
    Ok(certs[0].clone())
}
fn endpoint(value: &str) -> Result<reqwest::Url, String> {
    if value.len() > 2048 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("Invalid Spark address".into());
    }
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid Spark address")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port() != Some(9474)
    {
        return Err("Use an HTTPS Spark hostname or IP address on port 9474, without credentials, path, query, or fragment. The verified certificate must cover that host.".into());
    }
    Ok(url)
}
pub async fn pair(input: PairInput, directory: &Path) -> Result<PairingRecord, String> {
    if directory.join("spark-pairing.dpapi").exists() {
        return Err("A saved pairing already exists. Reconnect it or explicitly revoke and remove it before replacing.".into());
    }
    let url = endpoint(&input.url)?;
    let cert = certificate(&input.certificate)?;
    let fingerprint = input.fingerprint.replace(':', "").to_ascii_lowercase();
    if fingerprint.len() != 64 || fingerprint != hex::encode(Sha256::digest(&cert)) {
        return Err(
            "Certificate fingerprint does not match. Check the server's local output.".into(),
        );
    }
    if input.code.len() != 64 || !input.code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Enter the complete one-time pairing code".into());
    }
    let client = reqwest::Client::builder()
        .tls_built_in_root_certs(false)
        .add_root_certificate(
            reqwest::Certificate::from_der(&cert).map_err(|_| "Invalid TLS certificate")?,
        )
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "TLS client unavailable")?;
    let mut response = client
        .post(url.join("pair").map_err(|_| "Invalid Spark address")?)
        .json(&serde_json::json!({"code":input.code}))
        .send()
        .await
        .map_err(|_| "Secure pairing connection failed. Check Spark address and certificate.")?;
    if !response.status().is_success() {
        return Err("Pairing rejected. The code may be expired, used or locked.".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Pairing response interrupted")?
    {
        if body.len() + chunk.len() > 4096 {
            return Err("Pairing response exceeds limit".into());
        }
        body.extend_from_slice(&chunk);
    }
    let reply: PairReply = serde_json::from_slice(&body).map_err(|_| "Invalid pairing response")?;
    if reply.version != 1
        || reply.device_id.is_nil()
        || reply.credential.len() != 64
        || !reply.credential.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Invalid pairing response".into());
    }
    let record = PairingRecord {
        version: 1,
        url: url.to_string(),
        certificate: input.certificate,
        device_id: reply.device_id,
        credential: reply.credential,
    };
    let directory = directory.to_path_buf();
    let device = record.device_id;
    tokio::task::spawn_blocking(move || {
        save(&record, &directory)?;
        Ok(record)
    })
    .await
    .map_err(|_| {
        format!("Pairing save interrupted. Revoke device {device} on Spark before retrying.")
    })?
    .map_err(|_: String| {
        format!("Pairing could not be saved. Revoke device {device} on Spark before retrying.")
    })
}
fn save(record: &PairingRecord, directory: &Path) -> Result<(), String> {
    use std::io::Write;
    let bytes = serde_json::to_vec(record).map_err(|_| "Credential encoding failed")?;
    let protected = avesra_windows::credentials::protect(&bytes)
        .map_err(|_| "Windows credential protection failed")?;
    let temporary = directory.join(format!("spark-pairing-{}.tmp", Uuid::new_v4()));
    let path = directory.join("spark-pairing.dpapi");
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "Unable to save protected pairing")?;
        file.write_all(&protected)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Unable to save protected pairing")?;
        drop(file);
        std::fs::hard_link(&temporary, &path).map_err(|_| "Unable to publish protected pairing")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}
pub fn load(directory: &Path) -> Result<PairingRecord, String> {
    let path = directory.join("spark-pairing.dpapi");
    let metadata = std::fs::metadata(&path).map_err(|_| "No saved pairing")?;
    if metadata.len() > 131_072 {
        return Err("Invalid saved pairing".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "Pairing unavailable")?;
    let bytes = avesra_windows::credentials::unprotect(&bytes)
        .map_err(|_| "Pairing cannot be unlocked by this Windows user")?;
    let record: PairingRecord =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid saved pairing")?;
    record.validate()?;
    Ok(record)
}

#[derive(Serialize)]
pub struct SavedPairing {
    pub file_revision: String,
    pub device_id: Option<Uuid>,
    pub readable: bool,
}
pub fn saved_pairing(directory: &Path) -> Result<Option<SavedPairing>, String> {
    use std::io::Read;
    let path = directory.join("spark-pairing.dpapi");
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Saved pairing cannot be read".into()),
    };
    let mut bytes = Vec::new();
    file.take(131_073)
        .read_to_end(&mut bytes)
        .map_err(|_| "Saved pairing cannot be read")?;
    if bytes.len() > 131_072 {
        return Err("Saved pairing exceeds size limit".into());
    }
    let record = load(directory).ok();
    Ok(Some(SavedPairing {
        file_revision: hex::encode(Sha256::digest(bytes)),
        device_id: record.as_ref().map(|value| value.device_id),
        readable: record.is_some(),
    }))
}
pub fn forget(directory: &Path, revision: &str) -> Result<(), String> {
    let current = saved_pairing(directory)?.ok_or("No saved pairing")?;
    if revision != current.file_revision {
        return Err("Saved pairing changed. Review it again before removal.".into());
    }
    std::fs::remove_file(directory.join("spark-pairing.dpapi"))
        .map_err(|_| "Unable to remove saved pairing".into())
}
pub async fn run(
    app: tauri::AppHandle,
    record: PairingRecord,
    mut modes: tokio::sync::watch::Receiver<ModeSnapshot>,
    generation: u64,
) -> Result<(), String> {
    let mut url = endpoint(&record.url)?;
    url.set_scheme("wss").map_err(|_| "Invalid websocket URL")?;
    url.set_path("/control");
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(certificate(&record.certificate)?)
        .map_err(|_| "Invalid trust root")?;
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| "Invalid websocket request")?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", record.credential)
            .parse()
            .map_err(|_| "Invalid credential")?,
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_CONTROL_BYTES))
        .max_frame_size(Some(MAX_CONTROL_BYTES))
        .write_buffer_size(1024)
        .max_write_buffer_size(MAX_CONTROL_BYTES * 2);
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            Some(config),
            false,
            Some(Connector::Rustls(Arc::new(tls))),
        ),
    )
    .await
    .map_err(|_| "Spark connection timed out")?
    .map_err(|_| "Authenticated Spark connection failed")?;
    let (mut writer, mut reader) = socket.split();
    let response = tokio::time::timeout(Duration::from_secs(10), reader.next())
        .await
        .map_err(|_| "Spark handshake timed out")?
        .ok_or("Spark disconnected")?
        .map_err(|_| "Spark handshake failed")?;
    let Message::Text(text) = response else {
        return Err("Invalid Spark handshake".into());
    };
    let hello: SessionReply = serde_json::from_str(&text).map_err(|_| "Invalid Spark handshake")?;
    if hello.version != PROTOCOL_VERSION
        || hello.device_id != record.device_id
        || hello.session_id.is_nil()
        || hello.status != "owner_setup_required"
    {
        return Err("Unexpected Spark session".into());
    }
    let mut sequence = 1u64;
    let mut mode = modes.borrow().clone();
    let make = |sequence, mode: &ModeSnapshot, message| Envelope {
        version: PROTOCOL_VERSION,
        device_id: record.device_id,
        session_id: hello.session_id,
        request_id: Uuid::new_v4(),
        sequence,
        capture_epoch: mode.capture_epoch,
        playback_epoch: mode.playback_epoch,
        action_epoch: mode.action_epoch,
        message,
    };
    let initial = make(
        sequence,
        &mode,
        ControlMessage::Hello {
            capabilities: vec![],
        },
    );
    let message = serde_json::to_string(&initial).map_err(|_| "Handshake encoding failed")?;
    let mut pending = std::collections::VecDeque::from([initial]);
    let mut response_sequence = 0u64;
    tokio::time::timeout(
        Duration::from_secs(3),
        writer.send(Message::Text(message.into())),
    )
    .await
    .map_err(|_| "Spark send timed out")?
    .map_err(|_| "Spark send failed")?;
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut last_reply = tokio::time::Instant::now();
    let _voice = crate::voice::spawn(app.clone(), record.clone(), generation);
    loop {
        let message = tokio::select! {
         changed=modes.changed()=>{changed.map_err(|_|"Companion shutting down")?;mode=modes.borrow_and_update().clone();ControlMessage::Mode{muted:mode.muted,deafened:mode.deafened,paused:mode.paused}},
         _=interval.tick()=>{if last_reply.elapsed()>Duration::from_secs(25){return Err("Spark heartbeat timed out".into());}ControlMessage::Mode{muted:mode.muted,deafened:mode.deafened,paused:mode.paused}},
         response=reader.next()=>{
          let response=response.ok_or("Spark disconnected")?.map_err(|_|"Spark connection lost")?;
          let Message::Text(text)=response else{return Err("Spark closed the session".into());};
          let reply:ServerStatus=serde_json::from_str(&text).map_err(|_|"Invalid Spark status")?;
          let request=pending.pop_front().ok_or("Unexpected Spark reply")?;
          if reply.version!=PROTOCOL_VERSION||reply.device_id!=record.device_id||reply.session_id!=hello.session_id||reply.sequence!=response_sequence.checked_add(1).ok_or("Response sequence exhausted")?||reply.request_id!=request.request_id||reply.request_sequence!=request.sequence||reply.capture_epoch!=request.capture_epoch||reply.playback_epoch!=request.playback_epoch||reply.action_epoch!=request.action_epoch||reply.status!=ConnectionStatus::ConnectedOwnerSetupRequired{return Err("Spark response failed session validation".into());}
          if let ControlMessage::Mode{muted,deafened,paused}=request.message && (reply.muted!=muted||reply.deafened!=deafened||reply.paused!=paused){return Err("Spark did not acknowledge local modes".into());}
          response_sequence=reply.sequence;last_reply=tokio::time::Instant::now();
          let state=app.state::<Runtime>();let mut local=state.local.lock().map_err(|_|"Local state unavailable")?;
          if state.connection_generation.load(std::sync::atomic::Ordering::SeqCst)!=generation{return Err("Session replaced".into());}
          if local.capture_epoch==reply.capture_epoch&&local.playback_epoch==reply.playback_epoch&&local.action_epoch==reply.action_epoch {
            if !local.connected {local.connected=true;local.refresh();state.publish(&local);let _=app.emit("runtime-state",local.clone());}
            if let Ok(mut session)=state.acknowledged_session.lock(){*session=Some(SessionIdentity{id:hello.session_id,epoch:reply.capture_epoch,playback_epoch:reply.playback_epoch,generation});}
          }continue;
         }
        };
        if pending.len() >= 8 {
            return Err("Spark control queue is full".into());
        }
        sequence = sequence.checked_add(1).ok_or("Sequence exhausted")?;
        let request = make(sequence, &mode, message);
        let encoded = serde_json::to_string(&request).map_err(|_| "Mode encoding failed")?;
        pending.push_back(request);
        tokio::time::timeout(
            Duration::from_secs(3),
            writer.send(Message::Text(encoded.into())),
        )
        .await
        .map_err(|_| "Spark send timed out")?
        .map_err(|_| "Spark send failed")?;
    }
}
