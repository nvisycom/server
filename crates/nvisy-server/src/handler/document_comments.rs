//! Document comment management handlers for CRUD operations.
//!
//! This module provides comprehensive comment management functionality for documents,
//! files, and versions. Supports threaded conversations and @mentions.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::NewDocumentComment;
use nvisy_postgres::query::{
    DocumentCommentRepository, DocumentFileRepository, DocumentRepository,
    DocumentVersionRepository,
};
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthState, Json, Path, ValidateJson};
use crate::handler::documents::DocumentPathParams;
use crate::handler::request::{
    CreateDocumentComment, UpdateDocumentComment as UpdateCommentRequest,
};
use crate::handler::response::{DocumentComment, DocumentComments};
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for document comment operations.
const TRACING_TARGET: &str = "nvisy_server::handler::document_comments";

/// Combined path params for document and comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCommentPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
    /// Unique identifier of the comment.
    pub comment_id: Uuid,
}

/// Path params for file ID.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct FilePathParams {
    /// Unique identifier of the file.
    pub file_id: Uuid,
}

/// Path params for version ID.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct VersionPathParams {
    /// Unique identifier of the version.
    pub version_id: Uuid,
}

/// Creates a new comment on a document.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/documents/{documentId}/comments/",
    params(DocumentPathParams), tag = "comments",
    request_body(
        content = CreateDocumentComment,
        description = "New comment",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = CREATED,
            description = "Comment created",
            body = DocumentComment,
        ),
    ),
)]
async fn create_document_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    ValidateJson(request): ValidateJson<CreateDocumentComment>,
) -> Result<(StatusCode, Json<DocumentComment>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "Creating new comment on document",
    );

    let mut conn = pg_client.get_connection().await?;

    // Verify document exists and user has access
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    // Validate parent comment if provided
    if let Some(parent_id) = request.parent_comment_id {
        let Some(parent_comment) =
            DocumentCommentRepository::find_comment_by_id(&mut conn, parent_id).await?
        else {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment not found")
                .with_resource("comment"));
        };

        // Verify parent comment is on the same document
        if parent_comment.document_id != Some(path_params.document_id) {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment must belong to the same document")
                .with_resource("comment"));
        }
    }

    let new_comment = NewDocumentComment {
        document_id: Some(path_params.document_id),
        account_id: auth_claims.account_id,
        parent_comment_id: request.parent_comment_id,
        reply_to_account_id: request.reply_to_account_id,
        content: request.content.clone(),
        ..Default::default()
    };

    let comment = DocumentCommentRepository::create_comment(&mut conn, new_comment).await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        comment_id = comment.id.to_string(),
        "Comment created successfully",
    );

    Ok((StatusCode::CREATED, Json(comment.into())))
}

/// Returns all comments for a document.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/documents/{documentId}/comments/", tag = "comments",
    params(DocumentPathParams),
    request_body(
        content = Pagination,
        description = "Pagination parameters",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Comments listed",
            body = DocumentComments,
        ),
    )
)]
async fn list_document_comments(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<DocumentComments>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document exists and user has access
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    let comments = DocumentCommentRepository::find_comments_by_document(
        &mut conn,
        path_params.document_id,
        pagination.into(),
    )
    .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        comment_count = comments.len(),
        "Listed document comments"
    );

    Ok((
        StatusCode::OK,
        Json(comments.into_iter().map(Into::into).collect()),
    ))
}

/// Returns top-level comments for a document (excludes replies).
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/documents/{documentId}/comments/top-level/", tag = "comments",
    params(DocumentPathParams),
    request_body(
        content = Pagination,
        description = "Pagination parameters",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Top-level comments listed",
            body = DocumentComments,
        ),
    )
)]
async fn list_top_level_comments(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<DocumentComments>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document exists and user has access
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    let comments = DocumentCommentRepository::find_top_level_comments_by_document(
        &mut conn,
        path_params.document_id,
        pagination.into(),
    )
    .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        comment_count = comments.len(),
        "Listed top-level comments"
    );

    Ok((
        StatusCode::OK,
        Json(comments.into_iter().map(Into::into).collect()),
    ))
}

