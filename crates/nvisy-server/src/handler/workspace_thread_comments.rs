//! Comment handlers: posting, editing, and deleting the messages within a
//! thread. The thread lifecycle (open/close/reopen/rename/delete), the review
//! transitions, and the timeline live in the sibling `workspace_threads` module,
//! which also owns the helpers shared here (mention resolution, assistant
//! enqueue, lookups).

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceThreadComment, UpdateWorkspaceThreadComment};
use nvisy_postgres::query::{WorkspaceThreadCommentRepository, WorkspaceThreadRepository};
use nvisy_postgres::{AsyncConnection, PgClient};

use crate::extract::{Authorized, Json, Path, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceComment, UpdateWorkspaceComment, WorkspaceCommentPathParams,
    WorkspaceThreadPathParams,
};
use crate::handler::response::WorkspaceComment;
use crate::handler::utility::resolve_account_ref;
use crate::handler::workspace_threads::{
    MentionOutcome, TRACING_TARGET, emit_thread_event, enqueue_assistant_if_addressed,
    find_comment, find_thread, resolve_mentions, workspace_origin,
};
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{AssistantQueue, ServiceState, event};

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
    State(assistant): State<AssistantQueue>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceComment>,
) -> Result<(StatusCode, Json<WorkspaceComment>)> {
    tracing::debug!(target: TRACING_TARGET, "Posting comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // The thread must exist in the workspace (and be live).
    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    let MentionOutcome {
        recipients,
        addressed_assistant,
    } = resolve_mentions(&mut conn, workspace.id, &request.body, authz.account_id).await?;

    let author_username = resolve_account_ref(&mut conn, authz.account_id)
        .await?
        .username;

    // Post the comment, record its event, and — if the assistant was addressed —
    // queue the reply job, all in one transaction so they commit together.
    let (comment, queued_assistant) = conn
        .transaction(async |conn| {
            // Lock the thread and re-check its closed state inside the transaction:
            // a closed thread is a finished discussion, and the row lock serializes
            // against a concurrent close so a comment (and its ThreadCommentCreated
            // event) can never land after ThreadClosed. Reopen to continue.
            let locked = conn
                .lock_thread_in_workspace(workspace.id, thread.id)
                .await?
                .ok_or_else(|| Error::not_found("workspace_thread"))?;
            if locked.closed_at.is_some() {
                return Err(ErrorKind::Conflict
                    .with_message("This thread is closed; reopen it before posting a comment"));
            }

            let comment = conn
                .create_comment(NewWorkspaceThreadComment {
                    workspace_id: workspace.id,
                    thread_id: thread.id,
                    author_account_id: authz.account_id,
                    parent_id: None,
                    body: request.body,
                })
                .await?;

            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                event::WorkspaceEvent::ThreadCommentCreated(event::ThreadCommentCreated {
                    comment_id: comment.id,
                    thread_id: thread.id,
                    document_id: thread.document_id,
                    author_username: author_username.clone(),
                    mentioned: recipients,
                }),
            )
            .await?;

            let queued = enqueue_assistant_if_addressed(
                conn,
                addressed_assistant,
                authz.account_id,
                workspace.id,
                thread.id,
                comment.id,
            )
            .await?;
            Ok::<_, Error>((comment, queued))
        })
        .await?;

    if queued_assistant {
        assistant.wake_drainer();
    }

    let author = resolve_account_ref(&mut conn, comment.author_account_id).await?;

    tracing::info!(target: TRACING_TARGET, comment_id = %comment.id, "WorkspaceComment posted");

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
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceCommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspaceComment>,
) -> Result<(StatusCode, Json<WorkspaceComment>)> {
    tracing::debug!(target: TRACING_TARGET, "Editing comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let comment = find_comment(&mut conn, workspace.id, path_params.comment_id).await?;

    // Only the author may edit their own comment.
    if comment.author_account_id != authz.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("Only the author can edit this comment")
            .with_resource("workspace_thread_comment"));
    }

    let updated = conn
        .update_comment_body(
            comment.id,
            UpdateWorkspaceThreadComment {
                body: Some(request.body),
            },
        )
        .await?;

    let author = resolve_account_ref(&mut conn, updated.author_account_id).await?;

    tracing::info!(target: TRACING_TARGET, "WorkspaceComment edited");

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
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceCommentPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let comment = find_comment(&mut conn, workspace.id, path_params.comment_id).await?;

    // Only the author may delete their own comment.
    if comment.author_account_id != authz.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("Only the author can delete this comment")
            .with_resource("workspace_thread_comment"));
    }

    conn.delete_comment(comment.id).await?;

    tracing::info!(target: TRACING_TARGET, "WorkspaceComment deleted");

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
            "/workspaces/{workspaceSlug}/threads/{threadId}/comments/",
            post_with(create_comment, create_comment_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/comments/{commentId}/",
            patch_with(update_comment, update_comment_docs)
                .delete_with(delete_comment, delete_comment_docs),
        )
        .with_path_items(|item| item.tag("Comments"))
}
