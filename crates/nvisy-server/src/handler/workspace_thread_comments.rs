//! Comment handlers: posting, editing, and deleting the messages within a
//! thread. A thin HTTP layer over [`WorkspaceThreadService`], which owns comment
//! creation (mention resolution, assistant enqueue) alongside the thread
//! lifecycle it shares an aggregate with.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::domain;
use crate::extract::{Authorized, Json, Path, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceComment, UpdateWorkspaceComment, WorkspaceCommentPathParams,
    WorkspaceThreadPathParams,
};
use crate::handler::response::WorkspaceComment;
use crate::handler::utility::resolve_account_ref;
use crate::response::{ErrorResponse, Result};
use crate::service::{ServiceState, event};

/// Tracing target for comment operations.
const TRACING_TARGET: &str = "nvisy_server::handler::comments";

/// Posts a comment (message) in a thread.
///
/// `@username` mentions in the body notify those workspace members. Requires
/// `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn create_comment(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceComment>,
) -> Result<(StatusCode, Json<WorkspaceComment>)> {
    tracing::debug!(target: TRACING_TARGET, "Posting comment");

    let workspace = authz.workspace;
    let comment = threads
        .create_comment(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            path_params.thread_id,
            request.body,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let author = resolve_account_ref(&mut conn, comment.author_account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceComment::from_model(comment, author)),
    ))
}

fn create_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Post a comment")
        .description(
            "Posts a comment (message) in a thread. @username mentions notify those \
             members. Requires the Review permission. Returns 409 if the thread is \
             closed.",
        )
        .response::<201, Json<WorkspaceComment>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Edits a comment's body. Restricted to the comment's author.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        comment_id = %path_params.comment_id,
    )
)]
async fn update_comment(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceCommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspaceComment>,
) -> Result<(StatusCode, Json<WorkspaceComment>)> {
    tracing::debug!(target: TRACING_TARGET, "Editing comment");

    let workspace = authz.workspace;
    let updated = threads
        .update_comment(
            workspace.id,
            authz.account_id,
            path_params.comment_id,
            request.body,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let author = resolve_account_ref(&mut conn, updated.author_account_id).await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceComment::from_model(updated, author)),
    ))
}

fn update_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Edit a comment")
        .description("Edits a comment's body. Only the author may edit their own comment.")
        .response::<200, Json<WorkspaceComment>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a comment (soft delete). Restricted to the comment's author.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        comment_id = %path_params.comment_id,
    )
)]
async fn delete_comment(
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceCommentPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting comment");

    let workspace = authz.workspace;
    threads
        .delete_comment(workspace.id, authz.account_id, path_params.comment_id)
        .await?;

    Ok(StatusCode::OK)
}

fn delete_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete a comment")
        .description("Soft-deletes a comment. Only the author may delete their own comment.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns an [`ApiRouter`] with the comment routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/threads/{threadId}/comments/",
            post_with(create_comment, create_comment_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/comments/{commentId}/",
            patch_with(update_comment, update_comment_docs)
                .delete_with(delete_comment, delete_comment_docs),
        )
        .with_path_items(|item| item.tag("Comments"))
}
