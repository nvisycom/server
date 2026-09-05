//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli
//! ├── server: ServerConfig         # Host, port, TLS, shutdown
//! ├── middleware: MiddlewareArgs    # CORS, OpenAPI, recovery/timeouts
//! ├── service: ServiceArgs          # Postgres, NATS, S3, crypto, session keys, engine, health, sync, uploads
//! └── reqwest: ReqwestConfig        # HTTP client for webhook delivery
//! ```
//!
//! The library config types derive their clap/env wiring directly behind a
//! `cli` feature, so they are flattened straight into the argument tree. All
//! configuration can be provided via CLI arguments or environment variables.
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

mod server;

use std::process;

use clap::Parser;
use nvisy_server::ServiceArgs;
use nvisy_server::middleware::MiddlewareArgs;
use nvisy_server::service::ServiceState;
use nvisy_webhook::reqwest::{ReqwestClient, ReqwestConfig};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub use self::server::ServerConfig;
use crate::server::TRACING_TARGET_STARTUP;

/// Tracing target for configuration events.
pub const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServerConfig`]: Network binding and TLS
/// - [`MiddlewareArgs`]: HTTP middleware (CORS, OpenAPI, recovery)
/// - [`ServiceArgs`]: External services, resources, and request limits
/// - [`ReqwestConfig`]: HTTP client configuration for webhook delivery
#[derive(Debug, Clone, Parser)]
#[command(name = "nvisy")]
#[command(about = "Nvisy document processing server")]
#[command(version)]
pub struct Cli {
    /// Server network and lifecycle configuration.
    #[clap(flatten)]
    pub server: ServerConfig,

    /// HTTP middleware configuration (CORS, OpenAPI, recovery/timeouts).
    #[clap(flatten)]
    pub middleware: MiddlewareArgs,

    /// External services, resources, and request limits (Postgres, NATS, S3,
    /// crypto, session keys, engine, health, sync, uploads).
    #[clap(flatten)]
    pub service: ServiceArgs,

    /// HTTP client configuration for webhook delivery.
    #[clap(flatten)]
    pub reqwest: ReqwestConfig,
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

    /// Logs configuration at startup (no sensitive information).
    ///
    /// Build and server settings are binary-specific and logged here; the
    /// middleware and service echoes are owned by their aggregates so a
    /// downstream reuses them.
    pub fn log(&self) {
        Self::log_build_info();
        self.server.log();
        self.middleware.log();
        self.service.log();
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

    /// Initializes application state from CLI configuration.
    pub async fn service_state(&self) -> anyhow::Result<ServiceState> {
        let webhook = ReqwestClient::new(self.reqwest.clone()).into_service();
        Ok(ServiceState::from_args(self.service.clone(), webhook).await?)
    }
}
