#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Compile-time checks: ensure at least one backend is enabled for each service type.
#[cfg(not(any(feature = "mock", feature = "ollama")))]
compile_error!(
    "At least one embedding/VLM (visual language model) backend must be enabled. \
     Enable either the 'mock' (for testing) or 'ollama' (for production) feature. \
     Example: cargo build --features ollama"
);

// Compile-time checks: ensure at least one backend is enabled for each service type.
#[cfg(not(any(feature = "mock", feature = "olmocr")))]
compile_error!(
    "At least one OCR (optical character recognition) backend must be enabled. \
     Enable either the 'mock' (for testing) or 'olmocr' (for production) feature. \
     Example: cargo build --features olmocr"
);

mod config;
mod server;

use std::process;

use anyhow::Context;
use axum::Router;
use clap::Parser;
use nvisy_server::handler::{CustomRoutes, routes};
use nvisy_server::middleware::{
    RouterObservabilityExt, RouterRecoveryExt, RouterSecurityExt, SecurityHeadersConfig,
};
use nvisy_server::service::ServiceState;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::{Cli, MiddlewareConfig, log_server_config};

// Tracing target constants
pub const TRACING_TARGET_SERVER_STARTUP: &str = "nvisy_cli::server::startup";
pub const TRACING_TARGET_SERVER_SHUTDOWN: &str = "nvisy_cli::server::shutdown";
pub const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";

#[tokio::main]
async fn main() {
    let Err(error) = run().await else {
        tracing::info!(
            target: TRACING_TARGET_SERVER_SHUTDOWN,
            "application terminated successfully"
        );
        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(
            target: TRACING_TARGET_SERVER_SHUTDOWN,
            error = %error,
            "application terminated with error"
        );
    } else {
        eprintln!("Error: {error:#}");
    }

    process::exit(1);
}

/// Main application entry point.
async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    init_tracing();
    log_startup_info();
    log_server_config(&cli.server);

    cli.server
        .validate()
        .context("invalid server configuration")?;

    log_middleware_config(&cli.middleware);

    let state = create_service_state(&cli.service).await?;
    let router = create_router(state, &cli.middleware);

    server::serve(router, cli.server).await?;

    Ok(())
}

/// Creates the service state from configuration.
async fn create_service_state(
    config: &nvisy_server::service::ServiceConfig,
) -> anyhow::Result<ServiceState> {
    #[cfg(feature = "mock")]
    let ai_services = nvisy_test::create_mock_services();

    #[cfg(all(feature = "ollama", feature = "olmocr", not(feature = "mock")))]
    let ai_services = {
        let emb_service = nvisy_ollama::EmbeddingService::default();
        let vlm_service = nvisy_ollama::VlmService::default();
        let ocr_service = nvisy_olmocr2::OcrService::default();
        nvisy_core::AiServices::new(emb_service, vlm_service, ocr_service)
    };

    ServiceState::from_config(config.clone(), ai_services)
        .await
        .context("failed to create service state")
}

/// Creates the router with all middleware layers applied.
///
/// Middleware is applied in reverse order (last added = outermost):
/// 1. Recovery (outermost) - catches panics and enforces timeouts
/// 2. Observability - request IDs and tracing spans
/// 3. Security - CORS, security headers, compression
/// 4. Routes (innermost) - actual request handlers
fn create_router(state: ServiceState, middleware: &MiddlewareConfig) -> Router {
    let api_routes: Router = routes(CustomRoutes::new(), state.clone())
        .with_state(state)
        .into();

    api_routes
        .with_security(middleware.cors.clone(), SecurityHeadersConfig::default())
        .with_observability()
        .with_recovery(middleware.recovery.clone())
}

/// Initializes tracing with environment-based filtering.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Logs startup information.
fn log_startup_info() {
    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        version = env!("CARGO_PKG_VERSION"),
        "starting nvisy server"
    );

    tracing::debug!(
        target: TRACING_TARGET_SERVER_STARTUP,
        pid = process::id(),
        arch = std::env::consts::ARCH,
        os = std::env::consts::OS,
        features = ?enabled_features(),
        "build information"
    );
}

/// Logs middleware configuration.
fn log_middleware_config(config: &MiddlewareConfig) {
    tracing::info!(
        target: TRACING_TARGET_CONFIG,
        cors_origins = ?config.cors.allowed_origins,
        cors_credentials = config.cors.allow_credentials,
        openapi_path = %config.openapi.open_api_json,
        scalar_path = %config.openapi.scalar_ui,
        request_timeout_secs = config.recovery.request_timeout_secs,
        "middleware configuration"
    );
}

/// Returns a list of enabled compile-time features.
fn enabled_features() -> Vec<&'static str> {
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
