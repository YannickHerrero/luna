use std::{
    env, fs,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct Config {
    pub bind_host: String,
    pub port: u16,
    pub public_origin: Option<String>,
    pub allowed_tailnet_logins: Vec<String>,
    pub data_directory: PathBuf,
    pub credentials_directory: PathBuf,
    pub database_path: PathBuf,
    pub pi_session_directory: PathBuf,
    pub attachment_directory: PathBuf,
    pub repository_icon_directory: PathBuf,
    pub bridge_directory: PathBuf,
    pub web_directory: PathBuf,
    pub pi_executable: PathBuf,
    pub pi_bridge_path: PathBuf,
    pub title_model: String,
    pub event_retention_days: u32,
    pub attachment_retention_days: u32,
    pub transcription_model: String,
    pub transcription_api_key: Option<String>,
    pub transcription_base_url: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalConfig {
    bind_host: Option<String>,
    port: Option<u16>,
    public_origin: Option<String>,
    data_directory: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("home directory is unavailable")]
    MissingHome,
    #[error("configuration file is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("configuration file could not be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("environment file must be mode 600: {0}")]
    InsecureEnvironmentFile(PathBuf),
    #[error("invalid integer in {0}")]
    InvalidInteger(String),
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let base = BaseDirs::new().ok_or(ConfigError::MissingHome)?;
        let credentials_directory =
            env_path("LUNA_CREDENTIALS_DIR", base.home_dir().join(".config/luna"));
        let environment_file = env_path("LUNA_ENV_FILE", credentials_directory.join("server.env"));
        load_environment_file(&environment_file)?;

        let local_path = env_path(
            "LUNA_LOCAL_CONFIG",
            env::current_dir()?.join(".luna.local.json"),
        );
        let local = if local_path.exists() {
            serde_json::from_slice(&fs::read(local_path)?)?
        } else {
            LocalConfig::default()
        };
        let default_data = if cfg!(target_os = "macos") {
            base.home_dir()
                .join("Library/Application Support/Luna Server")
        } else {
            base.data_dir().join("luna")
        };
        let data_directory = env::var("LUNA_DATA_DIR")
            .ok()
            .or(local.data_directory)
            .map(|value| expand_home(&value))
            .transpose()?
            .unwrap_or(default_data);
        let root = env::current_dir()?;
        let data_fingerprint = format!(
            "{:x}",
            Sha256::digest(data_directory.to_string_lossy().as_bytes())
        );
        let bridge_directory = env_path(
            "LUNA_BRIDGE_DIR",
            PathBuf::from("/tmp").join(format!("luna-{}", &data_fingerprint[..12])),
        );
        let port = parse_u16("LUNA_PORT", local.port.unwrap_or(9870))?;
        let retention = parse_u32("LUNA_EVENT_RETENTION_DAYS", 30)?;
        let attachment_retention = parse_u32("LUNA_ATTACHMENT_RETENTION_DAYS", 30)?;
        let public_origin = env::var("LUNA_PUBLIC_ORIGIN").ok().or(local.public_origin);

        Ok(Self {
            bind_host: env::var("LUNA_BIND_HOST")
                .ok()
                .or(local.bind_host)
                .unwrap_or_else(|| "127.0.0.1".into()),
            port,
            public_origin,
            allowed_tailnet_logins: env::var("LUNA_ALLOWED_TAILNET_LOGINS")
                .unwrap_or_default()
                .split(',')
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty())
                .collect(),
            database_path: data_directory.join("luna.sqlite"),
            pi_session_directory: data_directory.join("pi-sessions"),
            attachment_directory: data_directory.join("attachments"),
            repository_icon_directory: data_directory.join("repository-icons"),
            bridge_directory,
            web_directory: env_path("LUNA_WEB_DIR", root.join("apps/web/out")),
            data_directory,
            credentials_directory,
            pi_executable: env_path(
                "LUNA_PI_EXECUTABLE",
                root.join("packages/pi-runtime/node_modules/.bin/pi"),
            ),
            pi_bridge_path: env_path(
                "LUNA_PI_BRIDGE",
                root.join("integrations/pi/luna-bridge.ts"),
            ),
            title_model: env::var("LUNA_TITLE_MODEL")
                .unwrap_or_else(|_| "openai-codex/gpt-5.6-luna".into()),
            event_retention_days: retention,
            attachment_retention_days: attachment_retention,
            transcription_model: env::var("LUNA_TRANSCRIPTION_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini-transcribe".into()),
            transcription_api_key: env::var("LUNA_TRANSCRIPTION_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    env::var("OPENAI_API_KEY")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
            transcription_base_url: env::var("LUNA_TRANSCRIPTION_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
        })
    }
}

fn env_path(name: &str, fallback: PathBuf) -> PathBuf {
    env::var(name).ok().map_or(fallback, |value| {
        expand_home(&value).unwrap_or_else(|_| PathBuf::from(value))
    })
}

fn expand_home(value: &str) -> Result<PathBuf, ConfigError> {
    if value == "~" || value.starts_with("~/") {
        let base = BaseDirs::new().ok_or(ConfigError::MissingHome)?;
        return Ok(base.home_dir().join(value.trim_start_matches("~/")));
    }
    Ok(PathBuf::from(value))
}

fn parse_u16(name: &str, fallback: u16) -> Result<u16, ConfigError> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| ConfigError::InvalidInteger(name.into())),
        Err(_) => Ok(fallback),
    }
}

fn parse_u32(name: &str, fallback: u32) -> Result<u32, ConfigError> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| ConfigError::InvalidInteger(name.into())),
        Err(_) => Ok(fallback),
    }
}

fn load_environment_file(path: &Path) -> Result<(), ConfigError> {
    if !path.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)?.permissions().mode() & 0o077 != 0 {
            return Err(ConfigError::InsecureEnvironmentFile(path.into()));
        }
    }
    dotenvy::from_path(path).map_err(|error| {
        ConfigError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })?;
    Ok(())
}
