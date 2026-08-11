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
use tokio_util::sync::CancellationToken;

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
    let router = create_router(state.clone(), &cli.middleware);

    // Create cancellation token for graceful shutdown of workers
    let cancel = CancellationToken::new();

    // Spawn webhook worker (logs lifecycle events internally)
    let webhook_worker = state.webhook_worker();
    let webhook_cancel = cancel.clone();
    let webhook_handle = tokio::spawn(async move {
        let _ = webhook_worker.run(webhook_cancel).await;
    });

    // Spawn connection sync worker (scheduler + job consumer + reaper)
    let sync_worker = state.connection_sync_worker();
    let sync_cancel = cancel.clone();
    let sync_handle = tokio::spawn(async move {
        let _ = sync_worker.run(sync_cancel).await;
    });

    // Spawn data-retention worker (expires stored data per workspace policy)
    let retention_worker = state.retention_worker();
    let retention_cancel = cancel.clone();
    let retention_handle = tokio::spawn(async move {
        let _ = retention_worker.run(retention_cancel).await;
    });

    // Spawn pipeline detection worker (runs each run's analyze off the request thread)
    let detection_worker = state.detection_worker();
    let detection_cancel = cancel.clone();
    let detection_handle = tokio::spawn(async move {
        let _ = detection_worker.run(detection_cancel).await;
    });

    // Run the HTTP server
    let server_result = server::serve(router, cli.server).await;

    // Signal workers to stop
    cancel.cancel();

    // Wait for workers to finish
    if let Err(err) = webhook_handle.await {
        tracing::error!(
            target: TRACING_TARGET_SHUTDOWN,
            error = %err,
            "Webhook worker task panicked"
        );
    }
    if let Err(err) = sync_handle.await {
        tracing::error!(
            target: TRACING_TARGET_SHUTDOWN,
            error = %err,
            "Connection sync worker task panicked"
        );
    }
    if let Err(err) = retention_handle.await {
        tracing::error!(
            target: TRACING_TARGET_SHUTDOWN,
            error = %err,
            "Retention worker task panicked"
        );
    }
    if let Err(err) = detection_handle.await {
        tracing::error!(
            target: TRACING_TARGET_SHUTDOWN,
            error = %err,
            "Detection worker task panicked"
        );
    }

    server_result?;
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
