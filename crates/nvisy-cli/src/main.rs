#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod config;
mod server;

use std::process;

use axum::Router;
use nvisy_reqwest::{WebhookClient, WebhookClientConfig};
use nvisy_server::handler::{CustomRoutes, routes};
use nvisy_server::middleware::{
    RouterObservabilityExt, RouterOpenApiExt, RouterRecoveryExt, RouterSecurityExt,
};
use nvisy_server::service::{ServiceConfig, ServiceState};
use nvisy_service::webhook::WebhookService;

use crate::config::{Cli, MiddlewareConfig, create_services};

// Tracing target constants
pub const TRACING_TARGET_SERVER_STARTUP: &str = "nvisy_cli::server::startup";
pub const TRACING_TARGET_SERVER_SHUTDOWN: &str = "nvisy_cli::server::shutdown";
pub const TRACING_TARGET_CONFIG: &str = "nvisy_cli::config";

#[tokio::main]
async fn main() {
    let Err(error) = run().await else {
        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(
            target: TRACING_TARGET_SERVER_SHUTDOWN,
            error = %error,
            "Application terminated with error"
        );
    } else {
        eprintln!("Error: {error:#}");
    }

    process::exit(1);
}

/// Main application entry point.
async fn run() -> anyhow::Result<()> {
    let cli = Cli::init();
    Cli::init_tracing();

    cli.validate()?;
    cli.log();

    let services = create_services(&cli)?;
    let state = create_service_state(&cli.service, services).await?;
    let router = create_router(state, &cli.middleware);

    server::serve(router, cli.server).await?;

    Ok(())
}

/// Creates the service state from configuration.
async fn create_service_state(
    config: &ServiceConfig,
    inference: nvisy_service::InferenceService,
) -> anyhow::Result<ServiceState> {
    let webhook_service = create_webhook_service()?;
    let state = ServiceState::new(config.clone(), inference, webhook_service).await?;
    Ok(state)
}

/// Creates the webhook service.
fn create_webhook_service() -> anyhow::Result<WebhookService> {
    let webhook_config = WebhookClientConfig::default();
    let webhook_client = WebhookClient::new(webhook_config)?;
    Ok(webhook_client.into_service())
}

/// Creates the router with all middleware layers applied.
fn create_router(state: ServiceState, middleware: &MiddlewareConfig) -> Router {
    let api_routes = routes(CustomRoutes::new(), state.clone()).with_state(state);

    api_routes
        .with_open_api(&middleware.openapi)
        .with_metrics()
        .with_security(&middleware.cors, &Default::default())
        .with_observability()
        .with_recovery(&middleware.recovery)
}
