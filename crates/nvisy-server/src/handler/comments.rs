//! File comment management handlers for CRUD operations.
//!
//! This module provides comment management functionality for files.
//! Supports threaded conversations and @mentions.

use aide::axum::ApiRouter;
use axum::http::StatusCode;
use nvisy_postgres::query::{DocumentCommentRepository, DocumentFileRepository};

use crate::extract::{AuthState, Json, Path, PgPool, ValidateJson};
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
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn post_comment(
    PgPool(mut conn): PgPool,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FilePathParams>,
    ValidateJson(request): ValidateJson<CreateDocumentComment>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::info!(target: TRACING_TARGET, "Creating comment");

    // Verify file exists
    let _ = find_file(&mut conn, path_params.file_id).await?;

    // Validate parent comment if provided
    if let Some(parent_id) = request.parent_comment_id {
        let parent_comment = find_comment(&mut conn, parent_id).await?;

        // Verify parent comment is on the same file
        if parent_comment.file_id != path_params.file_id {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment must belong to the same file")
                .with_resource("comment"));
        }
    }

    let comment = conn
        .create_comment(request.into_model(auth_claims.account_id, path_params.file_id))
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        comment_id = %comment.id,
        "Comment created successfully",
    );

    Ok((StatusCode::CREATED, Json(comment.into())))
}

/// Returns all comments for a file.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn list_comments(
    PgPool(mut conn): PgPool,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FilePathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Comments>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing file comments");

    // Verify file exists
    let _ = find_file(&mut conn, path_params.file_id).await?;

    let comments = conn
        .find_comments_by_file(path_params.file_id, pagination.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        comment_count = comments.len(),
        "File comments listed successfully",
    );

    Ok((
        StatusCode::OK,
        Json(comments.into_iter().map(Into::into).collect()),
    ))
}

/// Updates a comment by ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
        comment_id = %path_params.comment_id,
    )
)]
async fn update_comment(
    PgPool(mut conn): PgPool,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FileCommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateCommentRequest>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::info!(target: TRACING_TARGET, "Updating comment");

    // Verify file exists
    let _ = find_file(&mut conn, path_params.file_id).await?;

    // Fetch comment and verify ownership
    let existing_comment = find_comment(&mut conn, path_params.comment_id).await?;

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

    let comment = conn
        .update_comment(path_params.comment_id, request.into_model())
        .await?;

    tracing::info!(target: TRACING_TARGET, "Comment updated successfully");

    Ok((StatusCode::OK, Json(comment.into())))
}

/// Deletes a comment by ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
        comment_id = %path_params.comment_id,
    )
)]
async fn delete_comment(
    PgPool(mut conn): PgPool,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FileCommentPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Comment deletion requested");

    // Verify file exists
    let _ = find_file(&mut conn, path_params.file_id).await?;

    // Fetch comment and verify ownership
    let existing_comment = find_comment(&mut conn, path_params.comment_id).await?;

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

    conn.delete_comment(path_params.comment_id).await?;

    tracing::warn!(target: TRACING_TARGET, "Comment deleted successfully");

    Ok(StatusCode::OK)
}

/// Finds a file by ID or returns NotFound error.
async fn find_file(
    conn: &mut nvisy_postgres::PgConn,
    file_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::DocumentFile> {
    conn.find_document_file_by_id(file_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("File not found")
                .with_resource("file")
        })
}

/// Finds a comment by ID or returns NotFound error.
async fn find_comment(
    conn: &mut nvisy_postgres::PgConn,
    comment_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::DocumentComment> {
    conn.find_comment_by_id(comment_id).await?.ok_or_else(|| {
        ErrorKind::NotFound
            .with_message("Comment not found")
            .with_resource("comment")
    })
}

/// Returns a [`Router`] with all comment-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/{project_id}/files/{file_id}/comments",
            post(post_comment),
        )
        .api_route(
            "/projects/{project_id}/files/{file_id}/comments",
            get(list_comments),
        )
        .api_route(
            "/projects/{project_id}/files/{file_id}/comments/{comment_id}",
            patch(update_comment),
        )
        .api_route(
            "/projects/{project_id}/files/{file_id}/comments/{comment_id}",
            delete(delete_comment),
        )
        .with_path_items(|item| item.tag("Comments"))
}
