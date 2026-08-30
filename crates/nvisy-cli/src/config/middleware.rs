//! Middleware configuration for the HTTP server.
//!
//! This module provides CLI-configurable middleware settings including CORS,
//! OpenAPI documentation, and request recovery (timeouts/panic handling).
//!
//! The library `*Config` types derive `clap::Args` behind their `cli` feature,
//! so they are flattened directly into the argument tree here.
//!
//! # Example
//!
//! ```bash
//! # Configure CORS origins and request timeout
//! nvisy-cli --cors-origins "https://example.com" --request-timeout 60s
//! ```

use clap::Args;
use nvisy_server::middleware::{CorsConfig, OpenApiConfig, RecoveryConfig, UploadConfig};

use super::TRACING_TARGET_CONFIG;

/// Middleware configuration combining CORS, OpenAPI, and recovery settings.
///
/// This struct groups all HTTP middleware configurations that can be
/// customized via CLI arguments or environment variables.
#[derive(Debug, Clone, Args)]
pub struct MiddlewareConfig {
    /// CORS (Cross-Origin Resource Sharing) configuration.
    #[clap(flatten)]
    pub cors: CorsConfig,

    /// Request body size limits.
    #[clap(flatten)]
    pub upload: UploadConfig,

    /// OpenAPI documentation configuration.
    #[clap(flatten)]
    pub openapi: OpenApiConfig,

    /// Recovery middleware configuration.
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
            max_body_bytes = self.upload.max_body_bytes,
            max_file_body_bytes = self.upload.max_file_body_bytes,
            "Upload configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            openapi_path = %self.openapi.open_api_json,
            scalar_path = %self.openapi.scalar_ui,
            "OpenAPI configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            request_timeout = ?self.recovery.request_timeout,
            "Recovery configuration"
        );
    }
}
