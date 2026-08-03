use std::io::{self, Write};

use luna_protocol::PROTOCOL_VERSION;

use crate::{
    api::{LunaApi, ServerOrigin},
    config::{ClientProfile, ProfileStore},
};

pub async fn pair_interactively(
    store: &ProfileStore,
    profile_name: &str,
    server: Option<&str>,
    device_name: Option<&str>,
    replace: bool,
) -> Result<ClientProfile, SetupError> {
    let server = match server {
        Some(server) => server.trim().to_owned(),
        None => prompt("Luna server URL: ")?,
    };
    let origin = ServerOrigin::parse(&server)?;
    let device_name = device_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Luna TUI ({profile_name})"));
    if device_name.len() > 80 {
        return Err(SetupError::InvalidDeviceName);
    }

    let api = LunaApi::new(origin.clone(), None)?;
    let pairing = api.request_pairing_code().await?;
    println!(
        "A fresh pairing code was requested and expires at {}.\nRead the newest code from the Luna/Citadel server log.",
        pairing.expires_at
    );
    let code = prompt("Pairing code: ")?;
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SetupError::InvalidPairingCode);
    }
    let paired = api.exchange_pairing_code(&code, &device_name).await?;
    if paired.bootstrap.protocol_version != PROTOCOL_VERSION {
        return Err(SetupError::ProtocolMismatch {
            server: paired.bootstrap.protocol_version,
            client: PROTOCOL_VERSION,
        });
    }
    let profile = ClientProfile {
        server_url: origin.to_string(),
        device_id: paired.device_id,
        token: paired.token,
    };
    let path = store.save(profile_name, &profile, replace)?;
    println!(
        "Paired '{}' with {}. Credential saved securely at {}.",
        profile_name,
        origin,
        path.display()
    );
    Ok(profile)
}

fn prompt(label: &str) -> Result<String, io::Error> {
    print!("{label}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error(transparent)]
    Api(#[from] crate::api::ApiClientError),
    #[error(transparent)]
    InvalidServer(#[from] crate::api::ServerOriginError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("device names must contain between 1 and 80 characters")]
    InvalidDeviceName,
    #[error("pairing codes contain exactly six digits")]
    InvalidPairingCode,
    #[error("protocol mismatch: server uses {server}, but this client uses {client}")]
    ProtocolMismatch { server: u8, client: u8 },
    #[error("pairing input could not be read: {0}")]
    Io(#[from] io::Error),
}
