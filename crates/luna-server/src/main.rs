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
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    built.runtime.shutdown().await;
    built.maintenance.shutdown().await;
    built.database.close().await;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            signal::unix::signal(signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = signal::ctrl_c().await;
    }
}
