//! The blob-store client: connection config, the store handle, and health.

mod config;
mod connect;
mod health;
mod store;

pub use config::S3Config;
pub use store::{BlobStore, GetObject};
