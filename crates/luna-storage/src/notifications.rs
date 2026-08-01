use luna_protocol::ApnsEnvironment;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, StorageError};

pub struct NewApnsRegistration<'a> {
    pub device_id: Uuid,
    pub token: &'a str,
    pub environment: ApnsEnvironment,
    pub topic: &'a str,
    pub app_version: Option<&'a str>,
    pub registered_at: &'a str,
}

pub struct ApnsRegistration {
    pub id: Uuid,
    pub device_id: Uuid,
    pub token: String,
    pub environment: ApnsEnvironment,
    pub topic: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCycleOutcome {
    Ready,
    Attention,
    Interrupted,
}

pub struct PendingNotificationDelivery {
    pub delivery_id: Uuid,
    pub cycle_id: Uuid,
    pub conversation_id: Uuid,
    pub target_device_id: Uuid,
    pub message_id: Option<Uuid>,
    pub registration: ApnsRegistration,
    pub conversation_title: String,
    pub outcome: AgentCycleOutcome,
    pub attempts: u32,
}

#[derive(FromRow)]
struct RegistrationRow {
    id: String,
    device_id: String,
    token: String,
    environment: String,
    bundle_id: String,
}

#[derive(FromRow)]
struct PendingDeliveryRow {
    delivery_id: String,
    cycle_id: String,
    conversation_id: String,
    target_device_id: String,
    message_id: Option<String>,
    registration_id: String,
    token: String,
    environment: String,
    bundle_id: String,
    conversation_title: String,
    cycle_state: String,
    attempts: i64,
}

impl Database {
    pub async fn upsert_apns_registration(
        &self,
        registration: NewApnsRegistration<'_>,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        let platform: Option<String> =
            sqlx::query_scalar("SELECT platform FROM devices WHERE id = ? AND revoked_at IS NULL")
                .bind(registration.device_id.to_string())
                .fetch_optional(&mut *transaction)
                .await?;
        if !matches!(platform.as_deref(), Some("ios" | "ipados")) {
            return Err(StorageError::Conflict);
        }
        sqlx::query(
            "DELETE FROM apns_registrations WHERE token = ? AND NOT (device_id = ? AND environment = ? AND bundle_id = ?)",
        )
        .bind(registration.token)
        .bind(registration.device_id.to_string())
        .bind(environment_name(registration.environment))
        .bind(registration.topic)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE apns_registrations SET invalidated_at = ? WHERE device_id = ? AND invalidated_at IS NULL AND NOT (environment = ? AND bundle_id = ?)",
        )
        .bind(registration.registered_at)
        .bind(registration.device_id.to_string())
        .bind(environment_name(registration.environment))
        .bind(registration.topic)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO apns_registrations (id, device_id, token, environment, bundle_id, updated_at, invalidated_at) VALUES (?, ?, ?, ?, ?, ?, NULL) ON CONFLICT(device_id, environment, bundle_id) DO UPDATE SET token = excluded.token, updated_at = excluded.updated_at, invalidated_at = NULL",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(registration.device_id.to_string())
        .bind(registration.token)
        .bind(environment_name(registration.environment))
        .bind(registration.topic)
        .bind(registration.registered_at)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE devices SET notifications_enabled = 1, app_version = COALESCE(?, app_version) WHERE id = ?",
        )
        .bind(registration.app_version)
        .bind(registration.device_id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn disable_apns_for_device(
        &self,
        device_id: Uuid,
        disabled_at: &str,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "UPDATE apns_registrations SET invalidated_at = COALESCE(invalidated_at, ?) WHERE device_id = ?",
        )
        .bind(disabled_at)
        .bind(device_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("UPDATE devices SET notifications_enabled = 0 WHERE id = ?")
            .bind(device_id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn invalidate_apns_registration(
        &self,
        registration_id: Uuid,
        invalidated_at: &str,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        let device_id: Option<String> =
            sqlx::query_scalar("SELECT device_id FROM apns_registrations WHERE id = ?")
                .bind(registration_id.to_string())
                .fetch_optional(&mut *transaction)
                .await?;
        let Some(device_id) = device_id else {
            transaction.rollback().await?;
            return Ok(());
        };
        sqlx::query(
            "UPDATE apns_registrations SET invalidated_at = COALESCE(invalidated_at, ?) WHERE id = ?",
        )
        .bind(invalidated_at)
        .bind(registration_id.to_string())
        .execute(&mut *transaction)
        .await?;
        let has_active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM apns_registrations WHERE device_id = ? AND invalidated_at IS NULL)",
        )
        .bind(&device_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !has_active {
            sqlx::query("UPDATE devices SET notifications_enabled = 0 WHERE id = ?")
                .bind(device_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn complete_active_agent_cycle(
        &self,
        conversation_id: Uuid,
        message_id: Option<Uuid>,
        outcome: AgentCycleOutcome,
        completed_at: &str,
    ) -> Result<Option<PendingNotificationDelivery>, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let cycle: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT id, target_device_id FROM agent_cycles WHERE conversation_id = ? AND state = 'active'",
        )
        .bind(conversation_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((cycle_id, target_device_id)) = cycle else {
            transaction.rollback().await?;
            return Ok(None);
        };
        sqlx::query(
            "UPDATE agent_cycles SET state = ?, updated_at = ?, completed_at = ? WHERE id = ? AND state = 'active'",
        )
        .bind(outcome_name(outcome))
        .bind(completed_at)
        .bind(completed_at)
        .bind(&cycle_id)
        .execute(&mut *transaction)
        .await?;
        if outcome == AgentCycleOutcome::Interrupted {
            transaction.commit().await?;
            return Ok(None);
        }
        let Some(target_device_id) = target_device_id else {
            transaction.commit().await?;
            return Ok(None);
        };
        let registration = sqlx::query_as::<_, RegistrationRow>(
            "SELECT r.id, r.device_id, r.token, r.environment, r.bundle_id FROM apns_registrations r JOIN devices d ON d.id = r.device_id WHERE r.device_id = ? AND r.invalidated_at IS NULL AND d.notifications_enabled = 1 AND d.revoked_at IS NULL ORDER BY r.updated_at DESC LIMIT 1",
        )
        .bind(&target_device_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(registration) = registration else {
            transaction.commit().await?;
            return Ok(None);
        };
        let delivery_id = Uuid::now_v7();
        let inserted = sqlx::query(
            "INSERT INTO notification_deliveries (id, conversation_id, message_id, target_device_id, channel, status, response_code, created_at, completed_at, cycle_id, attempts) VALUES (?, ?, ?, ?, 'apns', 'pending', NULL, ?, NULL, ?, 0) ON CONFLICT(cycle_id, channel) DO NOTHING",
        )
        .bind(delivery_id.to_string())
        .bind(conversation_id.to_string())
        .bind(message_id.map(|id| id.to_string()))
        .bind(&target_device_id)
        .bind(completed_at)
        .bind(&cycle_id)
        .execute(&mut *transaction)
        .await?;
        if inserted.rows_affected() == 0 {
            transaction.commit().await?;
            return Ok(None);
        }
        let title: String = sqlx::query_scalar("SELECT title FROM conversations WHERE id = ?")
            .bind(conversation_id.to_string())
            .fetch_one(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(PendingNotificationDelivery {
            delivery_id,
            cycle_id: Uuid::parse_str(&cycle_id)?,
            conversation_id,
            target_device_id: Uuid::parse_str(&target_device_id)?,
            message_id,
            registration: map_registration(registration)?,
            conversation_title: title,
            outcome,
            attempts: 0,
        }))
    }

    pub async fn pending_notification_deliveries(
        &self,
        recovered_at: &str,
    ) -> Result<Vec<PendingNotificationDelivery>, StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "UPDATE notification_deliveries SET status = 'failed', response_code = 'registration_unavailable', completed_at = ? WHERE channel = 'apns' AND status IN ('pending', 'sending') AND NOT EXISTS (SELECT 1 FROM apns_registrations r JOIN devices d ON d.id = r.device_id WHERE r.device_id = notification_deliveries.target_device_id AND r.invalidated_at IS NULL AND d.notifications_enabled = 1 AND d.revoked_at IS NULL)",
        )
        .bind(recovered_at)
        .execute(&mut *transaction)
        .await?;
        let rows = sqlx::query_as::<_, PendingDeliveryRow>(
            "SELECT d.id AS delivery_id, d.cycle_id, d.conversation_id, d.target_device_id, d.message_id, r.id AS registration_id, r.token, r.environment, r.bundle_id, c.title AS conversation_title, a.state AS cycle_state, d.attempts FROM notification_deliveries d JOIN agent_cycles a ON a.id = d.cycle_id JOIN conversations c ON c.id = d.conversation_id JOIN apns_registrations r ON r.device_id = d.target_device_id AND r.invalidated_at IS NULL JOIN devices device ON device.id = r.device_id AND device.notifications_enabled = 1 AND device.revoked_at IS NULL WHERE d.channel = 'apns' AND d.status IN ('pending', 'sending') ORDER BY d.created_at, r.updated_at DESC",
        )
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        rows.into_iter().map(map_pending_delivery).collect()
    }

    pub async fn mark_notification_attempt(&self, delivery_id: Uuid) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE notification_deliveries SET status = 'sending', attempts = attempts + 1 WHERE id = ? AND status IN ('pending', 'sending')",
        )
        .bind(delivery_id.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn finish_notification_delivery(
        &self,
        delivery_id: Uuid,
        status: &str,
        response_code: Option<&str>,
        completed_at: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE notification_deliveries SET status = ?, response_code = ?, completed_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(response_code)
        .bind(completed_at)
        .bind(delivery_id.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn interrupt_active_agent_cycle(
        &self,
        conversation_id: Uuid,
        interrupted_at: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE agent_cycles SET state = 'interrupted', updated_at = ?, completed_at = ? WHERE conversation_id = ? AND state = 'active'",
        )
        .bind(interrupted_at)
        .bind(interrupted_at)
        .bind(conversation_id.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn orphaned_active_agent_cycle_conversations(
        &self,
    ) -> Result<Vec<Uuid>, StorageError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT a.conversation_id FROM agent_cycles a WHERE a.state = 'active' AND NOT EXISTS (SELECT 1 FROM dispatches d WHERE d.cycle_id = a.id AND d.state IN ('queued', 'pending', 'running')) ORDER BY a.started_at",
        )
        .fetch_all(self.pool())
        .await?;
        ids.into_iter()
            .map(|id| Uuid::parse_str(&id).map_err(StorageError::from))
            .collect()
    }
}

fn environment_name(environment: ApnsEnvironment) -> &'static str {
    match environment {
        ApnsEnvironment::Sandbox => "sandbox",
        ApnsEnvironment::Production => "production",
    }
}

fn parse_environment(value: &str) -> Result<ApnsEnvironment, StorageError> {
    match value {
        "sandbox" => Ok(ApnsEnvironment::Sandbox),
        "production" => Ok(ApnsEnvironment::Production),
        value => Err(StorageError::InvalidEnum(value.into())),
    }
}

fn outcome_name(outcome: AgentCycleOutcome) -> &'static str {
    match outcome {
        AgentCycleOutcome::Ready => "completed",
        AgentCycleOutcome::Attention => "failed",
        AgentCycleOutcome::Interrupted => "interrupted",
    }
}

fn map_registration(row: RegistrationRow) -> Result<ApnsRegistration, StorageError> {
    Ok(ApnsRegistration {
        id: Uuid::parse_str(&row.id)?,
        device_id: Uuid::parse_str(&row.device_id)?,
        token: row.token,
        environment: parse_environment(&row.environment)?,
        topic: row.bundle_id,
    })
}

fn map_pending_delivery(
    row: PendingDeliveryRow,
) -> Result<PendingNotificationDelivery, StorageError> {
    let outcome = match row.cycle_state.as_str() {
        "completed" => AgentCycleOutcome::Ready,
        "failed" => AgentCycleOutcome::Attention,
        value => return Err(StorageError::InvalidEnum(value.into())),
    };
    Ok(PendingNotificationDelivery {
        delivery_id: Uuid::parse_str(&row.delivery_id)?,
        cycle_id: Uuid::parse_str(&row.cycle_id)?,
        conversation_id: Uuid::parse_str(&row.conversation_id)?,
        target_device_id: Uuid::parse_str(&row.target_device_id)?,
        message_id: row
            .message_id
            .map(|value| Uuid::parse_str(&value))
            .transpose()?,
        registration: ApnsRegistration {
            id: Uuid::parse_str(&row.registration_id)?,
            device_id: Uuid::parse_str(&row.target_device_id)?,
            token: row.token,
            environment: parse_environment(&row.environment)?,
            topic: row.bundle_id,
        },
        conversation_title: row.conversation_title,
        outcome,
        attempts: u32::try_from(row.attempts).map_err(|_| StorageError::Conflict)?,
    })
}
