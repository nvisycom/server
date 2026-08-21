//! Workspace analytics handler: aggregate metrics over a workspace's files and
//! pipeline runs, for a dashboard.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceAnalyticsRepository;

use crate::extract::{AuthProvider, AuthState, Json, Permission, Query, WorkspaceContext};
use crate::handler::request::DateWindow;
use crate::handler::response::{ErrorResponse, RunTimeSeries, WorkspaceAnalytics};
use crate::handler::{Result, ServiceState};

/// Tracing target for workspace analytics operations.
const TRACING_TARGET: &str = "nvisy_server::handler::analytics";

/// Returns aggregate analytics for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn get_analytics(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
) -> Result<(StatusCode, Json<WorkspaceAnalytics>)> {
    tracing::debug!(target: TRACING_TARGET, "Computing workspace analytics");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let snapshot = conn.snapshot(workspace.id).await?;

    let analytics = WorkspaceAnalytics::from_snapshot(snapshot);

    Ok((StatusCode::OK, Json(analytics)))
}

fn get_analytics_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Workspace analytics")
        .description(
            "Returns aggregate analytics for a workspace: stored-file totals with a per-kind breakdown, pipeline-run health (status mix, error rate, and durations), and inference token usage (workspace totals plus a per-model breakdown). Breakdowns list every kind/status, zero-filled, in a stable order.",
        )
        .response::<200, Json<WorkspaceAnalytics>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a workspace's daily run activity over a date window.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn get_run_timeseries(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(window): Query<DateWindow>,
) -> Result<(StatusCode, Json<RunTimeSeries>)> {
    tracing::debug!(target: TRACING_TARGET, "Computing run time series");

    let window = window.resolve()?;

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let points = conn
        .runs_by_day(
            workspace.id,
            window.from_timestamp()?,
            window.to_timestamp()?,
        )
        .await?;
    let series = RunTimeSeries::from_window(window.from, window.to, points);

    Ok((StatusCode::OK, Json(series)))
}

fn get_run_timeseries_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Workspace run time series")
        .description(
            "Returns a workspace's daily pipeline-run activity over a date window: runs per day, plus each day's error rate and durations. Every day in the window is present (quiet days report runs: 0), so the series plots as a continuous line or a contribution-style calendar. The window is `from`/`to` (inclusive, YYYY-MM-DD); it defaults to the last 30 days and is capped at 366 days.",
        )
        .response::<200, Json<RunTimeSeries>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds the analytics routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/analytics/",
            get_with(get_analytics, get_analytics_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/analytics/runs/timeseries/",
            get_with(get_run_timeseries, get_run_timeseries_docs),
        )
        .with_path_items(|item| item.tag("Analytics"))
}
