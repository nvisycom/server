//! Middleware configuration for the HTTP server.
//!
//! This module provides CLI-configurable middleware settings including CORS,
//! OpenAPI documentation, and request recovery (timeouts/panic handling).
//!
//! Each field is a clap args struct that converts into the corresponding
//! plain config type owned by `nvisy-server`.
//!
//! # Example
//!
//! ```bash
//! # Configure CORS origins and request timeout
//! nvisy-cli --cors-origins "https://example.com" --request-timeout 60s
//! ```

use std::time::Duration;

use clap::Args;
use nvisy_server::middleware::{CorsConfig, OpenApiConfig, RecoveryConfig};

use super::TRACING_TARGET_CONFIG;

/// Middleware configuration combining CORS, OpenAPI, and recovery settings.
///
/// This struct groups all HTTP middleware configurations that can be
/// customized via CLI arguments or environment variables.
#[derive(Debug, Clone, Args)]
pub struct MiddlewareConfig {
    /// CORS (Cross-Origin Resource Sharing) configuration.
    #[clap(flatten)]
    pub cors: CorsArgs,

    /// OpenAPI documentation configuration.
    #[clap(flatten)]
    pub openapi: OpenApiArgs,

    /// Recovery middleware configuration.
    #[clap(flatten)]
    pub recovery: RecoveryArgs,
}

impl MiddlewareConfig {
    /// Returns the CORS configuration.
    pub fn cors(&self) -> CorsConfig {
        self.cors.clone().into()
    }

    /// Returns the OpenAPI configuration.
    pub fn openapi(&self) -> OpenApiConfig {
        self.openapi.clone().into()
    }

    /// Returns the recovery configuration.
    pub fn recovery(&self) -> RecoveryConfig {
        self.recovery.clone().into()
    }

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
            request_timeout = ?self.recovery.request_timeout,
            "Recovery configuration"
        );
    }
}

/// CORS arguments.
#[derive(Debug, Clone, Args)]
pub struct CorsArgs {
    /// List of allowed CORS origins (comma-separated).
    #[arg(long, env = "CORS_ORIGINS", value_delimiter = ',')]
    pub allowed_origins: Vec<String>,

    /// Maximum age for CORS preflight caching (e.g. `1h`).
    #[arg(
        long,
        env = "CORS_MAX_AGE",
        default_value = "1h",
        value_parser = humantime::parse_duration,
    )]
    pub max_age: Duration,

    /// Whether to allow credentials in CORS requests.
    #[arg(long, env = "CORS_ALLOW_CREDENTIALS", default_value = "true")]
    pub allow_credentials: bool,
}

impl From<CorsArgs> for CorsConfig {
    fn from(args: CorsArgs) -> Self {
        Self {
            allowed_origins: args.allowed_origins,
            max_age: args.max_age,
            allow_credentials: args.allow_credentials,
        }
    }
}

/// OpenAPI documentation path arguments.
#[derive(Debug, Clone, Args)]
pub struct OpenApiArgs {
    /// Path which exposes the OpenAPI JSON specification.
    #[arg(long, env = "OPENAPI_JSON_PATH", default_value = "/api/openapi.json")]
    pub open_api_json: String,

    /// Path which exposes the Scalar API reference UI.
    #[arg(long, env = "OPENAPI_SCALAR_PATH", default_value = "/api/scalar")]
    pub scalar_ui: String,
}

impl From<OpenApiArgs> for OpenApiConfig {
    fn from(args: OpenApiArgs) -> Self {
        Self {
            open_api_json: args.open_api_json,
            scalar_ui: args.scalar_ui,
        }
    }
}

/// Request recovery arguments.
#[derive(Debug, Clone, Args)]
pub struct RecoveryArgs {
    /// Maximum duration to wait for a request before timing out (e.g. `30s`).
    #[arg(
        long,
        env = "REQUEST_TIMEOUT",
        default_value = "30s",
        value_parser = humantime::parse_duration,
    )]
    pub request_timeout: Duration,
}

impl From<RecoveryArgs> for RecoveryConfig {
    fn from(args: RecoveryArgs) -> Self {
        Self {
            request_timeout: args.request_timeout,
        }
    }
}
