//! HTTP server implementation using enhanced lifecycle management.

use axum::Router;
use nvisy_server::extract::AppConnectInfo;
use tokio::net::TcpListener;

use crate::TRACING_TARGET_SERVER_STARTUP;
use crate::config::ServerConfig;
use crate::server::lifecycle::serve_with_shutdown;
use crate::server::{ServerResult, shutdown_signal};

/// Starts an HTTP server with enhanced lifecycle management.
pub async fn serve_http(app: Router, server_config: ServerConfig) -> ServerResult<()> {
    let server_addr = server_config.socket_addr();
    let shutdown_timeout = server_config.shutdown_timeout();
    let shutdown_signal = shutdown_signal(shutdown_timeout);

    serve_with_shutdown(&server_config, move || async move {
        let listener = TcpListener::bind(server_addr).await?;

        tracing::info!(
            target: TRACING_TARGET_SERVER_STARTUP,
            addr = %server_addr,
            "Server listening"
        );

        let app = app.into_make_service_with_connect_info::<AppConnectInfo>();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
    })
    .await
}

#[cfg(test)]
mod tests {}
