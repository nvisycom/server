//! System health monitoring and status check handlers.
//!
//! This module provides endpoints for monitoring the health and status of the
//! API server and its dependencies. It includes both public health checks and
//! authenticated detailed status information with simple caching.

use axum::extract::{FromRef, State};
use axum::http::StatusCode;
use nvisy_nats::NatsClient;
use nvisy_openrouter::LlmClient;
use nvisy_postgres::PgClient;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::request::monitor::MonitorStatusRequest;
use super::response::monitor::MonitorStatusResponse;
use crate::extract::{AuthState, Json, Version};
use crate::handler::Result;
use crate::service::{DataCollectionPolicy, HealthService, ServiceState};

/// Tracing target for monitor operations.
const TRACING_TARGET: &str = "nvisy_server::handler::monitors";

#[tracing::instrument(skip_all, fields(authenticated = auth_state.is_some()))]
#[utoipa::path(
    post,
    path = "/health",
    tag = "health",
    summary = "Get system health status",
    request_body(
        content = Option<MonitorStatusRequest>,
        description = "Optional health status request parameters",
        content_type = "application/json"
    ),
    responses(
        (
            status = 200,
            description = "System is healthy",
            body = MonitorStatusResponse,
        ),
        (
            status = 503,
            description = "System is unhealthy",
            body = MonitorStatusResponse,
        ),
    ),
)]
async fn health_status(
    State(service_state): State<ServiceState>,
    State(data_collection_policy): State<DataCollectionPolicy>,
    State(health_service): State<HealthService>,
    auth_state: Option<AuthState>,
    version: Version,
    request: Option<Json<MonitorStatusRequest>>,
) -> Result<(StatusCode, Json<MonitorStatusResponse>)> {
    let Json(_request) = request.unwrap_or_default();
    let is_authenticated = auth_state.is_some();

    tracing::debug!(
        target: TRACING_TARGET,
        authenticated = is_authenticated,
        version = %version,
        "Health status check requested"
    );

    // Get cached health status or perform new check
    let is_healthy = if is_authenticated {
        // For authenticated requests, perform detailed health checks
        // Extract individual clients from service state
        let pg_client = PgClient::from_ref(&service_state);
        let nats_client = NatsClient::from_ref(&service_state);
        let llm_client = LlmClient::from_ref(&service_state);

        health_service
            .is_healthy(&pg_client, &nats_client, &llm_client)
            .await
    } else {
        // For unauthenticated requests, just return a basic healthy status
        // This could be enhanced to do a minimal check if needed
        true
    };

    let response = MonitorStatusResponse {
        data_collection: data_collection_policy,
        updated_at: time::OffsetDateTime::now_utc(),
        is_healthy,
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
        "Health status response prepared"
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
    use crate::service::DataCollectionPolicy;

    #[tokio::test]
    async fn test_health_status_endpoint_unauthenticated() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = MonitorStatusRequest {
            data_collection: None,
        };

        let response = server.post("/health").json(&request).await;
        response.assert_status_success();

        let status_response = response.json::<MonitorStatusResponse>();

        // Unauthenticated requests should return healthy (basic check)
        assert!(status_response.is_healthy);
        assert!(!status_response.updated_at.to_string().is_empty());

        // Should have data collection policy
        assert!(matches!(
            status_response.data_collection,
            DataCollectionPolicy::NormalDataCollection
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_health_status_endpoint_with_prefer_policy() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = MonitorStatusRequest {
            data_collection: Some(DataCollectionPolicy::NormalDataCollection),
        };

        let response = server.post("/health").json(&request).await;
        response.assert_status_success();

        let status_response = response.json::<MonitorStatusResponse>();

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

        let status_response = response.json::<MonitorStatusResponse>();
        assert!(status_response.is_healthy);

        Ok(())
    }

    #[tokio::test]
    async fn test_health_endpoint_response_format() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let response = server.post("/health").await;
        response.assert_status_success();

        let status_response = response.json::<MonitorStatusResponse>();

        // Verify timestamp is recent (within last minute)
        let now = time::OffsetDateTime::now_utc();
        let response_time = status_response.updated_at;
        let time_diff = now - response_time;

        assert!(
            time_diff.whole_seconds() < 60,
            "Response timestamp should be recent"
        );

        assert!(status_response.is_healthy); // Should be healthy for basic checks

        Ok(())
    }

    // Note: Tests for authenticated requests would require setting up proper
    // authentication in the test server, which should be implemented when
    // the auth system testing infrastructure is available.
}
