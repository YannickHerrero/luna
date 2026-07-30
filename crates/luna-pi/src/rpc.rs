use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, Command},
    sync::{Mutex, broadcast, mpsc, oneshot, watch},
    time::timeout,
};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{PiError, normalization::NormalizedPiEvent, normalize_event};

const MAX_RPC_RECORD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PiProcessConfig {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub session_directory: PathBuf,
    pub session_path: Option<PathBuf>,
    pub extension_path: Option<PathBuf>,
    pub environment: HashMap<String, String>,
    pub request_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Exited { code: Option<i32> },
    Failed { message: String },
}

#[derive(Debug, Clone)]
pub struct PiEvent {
    pub raw: Value,
    pub normalized: NormalizedPiEvent,
}

#[derive(Debug, Clone)]
pub struct RpcResponse {
    pub command: String,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcDelivery {
    Normal,
    Steer,
    FollowUp,
}

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Result<RpcResponse, PiError>>>>>;

pub struct PiProcess {
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    pending: Pending,
    events: broadcast::Sender<PiEvent>,
    status: watch::Receiver<ProcessStatus>,
    shutdown: mpsc::Sender<()>,
    request_timeout: Duration,
}

impl PiProcess {
    pub async fn spawn(config: PiProcessConfig) -> Result<Self, PiError> {
        if !config.executable.exists() {
            return Err(PiError::ExecutableUnavailable(
                config.executable.display().to_string(),
            ));
        }
        tokio::fs::create_dir_all(&config.session_directory).await?;
        let mut command = Command::new(&config.executable);
        command
            .arg("--mode")
            .arg("rpc")
            .arg("--session-dir")
            .arg(&config.session_directory)
            .current_dir(&config.working_directory)
            .env("PI_SKIP_VERSION_CHECK", "1")
            .env("PI_TELEMETRY", "0")
            .envs(&config.environment)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(path) = &config.session_path {
            command.arg("--session").arg(path);
        }
        if let Some(path) = &config.extension_path {
            command.arg("--extension").arg(path);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let mut child = command.spawn().map_err(PiError::Spawn)?;
        let child_pid = child.id();
        let stdin = child.stdin.take().ok_or(PiError::NotRunning)?;
        let stdout = child.stdout.take().ok_or(PiError::NotRunning)?;
        let stderr = child.stderr.take().ok_or(PiError::NotRunning)?;
        let pending = Pending::default();
        let (events, _) = broadcast::channel(1_024);
        let (status_sender, status) = watch::channel(ProcessStatus::Starting);
        let (shutdown, mut shutdown_receiver) = mpsc::channel(1);

        tokio::spawn(read_stdout(stdout, pending.clone(), events.clone()));
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    warn!(target: "luna_pi::stderr", "{line}");
                }
            }
        });
        status_sender.send_replace(ProcessStatus::Running);
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    match status {
                        Ok(status) => { status_sender.send_replace(ProcessStatus::Exited { code: status.code() }); }
                        Err(error) => { status_sender.send_replace(ProcessStatus::Failed { message: error.to_string() }); }
                    }
                }
                _ = shutdown_receiver.recv() => {
                    match timeout(Duration::from_secs(3), child.wait()).await {
                        Ok(Ok(status)) => { status_sender.send_replace(ProcessStatus::Exited { code: status.code() }); }
                        Ok(Err(error)) => { status_sender.send_replace(ProcessStatus::Failed { message: error.to_string() }); }
                        Err(_) => {
                            #[cfg(unix)]
                            if let Some(pid) = child_pid.and_then(|pid| i32::try_from(pid).ok()) {
                                let _ = nix::sys::signal::killpg(
                                    nix::unistd::Pid::from_raw(pid),
                                    nix::sys::signal::Signal::SIGKILL,
                                );
                            }
                            #[cfg(not(unix))]
                            let _ = child.start_kill();
                            let status = child.wait().await.ok();
                            status_sender.send_replace(ProcessStatus::Exited { code: status.and_then(|value| value.code()) });
                        }
                    }
                }
            }
        });

        let process = Self {
            stdin: Arc::new(Mutex::new(Some(stdin))),
            pending,
            events,
            status,
            shutdown,
            request_timeout: config.request_timeout,
        };
        process.get_state().await?;
        Ok(process)
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<PiEvent> {
        self.events.subscribe()
    }

    #[must_use]
    pub fn status(&self) -> watch::Receiver<ProcessStatus> {
        self.status.clone()
    }

    pub async fn get_state(&self) -> Result<RpcResponse, PiError> {
        self.request(json!({ "type": "get_state" })).await
    }

    pub async fn get_entries(&self, since: Option<&str>) -> Result<RpcResponse, PiError> {
        self.request(match since {
            Some(since) => json!({ "type": "get_entries", "since": since }),
            None => json!({ "type": "get_entries" }),
        })
        .await
    }

    pub async fn prompt(
        &self,
        message: &str,
        images: &[RpcImage],
        delivery: RpcDelivery,
    ) -> Result<RpcResponse, PiError> {
        let mut request = json!({
            "type": "prompt",
            "message": message,
            "images": images,
        });
        match delivery {
            RpcDelivery::Normal => {}
            RpcDelivery::Steer => request["streamingBehavior"] = Value::String("steer".into()),
            RpcDelivery::FollowUp => {
                request["streamingBehavior"] = Value::String("followUp".into());
            }
        }
        self.request(request).await
    }

    pub async fn steer(&self, message: &str, images: &[RpcImage]) -> Result<RpcResponse, PiError> {
        self.request(json!({ "type": "steer", "message": message, "images": images }))
            .await
    }

    pub async fn abort(&self) -> Result<RpcResponse, PiError> {
        self.request(json!({ "type": "abort" })).await
    }

    pub async fn shutdown(&self) {
        self.stdin.lock().await.take();
        let _ = self.shutdown.send(()).await;
    }

    async fn request(&self, mut value: Value) -> Result<RpcResponse, PiError> {
        if *self.status.borrow() != ProcessStatus::Running {
            return Err(PiError::NotRunning);
        }
        let id = Uuid::new_v4().to_string();
        value["id"] = Value::String(id.clone());
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), sender);
        let serialized = serde_json::to_vec(&value)?;
        {
            let mut stdin = self.stdin.lock().await;
            let writer = stdin.as_mut().ok_or(PiError::NotRunning)?;
            writer.write_all(&serialized).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }
        match timeout(self.request_timeout, receiver).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => Err(PiError::ResponseChannelClosed),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(PiError::Timeout)
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcImage {
    #[serde(rename = "type")]
    pub content_type: &'static str,
    pub data: String,
    pub mime_type: String,
}

