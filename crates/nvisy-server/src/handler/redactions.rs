//! Redaction handlers: list a detection's redactions and read a redaction's
//! review audit.
//!
//! A redaction is produced by `POST /detections/{detectionId}/redactions/` (in
//! [`detections`](super::detections)); these endpoints read them back.

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use elide_pipeline::Audit;
use nvisy_postgres::query::WorkspaceRedactionRepository;
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use super::detections::find_detection;
use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, WorkspaceContext};
use crate::handler::request::{CursorPagination, DetectionPathParams, RedactionPathParams};
use crate::handler::response::{ErrorResponse, RedactionResult, RedactionsPage};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{ErrorKind, Result, ServiceState};
use crate::service::{EngineService, RunBlobStore};

/// Tracing target for redaction operations.
const TRACING_TARGET: &str = "nvisy_server::handler::redactions";

/// Lists a detection's redactions, most recent first, cursor-paginated.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn list_detection_redactions(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<RedactionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing detection redactions");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    // Confirm the detection exists in this workspace (404 otherwise) before
    // listing its redactions.
    let (detection, _pipeline) =
        find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;

    let page = conn
        .cursor_list_detection_redactions(detection.id, pagination.into())
        .await?;

    // Resolve the requesting account per row. A detection's redactions are few
    // (one per manual redact request), so a per-row lookup is acceptable here.
    let mut items = Vec::with_capacity(page.items.len());
    for redaction in page.items {
        let requested_by = resolve_account_ref(&mut conn, redaction.account_id).await?;
        items.push(RedactionResult::from_model(
            redaction,
            workspace.slug.clone(),
            requested_by,
        ));
    }
    let response = RedactionsPage::new(items, page.total, page.next_cursor);

    Ok((StatusCode::OK, Json(response)))
}

fn list_detection_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List detection redactions")
        .description(
            "Returns a detection's redactions, most recent first, cursor-paginated. Each \
             redaction is one redact pass with its own reviewer edits, output document, and \
             review audit.",
        )
        .response::<200, Json<RedactionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a redaction's review audit: the analysis with the reviewer's edits
/// applied and the per-entity redaction outcome recorded.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        redaction_id = %path_params.redaction_id,
    )
)]
async fn get_redaction_review(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<RedactionPathParams>,
) -> Result<(StatusCode, Json<Audit>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting redaction review audit");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let redaction =
        find_redaction(&mut conn, workspace.id, path_params.redaction_id.as_uuid()).await?;

    let review = blob
        .load_review_audit(&mut conn, &engine, workspace.id, redaction.review_file_id)
        .await?;

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
/// A [`RedactionId`](nvisy_postgres::types::RedactionId) is globally unique, so
/// the redaction is addressed by id alone and resolved within the workspace via
/// its detection's pipeline.
async fn find_redaction(
    conn: &mut PgConn,
    workspace_id: Uuid,
    redaction_id: Uuid,
) -> Result<nvisy_postgres::model::WorkspaceRedaction> {
    conn.find_redaction_in_workspace(workspace_id, redaction_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Redaction not found")
                .with_resource("redaction")
        })
}

/// Builds the redaction routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/redactions/",
            get_with(list_detection_redactions, list_detection_redactions_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/redactions/{redactionId}/review",
            get_with(get_redaction_review, get_redaction_review_docs),
        )
        .with_path_items(|item| item.tag("Redactions"))
}
