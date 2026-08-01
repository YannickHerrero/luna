use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use luna_protocol::{OpenAiUsageAvailability, OpenAiWeeklyUsage};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
    time::{Instant, timeout},
};

const WEEKLY_WINDOW_MINUTES: i64 = 7 * 24 * 60;

#[derive(Clone)]
pub struct OpenAiUsageService {
    executable: Arc<PathBuf>,
    cache_ttl: Duration,
    request_timeout: Duration,
    cache: Arc<Mutex<UsageCache>>,
}

#[derive(Default)]
struct UsageCache {
    last_attempt: Option<Instant>,
    last_valid: Option<OpenAiWeeklyUsage>,
    last_response: Option<OpenAiWeeklyUsage>,
}

impl OpenAiUsageService {
    #[must_use]
    pub fn new(executable: PathBuf, cache_ttl: Duration, request_timeout: Duration) -> Self {
        Self {
            executable: Arc::new(executable),
            cache_ttl,
            request_timeout,
            cache: Arc::new(Mutex::new(UsageCache::default())),
        }
    }

    pub async fn get(&self) -> OpenAiWeeklyUsage {
        let mut cache = self.cache.lock().await;
        if cache
            .last_attempt
            .is_some_and(|attempt| attempt.elapsed() < self.cache_ttl)
            && let Some(response) = &cache.last_response
        {
            return response.clone();
        }
        cache.last_attempt = Some(Instant::now());
        let response = match self.collect().await {
            Ok(usage) => {
                cache.last_valid = Some(usage.clone());
                usage
            }
            Err(_) => cache
                .last_valid
                .clone()
                .map_or_else(unavailable, |mut usage| {
                    usage.availability = OpenAiUsageAvailability::Stale;
                    usage
                }),
        };
        cache.last_response = Some(response.clone());
        response
    }

    async fn collect(&self) -> Result<OpenAiWeeklyUsage, OpenAiUsageError> {
        timeout(self.request_timeout, self.collect_inner())
            .await
            .map_err(|_| OpenAiUsageError::Timeout)?
    }

