//! Read-only exact current-session retirement; never a recovered planner claim.
use super::*;
use avesra_contracts::planner::retirement;
pub(crate) async fn inspect(
    record: &PairingRecord,
    request: &retirement::Request,
    current: impl Fn() -> Result<(), String>,
) -> Result<retirement::Reply, String> {
    request
        .validate()
        .map_err(|_| "Invalid retirement request")?;
    if request.current.device != record.device_id {
        return Err("Retirement device changed".into());
    }
    current()?;
    let work = async {
        let body = serde_json::to_vec(request).map_err(|_| "Retirement request unavailable")?;
        if body.len() > retirement::MAX_BYTES {
            return Err("Retirement request too large".into());
        }
        let mut response = actor_client(record, 5)?
            .post(
                endpoint(&record.url)?
                    .join("planner/retirement")
                    .map_err(|_| "Retirement endpoint unavailable")?,
            )
            .bearer_auth(&record.credential)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| "Retirement inspection interrupted")?;
        if !response.status().is_success() {
            return Err("Current controller retirement is unavailable".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Retirement reply interrupted")?
        {
            if bytes.len() + chunk.len() > retirement::MAX_BYTES {
                return Err("Retirement reply too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let reply: retirement::Reply =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid retirement reply")?;
        reply
            .validate(request)
            .map_err(|_| "Retirement identity changed")?;
        Ok(reply)
    };
    tokio::pin!(work);
    let deadline = tokio::time::sleep(Duration::from_millis(request.remaining_ms));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            value=&mut work=>{current()?;return value;}
            _=&mut deadline=>return Err("Retirement inspection expired".into()),
            _=tokio::time::sleep(Duration::from_millis(50))=>current()?,
        }
    }
}
