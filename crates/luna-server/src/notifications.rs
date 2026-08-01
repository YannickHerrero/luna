use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use luna_protocol::ApnsEnvironment;
use luna_storage::{AgentCycleOutcome, Database, PendingNotificationDelivery, StorageError};
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, time::Instant};
use uuid::Uuid;

use crate::auth::now;

const MAX_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct NotificationService {
    database: Database,
    provider: Arc<dyn NotificationProvider>,
    retry_delays: Arc<Vec<Duration>>,
}

impl NotificationService {
    pub fn from_apns_config(
        database: Database,
        key_path: Option<PathBuf>,
        key_id: Option<String>,
        team_id: Option<String>,
    ) -> Result<Self, NotificationConfigurationError> {
        let provider: Arc<dyn NotificationProvider> = match (key_path, key_id, team_id) {
            (Some(key_path), Some(key_id), Some(team_id)) => {
                Arc::new(ApnsHttpProvider::new(key_path, key_id, team_id)?)
            }
            (None, None, None) => Arc::new(UnavailableNotificationProvider),
            _ => return Err(NotificationConfigurationError::Incomplete),
        };
        Ok(Self::new(database, provider))
    }

    #[must_use]
    pub fn new(database: Database, provider: Arc<dyn NotificationProvider>) -> Self {
        Self {
            database,
            provider,
            retry_delays: Arc::new(vec![Duration::from_millis(250), Duration::from_secs(1)]),
        }
    }

    #[cfg(test)]
    fn with_retry_delays(mut self, retry_delays: Vec<Duration>) -> Self {
        self.retry_delays = Arc::new(retry_delays);
        self
    }

    pub async fn recover_pending(&self) -> Result<(), StorageError> {
        for delivery in self
            .database
            .pending_notification_deliveries(&timestamp())
            .await?
        {
            let service = self.clone();
            tokio::spawn(async move {
                service.deliver(delivery).await;
            });
        }
        Ok(())
    }

    pub async fn complete_cycle(
        &self,
        conversation_id: Uuid,
        message_id: Option<Uuid>,
        outcome: AgentCycleOutcome,
    ) -> Result<(), StorageError> {
        let Some(delivery) = self
            .database
            .complete_active_agent_cycle(conversation_id, message_id, outcome, &timestamp())
            .await?
        else {
            return Ok(());
        };
        let service = self.clone();
        tokio::spawn(async move {
            service.deliver(delivery).await;
        });
        Ok(())
    }

    async fn deliver(&self, delivery: PendingNotificationDelivery) {
        if delivery.attempts as usize >= MAX_ATTEMPTS {
            let _ = self
                .database
                .finish_notification_delivery(
                    delivery.delivery_id,
                    "failed",
                    Some("retry_exhausted"),
                    &timestamp(),
                )
                .await;
            return;
        }
        let request = NotificationRequest::from_delivery(&delivery);
        for attempt in delivery.attempts as usize..MAX_ATTEMPTS {
            let _ = self
                .database
                .mark_notification_attempt(delivery.delivery_id)
                .await;
            match self.provider.send(&request).await {
                NotificationProviderResult::Delivered => {
                    let _ = self
                        .database
                        .finish_notification_delivery(
                            delivery.delivery_id,
                            "delivered",
                            Some("success"),
                            &timestamp(),
                        )
                        .await;
                    return;
                }
                NotificationProviderResult::InvalidToken(code) => {
                    let _ = self
                        .database
                        .invalidate_apns_registration(delivery.registration.id, &timestamp())
                        .await;
                    let _ = self
                        .database
                        .finish_notification_delivery(
                            delivery.delivery_id,
                            "failed",
                            Some(&code),
                            &timestamp(),
                        )
                        .await;
                    return;
                }
                NotificationProviderResult::Failed(code) => {
                    let _ = self
                        .database
                        .finish_notification_delivery(
                            delivery.delivery_id,
                            "failed",
                            Some(&code),
                            &timestamp(),
                        )
                        .await;
                    return;
                }
                NotificationProviderResult::Retryable(code) => {
                    if attempt + 1 == MAX_ATTEMPTS {
                        let _ = self
                            .database
                            .finish_notification_delivery(
                                delivery.delivery_id,
                                "failed",
                                Some(&code),
                                &timestamp(),
                            )
                            .await;
                        return;
                    }
                    if let Some(delay) = self.retry_delays.get(attempt) {
                        tokio::time::sleep(*delay).await;
                    }
                }
            }
        }
    }
}

