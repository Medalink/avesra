//! One controller-incarnation owner for selected model startup, never inference.
use avesra_contracts::ErrorCode;
use avesra_server::audio::AudioClient;
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

pub(super) fn start(lanes: [(&'static str, Option<Arc<AudioClient>>); 4]) {
    // This task is retained by the runtime independently of any HTTP request.
    tokio::spawn(async move {
        let availability = Instant::now() + Duration::from_secs(60);
        for (lane, client) in lanes {
            let Some(client) = client else { continue };
            loop {
                match client.startup_health().await {
                    Ok(health) if health.state == "loaded_unqualified" => {
                        eprintln!("Audio startup {lane}: already loaded (unqualified)");
                        break;
                    }
                    Ok(health) if health.state == "unavailable" && !health.busy => {
                        let result = client.load().await;
                        if client.load_quarantined() {
                            eprintln!(
                                "Audio startup {lane}: local load-retirement quarantine; coordinator stopped"
                            );
                            return;
                        }
                        if result.is_err() && !client.load_attempted() {
                            eprintln!(
                                "Audio startup {lane}: preparation changed; coordinator stopped"
                            );
                            return;
                        }
                        match result {
                            Ok(()) => eprintln!("Audio startup {lane}: loaded (unqualified)"),
                            Err(error) => {
                                eprintln!("Audio startup {lane}: load not completed ({error})")
                            }
                        }
                        break;
                    }
                    Ok(_) if Instant::now() >= availability => {
                        eprintln!(
                            "Audio startup {lane}: existing work still unsettled; coordinator stopped"
                        );
                        return;
                    }
                    Ok(_) => {}
                    Err(ErrorCode::Unavailable | ErrorCode::Expired)
                        if Instant::now() < availability => {}
                    Err(error) => {
                        eprintln!("Audio startup {lane}: health unavailable ({error})");
                        break;
                    }
                }
                tokio::time::sleep_until(
                    (Instant::now() + Duration::from_secs(1)).min(availability),
                )
                .await;
            }
        }
    });
}
