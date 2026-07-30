#![forbid(unsafe_code)]

mod attachments;
mod auth;
mod conversations;
mod database;
mod error;
mod events;
mod messages;

pub use attachments::{NewAttachment, StoredAttachment};
pub use auth::{NewDevice, NewPairingCode};
pub use conversations::ConversationRuntimeRecord;
pub use database::Database;
pub use error::StorageError;
pub use messages::{AcceptedDispatch, NewUserMessage};

pub const SQLITE_APPLICATION_ID: i32 = 0x4C55_4E41;
