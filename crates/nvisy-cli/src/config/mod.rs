//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli
//! ├── service: ServiceConfig      # Database, NATS, auth keys
//! ├── middleware: MiddlewareConfig # CORS, OpenAPI, recovery/timeouts
//! ├── server: ServerConfig         # Host, port, TLS, shutdown
//! └── ollama: OllamaConfig         # Ollama embeddings/VLM/OCR
//! ```
//!
//! All configuration can be provided via CLI arguments or environment variables.
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
mod provider;
mod server;

use std::process;

use anyhow::Context;
use clap::Parser;
pub use middleware::MiddlewareConfig;
use nvisy_server::service::ServiceConfig;
pub use provider::create_services;
use serde::{Deserialize, Serialize};
pub use server::ServerConfig;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::{TRACING_TARGET_CONFIG, TRACING_TARGET_SERVER_STARTUP};

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServiceConfig`]: External service connections (Postgres, NATS)
/// - [`MiddlewareConfig`]: HTTP middleware (CORS, OpenAPI, recovery)
/// - [`ServerConfig`]: Network binding and TLS
/// - `OllamaConfig`: Ollama AI services configuration (feature-gated)
/// - `MockConfig`: Testing AI services configuration (feature-gated)
#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
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
    pub service: ServiceConfig,

    /// Ollama configuration for embeddings, VLM, and OCR.
    #[cfg(feature = "ollama")]
    #[clap(flatten)]
    pub ollama: nvisy_ollama::OllamaConfig,

    /// Mock configuration for embeddings, VLM, and OCR.
    #[cfg(feature = "mock")]
    #[clap(flatten)]
    pub mock: nvisy_service::inference::MockConfig,
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
            target: TRACING_TARGET_SERVER_STARTUP,
            version = env!("CARGO_PKG_VERSION"),
            pid = process::id(),
            arch = std::env::consts::ARCH,
            os = std::env::consts::OS,
            features = ?Self::enabled_features(),
            "Build information"
        );
    }

    /// Validates all configuration values.
    pub fn validate(&self) -> anyhow::Result<()> {
        self.server
            .validate()
            .context("invalid server configuration")?;
        Ok(())
    }

    /// Logs configuration at debug level (no sensitive information).
    pub fn log(&self) {
        Self::log_build_info();
        self.server.log();
        self.middleware.log();

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            postgres_max_connections = self.service.postgres_config.postgres_max_connections,
            postgres_connection_timeout_secs = ?self.service.postgres_config.postgres_connection_timeout_secs,
            postgres_idle_timeout_secs = ?self.service.postgres_config.postgres_idle_timeout_secs,
            "Database configuration"
        );
    }

    /// Returns a list of enabled compile-time features.
    fn enabled_features() -> Vec<&'static str> {
        [
            cfg!(feature = "tls").then_some("tls"),
            cfg!(feature = "otel").then_some("otel"),
            cfg!(feature = "dotenv").then_some("dotenv"),
            cfg!(feature = "mock").then_some("mock"),
            cfg!(feature = "ollama").then_some("ollama"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}
