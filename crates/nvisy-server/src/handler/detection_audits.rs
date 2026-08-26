//! Detection audit handlers: read and export a detection's analysis.
//!
//! Once a detection is complete, its `Audit` (the decrypted map of detected
//! findings) can be reviewed inline or downloaded as JSON or a zip of CSV tables.
//! The detection lifecycle itself (create, list, redact) lives in
//! [`detections`](super::detections).

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use elide_pipeline::Audit;
use elide_pipeline::export::{ExportCsv, ExportJson};
use nvisy_postgres::PgClient;

use super::detections::find_detection;
use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, WorkspaceContext};
use crate::handler::request::{DetectionPathParams, ExportFormat, ExportQuery};
use crate::handler::response::ErrorResponse;
use crate::handler::utility::{DownloadResponseExt, attachment_headers};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{EngineService, RunBlobStore, ServiceState};

/// Tracing target for detection audit operations.
const TRACING_TARGET: &str = "nvisy_server::handler::detection_audits";

/// Returns the detection's analysis (the detected findings) for review.
///
/// Fetches and decrypts the engine's `Audit` from the audit bucket. Available
/// once the detection is complete. Requires `ViewPipelines`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection_analysis(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
) -> Result<(StatusCode, Json<Audit>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection analysis");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (detection, _pipeline) =
        find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;

    let analyzed = blob
        .load_analyzed_document(&mut conn, &engine, workspace.id, &detection)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Detection analysis retrieved");

    Ok((StatusCode::OK, Json(analyzed)))
}

fn get_detection_analysis_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection findings")
        .description(
            "Returns the detection's detected findings (the analyzed document) for review.",
        )
        .response::<200, Json<Audit>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Downloads a detection's audit as a file, in the requested `format`.
///
/// `json` yields a pretty-printed JSON file with the full structure — body,
/// parts, context, and each entity's provenance chain. `csv` (the default)
/// yields a zip of `entities.csv`, `provenance.csv`, and `reviews.csv`, which
/// join on `entity_id`. Requires `ViewPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
        format = ?query.format,
    )
)]
async fn download_detection_audit(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
    Query(query): Query<ExportQuery>,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading detection audit");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (detection, _pipeline) =
        find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;
    let audit = blob
        .load_analyzed_document(&mut conn, &engine, workspace.id, &detection)
        .await?;

    let (content_type, filename, body) = match query.format {
        ExportFormat::Json => {
            let mut buffer = Vec::new();
            audit.write_json_pretty(&mut buffer).map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to export audit as JSON")
                    .with_context(err.to_string())
            })?;
            (
                "application/json",
                format!("audit-{}.json", detection.id),
                buffer,
            )
        }
        ExportFormat::Csv => {
            let archive = build_audit_csv_zip(&audit)?;
            (
                "application/zip",
                format!("audit-{}.csv.zip", detection.id),
                archive,
            )
        }
    };

    let headers = attachment_headers(
        &filename,
        HeaderValue::from_static(content_type),
        body.len() as u64,
    );

    tracing::debug!(target: TRACING_TARGET, "Detection audit exported");
    Ok((StatusCode::OK, headers, Body::from(body)))
}

fn download_detection_audit_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download detection audit")
        .description(
            "Downloads the detection's audit as a file. `format` is `csv` (default) — a zip of \
             entities.csv, provenance.csv, and reviews.csv — or `json`, a pretty-printed JSON file.",
        )
        .download_response(
            "The detection's exported audit.",
            &["application/zip", "application/json"],
        )
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Bundles the audit's CSV tables into an in-memory zip archive.
///
/// The engine's export writes one deflate-compressed CSV per table
/// (`entities.csv`, `provenance.csv`, `reviews.csv`), all joining on `entity_id`.
fn build_audit_csv_zip(audit: &Audit) -> Result<Vec<u8>> {
    let mut archive = std::io::Cursor::new(Vec::new());
    audit.write_zip(&mut archive).map_err(archive_error)?;
    Ok(archive.into_inner())
}

/// Maps an audit-archive build failure — CSV serialization, zip, or IO — to an
/// internal error.
fn archive_error(error: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to build audit archive")
        .with_context(error.to_string())
}

/// Builds the detection audit routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/analysis/",
            get_with(get_detection_analysis, get_detection_analysis_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/audit/",
            get_with(download_detection_audit, download_detection_audit_docs),
        )
        .with_path_items(|item| item.tag("Detections"))
}
