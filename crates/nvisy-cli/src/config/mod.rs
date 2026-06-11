//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli
//! ├── server: ServerConfig         # Host, port, TLS, shutdown
//! ├── middleware: MiddlewareConfig  # CORS, OpenAPI, recovery/timeouts
//! ├── service: ServiceArgs          # Database, NATS, auth keys
//! └── reqwest: ReqwestArgs          # HTTP client for webhooks
//! ```
//!
//! The `*Args` structs carry the clap/env wiring and convert into the plain
//! config types owned by the library crates. All configuration can be provided
//! via CLI arguments or environment variables.
//! Use `--help` to see all available options.
//!
//! # Example
//!
//! ```bash
//! # Configure database and server
//! nvisy-cli --postgres-url "postgresql://..." --port 8080
//!
//! # Or via environment variables
//! POSTGRES_URL="postgresql://..." PORT=8080 nvisy-cli
//! ```

mod middleware;
mod server;
mod service;
mod webhook;

use std::process;

use clap::Parser;
use nvisy_server::service::ServiceState;
use nvisy_webhook::WebhookService;
use nvisy_webhook::reqwest::ReqwestClient;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub use self::middleware::MiddlewareConfig;
pub use self::server::ServerConfig;
pub use self::service::ServiceArgs;
pub use self::webhook::ReqwestArgs;
use crate::server::TRACING_TARGET_STARTUP;

/// Tracing target for configuration events.
pub const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServerConfig`]: Network binding and TLS
/// - [`MiddlewareConfig`]: HTTP middleware (CORS, OpenAPI, recovery)
/// - [`ServiceArgs`]: External service connections (Postgres, NATS, auth keys)
/// - [`ReqwestArgs`]: HTTP client configuration for webhooks
#[derive(Debug, Clone, Parser)]
#[command(name = "nvisy")]
#[command(about = "Nvisy document processing server")]
#[command(version)]
pub struct Cli {
    /// Server network and lifecycle configuration.
    #[clap(flatten)]
    pub server: ServerConfig,

    /// HTTP middleware configuration (CORS, OpenAPI, timeouts).
    #[clap(flatten)]
    pub middleware: MiddlewareConfig,

    /// External service configuration (databases, message queues).
    #[clap(flatten)]
    pub service: ServiceArgs,

    /// HTTP client configuration for webhook delivery.
    #[clap(flatten)]
    pub reqwest: ReqwestArgs,
}

impl Cli {
    /// Loads environment variables from .env file (if enabled) and parses CLI arguments.
    ///
    /// This is the preferred way to initialize the CLI configuration as it ensures
    /// .env files are loaded before clap parses arguments, allowing environment
    /// variables from .env to be used as defaults.
    pub fn init() -> Self {
        Self::load_dotenv();
        Self::parse()
    }

    /// Loads environment variables from .env file if the dotenv feature is enabled.
    ///
    /// This should be called before parsing CLI arguments so that clap's `env`
    /// feature can pick up values from .env files.
    #[cfg(feature = "dotenv")]
    fn load_dotenv() {
        if let Err(err) = dotenvy::dotenv()
            && !err.not_found()
        {
            eprintln!("Warning: failed to load .env file: {err}");
        }
    }

    /// No-op when dotenv feature is disabled.
    #[cfg(not(feature = "dotenv"))]
    fn load_dotenv() {}

    /// Initializes tracing with environment-based filtering.
    pub fn init_tracing() {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    /// Logs build information at debug level.
    fn log_build_info() {
        tracing::debug!(
            target: TRACING_TARGET_STARTUP,
            version = env!("CARGO_PKG_VERSION"),
            pid = process::id(),
            arch = std::env::consts::ARCH,
            os = std::env::consts::OS,
            features = ?Self::enabled_features(),
            "Build information"
        );
    }

    /// Logs configuration at debug level (no sensitive information).
    pub fn log(&self) {
        Self::log_build_info();
        self.server.log();
        self.middleware.log();

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            postgres_max_connections = self.service.postgres.postgres_max_connections,
            postgres_connection_timeout = ?self.service.postgres.postgres_connection_timeout,
            postgres_idle_timeout = ?self.service.postgres.postgres_idle_timeout,
            "Database configuration"
        );
    }

    /// Returns a list of enabled compile-time features.
    fn enabled_features() -> Vec<&'static str> {
        [
            cfg!(feature = "tls").then_some("tls"),
            cfg!(feature = "otel").then_some("otel"),
            cfg!(feature = "dotenv").then_some("dotenv"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Creates webhook service from CLI configuration.
    pub fn webhook_service(&self) -> WebhookService {
        ReqwestClient::new(self.reqwest.clone().into()).into_service()
    }

    /// Initializes application state from CLI configuration.
    pub async fn service_state(&self) -> anyhow::Result<ServiceState> {
        let webhook = self.webhook_service();
        let service = self.service.clone();
        Ok(ServiceState::from_config(
            service.postgres.into(),
            service.nats.into(),
            service.session_keys.into(),
            service.master_key.into(),
            webhook,
        )
        .await?)
    }
}
