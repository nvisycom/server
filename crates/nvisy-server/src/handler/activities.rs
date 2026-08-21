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
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceActivityRepository;
use nvisy_postgres::types::WithAccountRef;
use serde::Serialize;

use crate::extract::{AuthProvider, AuthState, Json, Permission, Query, WorkspaceContext};
use crate::handler::request::{ActivityExportQuery, ExportFormat, MAX_EXPORT_ROWS};
use crate::handler::response::ErrorResponse;
use crate::handler::utility::attachment_headers;
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
            export.window.from_timestamp()?,
            export.window.to_timestamp()?,
            fetch_limit,
        )
        .await?;

    let truncated = rows.len() > MAX_EXPORT_ROWS;
    rows.truncate(MAX_EXPORT_ROWS);
    if truncated {
        // Rows come oldest-first and we keep the leading page, so it is the
        // newest rows in the window that are dropped past the cap.
        tracing::warn!(
            target: TRACING_TARGET,
            cap = MAX_EXPORT_ROWS,
            "Activity export hit the row cap; the newest rows in the window were dropped"
        );
    }

    let records: Vec<ActivityExportRow> = rows
        .into_iter()
        .map(ActivityExportRow::from_activity)
        .collect();

    let (content_type, extension, body) = match export.format {
        ExportFormat::Csv => (
            HeaderValue::from_static("text/csv; charset=utf-8"),
            "csv",
            render_csv(&records)?,
        ),
        ExportFormat::Json => (
            HeaderValue::from_static("application/json"),
            "json",
            serde_json::to_vec(&records).map_err(serialize_error)?,
        ),
    };

    let filename = format!(
        "activities-{}_{}.{extension}",
        export.window.from, export.window.to
    );
    let mut headers = attachment_headers(&filename, content_type, body.len() as u64);
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
             object's id and label, oldest first. At most 100,000 rows are returned (the oldest in \
             the window); a truncated export sets the `X-Export-Truncated` response header.",
        )
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Column headers for the CSV export, in field order.
const CSV_HEADER: [&str; 9] = [
    "timestamp",
    "activityType",
    "objectType",
    "actionType",
    "actor",
    "objectId",
    "objectLabel",
    "ipAddress",
    "userAgent",
];

/// Renders the rows as a CSV document: an explicit header row (so an empty
/// export is still a valid, self-describing file) followed by one row per
/// activity. Free-text cells are neutralized against spreadsheet formula
/// injection ([`csv_cell`]).
fn render_csv(records: &[ActivityExportRow]) -> Result<Vec<u8>> {
    // The header is written explicitly rather than via `serialize`, which emits
    // it only alongside the first record and so would omit it for an empty export.
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer.write_record(CSV_HEADER).map_err(serialize_error)?;
    for r in records {
        writer
            .write_record([
                r.timestamp.as_str(),
                &r.activity_type,
                &r.object_type,
                &r.action_type,
                &csv_cell(&r.actor),
                &r.object_id,
                &csv_cell(&r.object_label),
                &csv_cell(&r.ip_address),
                &csv_cell(&r.user_agent),
            ])
            .map_err(serialize_error)?;
    }
    writer
        .into_inner()
        .map_err(|err| serialize_error(err.error()))
}

/// Neutralizes a free-text cell against spreadsheet formula injection: a value
/// that (after leading whitespace) begins with `=`, `+`, `-`, or `@` is executed
/// as a formula when the CSV is opened in Excel/Sheets, so such a value is
/// prefixed with an apostrophe to force it to be read as text. Applied only to
/// CSV output; JSON carries the raw value.
fn csv_cell(value: &str) -> std::borrow::Cow<'_, str> {
    let trimmed = value.trim_start();
    if trimmed.starts_with(['=', '+', '-', '@']) {
        std::borrow::Cow::Owned(format!("'{value}"))
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

/// Maps an export serialization failure to an internal error.
fn serialize_error(error: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to render activity export")
        .with_context(error.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(actor: &str, user_agent: &str) -> ActivityExportRow {
        ActivityExportRow {
            timestamp: "2026-01-01T00:00:00Z".to_owned(),
            activity_type: "file.created".to_owned(),
            object_type: "file".to_owned(),
            action_type: "created".to_owned(),
            actor: actor.to_owned(),
            object_id: String::new(),
            object_label: String::new(),
            ip_address: String::new(),
            user_agent: user_agent.to_owned(),
        }
    }

    fn csv_string(records: &[ActivityExportRow]) -> String {
        String::from_utf8(render_csv(records).unwrap()).unwrap()
    }

    #[test]
    fn empty_export_still_has_the_header_row() {
        let csv = csv_string(&[]);
        assert_eq!(csv.trim(), CSV_HEADER.join(","));
    }

    #[test]
    fn header_is_written_once_for_a_non_empty_export() {
        let csv = csv_string(&[row("alice", "curl/8")]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], CSV_HEADER.join(","));
    }

    #[test]
    fn neutralizes_formula_prefixes_in_free_text_cells() {
        // A display name and user-agent starting with a formula char must be
        // prefixed with an apostrophe so a spreadsheet reads them as text.
        let csv = csv_string(&[row("=cmd()", "@SUM(A1)")]);
        let data = csv.lines().nth(1).unwrap();
        assert!(data.contains("'=cmd()"), "actor not neutralized: {data}");
        assert!(
            data.contains("'@SUM(A1)"),
            "user agent not neutralized: {data}"
        );
    }

    #[test]
    fn leaves_ordinary_values_unchanged() {
        assert_eq!(csv_cell("alice"), "alice");
        assert_eq!(csv_cell(""), "");
        // Neutralization checks the first non-whitespace char.
        assert_eq!(csv_cell("  =x"), "'  =x");
    }
}