pub struct NotificationRequest {
    token: String,
    environment: ApnsEnvironment,
    topic: String,
    apns_id: Uuid,
    payload: serde_json::Value,
}

impl NotificationRequest {
    fn from_delivery(delivery: &PendingNotificationDelivery) -> Self {
        let body = match delivery.outcome {
            AgentCycleOutcome::Ready => "Response ready",
            AgentCycleOutcome::Attention => "Agent needs attention",
            AgentCycleOutcome::Interrupted => "",
        };
        Self {
            token: delivery.registration.token.clone(),
            environment: delivery.registration.environment,
            topic: delivery.registration.topic.clone(),
            apns_id: delivery.delivery_id,
            payload: serde_json::json!({
                "aps": {
                    "alert": {
                        "title": delivery.conversation_title,
                        "body": body
                    },
                    "sound": "default",
                    "thread-id": delivery.conversation_id.to_string()
                },
                "conversationId": delivery.conversation_id,
                "url": format!("luna://conversation/{}", delivery.conversation_id)
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationProviderResult {
    Delivered,
    InvalidToken(String),
    Retryable(String),
    Failed(String),
}

#[async_trait]
pub trait NotificationProvider: Send + Sync {
    async fn send(&self, request: &NotificationRequest) -> NotificationProviderResult;
}

struct UnavailableNotificationProvider;

#[async_trait]
impl NotificationProvider for UnavailableNotificationProvider {
    async fn send(&self, _request: &NotificationRequest) -> NotificationProviderResult {
        NotificationProviderResult::Failed("provider_unavailable".into())
    }
}

struct ApnsHttpProvider {
    client: reqwest::Client,
    key: EncodingKey,
    key_id: String,
    team_id: String,
    bearer: Mutex<Option<CachedBearer>>,
}

struct CachedBearer {
    value: String,
    created_at: Instant,
}

#[derive(Serialize)]
struct ApnsClaims<'a> {
    iss: &'a str,
    iat: i64,
}

#[derive(Deserialize)]
struct ApnsErrorResponse {
    reason: Option<String>,
}

impl ApnsHttpProvider {
    fn new(
        key_path: PathBuf,
        key_id: String,
        team_id: String,
    ) -> Result<Self, NotificationConfigurationError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&key_path)?.permissions().mode() & 0o077 != 0 {
                return Err(NotificationConfigurationError::InsecureKeyFile);
            }
        }
        let key = EncodingKey::from_ec_pem(&std::fs::read(key_path)?)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            client,
            key,
            key_id,
            team_id,
            bearer: Mutex::new(None),
        })
    }

    async fn bearer(&self) -> Result<String, NotificationConfigurationError> {
        let mut cached = self.bearer.lock().await;
        if let Some(bearer) = &*cached
            && bearer.created_at.elapsed() < Duration::from_secs(50 * 60)
        {
            return Ok(bearer.value.clone());
        }
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key_id.clone());
        let value = encode(
            &header,
            &ApnsClaims {
                iss: &self.team_id,
                iat: time::OffsetDateTime::now_utc().unix_timestamp(),
            },
            &self.key,
        )?;
        *cached = Some(CachedBearer {
            value: value.clone(),
            created_at: Instant::now(),
        });
        Ok(value)
    }
}

#[async_trait]
impl NotificationProvider for ApnsHttpProvider {
    async fn send(&self, request: &NotificationRequest) -> NotificationProviderResult {
        let bearer = match self.bearer().await {
            Ok(value) => value,
            Err(_) => return NotificationProviderResult::Failed("provider_auth".into()),
        };
        let host = match request.environment {
            ApnsEnvironment::Sandbox => "https://api.sandbox.push.apple.com",
            ApnsEnvironment::Production => "https://api.push.apple.com",
        };
        let response = self
            .client
            .post(format!("{host}/3/device/{}", request.token))
            .bearer_auth(bearer)
            .header("apns-topic", &request.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header("apns-id", request.apns_id.to_string())
            .json(&request.payload)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(_) => return NotificationProviderResult::Retryable("transport".into()),
        };
        let status = response.status();
        if status == reqwest::StatusCode::NO_CONTENT {
            return NotificationProviderResult::Delivered;
        }
        let reason = response
            .json::<ApnsErrorResponse>()
            .await
            .ok()
            .and_then(|body| body.reason)
            .map_or_else(|| format!("http_{}", status.as_u16()), sanitize_code);
        if status == reqwest::StatusCode::GONE
            || matches!(
                reason.as_str(),
                "BadDeviceToken" | "DeviceTokenNotForTopic" | "Unregistered"
            )
        {
            NotificationProviderResult::InvalidToken(reason)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            NotificationProviderResult::Retryable(reason)
        } else {
            NotificationProviderResult::Failed(reason)
        }
    }
}

