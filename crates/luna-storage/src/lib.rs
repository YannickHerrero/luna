#![forbid(unsafe_code)]

mod auth;
mod conversations;
mod database;
mod error;
mod events;
mod messages;

pub use auth::{NewDevice, NewPairingCode};
pub use conversations::ConversationRuntimeRecord;
pub use database::Database;
pub use error::StorageError;
pub use messages::AcceptedDispatch;

pub const SQLITE_APPLICATION_ID: i32 = 0x4C55_4E41;
