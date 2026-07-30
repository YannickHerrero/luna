use std::time::Duration;

use luna_storage::Database;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub struct Maintenance {
    cancellation: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl Maintenance {
    #[must_use]
    pub fn spawn(database: Database, event_retention_days: u32) -> Self {
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            loop {
                let cutoff = OffsetDateTime::now_utc()
                    - time::Duration::days(i64::from(event_retention_days.max(1)));
                match cutoff.format(&Rfc3339) {
                    Ok(cutoff) => match database.prune_events_before(&cutoff).await {
                        Ok(removed) if removed > 0 => {
                            info!(removed, "Pruned expired synchronization events");
                        }
                        Ok(_) => {}
                        Err(error) => warn!("Unable to prune synchronization events: {error}"),
                    },
                    Err(error) => warn!("Unable to calculate event retention cutoff: {error}"),
                }
                tokio::select! {
                    () = task_cancellation.cancelled() => break,
                    () = tokio::time::sleep(Duration::from_secs(6 * 60 * 60)) => {}
                }
            }
        });
        Self {
            cancellation,
            task: Some(task),
        }
    }

    pub async fn shutdown(mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for Maintenance {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