impl RpcImage {
    #[must_use]
    pub fn new(data: String, mime_type: String) -> Self {
        Self {
            content_type: "image",
            data,
            mime_type,
        }
    }
}

async fn read_stdout(
    stdout: tokio::process::ChildStdout,
    pending: Pending,
    events: broadcast::Sender<PiEvent>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_jsonl_record(&mut reader).await {
            Ok(Some(value)) => {
                if value.get("type").and_then(Value::as_str) == Some("response") {
                    handle_response(value, &pending).await;
                } else {
                    let _ = events.send(PiEvent {
                        normalized: normalize_event(&value),
                        raw: value,
                    });
                }
            }
            Ok(None) => break,
            Err(error) => {
                warn!("Pi RPC stdout failed: {error}");
                break;
            }
        }
    }
    let mut pending = pending.lock().await;
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(PiError::NotRunning));
    }
}

async fn handle_response(value: Value, pending: &Pending) {
    let Some(id) = value.get("id").and_then(Value::as_str) else {
        debug!("Pi RPC response had no request id");
        return;
    };
    let Some(sender) = pending.lock().await.remove(id) else {
        debug!(request_id = id, "Pi RPC response had no pending request");
        return;
    };
    let command = value
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let response = if value.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(RpcResponse {
            command,
            data: value.get("data").cloned(),
        })
    } else {
        Err(PiError::Rejected {
            command,
            message: value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Pi rejected the command")
                .to_owned(),
        })
    };
    let _ = sender.send(response);
}

pub async fn read_jsonl_record<R>(reader: &mut R) -> Result<Option<Value>, PiError>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut record = Vec::new();
    let bytes = reader.read_until(b'\n', &mut record).await?;
    if bytes == 0 {
        return Ok(None);
    }
    if record.len() > MAX_RPC_RECORD_BYTES {
        return Err(PiError::RecordTooLarge(MAX_RPC_RECORD_BYTES));
    }
    if record.last() == Some(&b'\n') {
        record.pop();
    }
    if record.last() == Some(&b'\r') {
        record.pop();
    }
    Ok(Some(serde_json::from_slice(&record)?))
}
