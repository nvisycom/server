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

use super::TRACING_TARGET_CONFIG;

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

impl MiddlewareConfig {
    /// Logs middleware configuration at info level.
    pub fn log(&self) {
        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            origins = ?self.cors.allowed_origins,
            credentials = self.cors.allow_credentials,
            "CORS configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            openapi_path = %self.openapi.open_api_json,
            scalar_path = %self.openapi.scalar_ui,
            "OpenAPI configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            request_timeout_secs = self.recovery.request_timeout,
            "Recovery configuration"
        );
    }
}
