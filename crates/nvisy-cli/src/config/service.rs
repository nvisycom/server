//! External service configuration arguments (database, NATS, auth keys).
//!
//! Each `*Args` struct carries the clap/env wiring and converts into the plain
//! config type owned by the corresponding library crate.

use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use nvisy_nats::NatsConfig;
use nvisy_postgres::PgConfig;
use nvisy_server::service::{CryptoConfig, EngineConfig, HealthConfig, SessionKeysConfig};

/// Aggregated external-service arguments (database, NATS, auth keys).
#[derive(Debug, Clone, Args)]
pub struct ServiceArgs {
    /// Postgres database configuration.
    #[clap(flatten)]
    pub postgres: PgArgs,

    /// NATS configuration.
    #[clap(flatten)]
    pub nats: NatsArgs,

    /// JWT session key paths.
    #[clap(flatten)]
    pub session_keys: SessionKeysArgs,

    /// Master encryption key path.
    #[clap(flatten)]
    pub crypto: CryptoArgs,

    /// Redaction engine configuration.
    #[clap(flatten)]
    pub engine: EngineArgs,

    /// Health monitoring configuration.
    #[clap(flatten)]
    pub health: HealthArgs,
}

/// Postgres connection arguments.
#[derive(Debug, Clone, Args)]
pub struct PgArgs {
    /// PostgreSQL connection URL.
    #[arg(long = "postgres-url", env = "POSTGRES_URL")]
    pub postgres_url: String,

    /// Maximum number of connections in the pool.
    #[arg(
        long = "postgres-max-connections",
        env = "POSTGRES_MAX_CONNECTIONS",
        default_value = "10"
    )]
    pub postgres_max_connections: u32,

    /// Connection timeout (e.g. `30s`).
    #[arg(
        long = "postgres-connection-timeout",
        env = "POSTGRES_CONNECTION_TIMEOUT",
        value_parser = humantime::parse_duration,
    )]
    pub postgres_connection_timeout: Option<Duration>,

    /// Idle connection timeout (e.g. `10m`).
    #[arg(
        long = "postgres-idle-timeout",
        env = "POSTGRES_IDLE_TIMEOUT",
        value_parser = humantime::parse_duration,
    )]
    pub postgres_idle_timeout: Option<Duration>,
}

impl From<PgArgs> for PgConfig {
    fn from(args: PgArgs) -> Self {
        Self {
            postgres_url: args.postgres_url,
            postgres_max_connections: args.postgres_max_connections,
            postgres_connection_timeout: args.postgres_connection_timeout,
            postgres_idle_timeout: args.postgres_idle_timeout,
        }
    }
}

/// NATS connection arguments.
#[derive(Debug, Clone, Args)]
pub struct NatsArgs {
    /// NATS server URL (comma-separated for clustering).
    #[arg(long = "nats-url", env = "NATS_URL")]
    pub nats_url: String,

    /// Authentication token.
    #[arg(long = "nats-token", env = "NATS_TOKEN")]
    pub nats_token: String,

    /// Client connection name.
    #[arg(long = "nats-client-name", env = "NATS_CLIENT_NAME")]
    pub nats_client_name: Option<String>,

    /// Connection timeout (e.g. `30s`).
    #[arg(
        long = "nats-connect-timeout",
        env = "NATS_CONNECT_TIMEOUT",
        value_parser = humantime::parse_duration,
    )]
    pub nats_connect_timeout: Option<Duration>,

    /// Request timeout (e.g. `30s`).
    #[arg(
        long = "nats-request-timeout",
        env = "NATS_REQUEST_TIMEOUT",
        value_parser = humantime::parse_duration,
    )]
    pub nats_request_timeout: Option<Duration>,

    /// Maximum number of reconnection attempts (0 = unlimited).
    #[arg(long = "nats-max-reconnects", env = "NATS_MAX_RECONNECTS")]
    pub nats_max_reconnects: Option<usize>,
}

impl From<NatsArgs> for NatsConfig {
    fn from(args: NatsArgs) -> Self {
        Self {
            nats_url: args.nats_url,
            nats_token: args.nats_token,
            nats_client_name: args.nats_client_name,
            nats_connect_timeout: args.nats_connect_timeout,
            nats_request_timeout: args.nats_request_timeout,
            nats_max_reconnects: args.nats_max_reconnects,
        }
    }
}

/// JWT session key path arguments.
#[derive(Debug, Clone, Args)]
pub struct SessionKeysArgs {
    /// File path to the JWT decoding (public) key.
    #[arg(long, env = "AUTH_PUBLIC_PEM_FILEPATH", default_value = "./public.pem")]
    pub decoding_key: PathBuf,

    /// File path to the JWT encoding (private) key.
    #[arg(
        long,
        env = "AUTH_PRIVATE_PEM_FILEPATH",
        default_value = "./private.pem"
    )]
    pub encoding_key: PathBuf,
}

impl From<SessionKeysArgs> for SessionKeysConfig {
    fn from(args: SessionKeysArgs) -> Self {
        Self {
            decoding_key: args.decoding_key,
            encoding_key: args.encoding_key,
        }
    }
}

/// Encryption key path arguments.
#[derive(Debug, Clone, Args)]
pub struct CryptoArgs {
    /// File path to the 32-byte master encryption key.
    #[arg(
        long,
        env = "ENCRYPTION_KEY_FILEPATH",
        default_value = "./encryption.key"
    )]
    pub key_path: PathBuf,
}

/// Redaction engine arguments.
#[derive(Debug, Clone, Args)]
pub struct EngineArgs {
    /// Optional path to a JSON file with the engine's NER/LLM recognizer
    /// lineups. Absent means no NER/LLM recognizers (pattern recognizers still
    /// run); the inference-backed lineups are supplied alongside the sidecars.
    #[arg(long, env = "ENGINE_CONFIG_FILEPATH")]
    pub config_path: Option<PathBuf>,
}

impl From<EngineArgs> for EngineConfig {
    fn from(args: EngineArgs) -> Self {
        Self {
            config_path: args.config_path,
        }
    }
}

impl From<CryptoArgs> for CryptoConfig {
    fn from(args: CryptoArgs) -> Self {
        Self {
            key_path: args.key_path,
        }
    }
}

/// Health monitoring arguments.
#[derive(Debug, Clone, Args)]
pub struct HealthArgs {
    /// How long cached health results stay valid (e.g. `30s`).
    #[arg(
        long = "health-cache-duration",
        env = "HEALTH_CACHE_DURATION",
        default_value = "30s",
        value_parser = humantime::parse_duration,
    )]
    pub cache_duration: Duration,
}

impl From<HealthArgs> for HealthConfig {
    fn from(args: HealthArgs) -> Self {
        Self {
            cache_duration: args.cache_duration,
        }
    }
}
