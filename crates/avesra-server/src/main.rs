mod auth;
mod transport;
use axum::{Json, Router, routing::get};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
#[derive(Serialize)]
struct Status {
    product: &'static str,
    version: &'static str,
    protocol: u16,
    voice: &'static str,
    action_execution: &'static str,
}
async fn status() -> Json<Status> {
    Json(Status {
        product: "Avesra",
        version: env!("CARGO_PKG_VERSION"),
        protocol: avesra_contracts::PROTOCOL_VERSION,
        voice: "unavailable",
        action_execution: "disabled",
    })
}
fn initialize(directory: &Path, dns: &str) -> Result<(), Box<dyn std::error::Error>> {
    if dns.is_empty()
        || dns.len() > 253
        || !dns
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.')
    {
        return Err("Invalid server DNS name".into());
    }
    std::fs::create_dir(directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec![dns.to_owned()])?;
    auth::private_write(&directory.join("server-cert.pem"), cert.pem().as_bytes())?;
    auth::private_write(
        &directory.join("server-key.pem"),
        signing_key.serialize_pem().as_bytes(),
    )?;
    let _ = auth::AuthStore::open(&directory.join("authentication.db"))?;
    println!(
        "Avesra server initialized. Certificate SHA-256: {}",
        hex::encode(Sha256::digest(cert.der()))
    );
    println!(
        "No listener started. Create a pairing code through the authenticated local administration session."
    );
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice(){
        #[cfg(unix)]
        [_,command,socket]if command=="audio-health"=>{
            let client=avesra_server::audio::AudioClient::new(Path::new(socket))?;
            println!("{}",serde_json::to_string(&client.health().await?)?);
        },
        [_,command,directory,dns]if command=="init"=>initialize(Path::new(directory),dns)?,
        [_,command,directory]if command=="pair-code"=>{
            let directory=Path::new(directory);if !directory.join("server-cert.pem").is_file(){return Err("Initialize the server first".into());}
            let mut store=auth::AuthStore::open(&directory.join("authentication.db"))?;let code=store.new_code()?;
            let file=directory.join(format!("pairing-code-{}.txt",uuid::Uuid::new_v4()));auth::private_write(&file,code.as_bytes())?;
            println!("Pairing code written to {}. Expires in five minutes. Read it locally; never add it to logs or source.",file.display());
        },
        [_,command,directory,device]if command=="revoke"=>{let mut store=auth::AuthStore::open(&Path::new(directory).join("authentication.db"))?;store.revoke(uuid::Uuid::parse_str(device)?)?;println!("Device revoked.");},
        [_,command,directory]if command=="serve"=>{
            let directory=Path::new(directory);let tls=axum_server::tls_rustls::RustlsConfig::from_pem_file(directory.join("server-cert.pem"),directory.join("server-key.pem")).await?;
            let auth=auth::AuthStore::open(&directory.join("authentication.db"))?;
            let app=transport::router(auth,directory)?.route("/health",get(status));
            eprintln!("Avesra TLS control listening on port 9474; owner setup required; actions disabled");
            axum_server::bind_rustls("0.0.0.0:9474".parse::<std::net::SocketAddr>()?,tls).serve(app.into_make_service()).await?;
        },
        [_]=>{let listener=tokio::net::TcpListener::bind("127.0.0.1:9473").await?;eprintln!("Avesra loopback health on 9473; use serve for configured TLS transport");axum::serve(listener,Router::new().route("/health",get(status))).with_graceful_shutdown(async{let _=tokio::signal::ctrl_c().await;}).await?;},
        _=>return Err("Usage: avesra-server [init <new-private-directory> <dns-name> | pair-code <directory> | serve <directory> | revoke <directory> <device-uuid>]".into())
    }
    Ok(())
}
