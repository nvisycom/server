//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli
//! ├── service: ServiceConfig      # Database, NATS, auth keys
//! ├── middleware: MiddlewareConfig # CORS, OpenAPI, recovery/timeouts
//! ├── server: ServerConfig         # Host, port, TLS, shutdown
//! └── ollama: OllamaConfig         # Ollama embeddings/VLM/OCR (optional)
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

use clap::Parser;
pub use middleware::MiddlewareConfig;
use nvisy_server::service::ServiceConfig;
pub use provider::create_ai_services;
use serde::{Deserialize, Serialize};
pub use server::{ServerConfig, log_server_config};

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServiceConfig`]: External service connections (Postgres, NATS)
/// - [`MiddlewareConfig`]: HTTP middleware (CORS, OpenAPI, recovery)
/// - [`ServerConfig`]: Network binding and TLS
/// - [`OllamaConfig`]: Ollama AI services configuration (feature-gated)
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
    pub mock: nvisy_core::MockConfig,
}
