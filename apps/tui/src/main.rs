#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use luna_protocol::PROTOCOL_VERSION;
use luna_tui::{
    api::{LunaApi, ServerOrigin},
    config::{ProfileStore, validate_profile_name},
    setup::pair_interactively,
};

#[derive(Debug, Parser)]
#[command(name = "luna-tui", version, about = "Luna terminal client")]
struct Cli {
    /// Select an independent paired device credential.
    #[arg(long, default_value = "default", value_parser = parse_profile)]
    profile: String,
    /// Private Luna origin used during first-run pairing.
    #[arg(long)]
    server: Option<String>,
    /// Device name recorded by Luna during first-run pairing.
    #[arg(long)]
    device_name: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Pair or replace this TUI profile.
    Pair {
        /// Replace an existing local profile after successful pairing.
        #[arg(long)]
        replace: bool,
    },
}

fn parse_profile(value: &str) -> Result<String, String> {
    validate_profile_name(value).map_err(|error| error.to_string())?;
    Ok(value.into())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("luna-tui: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let store = ProfileStore::discover()?;
    let profile = match cli.command {
        Some(Command::Pair { replace }) => {
            pair_interactively(
                &store,
                &cli.profile,
                cli.server.as_deref(),
                cli.device_name.as_deref(),
                replace,
            )
            .await?
        }
        None => match store.load(&cli.profile) {
            Ok(profile) => profile,
            Err(error) if error.is_not_found() => {
                pair_interactively(
                    &store,
                    &cli.profile,
                    cli.server.as_deref(),
                    cli.device_name.as_deref(),
                    false,
                )
                .await?
            }
            Err(error) => return Err(error.into()),
        },
    };

    let api = LunaApi::new(
        ServerOrigin::parse(&profile.server_url)?,
        Some(profile.token),
    )?;
    let bootstrap = api.bootstrap().await?;
    if bootstrap.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "protocol mismatch: server uses {}, but this client uses {}",
            bootstrap.protocol_version, PROTOCOL_VERSION
        )
        .into());
    }
    println!(
        "Connected to Luna as '{}' with {} active conversation(s).",
        bootstrap.device.name,
        bootstrap.conversations.len()
    );
    Ok(())
}
