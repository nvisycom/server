//! NATS client connection management and configuration.

mod nats_client;
mod nats_config;

pub use nats_client::{NatsClient, NatsConnection};
pub use nats_config::NatsConfig;
