use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use luna_protocol::{Device, DevicePlatform};
use luna_storage::{Database, NewDevice, NewPairingCode, StorageError};
use rand::Rng;
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Clone)]
pub struct AuthService {
    database: Database,
}

pub struct PairedDevice {
    pub device: Device,
    pub token: String,
}

pub struct PairingCode {
    pub value: String,
    pub expires_at: String,
}

impl AuthService {
    #[must_use]
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn create_pairing_code(&self) -> Result<PairingCode, AuthError> {
        let code = format!("{:06}", rand::rng().random_range(0..1_000_000));
        let created = OffsetDateTime::now_utc();
        let created_at = created.format(&Rfc3339)?;
        let expires_at = (created + Duration::minutes(15)).format(&Rfc3339)?;
        self.database
            .create_pairing_code(NewPairingCode {
                id: Uuid::new_v4(),
                code_hash: &hash(&code),
                created_at: &created_at,
                expires_at: &expires_at,
            })
            .await?;
        Ok(PairingCode {
            value: code,
            expires_at,
        })
    }

    pub async fn exchange(
        &self,
        code: &str,
        name: &str,
        platform: DevicePlatform,
    ) -> Result<Option<PairedDevice>, AuthError> {
        let bytes: [u8; 32] = rand::random();
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let created_at = now()?;
        let device = self
            .database
            .redeem_pairing_code(
                &hash(&code.trim().to_uppercase()),
                NewDevice {
                    id: Uuid::new_v4(),
                    name,
                    platform,
                    credential_hash: &hash(&token),
                    created_at: &created_at,
                },
            )
            .await?;
        Ok(device.map(|device| PairedDevice { device, token }))
    }

    pub async fn authenticate(&self, token: &str) -> Result<Option<Device>, AuthError> {
        Ok(self
            .database
            .authenticate_device(&hash(token), &now()?)
            .await?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Time(#[from] time::error::Format),
}

pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub fn now() -> Result<String, time::error::Format> {
    OffsetDateTime::now_utc().format(&Rfc3339)
}
