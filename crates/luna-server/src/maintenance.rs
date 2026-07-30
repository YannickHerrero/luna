use std::{path::PathBuf, time::Duration};

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
    pub fn spawn(
        database: Database,
        event_retention_days: u32,
        attachment_directory: PathBuf,
        attachment_retention_days: u32,
    ) -> Self {
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
                let attachment_cutoff = OffsetDateTime::now_utc()
                    - time::Duration::days(i64::from(attachment_retention_days.max(1)));
                if let Ok(cutoff) = attachment_cutoff.format(&Rfc3339) {
                    match database.expired_attachment_files(&cutoff).await {
                        Ok(attachments) => {
                            for attachment in attachments {
                                let original = attachment_directory.join(&attachment.storage_key);
                                let thumbnail =
                                    attachment_directory.join(&attachment.thumbnail_storage_key);
                                let original_removed = remove_if_present(&original).await;
                                let thumbnail_removed = remove_if_present(&thumbnail).await;
                                if original_removed
                                    && thumbnail_removed
                                    && let Err(error) = database
                                        .mark_attachment_deleted(attachment.id, &cutoff)
                                        .await
                                {
                                    warn!(attachment_id = %attachment.id, "Unable to finalize attachment cleanup: {error}");
                                }
                            }
                        }
                        Err(error) => warn!("Unable to find expired attachments: {error}"),
                    }
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

async fn remove_if_present(path: &std::path::Path) -> bool {
    match tokio::fs::remove_file(path).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => {
            warn!(path = %path.display(), "Unable to remove expired attachment: {error}");
            false
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
