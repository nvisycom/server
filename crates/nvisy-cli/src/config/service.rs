//! External service configuration (database, NATS, auth keys).
//!
//! The library `*Config` types derive `clap::Args` behind their `cli` feature,
//! so they are flattened directly into the CLI argument tree here.

use clap::Args;
use nvisy_nats::NatsConfig;
use nvisy_postgres::PgConfig;
use nvisy_server::service::{
    CryptoConfig, EngineConfig, HealthConfig, S3Config, SessionKeysConfig, SyncConfig,
};

/// Aggregated external-service arguments (database, NATS, auth keys).
#[derive(Debug, Clone, Args)]
pub struct ServiceArgs {
    /// Postgres database configuration.
    #[clap(flatten)]
    pub postgres: PgConfig,

    /// NATS configuration.
    #[clap(flatten)]
    pub nats: NatsConfig,

    /// JWT session key paths.
    #[clap(flatten)]
    pub session_keys: SessionKeysConfig,

    /// Master encryption key path.
    #[clap(flatten)]
    pub crypto: CryptoConfig,

    /// Redaction engine configuration.
    #[clap(flatten)]
    pub engine: EngineConfig,

    /// Health monitoring configuration.
    #[clap(flatten)]
    pub health: HealthConfig,

    /// Connection sync configuration.
    #[clap(flatten)]
    pub sync: SyncConfig,

    /// S3-compatible blob storage configuration.
    #[clap(flatten)]
    pub s3: S3Config,
}
