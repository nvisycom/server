#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod config;
mod server;

use std::process;

use axum::Router;
use nvisy_server::handler::{CustomRoutes, routes};
use nvisy_server::middleware::*;
use nvisy_server::service::ServiceState;

use crate::config::{Cli, MiddlewareConfig};
use crate::server::TRACING_TARGET_SHUTDOWN;

#[tokio::main]
async fn main() {
    let Err(error) = run().await else {
        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(
            target: TRACING_TARGET_SHUTDOWN,
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

    // Initialize application state
    let state = cli.service_state().await?;

    // Build router
    let router = create_router(state, &cli.middleware);

    // Run the HTTP server
    server::serve(router, cli.server).await?;
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
