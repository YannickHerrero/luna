#![forbid(unsafe_code)]

mod error;
mod normalization;
mod rpc;

pub use error::PiError;
pub use normalization::{NormalizedPiEvent, normalize_event};
pub use rpc::{
    PiEvent, PiProcess, PiProcessConfig, ProcessStatus, RpcImage, RpcResponse, read_jsonl_record,
};

pub const PI_RPC_MODE: &str = "rpc";
