//! HTTP server startup and lifecycle management.

use std::future::Future;
use std::io;
use std::net::SocketAddr;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::ServerConfig;
use crate::server::{Result, ServerError, shutdown_signal};

/// Common server startup logic with graceful shutdown handling.
///
/// This function encapsulates the common pattern of:
/// 1. Logging server readiness
/// 2. Warning about security implications
/// 3. Running the server with error handling
/// 4. Logging shutdown status
pub(crate) async fn serve_with_shutdown<F>(
    server_config: &ServerConfig,
    serve_fn: impl FnOnce() -> F,
) -> Result<()>
where
    F: Future<Output = io::Result<()>>,
{
    tracing::info!(
        target: "server::startup",
        addr = %server_config.server_addr(),
        "Server is ready and listening for connections"
    );

    if server_config.binds_to_all_interfaces() {
        tracing::warn!(
            target: "server::startup",
            "Server is bound to all interfaces. Ensure firewall rules are properly configured."
        );
    }

    serve_fn().await.map_err(|err| {
        tracing::error!(
            target: "server::shutdown",
            error = %err,
            "Server encountered an error"
        );
        ServerError::Runtime(err)
    })?;

    tracing::info!(target: "server::shutdown", "Server shut down gracefully");
    Ok(())
}

/// Starts an HTTP server with graceful shutdown.
///
/// This function validates the configuration, binds to the specified address,
/// and starts serving requests with support for graceful shutdown.
///
/// # Arguments
///
/// * `app` - The Axum router to serve
/// * `server_config` - Server configuration including host, port, and timeouts
///
/// # Errors
///
/// Returns an error if:
/// - Server configuration is invalid
/// - Cannot bind to the specified address/port
/// - Server encounters a fatal error during operation
///
/// # Examples
///
/// ```no_run
/// use axum::Router;
/// use nvisy_cli::config::ServerConfig;
/// use nvisy_cli::server::serve_http;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let app = Router::new();
/// let config = ServerConfig::default();
///
/// serve_http(app, config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn serve_http(app: Router, server_config: ServerConfig) -> Result<()> {
    // Validate configuration before starting
    if let Err(validation_error) = server_config.validate() {
        tracing::error!(
            target: "server::startup",
            error = validation_error.to_string(),
            "Invalid server configuration"
        );

        return Err(ServerError::InvalidConfig(validation_error.to_string()));
    }

    // Log server configuration
    tracing::info!(
        target: "server::startup",
        host = server_config.host.to_string(),
        port = server_config.port,
        request_timeout_sec = server_config.request_timeout,
        shutdown_timeout_sec = server_config.shutdown_timeout,
        development_mode = server_config.is_development(),
        binds_to_all_interfaces = server_config.binds_to_all_interfaces(),
        "Server configuration loaded"
    );

    let server_addr = server_config.server_addr();

    // Bind to the address with error handling
    let listener = match TcpListener::bind(server_addr).await {
        Ok(listener) => {
            tracing::info!(
                target: "server::startup",
                addr = server_addr.to_string(),
                "Successfully bound to address"
            );

            listener
        }
        Err(listener_err) => {
            tracing::error!(
                target: "server::startup",
                addr = server_addr.to_string(),
                error = listener_err.to_string(),
                "Failed to bind to address"
            );

            return Err(ServerError::BindError {
                address: server_addr.to_string(),
                source: listener_err,
            });
        }
    };

    // Start server
    let shutdown_signal = shutdown_signal(server_config.shutdown_timeout());
    serve_with_shutdown(&server_config, || async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal)
        .await
    })
    .await
}
