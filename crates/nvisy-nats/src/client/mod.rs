//! NATS client connection management and configuration.

mod credentials;
mod nats_client;
mod nats_config;

pub use credentials::NatsCredentials;
pub use nats_client::{NatsClient, NatsConnection};
pub use nats_config::{NatsConfig, NatsTlsConfig};
