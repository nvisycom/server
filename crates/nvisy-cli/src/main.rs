#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

// Compile-time checks: ensure at least one backend is enabled for each service type

// Embedding/VLM backend: at least 'mock' or 'ollama' must be enabled
#[cfg(not(any(feature = "mock", feature = "ollama")))]
compile_error!(
    "At least one embedding/VLM backend must be enabled. \
     Enable either the 'mock' (for testing) or 'ollama' (for production) feature. \
     Example: cargo build --features ollama"
);

// OCR backend: at least 'mock' or 'olmocr' must be enabled
#[cfg(not(any(feature = "mock", feature = "olmocr")))]
compile_error!(
    "At least one OCR backend must be enabled. \
     Enable either the 'mock' (for testing) or 'olmocr' (for production) feature. \
     Example: cargo build --features olmocr"
);

mod config;
mod server;

use std::process;

use anyhow::Context;
use clap::Parser;
use nvisy_server::handler::{CustomRoutes, routes};
use nvisy_server::service::ServiceState;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::{CliConfig, ServerConfig, log_server_config};

// Tracing target constants
pub const TRACING_TARGET_SERVER_STARTUP: &str = "nvisy_cli::server::startup";
pub const TRACING_TARGET_SERVER_SHUTDOWN: &str = "nvisy_cli::server::shutdown";
pub const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";

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
}

impl Cli {
    /// Validates the CLI configuration
    fn validate(&self) -> anyhow::Result<()> {
        self.server
            .validate()
            .context("Invalid server configuration")?;
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

    // Convert CLI config to server configs
    let (service_config, cors_config, openapi_config) = cli.service.into_server_configs();

    // Log service configuration details
    tracing::info!(
        target: TRACING_TARGET_CONFIG,
        cors_origins = ?cors_config.allowed_origins,
        cors_credentials = cors_config.allow_credentials,
        openapi_json_path = %openapi_config.open_api_json,
        scalar_path = %openapi_config.scalar_ui,
        "service configuration loaded successfully"
    );

    // Create AI services based on enabled features
    #[cfg(feature = "mock")]
    let ai_services = nvisy_test::create_mock_services();

    // #[cfg(feature = "ollama")]
    // let emb_service: BoxedEmbeddingProvider = Box::new(nvisy_ollama::EmbeddingService::default());
    // #[cfg(all(feature = "mock", not(feature = "ollama")))]
    // let emb: BoxedEmbeddingProvider = Box::new(MockEmbeddingProvider::default());

    #[cfg(all(feature = "ollama", feature = "olmocr", not(feature = "mock")))]
    let ai_services = {
        let emb_service = nvisy_ollama::EmbeddingService::default();
        let vlm_service = nvisy_ollama::VlmService::default();
        let ocr_service = nvisy_olmocr2::OcrService::default();
        nvisy_core::AiServices::new(emb_service, vlm_service, ocr_service)
    };

    // Create service state
    let state = ServiceState::from_config(service_config, ai_services)
        .await
        .context("Failed to create service state")?;

    // Create routes
    let router = routes(CustomRoutes::new(), state.clone()).with_state(state);

    // Start the server
    server::serve(router.into(), cli.server).await?;

    Ok(())
}

/// Initializes tracing with environment-based filtering.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
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
        cfg!(feature = "otel").then_some("otel"),
        cfg!(feature = "mock").then_some("mock"),
        cfg!(feature = "ollama").then_some("ollama"),
        cfg!(feature = "olmocr").then_some("olmocr"),
    ]
    .into_iter()
    .flatten()
    .collect()
}
