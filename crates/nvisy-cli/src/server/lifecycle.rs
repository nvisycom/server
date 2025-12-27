//! Server lifecycle management.
//!
//! Provides server startup, shutdown, and health monitoring with
//! comprehensive error handling and observability.

use std::future::Future;
use std::io;
use std::time::Instant;

use crate::config::ServerConfig;
use crate::server::{ServerError, ServerResult};
use crate::{TRACING_TARGET_SERVER_SHUTDOWN, TRACING_TARGET_SERVER_STARTUP};

/// Serves with lifecycle management and graceful shutdown.
///
/// # Arguments
///
/// * `server_config` - Server configuration
/// * `serve_fn` - Function that returns the server future
///
/// # Errors
///
/// Returns detailed errors with recovery suggestions.
pub async fn serve_with_shutdown<F>(
    server_config: &ServerConfig,
    serve_fn: impl FnOnce() -> F,
) -> ServerResult<()>
where
    F: Future<Output = io::Result<()>>,
{
    let start_time = Instant::now();

    log_server_starting(server_config);

    validate_config(server_config)?;
    log_security_warnings(server_config);
    log_config_details(server_config);

    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        addr = %server_config.socket_addr(),
        "server ready and listening"
    );

    let result = serve_fn().await;

    handle_result(result, start_time)
}

/// Logs server starting message.
fn log_server_starting(config: &ServerConfig) {
    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        addr = %config.socket_addr(),
        version = env!("CARGO_PKG_VERSION"),
        "starting server"
    );
}

/// Validates server configuration.
fn validate_config(config: &ServerConfig) -> ServerResult<()> {
    if let Err(e) = config.validate() {
        tracing::error!(
            target: TRACING_TARGET_SERVER_STARTUP,
            error = %e,
            "configuration validation failed"
        );
        return Err(ServerError::invalid_config(&e));
    }
    Ok(())
}

/// Logs security warnings for potentially unsafe configurations.
fn log_security_warnings(config: &ServerConfig) {
    if config.binds_to_all_interfaces() {
        tracing::warn!(
            target: TRACING_TARGET_SERVER_STARTUP,
            "server bound to all interfaces (0.0.0.0) - ensure firewall is configured"
        );
    }
}

/// Logs configuration details.
fn log_config_details(config: &ServerConfig) {
    tracing::debug!(
        target: TRACING_TARGET_SERVER_STARTUP,
        host = %config.host,
        port = config.port,
        shutdown_timeout = config.shutdown_timeout,
        tls = config.is_tls_enabled(),
        "configuration active"
    );
}

/// Handles the server result and logs appropriate messages.
fn handle_result(result: io::Result<()>, start_time: Instant) -> ServerResult<()> {
    let uptime = start_time.elapsed();

    match result {
        Ok(()) => {
            tracing::info!(
                target: TRACING_TARGET_SERVER_SHUTDOWN,
                uptime_secs = uptime.as_secs(),
                "shutdown completed"
            );
            Ok(())
        }
        Err(err) => {
            let server_error = ServerError::Runtime(err);

            tracing::error!(
                target: TRACING_TARGET_SERVER_SHUTDOWN,
                error = %server_error,
                uptime_secs = uptime.as_secs(),
                recoverable = server_error.is_recoverable(),
                "fatal error"
            );

            if let Some(suggestion) = server_error.suggestion() {
                tracing::info!(
                    target: TRACING_TARGET_SERVER_SHUTDOWN,
                    suggestion = suggestion,
                    "recovery suggestion"
                );
            }

            log_error_context(&server_error);

            Err(server_error)
        }
    }
}

/// Logs error context for debugging.
fn log_error_context(error: &ServerError) {
    for (key, value) in error.context() {
        tracing::debug!(
            target: TRACING_TARGET_SERVER_SHUTDOWN,
            key = key,
            value = value,
            "error context"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serve_with_shutdown_success() {
        let config = ServerConfig::default();
        let result = serve_with_shutdown(&config, || async { Ok(()) }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn serve_with_shutdown_handles_error() {
        let config = ServerConfig::default();
        let result =
            serve_with_shutdown(&config, || async { Err(io::Error::other("test error")) }).await;

        assert!(matches!(result, Err(ServerError::Runtime(_))));
    }

    #[tokio::test]
    async fn serve_with_shutdown_validates_config() {
        let config = ServerConfig {
            port: 80, // Invalid for non-root
            ..Default::default()
        };

        let result = serve_with_shutdown(&config, || async { Ok(()) }).await;
        assert!(matches!(result, Err(ServerError::InvalidConfig(_))));
    }
}
