#![forbid(unsafe_code)]

use std::net::SocketAddr;

use luna_server::{app, config::Config};
use tokio::signal;
use tracing::warn;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "luna_server=info".into()),
        )
        .init();
    let config = Config::load()?;
    let address: SocketAddr = format!("{}:{}", config.bind_host, config.port).parse()?;
    let built = app::build(config).await?;
    warn!(pairing_code = %built.pairing_code, "Luna pairing code");
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, built.router)
        .with_graceful_shutdown(async {
            let _ = signal::ctrl_c().await;
        })
        .await?;
    built.runtime.shutdown().await;
    built.database.close().await;
    Ok(())
}