fn sanitize_code(value: String) -> String {
    let code: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .take(64)
        .collect();
    if code.is_empty() {
        "unknown".into()
    } else {
        code
    }
}

fn timestamp() -> String {
    now().unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[derive(Debug, thiserror::Error)]
pub enum NotificationConfigurationError {
    #[error("APNs provider configuration is incomplete")]
    Incomplete,
    #[error("APNs provider key file permissions must be 600")]
    InsecureKeyFile,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc, time::Duration};

    use async_trait::async_trait;
    use luna_protocol::{ApnsEnvironment, DevicePlatform, MessageDelivery};
    use luna_storage::{Database, NewApnsRegistration, NewDevice, NewPairingCode, NewUserMessage};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::{
        NotificationProvider, NotificationProviderResult, NotificationRequest, NotificationService,
    };

    struct FakeProvider {
        results: Mutex<VecDeque<NotificationProviderResult>>,
        requests: Mutex<Vec<serde_json::Value>>,
    }

    #[async_trait]
    impl NotificationProvider for FakeProvider {
        async fn send(&self, request: &NotificationRequest) -> NotificationProviderResult {
            self.requests.lock().await.push(request.payload.clone());
            self.results
                .lock()
                .await
                .pop_front()
                .unwrap_or(NotificationProviderResult::Delivered)
        }
    }

    #[tokio::test]
    async fn retries_without_leaking_message_content_and_invalidates_bad_tokens() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::connect(&directory.path().join("notifications.sqlite"))
            .await
            .expect("database");
        let device_id = Uuid::now_v7();
        database
            .create_pairing_code(NewPairingCode {
                id: Uuid::now_v7(),
                code_hash: "pairing-hash",
                created_at: "2026-03-20T11:59:00Z",
                expires_at: "2026-03-20T12:10:00Z",
            })
            .await
            .expect("pairing code");
        database
            .redeem_pairing_code(
                "pairing-hash",
                NewDevice {
                    id: device_id,
                    name: "iPhone",
                    platform: DevicePlatform::Ios,
                    credential_hash: "hash",
                    created_at: "2026-03-20T12:00:00Z",
                },
            )
            .await
            .expect("device")
            .expect("paired device");
        let conversation = database
            .create_conversation(Uuid::now_v7(), "/tmp", "2026-03-20T12:00:00Z")
            .await
            .expect("conversation");
        let token = "a".repeat(64);
        database
            .upsert_apns_registration(NewApnsRegistration {
                device_id,
                token: &token,
                environment: ApnsEnvironment::Sandbox,
                topic: "com.yannickherrero.luna",
                app_version: Some("1"),
                registered_at: "2026-03-20T12:00:00Z",
            })
            .await
            .expect("registration");
        database
            .accept_user_message(NewUserMessage {
                conversation_id: conversation.id,
                device_id,
                client_message_id: Uuid::now_v7(),
                text: "private complete message",
                attachment_ids: &[],
                delivery: MessageDelivery::Initial,
                accepted_at: "2026-03-20T12:00:01Z",
            })
            .await
            .expect("message");
        let provider = Arc::new(FakeProvider {
            results: Mutex::new(VecDeque::from([
                NotificationProviderResult::Retryable("http_500".into()),
                NotificationProviderResult::InvalidToken("BadDeviceToken".into()),
            ])),
            requests: Mutex::new(vec![]),
        });
        let service = NotificationService::new(database.clone(), provider.clone())
            .with_retry_delays(vec![Duration::ZERO, Duration::ZERO]);
        service
            .complete_cycle(
                conversation.id,
                None,
                luna_storage::AgentCycleOutcome::Ready,
            )
            .await
            .expect("complete cycle");
        tokio::time::sleep(Duration::from_millis(20)).await;

        let requests = provider.requests.lock().await;
        assert_eq!(requests.len(), 2);
        let serialized = serde_json::to_string(&requests[0]).expect("payload");
        assert!(serialized.contains("Response ready"));
        assert!(serialized.contains(&conversation.id.to_string()));
        assert!(!serialized.contains("private complete message"));
        drop(requests);
        let authenticated = database
            .authenticate_device("hash", "2026-03-20T12:01:00Z")
            .await
            .expect("authentication")
            .expect("device");
        assert!(!authenticated.notifications_enabled);
    }
}
