//! Pipeline-run audit handlers: read and export a run's analysis.
//!
//! Once a run is analyzed, its `Audit` (the decrypted map of detected findings)
//! can be reviewed inline or downloaded as JSON or a zip of CSV tables. The run
//! lifecycle itself (create, list, redact) lives in
//! [`pipeline_runs`](super::pipeline_runs).

use std::io::Write as _;

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use nvisy_engine::Audit;
use nvisy_postgres::PgClient;
use zip::write::SimpleFileOptions;

use super::pipeline_runs::find_pipeline_run;
use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, WorkspaceContext};
use crate::handler::request::PipelineRunPathParams;
use crate::handler::response::ErrorResponse;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{RunBlobStore, ServiceState};

/// Tracing target for pipeline audit operations.
const TRACING_TARGET: &str = "nvisy_server::handler::audits";

/// Returns the run's analysis (the detected findings) for review.
///
/// Fetches and decrypts the engine's `Audit` from the audit bucket. Available
/// once the run is analyzed. Requires `ViewPipelines`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn get_pipeline_run_analysis(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<Audit>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline run analysis");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (run, _pipeline) =
        find_pipeline_run(&mut conn, workspace.id, path_params.run_id.as_uuid()).await?;

    let analyzed = blob
        .load_analyzed_document(&mut conn, workspace.id, &run)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Pipeline run analysis retrieved");

    Ok((StatusCode::OK, Json(analyzed)))
}

fn get_pipeline_run_analysis_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get run detections")
        .description("Returns the run's detected findings (the analyzed document) for review.")
        .response::<200, Json<Audit>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Downloads a run's audit as a pretty-printed JSON file.
///
/// The full structure — body, parts, context, and each entity's provenance
/// chain. Requires `ViewPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn download_pipeline_run_audit_json(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading pipeline run audit (JSON)");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (run, _pipeline) =
        find_pipeline_run(&mut conn, workspace.id, path_params.run_id.as_uuid()).await?;
    let audit = blob
        .load_analyzed_document(&mut conn, workspace.id, &run)
        .await?;

    let mut buffer = Vec::new();
    audit.write_json(&mut buffer).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to export audit as JSON")
            .with_context(err.to_string())
    })?;

    let filename = format!("audit-{}.json", run.id);
    let headers = attachment_headers(
        &filename,
        HeaderValue::from_static("application/json"),
        buffer.len(),
    );

    tracing::debug!(target: TRACING_TARGET, "Pipeline run audit exported (JSON)");
    Ok((StatusCode::OK, headers, Body::from(buffer)))
}

fn download_pipeline_run_audit_json_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download run audit (JSON)")
        .description("Downloads the run's audit as a pretty-printed JSON file.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Downloads a run's audit as a zip of CSV tables.
///
/// The archive holds `entities.csv`, `provenance.csv`, and `reviews.csv`, which
/// join on `entity_id`. Requires `ViewPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn download_pipeline_run_audit_csv(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading pipeline run audit (CSV)");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (run, _pipeline) =
        find_pipeline_run(&mut conn, workspace.id, path_params.run_id.as_uuid()).await?;
    let audit = blob
        .load_analyzed_document(&mut conn, workspace.id, &run)
        .await?;

    let archive = build_audit_csv_zip(&audit)?;

    let filename = format!("audit-{}.csv.zip", run.id);
    let headers = attachment_headers(
        &filename,
        HeaderValue::from_static("application/zip"),
        archive.len(),
    );

    tracing::debug!(target: TRACING_TARGET, "Pipeline run audit exported (CSV)");
    Ok((StatusCode::OK, headers, Body::from(archive)))
}

fn download_pipeline_run_audit_csv_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download run audit (CSV)")
        .description(
            "Downloads the run's audit as a zip of entities.csv, provenance.csv, and reviews.csv.",
        )
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Bundles the audit's three CSV tables into an in-memory zip archive.
fn build_audit_csv_zip(audit: &Audit) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();

    let mut table = Vec::new();
    audit
        .write_entities_csv(&mut table)
        .map_err(archive_error)?;
    writer
        .start_file("entities.csv", options)
        .map_err(archive_error)?;
    writer.write_all(&table).map_err(archive_error)?;

    table.clear();
    audit
        .write_provenance_csv(&mut table)
        .map_err(archive_error)?;
    writer
        .start_file("provenance.csv", options)
        .map_err(archive_error)?;
    writer.write_all(&table).map_err(archive_error)?;

    table.clear();
    audit.write_reviews_csv(&mut table).map_err(archive_error)?;
    writer
        .start_file("reviews.csv", options)
        .map_err(archive_error)?;
    writer.write_all(&table).map_err(archive_error)?;

    Ok(writer.finish().map_err(archive_error)?.into_inner())
}

/// Maps an audit-archive build failure — CSV serialization, zip, or IO — to an
/// internal error.
fn archive_error(error: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to build audit archive")
        .with_context(error.to_string())
}

/// Builds the download response headers for an attachment.
///
/// `filename` is server-generated (a run UUID plus a fixed extension), so it
/// needs no sanitization for the quoted header value.
fn attachment_headers(filename: &str, content_type: HeaderValue, length: usize) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let disposition = format!("attachment; filename=\"{filename}\"")
        .parse()
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    headers.insert(CONTENT_DISPOSITION, disposition);
    headers.insert(CONTENT_TYPE, content_type);
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length as u64));
    headers
}

/// Builds the pipeline-run audit routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/runs/{runId}/detections/",
            get_with(get_pipeline_run_analysis, get_pipeline_run_analysis_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/runs/{runId}/audit/json",
            get_with(
                download_pipeline_run_audit_json,
                download_pipeline_run_audit_json_docs,
            ),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/runs/{runId}/audit/csv",
            get_with(
                download_pipeline_run_audit_csv,
                download_pipeline_run_audit_csv_docs,
            ),
        )
        .with_path_items(|item| item.tag("Pipeline Runs"))
}
