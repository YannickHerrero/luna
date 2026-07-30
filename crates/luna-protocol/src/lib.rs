#![forbid(unsafe_code)]

mod api;
mod common;
mod entities;
mod events;
mod openapi;

pub use api::*;
pub use common::*;
pub use entities::*;
pub use events::*;
pub use openapi::openapi;

pub const PROTOCOL_VERSION: u8 = 1;