/// Gets a specific comment by ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/documents/{documentId}/comments/{commentId}/", tag = "comments",
    params(DocumentCommentPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Comment not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Comment details",
            body = DocumentComment,
        ),
    ),
)]
async fn get_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentCommentPathParams>,
) -> Result<(StatusCode, Json<DocumentComment>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document exists
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    let Some(comment) =
        DocumentCommentRepository::find_comment_by_id(&mut conn, path_params.comment_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Comment not found: {}", path_params.comment_id))
            .with_resource("comment"));
    };

    // Verify comment belongs to the document in the path
    if comment.document_id != Some(path_params.document_id) {
        return Err(ErrorKind::NotFound
            .with_message("Comment does not belong to this document")
            .with_resource("comment"));
    }

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Retrieved comment details"
    );

    Ok((StatusCode::OK, Json(comment.into())))
}

/// Returns all replies to a comment.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/documents/{documentId}/comments/{commentId}/replies/", tag = "comments",
    params(DocumentCommentPathParams),
    request_body(
        content = Pagination,
        description = "Pagination parameters",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Replies listed",
            body = DocumentComments,
        ),
    )
)]
async fn list_comment_replies(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentCommentPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<DocumentComments>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document exists
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    // Verify comment exists and belongs to document
    let Some(parent_comment) =
        DocumentCommentRepository::find_comment_by_id(&mut conn, path_params.comment_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Comment not found: {}", path_params.comment_id))
            .with_resource("comment"));
    };

    if parent_comment.document_id != Some(path_params.document_id) {
        return Err(ErrorKind::NotFound
            .with_message("Comment does not belong to this document")
            .with_resource("comment"));
    }

    let replies = DocumentCommentRepository::find_comment_replies(
        &mut conn,
        path_params.comment_id,
        pagination.into(),
    )
    .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        reply_count = replies.len(),
        "Listed comment replies"
    );

    Ok((
        StatusCode::OK,
        Json(replies.into_iter().map(Into::into).collect()),
    ))
}

/// Updates a comment by ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/documents/{documentId}/comments/{commentId}/", tag = "comments",
    params(DocumentCommentPathParams),
    request_body(
        content = UpdateCommentRequest,
        description = "Comment changes",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Not authorized to update this comment",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Comment not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Comment updated",
            body = DocumentComment,
        ),
    ),
)]
async fn update_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentCommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateCommentRequest>,
) -> Result<(StatusCode, Json<DocumentComment>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Updating comment",
    );

    // Verify document exists
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    // Fetch comment and verify ownership in one query
    let Some(existing_comment) =
        DocumentCommentRepository::find_comment_by_id(&mut conn, path_params.comment_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Comment not found: {}", path_params.comment_id))
            .with_resource("comment"));
    };

    // Verify comment belongs to the document in the path
    if existing_comment.document_id != Some(path_params.document_id) {
        return Err(ErrorKind::NotFound
            .with_message("Comment does not belong to this document")
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

    let comment =
        DocumentCommentRepository::update_comment(&mut conn, path_params.comment_id, update_data)
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
#[utoipa::path(
    delete, path = "/documents/{documentId}/comments/{commentId}/", tag = "comments",
    params(DocumentCommentPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Not authorized to delete this comment",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Comment not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Comment deleted",
        ),
    ),
)]
async fn delete_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentCommentPathParams>,
) -> Result<StatusCode> {
    let mut conn = pg_client.get_connection().await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Comment deletion requested",
    );

    // Verify document exists
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", path_params.document_id))
            .with_resource("document"));
    };

    // Fetch comment and verify ownership in one query
    let Some(existing_comment) =
        DocumentCommentRepository::find_comment_by_id(&mut conn, path_params.comment_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Comment not found: {}", path_params.comment_id))
            .with_resource("comment"));
    };

    // Verify comment belongs to the document in the path
    if existing_comment.document_id != Some(path_params.document_id) {
        return Err(ErrorKind::NotFound
            .with_message("Comment does not belong to this document")
            .with_resource("comment"));
    }

    // Check ownership
    if existing_comment.account_id != auth_claims.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("You can only delete your own comments")
            .with_resource("comment"));
    }

    DocumentCommentRepository::delete_comment(&mut conn, path_params.comment_id).await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        comment_id = path_params.comment_id.to_string(),
        "Comment deleted successfully",
    );

    Ok(StatusCode::OK)
}

// ============================================================================
// File Comment Handlers
// ============================================================================

