//! Configuration management for the server.

mod server;
mod service;

pub use server::{ServerConfig, log_server_config};
pub use service::CliConfig;
