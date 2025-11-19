//! System health monitoring and status check handlers.
//!
//! This module provides endpoints for monitoring the health and status of the
//! API server and its dependencies. It includes both public health checks and
//! authenticated detailed status information with simple caching.

use axum::extract::State;
use axum::http::StatusCode;
use time::OffsetDateTime;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::request::CheckHealth;
use super::response::MonitorStatus;
use crate::extract::{AuthState, Json, Version};
use crate::handler::Result;
use crate::service::{HealthCache, ServiceState};

/// Tracing target for monitor operations.
const TRACING_TARGET: &str = "nvisy_server::handler::monitors";

/// Returns system health status.
#[tracing::instrument(skip_all, fields(authenticated = auth_state.is_some()))]
#[utoipa::path(
    post, path = "/health", tag = "monitors",
    request_body(
        content = Option<CheckHealth>,
        description = "Optional health status request parameters",
        content_type = "application/json"
    ),
    responses(
        (
            status = 200,
            description = "System is healthy",
            body = MonitorStatus,
        ),
        (
            status = 503,
            description = "System is unhealthy",
            body = MonitorStatus,
        ),
    ),
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

    tracing::debug!(
        target: TRACING_TARGET,
        authenticated = is_authenticated,
        version = %version,
        "health status check requested"
    );

    // Get cached health status or perform new check
    let explicitly_cached = request.use_cache.is_some_and(|c| c);
    let is_healthy = if is_authenticated && !explicitly_cached {
        health_service.is_healthy(service_state).await
    } else {
        health_service.get_cached_health()
    };

    let response = MonitorStatus {
        updated_at: OffsetDateTime::now_utc(),
        is_healthy,
        overall_status: if is_healthy {
            super::response::SystemStatus::Healthy
        } else {
            super::response::SystemStatus::Critical
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: 0, // TODO: Implement actual uptime tracking
        services: None,
        metrics: None,
        alerts: None,
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
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(health_status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn test_health_status_endpoint_unauthenticated() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = CheckHealth {
            timeout: None,
            use_cache: None,
        };

        let response = server.post("/health").json(&request).await;
        response.assert_status_success();

        let status_response = response.json::<MonitorStatus>();

        // Unauthenticated requests should return healthy (basic check)
        assert!(status_response.is_healthy);
        assert!(!status_response.updated_at.to_string().is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_health_status_endpoint_with_prefer_policy() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = CheckHealth {
            timeout: None,
            use_cache: Some(false),
        };

        let response = server.post("/health").json(&request).await;
        response.assert_status_success();

        let status_response = response.json::<MonitorStatus>();

        // Should still work without authentication
        assert!(status_response.is_healthy);
        assert!(!status_response.updated_at.to_string().is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_health_status_endpoint_empty_request() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        // Test with no request body
        let response = server.post("/health").await;
        response.assert_status_success();

        let status_response = response.json::<MonitorStatus>();
        assert!(status_response.is_healthy);

        Ok(())
    }

    // Note: Tests for authenticated requests would require setting up proper
    // authentication in the test server, which should be implemented when
    // the auth system testing infrastructure is available.
}
