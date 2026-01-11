//! Graceful shutdown signal handling.

use std::time::Duration;

use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix;

use super::TRACING_TARGET_SHUTDOWN;

/// Waits for a shutdown signal (SIGTERM or SIGINT/Ctrl+C).
///
/// This function listens for shutdown signals and returns when one is received:
/// - SIGTERM (Unix/Linux)
/// - SIGINT (Ctrl+C on all platforms)
///
/// # Arguments
///
/// * `shutdown_timeout` - Maximum duration to wait for cleanup operations
pub async fn shutdown_signal(shutdown_timeout: Duration) {
    let ctrl_c = async {
        if let Err(e) = ctrl_c().await {
            tracing::error!(
                target: TRACING_TARGET_SHUTDOWN,
                error = %e,
                "Failed to install Ctrl+C handler"
            );
        } else {
            tracing::info!(
                target: TRACING_TARGET_SHUTDOWN,
                "Received Ctrl+C signal, initiating graceful shutdown"
            );
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match unix::signal(unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
                tracing::info!(
                    target: TRACING_TARGET_SHUTDOWN,
                    "Received SIGTERM signal, initiating graceful shutdown"
                );
            }
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET_SHUTDOWN,
                    error = %e,
                    "Failed to install SIGTERM handler"
                );
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!(
        target: TRACING_TARGET_SHUTDOWN,
        timeout_secs = shutdown_timeout.as_secs(),
        "Graceful shutdown initiated"
    );
}
