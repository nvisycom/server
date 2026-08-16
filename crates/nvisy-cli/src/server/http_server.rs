//! HTTP server implementation using enhanced lifecycle management.

use std::io;
use std::time::Duration;

use axum::Router;
use nvisy_server::extract::AppConnectInfo;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use super::{TRACING_TARGET_SHUTDOWN, TRACING_TARGET_STARTUP};
use crate::config::ServerConfig;
use crate::server::lifecycle::serve_with_shutdown;
use crate::server::shutdown_signal;

/// Starts an HTTP server with enhanced lifecycle management.
///
/// On a shutdown signal the `shutdown` token is cancelled so long-lived handlers
/// (SSE streams) end promptly, then axum drains in-flight connections. The drain
/// is bounded by `shutdown_timeout`: a connection that does not close in time is
/// dropped rather than blocking process exit.
pub async fn serve_http(
    app: Router,
    server_config: ServerConfig,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let server_addr = server_config.socket_addr();
    let shutdown_timeout = server_config.shutdown_timeout();

    serve_with_shutdown(&server_config, move || async move {
        let listener = TcpListener::bind(server_addr).await?;

        tracing::info!(
            target: TRACING_TARGET_STARTUP,
            addr = %server_addr,
            "Server listening"
        );

        // Wait for the OS signal, then cancel the shared token so open-ended
        // handlers stop yielding and their connections can close.
        let graceful = {
            let shutdown = shutdown.clone();
            async move {
                shutdown_signal(shutdown_timeout).await;
                shutdown.cancel();
            }
        };

        let app = app.into_make_service_with_connect_info::<AppConnectInfo>();
        let serve = axum::serve(listener, app).with_graceful_shutdown(graceful);

        // Bound the graceful drain: if a connection has not closed within the
        // timeout after the signal, stop waiting and let the process exit.
        tokio::select! {
            result = serve => result,
            () = wait_then_timeout(&shutdown, shutdown_timeout) => {
                tracing::warn!(
                    target: TRACING_TARGET_SHUTDOWN,
                    timeout_secs = shutdown_timeout.as_secs(),
                    "Graceful shutdown timed out; forcing exit with connections still open",
                );
                Ok(())
            }
        }
    })
    .await
}

/// Completes `timeout` after the `shutdown` token is cancelled — the deadline
/// for the graceful drain. Never completes until shutdown begins, so it does not
/// race the server during normal operation.
async fn wait_then_timeout(shutdown: &CancellationToken, timeout: Duration) {
    shutdown.cancelled().await;
    tokio::time::sleep(timeout).await;
}
