use luna_protocol::{Device, DevicePlatform};
use sqlx::{FromRow, Row};
use uuid::Uuid;

use crate::{Database, StorageError};

pub struct NewPairingCode<'a> {
    pub id: Uuid,
    pub code_hash: &'a str,
    pub created_at: &'a str,
    pub expires_at: &'a str,
}

pub struct NewDevice<'a> {
    pub id: Uuid,
    pub name: &'a str,
    pub platform: DevicePlatform,
    pub credential_hash: &'a str,
    pub created_at: &'a str,
}

#[derive(FromRow)]
struct DeviceRow {
    id: String,
    name: String,
    platform: String,
    notifications_enabled: bool,
    created_at: String,
    last_seen_at: String,
}

fn platform_name(platform: DevicePlatform) -> &'static str {
    match platform {
        DevicePlatform::Ios => "ios",
        DevicePlatform::Ipados => "ipados",
        DevicePlatform::Tui => "tui",
        DevicePlatform::Web => "web",
    }
}

fn parse_platform(value: &str) -> Result<DevicePlatform, StorageError> {
    match value {
        "ios" => Ok(DevicePlatform::Ios),
        "ipados" => Ok(DevicePlatform::Ipados),
        "tui" => Ok(DevicePlatform::Tui),
        "web" => Ok(DevicePlatform::Web),
        value => Err(StorageError::InvalidEnum(value.into())),
    }
}

impl TryFrom<DeviceRow> for Device {
    type Error = StorageError;

    fn try_from(row: DeviceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&row.id)?,
            name: row.name,
            platform: parse_platform(&row.platform)?,
            notifications_enabled: row.notifications_enabled,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
        })
    }
}

impl Database {
    pub async fn create_pairing_code(&self, code: NewPairingCode<'_>) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "UPDATE pairing_codes SET expires_at = ? WHERE redeemed_at IS NULL AND expires_at > ?",
        )
        .bind(code.created_at)
        .bind(code.created_at)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO pairing_codes (id, code_hash, created_at, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(code.id.to_string())
        .bind(code.code_hash)
        .bind(code.created_at)
        .bind(code.expires_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn redeem_pairing_code(
        &self,
        code_hash: &str,
        device: NewDevice<'_>,
    ) -> Result<Option<Device>, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let pairing = sqlx::query(
            "SELECT id FROM pairing_codes WHERE code_hash = ? AND redeemed_at IS NULL AND expires_at > ?",
        )
        .bind(code_hash)
        .bind(device.created_at)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(pairing) = pairing else {
            transaction.rollback().await?;
            return Ok(None);
        };
        let pairing_id: String = pairing.get("id");
        let claimed = sqlx::query(
            "UPDATE pairing_codes SET redeemed_at = ? WHERE id = ? AND redeemed_at IS NULL",
        )
        .bind(device.created_at)
        .bind(pairing_id)
        .execute(&mut *transaction)
        .await?;
        if claimed.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO devices (id, name, platform, credential_hash, notifications_enabled, created_at, last_seen_at) VALUES (?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(device.id.to_string())
        .bind(device.name)
        .bind(platform_name(device.platform))
        .bind(device.credential_hash)
        .bind(device.created_at)
        .bind(device.created_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(Some(Device {
            id: device.id,
            name: device.name.into(),
            platform: device.platform,
            notifications_enabled: false,
            created_at: device.created_at.into(),
            last_seen_at: device.created_at.into(),
        }))
    }

    pub async fn authenticate_device(
        &self,
        credential_hash: &str,
        last_seen_at: &str,
    ) -> Result<Option<Device>, StorageError> {
        let row = sqlx::query_as::<_, DeviceRow>(
            "SELECT id, name, platform, notifications_enabled, created_at, last_seen_at FROM devices WHERE credential_hash = ? AND revoked_at IS NULL",
        )
        .bind(credential_hash)
        .fetch_optional(self.pool())
        .await?;
        let Some(mut row) = row else {
            return Ok(None);
        };
        sqlx::query("UPDATE devices SET last_seen_at = ? WHERE id = ?")
            .bind(last_seen_at)
            .bind(&row.id)
            .execute(self.pool())
            .await?;
        row.last_seen_at = last_seen_at.into();
        Ok(Some(row.try_into()?))
    }
}
