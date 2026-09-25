use axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Serialize)]
struct Status {
    product: &'static str,
    version: &'static str,
    protocol: u16,
    paired_transport: &'static str,
    voice: &'static str,
    action_execution: &'static str,
}
async fn status() -> Json<Status> {
    Json(Status {
        product: "Avesra",
        version: env!("CARGO_PKG_VERSION"),
        protocol: avesra_contracts::PROTOCOL_VERSION,
        paired_transport: "unavailable",
        voice: "unavailable",
        action_execution: "disabled",
    })
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only a loopback health surface exists at this checkpoint. No action or media routes.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9473").await?;
    eprintln!(
        "Avesra loopback health: 127.0.0.1:9473; paired transport unavailable; actions disabled"
    );
    axum::serve(listener, Router::new().route("/health", get(status)))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
