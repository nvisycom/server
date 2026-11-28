//! Workspace analytics handler: aggregate metrics over a workspace's files and
//! detections, for a dashboard.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;

use crate::domain::WorkspaceAnalyticsService;
use crate::extract::{Authorized, Json, Query, markers};
use crate::handler::ServiceState;
use crate::handler::request::DateWindow;
use crate::handler::response::{WorkspaceAnalytics, WorkspaceDetectionTimeSeries};
use crate::response::Result;

/// Tracing target for workspace analytics operations.
const TRACING_TARGET: &str = "nvisy_server::handler::analytics";

/// Returns aggregate analytics for a workspace.
#[tracing::instrument(skip_all)]
async fn get_analytics(
    State(analytics): State<WorkspaceAnalyticsService>,
    authz: Authorized<markers::ViewAnalytics>,
) -> Result<Json<WorkspaceAnalytics>> {
    tracing::debug!(target: TRACING_TARGET, "Computing workspace analytics");

    let snapshot = analytics.snapshot(authz.workspace.id).await?;
    Ok(Json(WorkspaceAnalytics::from_snapshot(snapshot)))
}

fn get_analytics_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Workspace analytics")
        .description(
            "Returns aggregate analytics for a workspace: stored-file totals with a per-kind breakdown, and detection health (status mix, error rate, and durations). Breakdowns list every kind/status, zero-filled, in a stable order.",
        )
        .response::<200, Json<WorkspaceAnalytics>>()
}

/// Returns a workspace's daily detection activity over a date window.
#[tracing::instrument(skip_all)]
async fn get_detection_timeseries(
    State(analytics): State<WorkspaceAnalyticsService>,
    authz: Authorized<markers::ViewAnalytics>,
    Query(window): Query<DateWindow>,
) -> Result<Json<WorkspaceDetectionTimeSeries>> {
    tracing::debug!(target: TRACING_TARGET, "Computing detection time series");

    let window = window.resolve()?;

    let points = analytics
        .detections_by_day(
            authz.workspace.id,
            window.from_timestamp()?,
            window.to_timestamp()?,
        )
        .await?;
    let series = WorkspaceDetectionTimeSeries::from_window(window.from, window.to, points);

    Ok(Json(series))
}

fn get_detection_timeseries_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Workspace detection time series")
        .description(
            "Returns a workspace's daily detection activity over a date window: detections per day, plus each day's error rate and durations. Every day in the window is present (quiet days report detections: 0), so the series plots as a continuous line or a contribution-style calendar. The window is `from`/`to` (inclusive, YYYY-MM-DD); it defaults to the last 30 days and is capped at 366 days.",
        )
        .response::<200, Json<WorkspaceDetectionTimeSeries>>()
}

/// Builds the analytics routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::get_with;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/analytics",
            get_with(get_analytics, get_analytics_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/analytics/detections/timeseries",
            get_with(get_detection_timeseries, get_detection_timeseries_docs),
        )
        .with_path_items(|item| item.tag("Analytics"))
}