    async fn collect_inner(&self) -> Result<OpenAiWeeklyUsage, OpenAiUsageError> {
        let mut child = Command::new(self.executable.as_ref())
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdin = child.stdin.take().ok_or(OpenAiUsageError::MissingPipe)?;
        for request in [
            serde_json::json!({
                "id": 1,
                "method": "initialize",
                "params": {
                    "clientInfo": {
                        "name": "luna-server",
                        "title": "Luna Server",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            serde_json::json!({"method": "initialized", "params": {}}),
            serde_json::json!({
                "id": 2,
                "method": "account/rateLimits/read",
                "params": {}
            }),
        ] {
            stdin.write_all(&serde_json::to_vec(&request)?).await?;
            stdin.write_all(b"\n").await?;
        }
        stdin.flush().await?;

        let stdout = child.stdout.take().ok_or(OpenAiUsageError::MissingPipe)?;
        let mut lines = BufReader::new(stdout).lines();
        let response = loop {
            let line = lines
                .next_line()
                .await?
                .ok_or(OpenAiUsageError::SourceUnavailable)?;
            let envelope: serde_json::Value = serde_json::from_str(&line)?;
            if envelope.get("id").and_then(serde_json::Value::as_i64) != Some(2) {
                continue;
            }
            let result = envelope
                .get("result")
                .cloned()
                .ok_or(OpenAiUsageError::SourceUnavailable)?;
            break serde_json::from_value::<CodexRateLimitsResponse>(result)?;
        };
        let _ = child.kill().await;
        sanitized_usage(&response)
    }
}

fn unavailable() -> OpenAiWeeklyUsage {
    OpenAiWeeklyUsage {
        availability: OpenAiUsageAvailability::Unavailable,
        used_percent: None,
        resets_at: None,
        collected_at: None,
    }
}

fn sanitized_usage(
    response: &CodexRateLimitsResponse,
) -> Result<OpenAiWeeklyUsage, OpenAiUsageError> {
    let mut snapshots = Vec::new();
    if response
        .rate_limits
        .limit_id
        .as_deref()
        .is_none_or(|id| id == "codex")
    {
        snapshots.push(&response.rate_limits);
    }
    if let Some(codex) = response
        .rate_limits_by_limit_id
        .as_ref()
        .and_then(|limits| limits.get("codex"))
    {
        snapshots.push(codex);
    }
    let window = snapshots
        .into_iter()
        .flat_map(|snapshot| [snapshot.primary.as_ref(), snapshot.secondary.as_ref()])
        .flatten()
        .find(|window| window.window_duration_mins == Some(WEEKLY_WINDOW_MINUTES))
        .ok_or(OpenAiUsageError::WeeklyLimitUnavailable)?;
    let used_percent = u8::try_from(window.used_percent)
        .ok()
        .filter(|percent| *percent <= 100)
        .ok_or(OpenAiUsageError::InvalidPercentage)?;
    let reset = window
        .resets_at
        .ok_or(OpenAiUsageError::WeeklyLimitUnavailable)?;
    let resets_at = OffsetDateTime::from_unix_timestamp(reset)
        .map_err(|_| OpenAiUsageError::InvalidTimestamp)?
        .format(&Rfc3339)
        .map_err(|_| OpenAiUsageError::InvalidTimestamp)?;
    let collected_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| OpenAiUsageError::InvalidTimestamp)?;
    Ok(OpenAiWeeklyUsage {
        availability: OpenAiUsageAvailability::Available,
        used_percent: Some(used_percent),
        resets_at: Some(resets_at),
        collected_at: Some(collected_at),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexRateLimitsResponse {
    rate_limits: CodexRateLimitSnapshot,
    rate_limits_by_limit_id: Option<HashMap<String, CodexRateLimitSnapshot>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexRateLimitSnapshot {
    limit_id: Option<String>,
    primary: Option<CodexRateLimitWindow>,
    secondary: Option<CodexRateLimitWindow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexRateLimitWindow {
    used_percent: i32,
    resets_at: Option<i64>,
    window_duration_mins: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
enum OpenAiUsageError {
    #[error("OpenAI usage collection timed out")]
    Timeout,
    #[error("OpenAI usage source is unavailable")]
    SourceUnavailable,
    #[error("OpenAI weekly limit is unavailable")]
    WeeklyLimitUnavailable,
    #[error("OpenAI usage percentage is invalid")]
    InvalidPercentage,
    #[error("OpenAI usage timestamp is invalid")]
    InvalidTimestamp,
    #[error("OpenAI usage process pipe is unavailable")]
    MissingPipe,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, time::Duration};

    use luna_protocol::OpenAiUsageAvailability;

    use super::{
        CodexRateLimitSnapshot, CodexRateLimitWindow, CodexRateLimitsResponse, OpenAiUsageService,
        sanitized_usage,
    };

    #[test]
    fn selects_only_the_general_codex_weekly_bucket() {
        let response = CodexRateLimitsResponse {
            rate_limits: CodexRateLimitSnapshot {
                limit_id: Some("codex".into()),
                primary: Some(CodexRateLimitWindow {
                    used_percent: 63,
                    resets_at: Some(1_700_000_000),
                    window_duration_mins: Some(10_080),
                }),
                secondary: None,
            },
            rate_limits_by_limit_id: Some(HashMap::from([(
                "codex_bengalfox".into(),
                CodexRateLimitSnapshot {
                    limit_id: Some("codex_bengalfox".into()),
                    primary: Some(CodexRateLimitWindow {
                        used_percent: 1,
                        resets_at: Some(1_800_000_000),
                        window_duration_mins: Some(10_080),
                    }),
                    secondary: None,
                },
            )])),
        };
        let usage = sanitized_usage(&response).expect("weekly usage");
        assert_eq!(usage.availability, OpenAiUsageAvailability::Available);
        assert_eq!(usage.used_percent, Some(63));
        assert_eq!(usage.resets_at.as_deref(), Some("2023-11-14T22:13:20Z"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn caches_success_and_serves_it_as_stale_after_a_failure() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp directory");
        let executable = directory.path().join("fake-codex");
        fs::write(
            &executable,
            r#"#!/bin/sh
read initialize
printf '%s\n' '{"id":1,"result":{}}'
read initialized
read limits
printf '%s\n' '{"id":2,"result":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":42,"resetsAt":1700000000,"windowDurationMins":10080}}}}'
"#,
        )
        .expect("fake Codex");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("permissions");
        let service =
            OpenAiUsageService::new(executable.clone(), Duration::ZERO, Duration::from_secs(2));
        let available = service.get().await;
        assert_eq!(available.used_percent, Some(42));

        fs::write(&executable, "#!/bin/sh\nexit 1\n").expect("failing Codex");
        let stale = service.get().await;
        assert_eq!(stale.availability, OpenAiUsageAvailability::Stale);
        assert_eq!(stale.used_percent, Some(42));
        assert_eq!(stale.collected_at, available.collected_at);

        let unavailable =
            OpenAiUsageService::new(executable, Duration::from_secs(60), Duration::from_secs(2))
                .get()
                .await;
        assert_eq!(
            unavailable.availability,
            OpenAiUsageAvailability::Unavailable
        );
        assert_eq!(unavailable.used_percent, None);
        assert_eq!(unavailable.resets_at, None);
        assert_eq!(unavailable.collected_at, None);
    }
}
