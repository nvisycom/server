#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod config;
mod server;

use std::process;

use axum::Router;
use nvisy_server::handler::{CustomRoutes, routes};
use nvisy_server::middleware::{
    RouterObservabilityExt, RouterOpenApiExt, RouterRecoveryExt, RouterSecurityExt,
};
use nvisy_server::service::ServiceState;
use nvisy_worker::{WorkerHandles, WorkerState};

use crate::config::{Cli, MiddlewareConfig};

/// Tracing target for server startup events.
pub const TRACING_TARGET_SERVER_STARTUP: &str = "nvisy_cli::server::startup";

/// Tracing target for server shutdown events.
pub const TRACING_TARGET_SERVER_SHUTDOWN: &str = "nvisy_cli::server::shutdown";

/// Tracing target for configuration events.
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

    cli.log();

    // Create services
    let webhook = cli.webhook_service();

    // Initialize application state
    let state = ServiceState::from_config(cli.service.clone(), webhook).await?;

    // Create worker state and spawn background workers
    let worker_state = WorkerState::new(state.postgres.clone(), state.nats.clone());
    let workers = WorkerHandles::spawn(&worker_state);
    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        "Document processing workers started"
    );

    // Build router
    let router = create_router(state, &cli.middleware);

    // Run the HTTP server
    let result = server::serve(router, cli.server).await;

    // Shutdown workers
    tracing::info!(
        target: TRACING_TARGET_SERVER_SHUTDOWN,
        "Stopping document processing workers"
    );
    workers.shutdown();

    result?;
    Ok(())
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
