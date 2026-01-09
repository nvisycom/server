//! Server lifecycle management.
//!
//! Provides server startup, shutdown, and health monitoring with
//! comprehensive error handling and observability.

use std::future::Future;
use std::io;
use std::time::Instant;

use crate::config::ServerConfig;
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
) -> io::Result<()>
where
    F: Future<Output = io::Result<()>>,
{
    let start_time = Instant::now();

    log_security_warnings(server_config);
    log_config_details(server_config);

    let result = serve_fn().await;

    handle_result(result, start_time)
}

/// Logs security warnings for potentially unsafe configurations.
fn log_security_warnings(config: &ServerConfig) {
    if config.binds_to_all_interfaces() {
        tracing::warn!(
            target: TRACING_TARGET_SERVER_STARTUP,
            "Server bound to all interfaces (0.0.0.0) - ensure firewall is configured"
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
        "Server configuration"
    );
}

/// Handles the server result and logs appropriate messages.
fn handle_result(result: io::Result<()>, start_time: Instant) -> io::Result<()> {
    let uptime = start_time.elapsed();

    match result {
        Ok(()) => {
            tracing::info!(
                target: TRACING_TARGET_SERVER_SHUTDOWN,
                uptime_secs = uptime.as_secs(),
                "Shutdown completed"
            );
            Ok(())
        }
        Err(err) => {
            tracing::error!(
                target: TRACING_TARGET_SERVER_SHUTDOWN,
                error = %err,
                kind = ?err.kind(),
                uptime_secs = uptime.as_secs(),
                "Fatal error"
            );

            if let Some(suggestion) = error_suggestion(&err) {
                tracing::info!(
                    target: TRACING_TARGET_SERVER_SHUTDOWN,
                    suggestion = suggestion,
                    "Recovery suggestion"
                );
            }

            Err(err)
        }
    }
}

/// Provides a human-readable suggestion for resolving an IO error.
fn error_suggestion(err: &io::Error) -> Option<&'static str> {
    match err.kind() {
        io::ErrorKind::PermissionDenied => {
            Some("Try using a port above 1024 or run with appropriate privileges")
        }
        io::ErrorKind::AddrInUse => {
            Some("The port is already in use. Try a different port or stop the conflicting service")
        }
        io::ErrorKind::AddrNotAvailable => {
            Some("The address is not available. Check network interface configuration")
        }
        io::ErrorKind::NotFound => Some("Check that the required files exist"),
        io::ErrorKind::InvalidData => {
            Some("Check that certificate files are in correct PEM format")
        }
        _ => None,
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

        assert!(result.is_err());
    }
}
