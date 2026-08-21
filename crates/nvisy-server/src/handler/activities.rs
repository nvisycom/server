//! Workspace activity handlers: the live activity feed and its bounded export.
//!
//! The feed is a cursor-paginated list of the workspace's activity log; the
//! export renders a date window of that log as a downloadable file, in CSV (the
//! default) or JSON.

use std::borrow::Cow;

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::WorkspaceActivity;
use nvisy_postgres::query::WorkspaceActivityRepository;
use nvisy_postgres::types::WithAccountRef;
use serde::Serialize;

use crate::extract::{AuthProvider, AuthState, Json, Permission, Query, WorkspaceContext};
use crate::handler::request::{
    ActivityExportQuery, ActivityListQuery, ExportFormat, MAX_EXPORT_ROWS,
};
use crate::handler::response::{ActivitiesPage, Activity, ErrorResponse};
use crate::handler::utility::{DownloadResponseExt, attachment_headers};
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
    fn from_activity(row: WithAccountRef<WorkspaceActivity>) -> Self {
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

    /// The row's cells in [`CSV_HEADER`] order, with the free-text ones
    /// (`actor`, `object_label`, `ip_address`, `user_agent`) neutralized against
    /// spreadsheet formula injection ([`csv_cell`]). Server-controlled cells (the
    /// timestamp, the type halves, the object id) are passed through as-is.
    fn csv_fields(&self) -> [Cow<'_, str>; 9] {
        [
            Cow::Borrowed(self.timestamp.as_str()),
            Cow::Borrowed(&self.activity_type),
            Cow::Borrowed(&self.object_type),
            Cow::Borrowed(&self.action_type),
            csv_cell(&self.actor),
            Cow::Borrowed(&self.object_id),
            csv_cell(&self.object_label),
            csv_cell(&self.ip_address),
            csv_cell(&self.user_agent),
        ]
    }
}

/// Lists a workspace's activity log, most recent first, cursor-paginated.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_activities(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(query): Query<ActivityListQuery>,
) -> Result<(StatusCode, Json<ActivitiesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace activities");

    let filter = query.to_filter()?;

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let page = conn
        .cursor_list_workspace_activity(workspace.id, filter, query.pagination.into())
        .await?;

    let response = ActivitiesPage::from_cursor_page(page, |wc| {
        Activity::from_model(wc.item, workspace.slug.clone(), wc.account.into())
    });

    tracing::debug!(
        target: TRACING_TARGET,
        activity_count = response.items.len(),
        "Workspace activities listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_activities_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace activities")
        .description(
            "Returns the workspace's activity log, most recent first, cursor-paginated. \
             Optional filters: `type` (repeatable, e.g. `file.created`), `actor` (an account id), \
             and a `from`/`to` day range (each bound narrows only when given; the feed is \
             otherwise all-time).",
        )
        .response::<200, Json<ActivitiesPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
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
        .list_workspace_activity_for_export(workspace.id, export.filter.clone(), fetch_limit)
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
             and is capped at 366 days. The same `type` (repeatable) and `actor` filters as the \
             feed apply. `format` is `csv` (default) or `json`. Each activity is a flat row: \
             timestamp, the type split into object/action, the actor, and the acted-on object's \
             id and label, oldest first. At most 100,000 rows are returned (the oldest in the \
             window); a truncated export sets the `X-Export-Truncated` response header.",
        )
        .download_response(
            "The exported activity log.",
            &["text/csv", "application/json"],
        )
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
        let fields = r.csv_fields();
        writer
            .write_record(fields.iter().map(AsRef::<str>::as_ref))
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
fn csv_cell(value: &str) -> Cow<'_, str> {
    let trimmed = value.trim_start();
    if trimmed.starts_with(['=', '+', '-', '@']) {
        Cow::Owned(format!("'{value}"))
    } else {
        Cow::Borrowed(value)
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
            "/workspaces/{workspaceSlug}/activities/",
            get_with(list_activities, list_activities_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/activities/export",
            get_with(export_activities, export_activities_docs),
        )
        .with_path_items(|item| item.tag("Activities"))
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

    #[test]
    fn csv_header_matches_the_struct_serde_field_names() {
        // The header is written by hand (the row cells are too, so `csv_cell` can
        // neutralize a subset), so guard it against drift from the struct: the
        // `csv` serializer emits headers from the serde field names, honoring
        // `rename_all = "camelCase"`, so what it emits is the source of truth.
        let mut w = csv::WriterBuilder::new()
            .has_headers(true)
            .from_writer(Vec::new());
        w.serialize(row("a", "b")).unwrap();
        let out = String::from_utf8(w.into_inner().unwrap()).unwrap();
        let derived_header = out.lines().next().unwrap();
        assert_eq!(derived_header, CSV_HEADER.join(","));
    }
}
