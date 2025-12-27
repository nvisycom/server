//! Middleware configuration for the HTTP server.
//!
//! This module provides CLI-configurable middleware settings including CORS,
//! OpenAPI documentation, and request recovery (timeouts/panic handling).
//!
//! All middleware configs are re-exported from `nvisy-server` and support
//! both CLI arguments and environment variables.
//!
//! # Example
//!
//! ```bash
//! # Configure CORS origins and request timeout
//! nvisy-cli --cors-origins "https://example.com" --request-timeout 60
//! ```

use clap::Args;
use nvisy_server::middleware::{CorsConfig, OpenApiConfig, RecoveryConfig};
use serde::{Deserialize, Serialize};

/// Middleware configuration combining CORS, OpenAPI, and recovery settings.
///
/// This struct groups all HTTP middleware configurations that can be
/// customized via CLI arguments or environment variables.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    /// CORS (Cross-Origin Resource Sharing) configuration.
    ///
    /// Controls which origins can access the API and what credentials
    /// are allowed in cross-origin requests.
    #[clap(flatten)]
    pub cors: CorsConfig,

    /// OpenAPI documentation configuration.
    ///
    /// Configures the paths where the OpenAPI JSON specification
    /// and Scalar UI are served.
    #[clap(flatten)]
    pub openapi: OpenApiConfig,

    /// Recovery middleware configuration.
    ///
    /// Controls request timeout and panic recovery behavior.
    #[clap(flatten)]
    pub recovery: RecoveryConfig,
}
