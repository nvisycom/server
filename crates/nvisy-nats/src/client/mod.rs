//! NATS client connection management and configuration.

#[allow(clippy::module_inception)]
mod client;
mod config;

pub use client::{NatsClient, NatsConnection};
pub use config::{NatsConfig, NatsCredentials, NatsTlsConfig};
