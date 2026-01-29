//! Recovery middleware for handling errors, panics, and timeouts.
//!
//! This module provides middleware for recovering from various error conditions
//! in the request/response lifecycle, ensuring graceful degradation and proper
//! error responses to clients.

use std::any::Any;
use std::future::ready;
use std::time::Duration;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::response::{IntoResponse, Response};
#[cfg(feature = "config")]
use clap::Args;
use futures::future::{BoxFuture, FutureExt};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::catch_panic::CatchPanicLayer;

use crate::handler::{Error, ErrorKind};

/// Tracing target for error recovery.
const TRACING_TARGET_ERROR: &str = "nvisy_server::recovery::error";

/// Tracing target for panic recovery.
const TRACING_TARGET_PANIC: &str = "nvisy_server::recovery::panic";

type ResponseFut = BoxFuture<'static, Response>;
type Panic = Box<dyn Any + Send + 'static>;

/// Configuration for recovery middleware behavior.
///
/// This struct controls how the recovery middleware handles various
/// error conditions including timeouts and panic recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
#[must_use = "config does nothing unless you use it"]
pub struct RecoveryConfig {
    /// Maximum duration in seconds to wait for a request to complete before timing out.
    /// Requests exceeding this duration receive a 500 response with a timeout message.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "REQUEST_TIMEOUT", default_value = "30")
    )]
    pub request_timeout: u64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            request_timeout: 30,
        }
    }
}

impl RecoveryConfig {
    /// Creates a new configuration with the specified request timeout in seconds.
    pub fn with_timeout_secs(secs: u64) -> Self {
        Self {
            request_timeout: secs,
        }
    }

    /// Returns the request timeout as a Duration.
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout)
    }
}

/// Extension trait for `axum::`[`Router`] to apply recovery middleware.
///
/// This trait provides convenient methods to add error recovery capabilities
/// to your Axum router, protecting against panics and enforcing timeouts.
pub trait RouterRecoveryExt<S> {
    /// Layers recovery middleware with the provided configuration.
    ///
    /// This middleware stack handles request timeouts, panics in handlers,
    /// and Tower service errors, converting them to appropriate HTTP responses.
    fn with_recovery(self, config: &RecoveryConfig) -> Self;

    /// Layers recovery middleware with default configuration.
    ///
    /// Uses a 30-second timeout suitable for most production environments.
    fn with_default_recovery(self) -> Self;
}

impl<S> RouterRecoveryExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_recovery(self, config: &RecoveryConfig) -> Self {
        let middlewares = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_error))
            .layer(CatchPanicLayer::custom(catch_panic))
            .layer(TimeoutLayer::new(config.request_timeout()));

        self.layer(middlewares)
    }

    fn with_default_recovery(self) -> Self {
        self.with_recovery(&RecoveryConfig::default())
    }
}

fn handle_error(err: tower::BoxError) -> ResponseFut {
    use axum_client_ip::Rejection as IpRejection;
    use tower::timeout::error::Elapsed;

    let error = if let Some(_elapsed) = err.downcast_ref::<Elapsed>() {
        tracing::error!(
            target: TRACING_TARGET_ERROR,
            error = %err,
            "request timeout exceeded"
        );

        Error::new(ErrorKind::InternalServerError)
            .with_message("Request timeout")
            .with_context("The request took too long to process and was terminated")
    } else if let Some(_ip_rejection) = err.downcast_ref::<IpRejection>() {
        tracing::error!(
            target: TRACING_TARGET_ERROR,
            error = %err,
            "failed to extract client IP address"
        );

        Error::new(ErrorKind::InternalServerError)
            .with_message("IP address extraction failed")
            .with_context("Could not determine client IP address")
    } else {
        tracing::error!(
            target: TRACING_TARGET_ERROR,
            error = %err,
            "unknown middleware error"
        );

        Error::new(ErrorKind::InternalServerError)
            .with_message("An unexpected error occurred")
            .with_context(err.to_string())
    };

    ready(error.into_response()).boxed()
}

fn catch_panic(err: Panic) -> Response {
    // If the panic is an Error, return it directly.
    if let Some(error) = err.downcast_ref::<Error>() {
        tracing::error!(
            target: TRACING_TARGET_PANIC,
            error = %error,
            "service panic"
        );
        return error.clone().into_response();
    }

    let message = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic type");

    tracing::error!(
        target: TRACING_TARGET_PANIC,
        message = %message,
        "service panic"
    );

    Error::new(ErrorKind::InternalServerError)
        .with_message("An unexpected panic occurred")
        .into_response()
}
