//! System health monitoring and status check handlers.
//!
//! This module provides endpoints for monitoring the health and status of the
//! API server and its dependencies. It includes both public health checks and
//! authenticated detailed status information with simple caching.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use jiff::Timestamp;
use nvisy_core::ServiceStatus;

use super::request::CheckHealth;
use super::response::MonitorStatus;
use crate::extract::{AuthState, Json, Version};
use crate::handler::Result;
use crate::service::{HealthCache, ServiceState};

/// Tracing target for monitor operations.
const TRACING_TARGET: &str = "nvisy_server::handler::monitors";

/// Returns system health status.
///
/// This endpoint provides health information about the API server and its
/// dependencies. The response includes the current status, timestamp, and
/// application version.
///
/// # Behavior
///
/// - **Unauthenticated requests**: Always return cached health status for performance
/// - **Authenticated requests**: Perform real-time health check unless `use_cache` is true
/// - **Administrator requests**: Can force real-time checks even when caching is preferred
///
/// # Response Codes
///
/// - `200 OK` - System is healthy
/// - `503 Service Unavailable` - System is unhealthy
#[tracing::instrument(
    skip_all,
    fields(
        authenticated = auth_state.is_some(),
        is_administrator = auth_state.as_ref().map(|a| a.is_administrator).unwrap_or(false),
        account_id = auth_state.as_ref().map(|a| a.account_id.to_string()),
    )
)]
async fn health_status(
    State(service_state): State<ServiceState>,
    State(health_service): State<HealthCache>,
    auth_state: Option<AuthState>,
    version: Version,
    request: Option<Json<CheckHealth>>,
) -> Result<(StatusCode, Json<MonitorStatus>)> {
    let Json(request) = request.unwrap_or_default();

    let is_authenticated = auth_state.is_some();
    let is_administrator = auth_state
        .as_ref()
        .is_some_and(|auth| auth.is_administrator);
    let account_id = auth_state.as_ref().map(|auth| auth.account_id);

    tracing::debug!(
        target: TRACING_TARGET,
        ?account_id,
        is_authenticated,
        is_administrator,
        version = %version,
        use_cache = request.use_cache,
        timeout_ms = request.timeout.unwrap_or(5000),
        "Health status check requested"
    );

    // Determine whether to use cached or real-time health check
    // - Unauthenticated: always use cache (fast response for load balancers)
    // - Authenticated non-admin: use cache if explicitly requested, otherwise real-time
    // - Administrator: real-time check unless explicitly cached
    let use_cached = if !is_authenticated {
        true
    } else {
        request.use_cache.unwrap_or(false)
    };

    let is_healthy = if use_cached {
        tracing::trace!(
            target: TRACING_TARGET,
            "Using cached health status"
        );
        health_service.get_cached_health()
    } else {
        tracing::trace!(
            target: TRACING_TARGET,
            "Performing real-time health check"
        );
        health_service.is_healthy(&service_state).await
    };

    let status = if is_healthy {
        ServiceStatus::Healthy
    } else {
        ServiceStatus::Unhealthy
    };

    let status_code = if is_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = MonitorStatus {
        checked_at: Timestamp::now(),
        status,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    tracing::info!(
        target: TRACING_TARGET,
        is_healthy,
        used_cache = use_cached,
        "Health status response"
    );

    Ok((status_code, Json(response)))
}

fn health_status_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Health status")
        .description("Returns system health status. Unauthenticated requests use cache; authenticated requests perform real-time checks.")
        .response::<200, Json<MonitorStatus>>()
        .response::<503, Json<MonitorStatus>>()
}

/// Returns a [`Router`] with all health monitoring routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/health", get_with(health_status, health_status_docs))
        .with_path_items(|item| item.tag("Health"))
}
