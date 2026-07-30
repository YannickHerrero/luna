#![forbid(unsafe_code)]

mod database;
mod error;

pub use database::Database;
pub use error::StorageError;

pub const SQLITE_APPLICATION_ID: i32 = 0x4C55_4E41;
