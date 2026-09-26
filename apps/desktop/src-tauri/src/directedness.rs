//! Native-only transient observations. No arbitrary-transcript IPC or acceptance.
use super::{PairingRecord, SessionIdentity, actor_client, bounded_response_with_limit, endpoint};
use avesra_contracts::directedness::{self, Category, Operation};
use avesra_core::voice::Context;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub adapter_revision: String,
    pub artifact_revision: String,
    pub engine_incarnation: String,
    pub quality_fingerprint: Option<String>,
}
pub struct Observation {
    pub binding: Binding,
    pub category: Category,
}
pub async fn metadata(
    pairing: &PairingRecord,
    session: SessionIdentity,
    context: &Context,
    current: impl Fn() -> Result<(), String>,
) -> Result<Binding, String> {
    let reply = operation(pairing, session, context, Operation::Inspect, current).await?;
    Ok(binding(reply))
}
pub async fn classify(
    pairing: &PairingRecord,
    session: SessionIdentity,
    utterance: Uuid,
    context: &Context,
    transcript: &str,
    current: impl Fn() -> Result<(), String>,
) -> Result<Observation, String> {
    let reply = operation(
        pairing,
        session,
        context,
        Operation::Classify {
            utterance,
            transcript: transcript.into(),
        },
        current,
    )
    .await?;
    let category = reply
        .category
        .ok_or("Directedness classification missing")?;
    Ok(Observation {
        binding: binding(reply),
        category,
    })
}
fn binding(reply: directedness::Reply) -> Binding {
    Binding {
        adapter_revision: reply.adapter_revision,
        artifact_revision: reply.artifact_revision,
        engine_incarnation: reply.engine_incarnation,
        quality_fingerprint: reply.quality_fingerprint,
    }
}
async fn operation(
    pairing: &PairingRecord,
    session: SessionIdentity,
    context: &Context,
    operation: Operation,
    current: impl Fn() -> Result<(), String>,
) -> Result<directedness::Reply, String> {
    let started = Instant::now();
    current()?;
    if pairing.device_id != context.device
        || session.device != context.device
        || session.id != context.session
        || session.epoch != context.capture_epoch
        || session.action_epoch != context.action_epoch
        || pairing.server_fingerprint()? != hex::encode(session.server_fingerprint)
    {
        return Err("Directedness source context changed".into());
    }
    let mut request = directedness::Request {
        version: directedness::VERSION,
        request: Uuid::new_v4(),
        context: directedness::Context {
            device: context.device,
            session: context.session,
            capture_epoch: context.capture_epoch,
            action_epoch: context.action_epoch,
            microphone: context.microphone.clone(),
            actor: context.actor,
            grant_revision: context.grant_revision,
        },
        remaining_ms: 30_000,
        operation,
    };
    request
        .validate()
        .map_err(|_| "Invalid directedness observation")?;
    let client = actor_client(pairing, 30)?;
    let url = endpoint(&pairing.url)?
        .join("voice-directedness")
        .map_err(|_| "Invalid directedness endpoint")?;
    current()?;
    request.remaining_ms = u64::try_from(
        Duration::from_secs(30)
            .saturating_sub(started.elapsed())
            .as_millis(),
    )
    .map_err(|_| "Directedness budget expired")?;
    request
        .validate()
        .map_err(|_| "Directedness budget expired")?;
    let bytes = serde_json::to_vec(&request).map_err(|_| "Directedness encoding failed")?;
    if bytes.len() > 32_768 {
        return Err("Directedness request exceeds limit".into());
    }
    let query = async {
        let response = client
            .post(url)
            .bearer_auth(&pairing.credential)
            .header("Content-Type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|_| "Directedness service unavailable")?;
        let reply: directedness::Reply =
            serde_json::from_value(bounded_response_with_limit(response, 8192).await?)
                .map_err(|_| "Malformed directedness response")?;
        reply
            .validate(&request)
            .map_err(|_| "Directedness response context changed")?;
        let expected = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}\n{}\n{}\n{}",
                    directedness::POLICY,
                    reply.artifact_revision,
                    reply.engine_incarnation,
                    reply
                        .quality_fingerprint
                        .as_deref()
                        .unwrap_or("unavailable")
                )
                .as_bytes()
            )
        );
        if reply.adapter_revision != expected {
            return Err("Directedness policy binding changed".into());
        }
        Ok(reply)
    };
    tokio::pin!(query);
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {biased;
            _=tokio::time::sleep_until((started+Duration::from_secs(30)).into())=>return Err("Directedness observation expired".into()),
            _=tick.tick()=>current()?,
            result=&mut query=>{current()?;return result;}
        }
    }
}
