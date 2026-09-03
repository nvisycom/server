//! Detection audit handlers: read and export a detection's analysis, and read
//! the enrichment intermediates it extracted.
//!
//! Once a detection is complete, its `Audit` (the decrypted map of detected
//! findings) can be reviewed inline or downloaded as JSON or a zip of CSV tables,
//! and the enrichment intermediates (an image's OCR layout, an audio clip's
//! transcript) can be read for client-side search and entity addition. The
//! detection lifecycle itself (create, list, redact) lives in
//! [`detections`](super::detections).

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use elide_pipeline::export::{ExportCsv, ExportJson};
use elide_pipeline::{ArtifactSet, Audit};
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

    // Resolve the detection and its audit file row under a scoped connection, then
    // release it before the object-store load below so the pooled connection is
    // not held across the NATS round-trip.
    let audit_file = {
        let mut conn = pg_client.get_connection().await?;

        auth_state
            .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
            .await?;

        let (detection, _pipeline) =
            find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;

        blob.resolve_audit_file(&mut conn, workspace.id, &detection)
            .await?
    };

    let analyzed = blob.load_audit(&engine, workspace.id, &audit_file).await?;

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

/// Returns the detection's enrichment intermediates — an image's OCR layout, an
/// audio clip's transcript, tokenized text — as the content the analysis
/// extracted, so a client can search it and add entities the analysis missed.
///
/// Available only for a detection whose analysis enriched (ran an enricher for a
/// group); a detection with no enrichment has none — 404. Requires
/// `ViewPipelines`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection_intermediates(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
) -> Result<(StatusCode, Json<ArtifactSet>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection intermediates");

    // Resolve the detection and its intermediates file row under a scoped
    // connection, then release it before the object-store load so the pooled
    // connection is not held across the NATS round-trip.
    let intermediates_file = {
        let mut conn = pg_client.get_connection().await?;

        auth_state
            .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
            .await?;

        let (detection, _pipeline) =
            find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;

        blob.resolve_intermediates_file(&mut conn, workspace.id, &detection)
            .await?
    };

    let intermediates = blob
        .load_intermediates(&engine, workspace.id, &intermediates_file)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Detection intermediates retrieved");

    Ok((StatusCode::OK, Json(intermediates)))
}

fn get_detection_intermediates_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection intermediates")
        .description(
            "Returns the detection's enrichment intermediates — an image's OCR layout, an audio \
             clip's transcript, or tokenized text — as a `parts` list, each part carrying its \
             path `id`, `modality`, and the extracted `artifact`, so a client can search the \
             content and add entities the analysis missed. A detection whose analysis ran no \
             enricher has no intermediates (404).",
        )
        .response::<200, Json<ArtifactSet>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
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

    // Resolve the detection and its audit file row under a scoped connection, then
    // release it before the object-store load below so the pooled connection is
    // not held across the NATS round-trip.
    let (detection_id, audit_file) = {
        let mut conn = pg_client.get_connection().await?;
        auth_state
            .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
            .await?;

        let (detection, _pipeline) =
            find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;
        let audit_file = blob
            .resolve_audit_file(&mut conn, workspace.id, &detection)
            .await?;
        (detection.id, audit_file)
    };

    let audit = blob.load_audit(&engine, workspace.id, &audit_file).await?;

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
                format!("audit-{detection_id}.json"),
                buffer,
            )
        }
        ExportFormat::Csv => {
            let archive = build_audit_csv_zip(&audit)?;
            (
                "application/zip",
                format!("audit-{detection_id}.csv.zip"),
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
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/intermediates/",
            get_with(
                get_detection_intermediates,
                get_detection_intermediates_docs,
            ),
        )
        .with_path_items(|item| item.tag("Detections"))
}
