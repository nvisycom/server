//! Observability middleware for monitoring, tracing, and metrics.
//!
//! This module provides middleware for comprehensive request observability
//! including distributed tracing with unique request IDs, structured logging,
//! and request metrics with performance monitoring.

use std::time::Instant;

use axum::Router;
use axum::extract::Request;
use axum::http::header;
use axum::middleware::{Next, from_fn};
use axum::response::Response;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;

use super::RouteCategory;

/// Tracing target for request metrics.
const TRACING_TARGET_METRICS: &str = "nvisy_server::metrics";

/// Tracing target for the per-request span.
const TRACING_TARGET_REQUEST: &str = "nvisy_server::request";

/// Builds the per-request tracing span carrying the method and path.
///
/// Every event logged while handling the request inherits these fields, so
/// error responses (4xx/5xx) are always attributable to a route.
fn request_span(request: &Request) -> tracing::Span {
    tracing::info_span!(
        target: TRACING_TARGET_REQUEST,
        "request",
        method = %request.method(),
        path = %request.uri().path(),
    )
}

/// Extension trait for `axum::`[`Router`] to apply observability middleware.
///
/// This trait provides convenient methods to add observability features
/// including request tracing, unique request IDs, and metrics collection.
pub trait RouterObservabilityExt<S> {
    /// Layers observability middleware for request tracing and logging.
    ///
    /// This middleware stack generates unique request IDs, adds structured
    /// logging spans for each request, propagates request IDs to responses,
    /// and marks sensitive headers for redaction in logs.
    fn with_observability(self) -> Self;

    /// Layers metrics middleware for request tracking and performance monitoring.
    ///
    /// This middleware tracks request counts by category, response times,
    /// request/response body sizes, and client IP addresses.
    fn with_metrics(self) -> Self;
}

impl<S> RouterObservabilityExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_observability(self) -> Self {
        self.layer(PropagateRequestIdLayer::new(
            header::HeaderName::from_static("x-request-id"),
        ))
        .layer(SetSensitiveRequestHeadersLayer::new([
            header::AUTHORIZATION,
            header::COOKIE,
        ]))
        .layer(TraceLayer::new_for_http().make_span_with(request_span))
        .layer(SetRequestIdLayer::new(
            header::HeaderName::from_static("x-request-id"),
            MakeRequestUuid,
        ))
    }

    fn with_metrics(self) -> Self {
        self.layer(ServiceBuilder::new().layer(from_fn(track_categorized_metrics)))
    }
}

/// Request metrics middleware with categorization and timing.
pub async fn track_categorized_metrics(request: Request, next: Next) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let category = RouteCategory::from_uri(&uri);

    let request_size = request
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    tracing::trace!(
        target: TRACING_TARGET_METRICS,
        method = %method,
        uri = %uri,
        category = category.as_str(),
        request_size = request_size,
        "request started"
    );

    let response = next.run(request).await;
    let duration = start_time.elapsed();

    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    tracing::trace!(
        target: TRACING_TARGET_METRICS,
        method = %method,
        uri = %uri,
        category = category.as_str(),
        status = %response.status(),
        duration_ms = duration.as_millis() as u64,
        request_size = request_size,
        response_size = response_size,
        "request completed"
    );

    response
}
