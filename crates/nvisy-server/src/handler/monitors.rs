//! Health-probe handlers, split along the two questions an orchestrator asks:
//!
//! - **Liveness** (`/health/live`): is the process up? A static `200`, with no
//!   dependency probing. A failing dependency must not fail this — restarting the
//!   process would not bring Postgres back, so liveness stays green while a
//!   dependency is down and only readiness reports the outage.
//! - **Readiness** (`/health/ready`, aliased at `/health`): can the server serve
//!   traffic? Probes every dependency (Postgres, NATS, blob store, webhook
//!   delivery) through [`HealthService`], so a load balancer can drain this
//!   instance while the pod itself stays up. Cached with a short TTL, so frequent
//!   probes stay cheap.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_core::health::HealthStatus;

use super::response::Health;
use crate::extract::Json;
use crate::service::{HealthService, ServiceState};

/// Tracing target for monitor operations.
const TRACING_TARGET: &str = "nvisy_server::handler::monitors";

/// Liveness: confirms the process is running and able to answer.
///
/// Always `200 OK` — it probes no dependencies, so a dependency outage never
/// trips it. Use it for an orchestrator's liveness probe, where a failure means
/// "restart me"; a dependency being down is readiness's job, not a reason to
/// restart.
async fn liveness() -> StatusCode {
    StatusCode::OK
}

fn liveness_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Liveness probe")
        .description("Returns 200 if the process is running. Probes no dependencies.")
        .response::<200, ()>()
}

/// Readiness: reports whether the server can serve traffic, dependencies included.
///
/// The response carries the overall status, a per-component breakdown, and when
/// the underlying probe ran. Results are served from a short-lived cache and
/// refreshed once stale, so this is safe to poll frequently.
///
/// - `200 OK` when healthy or degraded (still serving).
/// - `503 Service Unavailable` when unhealthy (drain this instance).
#[tracing::instrument(skip_all)]
async fn readiness(State(health): State<HealthService>) -> (StatusCode, Json<Health>) {
    let reading = health.report().await;

    let status_code = match reading.report.status {
        HealthStatus::Healthy | HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    tracing::debug!(
        target: TRACING_TARGET,
        status = ?reading.report.status,
        components = reading.report.components.len(),
        "Readiness probe response",
    );

    (status_code, Json(reading.into()))
}

fn readiness_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Readiness probe")
        .description(
            "Reports the server's health and that of its dependencies, served from \
             a short-lived cache. 200 when healthy or degraded, 503 when unhealthy. \
             Safe to poll frequently.",
        )
        .response::<200, Json<Health>>()
        .response::<503, Json<Health>>()
}

/// Returns a [`Router`] with the health-probe routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::get_with;

    ApiRouter::new()
        .api_route("/health/live", get_with(liveness, liveness_docs))
        .api_route("/health/ready", get_with(readiness, readiness_docs))
        // `/health` is the conventional default probe; alias it to readiness.
        .api_route("/health", get_with(readiness, readiness_docs))
        .with_path_items(|item| item.tag("Health"))
}
