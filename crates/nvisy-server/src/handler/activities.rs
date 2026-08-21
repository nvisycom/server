//! Workspace activity export handler.
//!
//! Exports a workspace's activity log over a date window as a downloadable file,
//! in CSV (the default) or JSON. The live, paginated activity feed lives in
//! [`workspaces`](super::workspaces); this module is only the bounded export.

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceActivityRepository;
use nvisy_postgres::types::WithAccountRef;
use serde::Serialize;

use crate::extract::{AuthProvider, AuthState, Json, Permission, Query, WorkspaceContext};
use crate::handler::request::{ActivityExportQuery, ExportFormat, MAX_EXPORT_ROWS};
use crate::handler::response::ErrorResponse;
use crate::handler::{Error, ErrorKind, Result, ServiceState};

/// Tracing target for activity export operations.
const TRACING_TARGET: &str = "nvisy_server::handler::activities";

/// One activity as a flat export row. The activity type is split into its
/// object/action halves, and the acted-on object into an id and a human label,
/// so every column is a scalar — no nested params blob.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityExportRow {
    /// When the activity occurred (RFC 3339, UTC).
    timestamp: String,
    /// The full dotted activity type, e.g. `file.created`.
    activity_type: String,
    /// The object half of the type, e.g. `file` or `pipeline.run`.
    object_type: String,
    /// The action half of the type, e.g. `created`.
    action_type: String,
    /// The account that performed the activity (display name, else username).
    actor: String,
    /// Stable id of the acted-on object, when it has one; empty otherwise.
    object_id: String,
    /// Human-readable name of the acted-on object (slug, filename, email), when
    /// it has one; empty otherwise.
    object_label: String,
    /// Originating IP address, when recorded; empty otherwise.
    ip_address: String,
    /// Originating user agent, when recorded; empty otherwise.
    user_agent: String,
}

impl ActivityExportRow {
    /// Builds a row from a joined activity, deriving the split columns from the
    /// activity type and its typed params.
    fn from_activity(row: WithAccountRef<nvisy_postgres::model::WorkspaceActivity>) -> Self {
        let WithAccountRef { item, account } = row;
        let payload = item.params.optional();
        let actor = account
            .display_name
            .unwrap_or_else(|| account.username.to_string());
        Self {
            timestamp: jiff::Timestamp::from(item.created_at).to_string(),
            activity_type: item.activity_type.as_tag().to_owned(),
            object_type: item.activity_type.object_type().to_owned(),
            action_type: item.activity_type.action_type().to_owned(),
            actor,
            object_id: payload
                .as_ref()
                .and_then(|p| p.object_id())
                .unwrap_or_default(),
            object_label: payload.and_then(|p| p.object_label()).unwrap_or_default(),
            ip_address: item.ip_address.map(|ip| ip.to_string()).unwrap_or_default(),
            user_agent: item.user_agent.unwrap_or_default(),
        }
    }
}

/// Exports a workspace's activity log over a date window as CSV or JSON.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn export_activities(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(query): Query<ActivityExportQuery>,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Exporting workspace activities");

    let export = query.resolve()?;

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    // Fetch one past the cap so a full result signals truncation.
    let fetch_limit = (MAX_EXPORT_ROWS + 1) as i64;
    let mut rows = conn
        .list_workspace_activity_between(
            workspace.id,
            export.from_timestamp()?,
            export.to_timestamp()?,
            fetch_limit,
        )
        .await?;

    let truncated = rows.len() > MAX_EXPORT_ROWS;
    rows.truncate(MAX_EXPORT_ROWS);
    if truncated {
        tracing::warn!(
            target: TRACING_TARGET,
            cap = MAX_EXPORT_ROWS,
            "Activity export hit the row cap; older rows in the window were dropped"
        );
    }

    let records: Vec<ActivityExportRow> = rows
        .into_iter()
        .map(ActivityExportRow::from_activity)
        .collect();

    let (content_type, extension, body) = match export.format {
        ExportFormat::Csv => ("text/csv; charset=utf-8", "csv", render_csv(&records)?),
        ExportFormat::Json => (
            "application/json",
            "json",
            serde_json::to_vec(&records).map_err(serialize_error)?,
        ),
    };

    let filename = format!("activities-{}_{}.{extension}", export.from, export.to);
    let mut headers = attachment_headers(&filename, content_type, body.len());
    // A truncated export is still a valid file; flag the drop in a header so a
    // client can surface it without having to parse the body.
    if truncated {
        headers.insert("X-Export-Truncated", HeaderValue::from_static("true"));
    }

    tracing::debug!(
        target: TRACING_TARGET,
        row_count = records.len(),
        truncated,
        "Workspace activities exported"
    );
    Ok((StatusCode::OK, headers, Body::from(body)))
}

fn export_activities_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Export workspace activities")
        .description(
            "Exports a workspace's activity log over a date window as a downloadable file. \
             The window is `from`/`to` (inclusive, YYYY-MM-DD); it defaults to the last 30 days \
             and is capped at 366 days. `format` is `csv` (default) or `json`. Each activity is a \
             flat row: timestamp, the type split into object/action, the actor, and the acted-on \
             object's id and label. At most 100,000 rows are returned; a truncated export sets the \
             `X-Export-Truncated` response header.",
        )
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Renders the rows as a CSV document (header row plus one row per activity).
fn render_csv(records: &[ActivityExportRow]) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    for record in records {
        writer.serialize(record).map_err(serialize_error)?;
    }
    writer
        .into_inner()
        .map_err(|err| serialize_error(err.error()))
}

/// Maps an export serialization failure to an internal error.
fn serialize_error(error: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to render activity export")
        .with_context(error.to_string())
}

/// Builds the download response headers for an attachment.
///
/// `filename` is server-generated (a fixed stem plus ISO dates and extension),
/// so it needs no sanitization for the quoted header value.
fn attachment_headers(filename: &str, content_type: &'static str, length: usize) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let disposition = format!("attachment; filename=\"{filename}\"")
        .parse()
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    headers.insert(CONTENT_DISPOSITION, disposition);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length as u64));
    headers
}

/// Builds the activity export routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/activities/export",
            get_with(export_activities, export_activities_docs),
        )
        .with_path_items(|item| item.tag("Workspaces"))
}
