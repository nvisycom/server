//! HTTP server implementation using enhanced lifecycle management.

use std::net::SocketAddr;

use axum::Router;
use tokio::net::TcpListener;

use crate::TRACING_TARGET_SERVER_STARTUP;
use crate::config::ServerConfig;
#[cfg(feature = "telemetry")]
use crate::server::serve_with_shutdown_and_telemetry;
use crate::server::{ServerResult, serve_with_shutdown, shutdown_signal};

/// Starts an HTTP server with enhanced lifecycle management.
pub async fn serve_http(app: Router, server_config: ServerConfig) -> ServerResult<()> {
    let service_name = "http-server";
    let server_addr = server_config.server_addr();
    let shutdown_timeout = server_config.shutdown_timeout();
    let shutdown_signal = shutdown_signal(shutdown_timeout);

    serve_with_shutdown(&server_config, service_name, move || async move {
        let listener = TcpListener::bind(server_addr).await?;

        tracing::info!(
            target: TRACING_TARGET_SERVER_STARTUP,
            addr = %server_addr,
            "HTTP server bound and ready"
        );

        let app = app.into_make_service_with_connect_info::<SocketAddr>();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
    })
    .await
}

/// Starts an HTTP server with telemetry support.
#[cfg(feature = "telemetry")]
pub async fn serve_http_with_telemetry(
    app: Router,
    server_config: ServerConfig,
    telemetry_context: Option<&crate::telemetry::TelemetryContext>,
) -> ServerResult<()> {
    let service_name = "http-server";
    let server_addr = server_config.server_addr();
    let shutdown_timeout = server_config.shutdown_timeout();
    let shutdown_signal = shutdown_signal(shutdown_timeout);

    serve_with_shutdown_and_telemetry(
        &server_config,
        service_name,
        telemetry_context,
        move || async move {
            let listener = TcpListener::bind(server_addr).await?;

            tracing::info!(
                target: TRACING_TARGET_SERVER_STARTUP,
                addr = %server_addr,
                telemetry_enabled = telemetry_context.is_some(),
                "HTTP server bound with telemetry support"
            );

            let app = app.into_make_service_with_connect_info::<SocketAddr>();
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal)
                .await
        },
    )
    .await
}

#[cfg(test)]
mod tests {}