/// Creates a new comment on a file.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/files/{fileId}/comments/",
    params(FilePathParams), tag = "comments",
    request_body(
        content = CreateDocumentComment,
        description = "New comment",
        content_type = "application/json",
    ),
    responses(
        (status = BAD_REQUEST, description = "Bad request", body = ErrorResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Internal server error", body = ErrorResponse),
        (status = CREATED, description = "Comment created", body = DocumentComment),
    ),
)]
async fn create_file_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FilePathParams>,
    ValidateJson(request): ValidateJson<CreateDocumentComment>,
) -> Result<(StatusCode, Json<DocumentComment>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify file exists and get document_id
    let Some(file) =
        DocumentFileRepository::find_document_file_by_id(&mut conn, path_params.file_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("File not found: {}", path_params.file_id))
            .with_resource("file"));
    };

    // Verify user has access to the document
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, file.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", file.document_id))
            .with_resource("document"));
    };

    // Validate parent comment if provided
    if let Some(parent_id) = request.parent_comment_id {
        let Some(parent_comment) =
            DocumentCommentRepository::find_comment_by_id(&mut conn, parent_id).await?
        else {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment not found")
                .with_resource("comment"));
        };

        if parent_comment.document_file_id != Some(path_params.file_id) {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment must belong to the same file")
                .with_resource("comment"));
        }
    }

    let new_comment = NewDocumentComment {
        document_file_id: Some(path_params.file_id),
        account_id: auth_claims.account_id,
        parent_comment_id: request.parent_comment_id,
        reply_to_account_id: request.reply_to_account_id,
        content: request.content.clone(),
        ..Default::default()
    };

    let comment = DocumentCommentRepository::create_comment(&mut conn, new_comment).await?;

    Ok((StatusCode::CREATED, Json(comment.into())))
}

/// Returns all comments for a file.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/files/{fileId}/comments/", tag = "comments",
    params(FilePathParams),
    request_body(content = Pagination, description = "Pagination parameters"),
    responses(
        (status = OK, description = "Comments listed", body = DocumentComments),
    )
)]
async fn list_file_comments(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<FilePathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<DocumentComments>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify file exists and get document_id
    let Some(file) =
        DocumentFileRepository::find_document_file_by_id(&mut conn, path_params.file_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("File not found: {}", path_params.file_id))
            .with_resource("file"));
    };

    // Verify user has access to the document
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, file.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", file.document_id))
            .with_resource("document"));
    };

    let comments = DocumentCommentRepository::find_comments_by_file(
        &mut conn,
        path_params.file_id,
        pagination.into(),
    )
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

// ============================================================================
// Version Comment Handlers
// ============================================================================

/// Creates a new comment on a version.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/versions/{versionId}/comments/",
    params(VersionPathParams), tag = "comments",
    request_body(
        content = CreateDocumentComment,
        description = "New comment",
        content_type = "application/json",
    ),
    responses(
        (status = BAD_REQUEST, description = "Bad request", body = ErrorResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Internal server error", body = ErrorResponse),
        (status = CREATED, description = "Comment created", body = DocumentComment),
    ),
)]
async fn create_version_comment(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<VersionPathParams>,
    ValidateJson(request): ValidateJson<CreateDocumentComment>,
) -> Result<(StatusCode, Json<DocumentComment>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify version exists and get document_id
    let Some(version) =
        DocumentVersionRepository::find_document_version_by_id(&mut conn, path_params.version_id)
            .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Version not found: {}", path_params.version_id))
            .with_resource("version"));
    };

    // Verify user has access to the document
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, version.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", version.document_id))
            .with_resource("document"));
    };

    // Validate parent comment if provided
    if let Some(parent_id) = request.parent_comment_id {
        let Some(parent_comment) =
            DocumentCommentRepository::find_comment_by_id(&mut conn, parent_id).await?
        else {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment not found")
                .with_resource("comment"));
        };

        if parent_comment.document_version_id != Some(path_params.version_id) {
            return Err(ErrorKind::BadRequest
                .with_message("Parent comment must belong to the same version")
                .with_resource("comment"));
        }
    }

    let new_comment = NewDocumentComment {
        document_version_id: Some(path_params.version_id),
        account_id: auth_claims.account_id,
        parent_comment_id: request.parent_comment_id,
        reply_to_account_id: request.reply_to_account_id,
        content: request.content.clone(),
        ..Default::default()
    };

    let comment = DocumentCommentRepository::create_comment(&mut conn, new_comment).await?;

    Ok((StatusCode::CREATED, Json(comment.into())))
}

