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
use elide_pipeline::Audit;
use nvisy_postgres::model::{Workspace, WorkspaceRedaction};
use nvisy_postgres::types::{CursorPage, WithAccountRef};

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, markers};
use crate::handler::ServiceState;
use crate::handler::request::{
    CursorPagination, WorkspaceDetectionPathParams, WorkspaceRedactionPathParams,
    WorkspaceRedactionsQuery,
};
use crate::handler::response::{WorkspaceRedactionResult, WorkspaceRedactionsPage, WorkspaceRef};
use crate::response::{ErrorResponse, Result};

/// Tracing target for redaction operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_redactions";

/// Lists a detection's redactions, most recent first, cursor-paginated.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
    )
)]
async fn list_detection_redactions(
    State(redactions): State<domain::WorkspaceRedactionService>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<Json<WorkspaceRedactionsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing detection redactions");

    let workspace = &authz.workspace;
    let page = redactions
        .list_for_detection(
            workspace.id,
            path_params.detection_id.as_uuid(),
            pagination.into_cursor(),
        )
        .await?;
    Ok(Json(redactions_page(workspace, page)))
}

fn list_detection_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List detection redactions")
        .description(
            "Returns a detection's redactions, most recent first, cursor-paginated. Each \
             redaction is one redact pass with its own reviewer edits, output document, and \
             review audit.",
        )
        .response::<200, Json<WorkspaceRedactionsPage>>()
}

/// Lists all redactions across the workspace, most recent first, cursor-paginated.
///
/// Aggregates redactions from every detection in the workspace, with optional
/// detection and document filters — the document-scoped view a document's
/// redactions tab renders from, without fanning out one request per detection.
/// Requires `ViewDetections`.
#[tracing::instrument(skip_all)]
async fn list_workspace_redactions(
    State(redactions): State<domain::WorkspaceRedactionService>,
    authz: Authorized<markers::ViewDetections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceRedactionsQuery>,
) -> Result<Json<WorkspaceRedactionsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace redactions");

    let workspace = &authz.workspace;
    let page = redactions
        .list(workspace.id, pagination.into_cursor(), &query.into())
        .await?;
    Ok(Json(redactions_page(workspace, page)))
}

fn list_workspace_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace redactions")
        .description(
            "Returns all redactions across the workspace, most recent first, cursor-paginated, \
             with optional detection and document filters. Each redaction is one redact pass \
             with its own reviewer edits, output document, and review audit.",
        )
        .response::<200, Json<WorkspaceRedactionsPage>>()
}

/// Returns a redaction's review audit: the analysis with the reviewer's edits
/// applied and the per-entity redaction outcome recorded.
#[tracing::instrument(
    skip_all,
    fields(
        redaction_id = %path_params.redaction_id,
    )
)]
async fn get_redaction_review(
    State(redactions): State<domain::WorkspaceRedactionService>,
    authz: Authorized<markers::DownloadAudit>,
    Path(path_params): Path<WorkspaceRedactionPathParams>,
) -> Result<Json<Audit>> {
    tracing::debug!(target: TRACING_TARGET, "Getting redaction review audit");

    let review = redactions
        .review_audit(authz.workspace.id, path_params.redaction_id.as_uuid())
        .await?;
    Ok(Json(review))
}

fn get_redaction_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get redaction review")
        .description(
            "Returns the redaction's review audit: the detection analysis with the reviewer's \
             edits applied and the per-entity redaction outcome recorded.",
        )
        .response::<200, Json<Audit>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Maps a page of redactions, each with its requester, to the response.
fn redactions_page(
    workspace: &Workspace,
    page: CursorPage<WithAccountRef<WorkspaceRedaction>>,
) -> WorkspaceRedactionsPage {
    WorkspaceRedactionsPage::from_cursor_page(page, |row| {
        WorkspaceRedactionResult::from_model(
            &row.item,
            WorkspaceRef::from(workspace),
            row.account.into(),
        )
    })
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
