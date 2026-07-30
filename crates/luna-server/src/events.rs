use luna_protocol::{ServerEvent, ServerEventEnvelope};
use luna_storage::Database;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Clone)]
pub struct EventHub {
    database: Database,
    sender: broadcast::Sender<ServerEventEnvelope>,
}

impl EventHub {
    #[must_use]
    pub fn new(database: Database) -> Self {
        let (sender, _) = broadcast::channel(4_096);
        Self { database, sender }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEventEnvelope> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: ServerEventEnvelope) {
        let _ = self.sender.send(event);
    }

    pub async fn append(
        &self,
        conversation_id: Option<Uuid>,
        aggregate_id: Option<Uuid>,
        event: &ServerEvent,
        created_at: &str,
    ) -> Result<ServerEventEnvelope, AppError> {
        let envelope = self
            .database
            .append_event(conversation_id, aggregate_id, event, created_at)
            .await?;
        self.publish(envelope.clone());
        Ok(envelope)
    }
}
