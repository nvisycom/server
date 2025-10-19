#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod telemetry;

pub mod config;
pub mod server;

use std::process;

use anyhow::Context;
use clap::Parser;

use crate::config::{ServerConfig, log_server_config};
use crate::telemetry::init_tracing;

// Tracing target constants for consistent logging
const TRACING_TARGET_MAIN: &str = "nvisy_cli::main";
const TRACING_TARGET_STARTUP: &str = "nvisy_cli::startup";
const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";
const TRACING_TARGET_SHUTDOWN: &str = "nvisy_cli::shutdown";

/// Nvisy CLI - Document processing server.
///
/// A high-performance HTTP server for document processing with support for
/// TLS, graceful shutdown, and structured logging.
#[derive(Debug, Parser)]
#[command(
    name = "nvisy-cli",
    version,
    about = "Document processing server",
    long_about = "A high-performance HTTP server for document processing with support for TLS, graceful shutdown, and structured logging."
)]
struct Cli {
    /// Server configuration options.
    #[command(flatten)]
    server: ServerConfig,
}

#[tokio::main]
async fn main() {
    let Err(error) = run_application().await else {
        tracing::info!(
            target: TRACING_TARGET_MAIN,
            "Application terminated successfully"
        );

        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(
            target: TRACING_TARGET_MAIN,
            error = %error,
            "Application terminated with error: {:#}",
            error
        );
    } else {
        eprintln!("❌ Error: {:#}", error);
    }

    process::exit(1);
}

/// Main application entry point with comprehensive error handling.
async fn run_application() -> anyhow::Result<()> {
    // Parse CLI arguments early
    let cli = Cli::parse();

    // Initialize tracing based on feature flags
    init_tracing().context("Failed to initialize tracing")?;

    // Log startup information
    log_startup_info();

    // Validate configuration
    cli.server
        .validate()
        .context("Invalid server configuration")?;

    // Log server configuration
    log_server_config(&cli.server);

    // TODO: Create ServiceConfig and initialize ServiceState
    // For now, we'll show a warning that service configuration is needed
    tracing::warn!(
        target: TRACING_TARGET_STARTUP,
        "Service configuration not yet implemented - need to create ServiceConfig from CLI args"
    );

    // TODO: Build router and start server
    // let service_config = create_service_config_from_cli()?;
    // let service_state = nvisy_server::service::ServiceState::from_config(&service_config)
    //     .await
    //     .context("Failed to initialize service state")?;
    //
    // let router = nvisy_server::handler::openapi_routes(service_state);
    // server::serve(router, cli.server).await?;

    tracing::warn!(
        target: TRACING_TARGET_STARTUP,
        "Server startup will be implemented - router building needs ServiceState"
    );

    Ok(())
}

/// Logs startup information including version and runtime details.
fn log_startup_info() {
    tracing::info!(
        target: TRACING_TARGET_STARTUP,
        version = env!("CARGO_PKG_VERSION"),
        rust_version = env!("CARGO_PKG_RUST_VERSION"),
        "Starting Nvisy CLI"
    );

    // Log system information
    tracing::debug!(
        target: TRACING_TARGET_STARTUP,
        pid = process::id(),
        arch = std::env::consts::ARCH,
        os = std::env::consts::OS,
        "System information"
    );
}
