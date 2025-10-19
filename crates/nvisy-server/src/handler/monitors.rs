use axum::extract::State;
use axum::http::StatusCode;
use nvisy_openrouter::OpenRouter;
use nvisy_postgres::PgDatabase;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::extract::{AuthState, Json};
use crate::service::{RegionalPolicy, ServiceState};

/// Tracing target for monitor operations.
const TRACING_TARGET: &str = "nvisy::handler::monitors";

/// Health status for system components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Online,
    Degraded,
    Offline,
}

impl ToString for HealthStatus {
    fn to_string(&self) -> String {
        match self {
            HealthStatus::Online => "online".to_string(),
            HealthStatus::Degraded => "degraded".to_string(),
            HealthStatus::Offline => "offline".to_string(),
        }
    }
}

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MonitorStatusRequest {
    /// Preferred regional policy for data collection.
    pub prefer_policy: Option<RegionalPolicy>,
}

/// Current state and status message for a system feature.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct FeatureState {
    pub is_operational: bool,
    /// Current operational status of the feature.
    pub status: String,
    /// Human-readable status message or error description.
    pub message: Option<String>,
}

impl FeatureState {
    /// Creates a new [`FeatureState`].
    #[inline]
    pub fn new(status: ComponentStatus) -> Self {
        Self {
            is_operational: status.is_operational(),
            status: status.health_status.to_string(),
            message: status.message.map(|m| m.to_string()),
        }
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

/// Detailed system component statuses (requires authentication).
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct FeatureStatuses {
    /// Database and API gateway server status.
    pub gateway_server: FeatureState,
    /// Background worker runtime status.
    pub worker_runtime: FeatureState,
    /// AI assistant chat service status.
    pub assistant_chat: FeatureState,
}

/// System monitoring status response with optional detailed component information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MonitorStatusResponse {
    /// Current regional data collection policy in effect.
    pub regional_policy: RegionalPolicy,
    /// Timestamp when this status was generated.
    pub updated_at: OffsetDateTime,
    /// Detailed component statuses (only included for authenticated requests).
    #[serde(flatten)]
    pub features: Option<FeatureStatuses>,
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
    State(pg_database): State<PgDatabase>,
    State(openrouter_client): State<OpenRouter>,
    State(regional_policy): State<RegionalPolicy>,
    auth_state: Option<AuthState>,
    request: Option<Json<MonitorStatusRequest>>,
) -> (StatusCode, Json<MonitorStatusResponse>) {
    let Json(request) = request.unwrap_or_default();
    let mut response = MonitorStatusResponse {
        regional_policy,
        features: None,
        updated_at: OffsetDateTime::now_utc(),
    };

    let prefer_policy = request.prefer_policy.unwrap_or_default();
    tracing::trace!(
        target: TRACING_TARGET,
        current_policy = %regional_policy,
        requested_policy = %prefer_policy,
        "current monitor status was requested",
    );

    if let Some(AuthState(_)) = auth_state {
        let pg_status = pg_database.current_status().await;
        let pg_database = check_service_status(pg_status, "Postgres");

        let openrouter_status = openrouter_client.current_status().await;
        let openrouter_client = check_service_status(openrouter_status, "OpenRouter");

        // TODO: Add proper worker runtime service dependency injection
        let worker_runtime = FeatureState {
            is_operational: true,
            status: HealthStatus::Online.to_string(),
            message: None,
        };

        response.features = Some(FeatureStatuses {
            gateway_server: pg_database,
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

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(monitor_status))
}

#[cfg(test)]
mod test {
    use crate::handler::monitors::{MonitorStatusRequest, MonitorStatusResponse, routes};
    use crate::handler::test::create_test_server_with_router;
    use crate::service::RegionalPolicy;

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
            prefer_policy: Some(RegionalPolicy::NormalDataCollection),
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
