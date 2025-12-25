//! File comment management handlers for CRUD operations.
//!
//! This module provides comment management functionality for files.
//! Supports threaded conversations and @mentions.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::NewDocumentComment;
use nvisy_postgres::query::{DocumentCommentRepository, DocumentFileRepository};

use crate::extract::{AuthState, Json, Path, ValidateJson};
use crate::handler::request::{
    CreateDocumentComment, FileCommentPathParams, FilePathParams, Pagination,
    UpdateDocumentComment as UpdateCommentRequest,
};
use crate::handler::response::{Comment, Comments};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for file comment operations.
const TRACING_TARGET: &str = "nvisy_server::handler::comments";

/// Creates a new comment on a file.
#[tracing::instrument(skip_all)]
async fn post_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FilePathParams>,
    ValidateJson(request): ValidateJson<CreateDocumentComment>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        file_id = path_params.file_id.to_string(),
        "Creating new comment on file",
    );

    // Verify file exists
    let Some(_file) = pg_client
        .find_document_file_by_id(path_params.file_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("File not found: {}", path_params.file_id))
            .with_resource("file"));
    };

    // Validate parent comment if provided
    if let Some(parent_id) = request.parent_comment_id {
        let Some(parent_comment) = pg_client.find_comment_by_id(parent_id).await? else {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment not found")
                .with_resource("comment"));
        };

        // Verify parent comment is on the same file
        if parent_comment.file_id != path_params.file_id {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment must belong to the same file")
                .with_resource("comment"));
        }
    }

    let new_comment = NewDocumentComment {
        file_id: path_params.file_id,
        account_id: auth_claims.account_id,
        parent_comment_id: request.parent_comment_id,
        reply_to_account_id: request.reply_to_account_id,
        content: request.content.clone(),
        ..Default::default()
    };

    let comment = pg_client.create_comment(new_comment).await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        file_id = path_params.file_id.to_string(),
        comment_id = comment.id.to_string(),
        "Comment created successfully",
    );

    Ok((StatusCode::CREATED, Json(comment.into())))
}

/// Returns all comments for a file.
#[tracing::instrument(skip_all)]
async fn list_comments(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FilePathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Comments>)> {
    // Verify file exists
    let Some(_file) = pg_client
        .find_document_file_by_id(path_params.file_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("File not found: {}", path_params.file_id))
            .with_resource("file"));
    };

    let comments = pg_client
        .find_comments_by_file(path_params.file_id, pagination.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        file_id = path_params.file_id.to_string(),
        comment_count = comments.len(),
        "Listed file comments"
    );

    Ok((
        StatusCode::OK,
        Json(comments.into_iter().map(Into::into).collect()),
    ))
}

/// Updates a comment by ID.
#[tracing::instrument(skip_all)]
async fn update_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FileCommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateCommentRequest>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Updating comment",
    );

    // Verify file exists
    let Some(_file) = pg_client
        .find_document_file_by_id(path_params.file_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("File not found: {}", path_params.file_id))
            .with_resource("file"));
    };

    // Fetch comment and verify ownership
    let Some(existing_comment) = pg_client.find_comment_by_id(path_params.comment_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Comment not found: {}", path_params.comment_id))
            .with_resource("comment"));
    };

    // Verify comment belongs to the file in the path
    if existing_comment.file_id != path_params.file_id {
        return Err(ErrorKind::NotFound
            .with_message("Comment does not belong to this file")
            .with_resource("comment"));
    }

    // Check ownership
    if existing_comment.account_id != auth_claims.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("You can only update your own comments")
            .with_resource("comment"));
    }

    let update_data = nvisy_postgres::model::UpdateDocumentComment {
        content: request.content,
        ..Default::default()
    };

    let comment = pg_client
        .update_comment(path_params.comment_id, update_data)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Comment updated successfully",
    );

    Ok((StatusCode::OK, Json(comment.into())))
}

/// Deletes a comment by ID.
#[tracing::instrument(skip_all)]
async fn delete_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FileCommentPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Comment deletion requested",
    );

    // Verify file exists
    let Some(_file) = pg_client
        .find_document_file_by_id(path_params.file_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("File not found: {}", path_params.file_id))
            .with_resource("file"));
    };

    // Fetch comment and verify ownership
    let Some(existing_comment) = pg_client.find_comment_by_id(path_params.comment_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Comment not found: {}", path_params.comment_id))
            .with_resource("comment"));
    };

    // Verify comment belongs to the file in the path
    if existing_comment.file_id != path_params.file_id {
        return Err(ErrorKind::NotFound
            .with_message("Comment does not belong to this file")
            .with_resource("comment"));
    }

    // Check ownership
    if existing_comment.account_id != auth_claims.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("You can only delete your own comments")
            .with_resource("comment"));
    }

    pg_client.delete_comment(path_params.comment_id).await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Comment deleted successfully",
    );

    Ok(StatusCode::OK)
}

/// Returns a [`Router`] with all comment-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/:project_id/files/:file_id/comments",
            post(post_comment),
        )
        .api_route(
            "/projects/:project_id/files/:file_id/comments",
            get(list_comments),
        )
        .api_route(
            "/projects/:project_id/files/:file_id/comments/:comment_id",
            patch(update_comment),
        )
        .api_route(
            "/projects/:project_id/files/:file_id/comments/:comment_id",
            delete(delete_comment),
        )
}
