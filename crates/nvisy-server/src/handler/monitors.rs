//! System health monitoring and status check handlers.
//!
//! This module provides endpoints for monitoring the health and status of the
//! API server and its dependencies. It includes both public health checks and
//! authenticated detailed status information with simple caching.

use aide::axum::ApiRouter;
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
#[tracing::instrument(skip_all, fields(authenticated = auth_state.is_some()))]
async fn health_status(
    State(service_state): State<ServiceState>,
    State(health_service): State<HealthCache>,
    auth_state: Option<AuthState>,
    version: Version,
    request: Option<Json<CheckHealth>>,
) -> Result<(StatusCode, Json<MonitorStatus>)> {
    let Json(request) = request.unwrap_or_default();
    let is_authenticated = auth_state.is_some();

    tracing::debug!(
        target: TRACING_TARGET,
        authenticated = is_authenticated,
        version = %version,
        "health status check requested"
    );

    // Get cached health status or perform new check
    let explicitly_cached = request.use_cache.is_some_and(|c| c);
    let is_healthy = if is_authenticated && !explicitly_cached {
        health_service.is_healthy(&service_state).await
    } else {
        health_service.get_cached_health()
    };

    let status = if is_healthy {
        ServiceStatus::Healthy
    } else {
        ServiceStatus::Unhealthy
    };

    let response = MonitorStatus {
        checked_at: Timestamp::now(),
        status,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let status_code = if is_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    tracing::info!(
        target: TRACING_TARGET,
        authenticated = is_authenticated,
        is_healthy = is_healthy,
        status_code = status_code.as_u16(),
        "health status response prepared"
    );

    Ok((status_code, Json(response)))
}

/// Returns a [`Router`] with all health monitoring routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new().api_route("/health", post(health_status))
}