/// Returns all comments for a version.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/versions/{versionId}/comments/", tag = "comments",
    params(VersionPathParams),
    request_body(content = Pagination, description = "Pagination parameters"),
    responses(
        (status = OK, description = "Comments listed", body = DocumentComments),
    )
)]
async fn list_version_comments(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<VersionPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<DocumentComments>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify version exists and get document_id
    let Some(version) =
        DocumentVersionRepository::find_document_version_by_id(&mut conn, path_params.version_id)
            .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Version not found: {}", path_params.version_id))
            .with_resource("version"));
    };

    // Verify user has access to the document
    let Some(_document) =
        DocumentRepository::find_document_by_id(&mut conn, version.document_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Document not found: {}", version.document_id))
            .with_resource("document"));
    };

    let comments = DocumentCommentRepository::find_comments_by_version(
        &mut conn,
        path_params.version_id,
        pagination.into(),
    )
    .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        version_id = path_params.version_id.to_string(),
        comment_count = comments.len(),
        "Listed version comments"
    );

    Ok((
        StatusCode::OK,
        Json(comments.into_iter().map(Into::into).collect()),
    ))
}

/// Returns a [`Router`] with all comment-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        // Document comment routes
        .routes(routes!(
            create_document_comment,
            list_document_comments,
            list_top_level_comments
        ))
        .routes(routes!(
            get_comment,
            list_comment_replies,
            update_comment,
            delete_comment
        ))
        // File comment routes
        .routes(routes!(create_file_comment, list_file_comments))
        // Version comment routes
        .routes(routes!(create_version_comment, list_version_comments))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn test_create_comment_success() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let document_id = Uuid::new_v4();
        let request = CreateDocumentComment {
            content: "This is a test comment".to_string(),
            parent_comment_id: None,
            reply_to_account_id: None,
        };

        let response = server
            .post(&format!("/documents/{}/comments/", document_id))
            .json(&request)
            .await;
        response.assert_status(StatusCode::CREATED);

        let body: DocumentComment = response.json();
        assert!(!body.comment_id.is_nil());
        assert_eq!(body.content, Some("This is a test comment".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_list_comments() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let document_id = Uuid::new_v4();

        // Create a comment first
        let request = CreateDocumentComment {
            content: "Test comment for listing".to_string(),
            parent_comment_id: None,
            reply_to_account_id: None,
        };
        server
            .post(&format!("/documents/{}/comments/", document_id))
            .json(&request)
            .await;

        // List comments
        let pagination = Pagination::default().with_limit(10);
        let response = server
            .get(&format!("/documents/{}/comments/", document_id))
            .json(&pagination)
            .await;
        response.assert_status_ok();

        let body: DocumentComments = response.json();
        assert!(!body.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_comment() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let document_id = Uuid::new_v4();

        // Create a comment
        let create_request = CreateDocumentComment {
            content: "Original comment".to_string(),
            parent_comment_id: None,
            reply_to_account_id: None,
        };
        let create_response = server
            .post(&format!("/documents/{}/comments/", document_id))
            .json(&create_request)
            .await;
        let created: DocumentComment = create_response.json();

        // Update the comment
        let update_request = UpdateCommentRequest {
            content: Some("Updated comment".to_string()),
        };

        let response = server
            .patch(&format!(
                "/documents/{}/comments/{}/",
                document_id, created.comment_id
            ))
            .json(&update_request)
            .await;
        response.assert_status_ok();

        let body: DocumentComment = response.json();
        assert_eq!(body.content, Some("Updated comment".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_comment() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let document_id = Uuid::new_v4();

        // Create a comment
        let request = CreateDocumentComment {
            content: "Comment to delete".to_string(),
            parent_comment_id: None,
            reply_to_account_id: None,
        };
        let create_response = server
            .post(&format!("/documents/{}/comments/", document_id))
            .json(&request)
            .await;
        let created: DocumentComment = create_response.json();

        // Delete the comment
        let response = server
            .delete(&format!(
                "/documents/{}/comments/{}/",
                document_id, created.comment_id
            ))
            .await;
        response.assert_status_ok();

        // Verify it's deleted by trying to read it
        let response = server
            .get(&format!(
                "/documents/{}/comments/{}/",
                document_id, created.comment_id
            ))
            .await;
        response.assert_status_not_found();

        Ok(())
    }
}
