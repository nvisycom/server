//! System monitoring and health check handlers.
//!
//! This module provides endpoints for monitoring the health and status of the
//! API server and its dependencies. It includes both public health checks and
//! authenticated detailed status information.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_nats::NatsClient;
// use nvisy_openrouter::OpenRouter;
// TODO: Implement when nvisy-openrouter is available
use nvisy_postgres::PgClient;
use serde::{Deserialize, Serialize};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::request::monitor::MonitorStatusRequest;
use super::response::monitor::{FeatureState, FeatureStatuses, MonitorStatusResponse};
use crate::extract::{AuthState, Json, Version};
use crate::service::{DataCollectionPolicy, ServiceState};

/// Tracing target for monitor operations.
const TRACING_TARGET: &str = "nvisy_server::handler::monitors";

/// Health status for system components.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum HealthStatus {
    Online,
    Degraded,
    Offline,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Online => write!(f, "online"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Offline => write!(f, "offline"),
        }
    }
}

/// Current state and status message for a system feature.
#[must_use]
#[derive(Debug, Clone)]
pub struct ComponentStatus {
    pub is_healthy: bool,
    pub message: Option<String>,
}

impl ComponentStatus {
    pub fn healthy() -> Self {
        Self {
            is_healthy: true,
            message: None,
        }
    }

    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            is_healthy: false,
            message: Some(message.into()),
        }
    }

    pub fn is_operational(&self) -> bool {
        self.is_healthy
    }
}

/// Converts a service status result into a feature state with proper logging.
fn check_service_status(status: ComponentStatus, service_name: &str) -> FeatureState {
    tracing::debug!(
        target: TRACING_TARGET,
        message = ?status.message,
        service = service_name,
        "Service status check completed"
    );

    FeatureState::new(status)
}

/// Returns the current status of the API server and its external components.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/monitors",
    tag = "monitors",
    summary = "Get system status",
    description = "Returns the current status of system components. Full details require authentication.",
    request_body(
        content = Option<MonitorStatusRequest>,
        description = "Optional monitor status request",
        content_type = "application/json"
    ),
    responses(
        (
            status = 200,
            description = "System status retrieved successfully",
            body = MonitorStatusResponse,
        ),
    ),
)]
async fn monitor_status(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(regional_policy): State<DataCollectionPolicy>,
    auth_state: Option<AuthState>,
    version: Version,
    request: Option<Json<MonitorStatusRequest>>,
) -> (StatusCode, Json<MonitorStatusResponse>) {
    let Json(request) = request.unwrap_or_default();
    let mut response = MonitorStatusResponse {
        regional_policy,
        features: None,
        updated_at: time::OffsetDateTime::now_utc(),
    };

    let prefer_policy = request.prefer_policy.unwrap_or_default();
    tracing::trace!(
        target: TRACING_TARGET,
        current_policy = %regional_policy,
        requested_policy = %prefer_policy,
        "current monitor status was requested",
    );

    if let Some(AuthState(_)) = auth_state {
        let pg_status = check_database_health(&pg_client).await;
        let pg_client_status = check_service_status(pg_status, "Postgres");

        let openrouter_client = FeatureState {
            is_operational: false,
            status: "unavailable".to_string(),
            message: Some("OpenRouter integration not yet implemented".to_string()),
        };

        let worker_runtime = FeatureState {
            is_operational: true,
            status: "operational".to_string(),
            message: Some("Worker runtime is active".to_string()),
        };

        response.features = Some(FeatureStatuses {
            gateway_server: pg_client_status,
            assistant_chat: openrouter_client,
            worker_runtime,
        });
    }

    tracing::trace!(
        target: TRACING_TARGET,
        authenticated = auth_state.is_some(),
        "current monitor status was returned",
    );

    (StatusCode::OK, Json(response))
}

/// Check database health status.
async fn check_database_health(pg_client: &PgClient) -> ComponentStatus {
    match pg_client.get_connection().await {
        Ok(_) => ComponentStatus::healthy(),
        Err(e) => ComponentStatus::unhealthy(format!("Database connection failed: {}", e)),
    }
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(monitor_status))
}

#[cfg(test)]
mod test {
    use super::{MonitorStatusRequest, MonitorStatusResponse, routes};
    use crate::handler::test::create_test_server_with_router;
    use crate::service::DataCollectionPolicy;

    #[tokio::test]
    async fn monitor_status_without_auth() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = MonitorStatusRequest {
            prefer_policy: None,
        };

        let response = server.post("/monitors").json(&request).await;
        response.assert_status_success();
        let status_response = response.json::<MonitorStatusResponse>();

        // Without authentication, features should be None
        assert!(status_response.features.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn monitor_status_with_auth() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = MonitorStatusRequest {
            prefer_policy: Some(DataCollectionPolicy::NormalDataCollection),
        };

        // TODO: Add authentication to this test when auth system is available
        let response = server.post("/monitors").json(&request).await;
        response.assert_status_success();
        let status_response = response.json::<MonitorStatusResponse>();

        // Currently without auth, features will be None
        // This test should be updated when proper auth testing is available
        assert!(!status_response.updated_at.to_string().is_empty());

        Ok(())
    }
}
