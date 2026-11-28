//! Detection audit handlers: read and export a detection's analysis, and read
//! the enrichment intermediates it extracted.
//!
//! Once a detection is complete, its `Audit` (the decrypted map of detected
//! findings) can be reviewed inline or downloaded as JSON or a zip of CSV tables,
//! and the enrichment intermediates (an image's OCR layout, an audio clip's
//! transcript) can be read for client-side search and entity addition. The
//! detection lifecycle itself (create, list, redact) lives in [`detections`].
//!
//! [`detections`]: super::detections

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue};
use elide_pipeline::export::{ExportCsv, ExportJson};
use elide_pipeline::{ArtifactSet, Audit};

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, markers};
use crate::handler::request::{ExportFormat, ExportQuery, WorkspaceDetectionPathParams};
use crate::handler::utility::DownloadDocs;
use crate::response::{Error, ErrorKind, ErrorResponse, Result, attachment_headers};
use crate::service::ServiceState;

/// Tracing target for detection audit operations.
const TRACING_TARGET: &str = "nvisy_server::handler::detection_audits";

/// Returns the detection's analysis (the detected findings) for review.
///
/// Fetches and decrypts the engine's `Audit` from the audit bucket. Available
/// once the detection is complete. Requires `DownloadAudit`.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection_analysis(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::DownloadAudit>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
) -> Result<Json<Audit>> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection analysis");

    let analyzed = detections
        .audit(authz.workspace.id, path_params.detection_id.as_uuid())
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Detection analysis retrieved");

    Ok(Json(analyzed))
}

fn get_detection_analysis_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection findings")
        .description(
            "Returns the detection's detected findings (the analyzed document) for review.",
        )
        .response::<200, Json<Audit>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Returns the detection's enrichment intermediates — an image's OCR layout, an
/// audio clip's transcript, tokenized text — as the content the analysis
/// extracted, so a client can search it and add entities the analysis missed.
///
/// Available only for a detection whose analysis enriched (ran an enricher for a
/// group); a detection with no enrichment has none — 404. Requires
/// `DownloadOriginalDocuments`, since intermediates carry the original document's
/// content.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection_intermediates(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::DownloadOriginalDocuments>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
) -> Result<Json<ArtifactSet>> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection intermediates");

    let intermediates = detections
        .intermediates(authz.workspace.id, path_params.detection_id.as_uuid())
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Detection intermediates retrieved");

    Ok(Json(intermediates))
}

fn get_detection_intermediates_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection intermediates")
        .description(
            "Returns the detection's enrichment intermediates — an image's OCR layout, an audio \
             clip's transcript, or tokenized text — as an object with a `parts` list, each part \
             carrying its path `id`, `modality`, and the extracted `artifact`, so a client can \
             search the content and add entities the analysis missed. A detection whose analysis \
             ran no enricher has no intermediates (404).",
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
/// join on `entity_id`. Requires `DownloadAudit` permission.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
        format = ?query.format,
    )
)]
async fn download_detection_audit(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::DownloadAudit>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
    Query(query): Query<ExportQuery>,
) -> Result<(HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading detection audit");

    let detection_id = path_params.detection_id.as_uuid();
    let audit = detections.audit(authz.workspace.id, detection_id).await?;

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
        u64::try_from(body.len()).unwrap_or(u64::MAX),
    );

    tracing::debug!(target: TRACING_TARGET, "Detection audit exported");
    Ok((headers, Body::from(body)))
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
fn archive_error(error: impl std::fmt::Display) -> Error {
    ErrorKind::InternalServerError
        .with_message("Failed to build audit archive")
        .with_context(error.to_string())
}

/// Builds the detection audit routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/analysis",
            get_with(get_detection_analysis, get_detection_analysis_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/audit",
            get_with(download_detection_audit, download_detection_audit_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/intermediates",
            get_with(
                get_detection_intermediates,
                get_detection_intermediates_docs,
            ),
        )
        .with_path_items(|item| item.tag("Detections"))
}
