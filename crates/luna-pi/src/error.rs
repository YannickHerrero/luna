#[derive(Debug, thiserror::Error)]
pub enum PiError {
    #[error("Pi executable is unavailable: {0}")]
    ExecutableUnavailable(String),
    #[error("Pi process could not be started: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Pi RPC I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Pi RPC returned invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Pi RPC record exceeded the {0} byte limit")]
    RecordTooLarge(usize),
    #[error("Pi RPC request timed out")]
    Timeout,
    #[error("Pi RPC process is not running")]
    NotRunning,
    #[error("Pi RPC rejected {command}: {message}")]
    Rejected { command: String, message: String },
    #[error("Pi RPC response channel closed")]
    ResponseChannelClosed,
}
