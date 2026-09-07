//! NATS client connection management and configuration.

mod config;
mod service;

pub use config::NatsConfig;
pub use service::NatsClient;
