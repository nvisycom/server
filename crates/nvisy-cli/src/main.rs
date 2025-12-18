#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

mod config;
mod server;
mod telemetry;

use std::process;

use anyhow::Context;
use clap::Parser;

#[cfg(feature = "telemetry")]
use crate::config::TelemetryConfig;
use crate::config::{CliConfig, ServerConfig, log_server_config};
#[cfg(feature = "telemetry")]
use crate::telemetry::TelemetryContext;
use crate::telemetry::init_tracing;

// Tracing target constants
pub const TRACING_TARGET_SERVER_STARTUP: &str = "nvisy_cli::server::startup";
pub const TRACING_TARGET_SERVER_SHUTDOWN: &str = "nvisy_cli::server::shutdown";
pub const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";
#[cfg(feature = "telemetry")]
pub const TRACING_TARGET_TELEMETRY: &str = "nvisy_cli::telemetry";

/// Nvisy CLI - Document processing server
#[derive(Debug, Parser)]
#[command(
    name = "nvisy-cli",
    version,
    about = "Document processing server",
    long_about = "A high-performance HTTP server for document processing with support for TLS, graceful shutdown, and structured logging."
)]
struct Cli {
    /// HTTP server configuration options.
    ///
    /// Controls network binding, timeouts, TLS settings, and performance parameters
    /// for the HTTP server. All server options can be overridden via environment
    /// variables or command-line arguments.
    #[command(flatten)]
    server: ServerConfig,

    /// Service and business logic configuration options.
    ///
    /// Contains settings for external services like `PostgreSQL` and `NATS`,
    /// as well as API documentation and CORS policy configuration.
    #[command(flatten)]
    service: CliConfig,

    /// Telemetry configuration for usage and crash reporting.
    ///
    /// Controls whether anonymous usage statistics and crash reports are collected
    /// to help improve the software. All telemetry is strictly opt-in and respects
    /// user privacy - no personally identifiable information is ever transmitted.
    ///
    /// Users can disable telemetry at any time without affecting functionality.
    #[cfg(feature = "telemetry")]
    #[command(flatten)]
    telemetry: TelemetryConfig,
}

impl Cli {
    /// Validates the CLI configuration
    fn validate(&self) -> anyhow::Result<()> {
        self.server
            .validate()
            .context("Invalid server configuration")?;
        // Service config validation would go here if needed
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let Err(error) = run_application().await else {
        tracing::info!(target: TRACING_TARGET_SERVER_SHUTDOWN, "Application terminated successfully");
        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(target: TRACING_TARGET_SERVER_SHUTDOWN, error = %error, "Application terminated with error");
    } else {
        eprintln!("Application terminated with error: {error:#}");
    }

    process::exit(1);
}

/// Main application entry point
async fn run_application() -> anyhow::Result<()> {
    // Parse and validate CLI arguments
    let cli = Cli::parse();
    cli.validate().context("CLI validation failed")?;

    // Initialize tracing
    init_tracing();

    // Log startup information
    log_startup_info();
    log_server_config(&cli.server);

    // Create telemetry context if enabled
    #[cfg(feature = "telemetry")]
    let telemetry_context = create_telemetry_context(&cli.telemetry);

    // Convert CLI config to server configs
    let (_service_config, cors_config, openapi_config) = cli.service.into_server_configs();

    // Create a simple router for now (nvisy-server integration removed for clean build)
    let router = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async { "Hello from Nvisy CLI!" }),
        )
        .route("/health", axum::routing::get(|| async { "OK" }));

    // Log service configuration details
    tracing::info!(
        target: TRACING_TARGET_CONFIG,
        cors_origins = ?cors_config.allowed_origins,
        cors_credentials = cors_config.allow_credentials,
        openapi_json_path = %openapi_config.open_api_json,
        swagger_path = %openapi_config.swagger_ui,
        "service configuration loaded successfully"
    );

    // Start the server with enhanced error handling and logging
    #[cfg(feature = "telemetry")]
    {
        if telemetry_context.is_some() {
            server::serve_with_telemetry(router, cli.server, telemetry_context.as_ref()).await?;
        } else {
            server::serve(router, cli.server).await?;
        }
    }

    #[cfg(not(feature = "telemetry"))]
    {
        server::serve(router, cli.server).await?;
    }

    Ok(())
}

#[cfg(feature = "telemetry")]
fn create_telemetry_context(telemetry_config: &TelemetryConfig) -> Option<TelemetryContext> {
    if !telemetry_config.enabled {
        tracing::debug!(
            target: TRACING_TARGET_SERVER_STARTUP,
            "telemetry disabled by configuration"
        );

        return None;
    }

    let context = TelemetryContext::new(telemetry_config.clone(), true);
    match context.validate() {
        Ok(()) => {
            tracing::info!(
                target: TRACING_TARGET_SERVER_STARTUP,
                session_id = %context.session_id,
                endpoint = %context.endpoint(),
                collect_usage = context.should_collect_usage(),
                collect_crashes = context.should_collect_crashes(),
                "telemetry context initialized"
            );

            Some(context)
        }
        Err(error) => {
            tracing::warn!(
                target: TRACING_TARGET_SERVER_STARTUP,
                error = %error,
                "failed to create telemetry context"
            );

            None
        }
    }
}

/// Logs startup information
fn log_startup_info() {
    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        version = env!("CARGO_PKG_VERSION"),
        rust_version = env!("CARGO_PKG_RUST_VERSION"),
        "starting Nvisy CLI"
    );

    tracing::debug!(
        target: TRACING_TARGET_SERVER_STARTUP,
        pid = process::id(),
        arch = std::env::consts::ARCH,
        os = std::env::consts::OS,
        features = ?get_enabled_features(),
        "system and build information"
    );
}

/// Returns a list of enabled compile-time features
#[must_use]
fn get_enabled_features() -> Vec<&'static str> {
    [
        cfg!(feature = "tls").then_some("tls"),
        cfg!(feature = "telemetry").then_some("telemetry"),
        cfg!(feature = "otel").then_some("otel"),
    ]
    .into_iter()
    .flatten()
    .collect()
}
