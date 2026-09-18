//! Redaction handlers: list a detection's redactions and read a redaction's
//! review audit.
//!
//! A redaction is produced by `POST /detections/{detectionId}/redactions/` (in
//! [`detections`]); these endpoints read them back.
//!
//! [`detections`]: super::detections

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use elide_pipeline::Audit;
use nvisy_postgres::query::WorkspaceRedactionRepository;
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, markers};
use crate::handler::ServiceState;
use crate::handler::request::{
    CursorPagination, WorkspaceDetectionPathParams, WorkspaceRedactionPathParams,
    WorkspaceRedactionsQuery,
};
use crate::handler::response::{WorkspaceRedactionResult, WorkspaceRedactionsPage};
use crate::handler::utility::{resolve_account_ref, resolve_account_refs};
use crate::response::{ErrorKind, ErrorResponse, Result};
use crate::service::{ArtifactReader, EngineService};

/// Tracing target for redaction operations.
const TRACING_TARGET: &str = "nvisy_server::handler::redactions";

/// Lists a detection's redactions, most recent first, cursor-paginated.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn list_detection_redactions(
    State(pg_client): State<PgClient>,
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspaceRedactionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing detection redactions");

    let workspace = authz.workspace;

    // Confirm the detection exists in this workspace (404 otherwise) before
    // listing its redactions.
    let (detection, _pipeline) = detections
        .find(workspace.id, path_params.detection_id.as_uuid())
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let page = conn
        .cursor_list_detection_redactions(detection.id, pagination.into_cursor())
        .await?;

    // Resolve the requesting account per row. A detection's redactions are few
    // (one per manual redact request), so a per-row lookup is acceptable here.
    let mut items = Vec::with_capacity(page.items.len());
    for redaction in page.items {
        let requested_by = resolve_account_ref(&mut conn, redaction.account_id).await?;
        items.push(WorkspaceRedactionResult::from_model(
            &redaction,
            workspace.id,
            workspace.handle.clone(),
            requested_by,
        ));
    }
    let response = WorkspaceRedactionsPage::new(items, page.total, page.next_cursor);

    Ok((StatusCode::OK, Json(response)))
}

fn list_detection_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List detection redactions")
        .description(
            "Returns a detection's redactions, most recent first, cursor-paginated. Each \
             redaction is one redact pass with its own reviewer edits, output document, and \
             review audit.",
        )
        .response::<200, Json<WorkspaceRedactionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists all redactions across the workspace, most recent first, cursor-paginated.
///
/// Aggregates redactions from every detection in the workspace, with optional
/// detection and document filters — the document-scoped view a document's
/// redactions tab renders from, without fanning out one request per detection.
/// Requires `ViewDetections`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_workspace_redactions(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewDetections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceRedactionsQuery>,
) -> Result<(StatusCode, Json<WorkspaceRedactionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace redactions");

    let workspace = authz.workspace;

    let mut conn = pg_client.get_connection().await?;
    let page = conn
        .cursor_list_workspace_redactions(workspace.id, pagination.into_cursor(), &query.into())
        .await?;

    // Resolve the requesting accounts in one query keyed by id: this listing spans
    // every detection in the workspace, so a per-row lookup would be an N+1. A row
    // whose account is missing is a server-side inconsistency (as elsewhere).
    let account_ids: Vec<Uuid> = page.items.iter().map(|r| r.account_id).collect();
    let accounts = resolve_account_refs(&mut conn, &account_ids).await?;
    let mut items = Vec::with_capacity(page.items.len());
    for redaction in &page.items {
        let requested_by = accounts
            .get(&redaction.account_id)
            .cloned()
            .ok_or_else(|| ErrorKind::InternalServerError.with_message("account not found"))?;
        items.push(WorkspaceRedactionResult::from_model(
            redaction,
            workspace.id,
            workspace.handle.clone(),
            requested_by,
        ));
    }
    let response = WorkspaceRedactionsPage::new(items, page.total, page.next_cursor);

    Ok((StatusCode::OK, Json(response)))
}

fn list_workspace_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace redactions")
        .description(
            "Returns all redactions across the workspace, most recent first, cursor-paginated, \
             with optional detection and document filters. Each redaction is one redact pass \
             with its own reviewer edits, output document, and review audit.",
        )
        .response::<200, Json<WorkspaceRedactionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a redaction's review audit: the analysis with the reviewer's edits
/// applied and the per-entity redaction outcome recorded.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        redaction_id = %path_params.redaction_id,
    )
)]
async fn get_redaction_review(
    State(pg_client): State<PgClient>,
    State(blob): State<ArtifactReader>,
    State(engine): State<EngineService>,
    authz: Authorized<markers::DownloadAudit>,
    Path(path_params): Path<WorkspaceRedactionPathParams>,
) -> Result<(StatusCode, Json<Audit>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting redaction review audit");

    let workspace = authz.workspace;

    // Resolve the redaction and its review audit file row under a scoped
    // connection, then release it before the object-store load so the pooled
    // connection is not held across the NATS round-trip.
    let review_blob = {
        let mut conn = pg_client.get_connection().await?;

        let redaction =
            find_redaction(&mut conn, workspace.id, path_params.redaction_id.as_uuid()).await?;

        blob.resolve_review_blob(&mut conn, &redaction).await?
    };

    let review = blob.load_audit(&engine, workspace.id, &review_blob).await?;

    Ok((StatusCode::OK, Json(review)))
}

fn get_redaction_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get redaction review")
        .description(
            "Returns the redaction's review audit: the detection analysis with the reviewer's \
             edits applied and the per-entity redaction outcome recorded.",
        )
        .response::<200, Json<Audit>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Loads a redaction scoped to the workspace, mapping a missing one to a 404.
///
/// A [`RedactionId`] is globally unique, so the redaction is addressed by id
/// alone and resolved within the workspace via its detection's pipeline.
///
/// [`RedactionId`]: nvisy_postgres::types::RedactionId
async fn find_redaction(
    conn: &mut PgConn,
    workspace_id: Uuid,
    redaction_id: Uuid,
) -> Result<nvisy_postgres::model::WorkspaceRedaction> {
    conn.find_redaction_in_workspace(workspace_id, redaction_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_message("Redaction not found"))
}

/// Builds the redaction routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/redactions",
            get_with(list_workspace_redactions, list_workspace_redactions_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/redactions",
            get_with(list_detection_redactions, list_detection_redactions_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/redactions/{redactionId}/review",
            get_with(get_redaction_review, get_redaction_review_docs),
        )
        .with_path_items(|item| item.tag("Redactions"))
}
