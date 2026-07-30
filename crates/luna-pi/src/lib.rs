#![forbid(unsafe_code)]

mod bridge;
mod error;
mod normalization;
mod rpc;
mod session;

pub use bridge::{BridgeError, BridgeEvent, PiBridge};
pub use error::PiError;
pub use normalization::{NormalizedPiEvent, normalize_event};
pub use rpc::{
    PiEvent, PiProcess, PiProcessConfig, ProcessStatus, RpcDelivery, RpcImage, RpcResponse,
    read_jsonl_record,
};
pub use session::{ManagedSession, SessionError, SessionRuntimeConfig, SessionSupervisor};

pub const PI_RPC_MODE: &str = "rpc";
