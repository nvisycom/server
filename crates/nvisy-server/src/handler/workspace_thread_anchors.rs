//! Thread-anchor handlers: adding and removing a thread's location pins. The
//! thread lifecycle and timeline live in the sibling `workspace_threads` module,
//! which owns the lookup and event helpers shared here.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::NewWorkspaceThreadAnchor;
use nvisy_postgres::query::{
    AddAnchorOutcome, MAX_THREAD_ANCHORS, WorkspaceThreadAnchorRepository,
};
use nvisy_postgres::{AsyncConnection, PgClient};

use crate::extract::{Authorized, Json, Path, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    AddThreadAnchor, CommentAnchor, ThreadAnchorPathParams, ThreadPathParams,
};
use crate::handler::response::ThreadAnchor;
use crate::handler::workspace_threads::{
    TRACING_TARGET, emit_thread_event, find_thread, workspace_origin,
};
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{ServiceState, ThreadAnchorAdded, ThreadAnchorRemoved, WorkspaceEvent};

/// Adds an anchor (location pin) to a thread. Requires `Comment`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn add_anchor(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Comment>,
    Path(path_params): Path<ThreadPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<AddThreadAnchor>,
) -> Result<(StatusCode, Json<ThreadAnchor>)> {
    tracing::debug!(target: TRACING_TARGET, "Adding thread anchor");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    // Only a file-pinned thread can carry anchors (there is no file to pin into
    // for a workspace-level thread).
    let file_id = thread.file_id.ok_or_else(|| {
        ErrorKind::BadRequest.with_message("A workspace-level thread has no file to anchor to")
    })?;

    let anchor_json = encode_anchor(&request.anchor)?;

    let anchor = conn
        .transaction(async |conn| {
            let AddAnchorOutcome::Added(anchor) = conn
                .add_thread_anchor(
                    workspace.id,
                    NewWorkspaceThreadAnchor {
                        thread_id: thread.id,
                        anchor: anchor_json,
                    },
                    authz.account_id,
                )
                .await?
            else {
                return Err(ErrorKind::BadRequest.with_message(format!(
                    "A thread may have at most {MAX_THREAD_ANCHORS} anchors"
                )));
            };
            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::ThreadAnchorAdded(ThreadAnchorAdded {
                    thread_id: thread.id,
                    anchor_id: anchor.id,
                    file_id: Some(file_id),
                }),
            )
            .await?;
            Ok::<_, Error>(anchor)
        })
        .await?;

    tracing::info!(target: TRACING_TARGET, anchor_id = %anchor.id, "Anchor added");

    let response = ThreadAnchor::from_model(anchor)
        .ok_or_else(|| ErrorKind::InternalServerError.with_message("Failed to encode anchor"))?;
    Ok((StatusCode::CREATED, Json(response)))
}

fn add_anchor_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Add a thread anchor")
        .description(
            "Adds a location pin to a file thread, recording an anchor.added timeline \
             event. Requires the Comment permission.",
        )
        .response::<201, Json<ThreadAnchor>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Removes an anchor from a thread (soft delete). Requires `Comment`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
        anchor_id = %path_params.anchor_id,
    )
)]
async fn remove_anchor(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Comment>,
    Path(path_params): Path<ThreadAnchorPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Removing thread anchor");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;
    conn.find_thread_anchor(thread.id, path_params.anchor_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread_anchor"))?;

    conn.transaction(async |conn| {
        let anchor = conn
            .remove_thread_anchor(workspace.id, path_params.anchor_id, authz.account_id)
            .await?;
        emit_thread_event(
            conn,
            workspace_origin(workspace.id, authz.account_id, &security),
            WorkspaceEvent::ThreadAnchorRemoved(ThreadAnchorRemoved {
                thread_id: thread.id,
                anchor_id: anchor.id,
                file_id: thread.file_id,
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "Anchor removed");

    Ok(StatusCode::OK)
}

fn remove_anchor_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Remove a thread anchor")
        .description(
            "Soft-removes a location pin from a thread, recording an anchor.removed \
             timeline event. Requires the Comment permission.",
        )
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Encodes one typed anchor into its stored JSON.
pub(crate) fn encode_anchor(anchor: &CommentAnchor) -> Result<serde_json::Value> {
    serde_json::to_value(anchor).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to encode thread anchor")
            .with_context(err.to_string())
    })
}

/// Encodes a list of typed anchors into their stored JSON.
pub(crate) fn encode_anchors(anchors: Vec<CommentAnchor>) -> Result<Vec<serde_json::Value>> {
    anchors.iter().map(encode_anchor).collect()
}

/// Returns an [`ApiRouter`] with the thread-anchor routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/threads/{threadId}/anchors/",
            post_with(add_anchor, add_anchor_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/threads/{threadId}/anchors/{anchorId}/",
            delete_with(remove_anchor, remove_anchor_docs),
        )
        .with_path_items(|item| item.tag("Threads"))
}
