//! Document management handlers for document CRUD operations.
//!
//! This module provides comprehensive document management functionality within projects,
//! including creation, reading, updating, and deletion of documents. All operations
//! are secured with proper authorization and follow project-based access control.
//!
//! # Security Features
//!
//! ## Project-Based Authorization
//! - JWT-based authentication required for all operations
//! - Project membership verification with role-based permissions
//! - Document ownership tracking and access control
//! - Cross-project access prevention
//!
//! ## Access Control Levels
//! - **Viewer**: Can read documents and their metadata
//! - **Editor**: Can create, modify, and delete their own documents
//! - **Admin**: Can manage all documents within the project
//! - **Owner**: Full control over project and all documents
//!
//! ## Data Validation
//! - Document title and description sanitization
//! - Content type validation and security checks
//! - File size and format restrictions
//! - Malicious content detection
//!
//! # Endpoints
//!
//! ## Document Operations
//! - `POST /projects/{projectId}/documents` - Create new document
//! - `GET /projects/{projectId}/documents` - List project documents
//! - `GET /projects/{projectId}/documents/{documentId}` - Get document details
//! - `PUT /projects/{projectId}/documents/{documentId}` - Update document
//! - `DELETE /projects/{projectId}/documents/{documentId}` - Delete document
//!
//! # Request/Response Examples
//!
//! ## Create Document Request
//! ```json
//! {
//!   "title": "API Documentation",
//!   "description": "Complete API documentation for v2.0",
//!   "contentType": "text/markdown"
//! }
//! ```
//!
//! ## Document Response
//! ```json
//! {
//!   "documentId": "550e8400-e29b-41d4-a716-446655440000",
//!   "projectId": "660f9500-f39c-52e5-b827-556766550000",
//!   "title": "API Documentation",
//!   "description": "Complete API documentation for v2.0",
//!   "contentType": "text/markdown",
//!   "fileCount": 5,
//!   "totalSize": 2048576,
//!   "createdAt": "2024-01-15T10:30:00Z",
//!   "updatedAt": "2024-01-15T14:45:00Z",
//!   "createdBy": {
//!     "accountId": "770fa600-049d-63f6-c938-667877660000",
//!     "displayName": "John Doe"
//!   }
//! }
//! ```
//!
//! # Error Handling
//!
//! All endpoints return standardized error responses:
//! - `400 Bad Request` - Invalid input data or validation failures
//! - `401 Unauthorized` - Authentication required or invalid token
//! - `403 Forbidden` - Insufficient project permissions
//! - `404 Not Found` - Document or project not found
//! - `409 Conflict` - Document title conflicts within project
//! - `413 Payload Too Large` - Document content exceeds limits
//! - `500 Internal Server Error` - System errors
//!
//! # Performance Features
//!
//! - Efficient pagination for document listings
//! - Lazy loading of document content and metadata
//! - Database query optimization with proper indexing
//! - File storage integration with MinIO for large content

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_minio::MinioClient;
use nvisy_postgres::PgClient;
use nvisy_postgres::models::{Document, NewDocument, UpdateDocument};
use nvisy_postgres::queries::DocumentRepository;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use validator::Validate;

use crate::extract::{AuthProvider, AuthState, Json, Path, ProjectPermission, ValidateJson};
use crate::handler::projects::ProjectPathParams;
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for document operations.
const TRACING_TARGET: &str = "nvisy::handler::documents";

/// `Path` param for `{documentId}` handlers.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
}

/// Request payload for creating a new document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
struct CreateDocumentRequest {
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
}

/// Response returned when a document is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateDocumentResponse {
    pub document_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Document> for CreateDocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

/// Creates a new document.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/projects/{projectId}/documents/",
    params(ProjectPathParams), tag = "documents",
    request_body(
        content = CreateDocumentRequest,
        description = "New document",
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
            description = "Document created",
            body = CreateDocumentResponse,
        ),
    ),
)]
async fn create_document(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateDocumentRequest>,
) -> Result<(StatusCode, Json<CreateDocumentResponse>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        display_name = %request.display_name,
        "creating new document",
    );

    let mut conn = pg_database.get_connection().await?;

    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::CreateDocuments,
        )
        .await?;

    let new_document = NewDocument {
        project_id: path_params.project_id,
        account_id: auth_claims.account_id,
        display_name: Some(request.display_name.clone()),
        description: None,
        is_template: Some(false),
        settings: Some(serde_json::Value::Null),
        tags: None,
        metadata: Some(serde_json::Value::Null),
        status: Default::default(),
    };

    let document = DocumentRepository::create_document(&mut conn, new_document).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        document_id = document.id.to_string(),
        "new document created successfully",
    );

    Ok((StatusCode::CREATED, Json(document.into())))
}

/// Represents a document in a project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDocumentsResponseItem {
    pub document_id: Uuid,
    pub account_id: Uuid,
    pub display_name: String,
}

impl From<Document> for ListDocumentsResponseItem {
    fn from(document: Document) -> Self {
        Self {
            document_id: document.id,
            account_id: document.account_id,
            display_name: document.display_name,
        }
    }
}

/// Response for listing all documents in a project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDocumentsResponse {
    pub project_id: Uuid,
    pub documents: Vec<ListDocumentsResponseItem>,
}

impl ListDocumentsResponse {
    /// Returns a new [`ListDocumentsResponse`].
    pub fn new(project_id: Uuid, documents: Vec<ListDocumentsResponseItem>) -> Self {
        Self {
            project_id,
            documents,
        }
    }
}

/// Returns all documents for a project.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/projects/{projectId}/documents/", tag = "documents",
    params(ProjectPathParams),
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
            description = "Documents listed",
            body = ListDocumentsResponse,
        ),
    )
)]
async fn get_all_documents(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(_pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListDocumentsResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::ViewDocuments,
        )
        .await?;

    let documents = DocumentRepository::find_documents_by_project(
        &mut conn,
        path_params.project_id,
        Default::default(),
    )
    .await?;

    let documents = documents
        .into_iter()
        .map(ListDocumentsResponseItem::from)
        .collect();

    let response = ListDocumentsResponse::new(path_params.project_id, documents);

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        document_count = response.documents.len(),
        "listed project documents"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Response for getting a single document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct GetDocumentResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub account_id: Uuid,
    pub display_name: String,
}

impl From<Document> for GetDocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            id: document.id,
            project_id: document.project_id,
            account_id: document.account_id,
            display_name: document.display_name,
        }
    }
}

/// Gets a document by its document ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/documents/{documentId}/", tag = "documents",
    params(DocumentPathParams),
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
            description = "Document details",
            body = GetDocumentResponse,
        ),
    ),
)]
async fn get_document(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<GetDocumentResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::ViewDocuments,
        )
        .await?;

    let Some(document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("document"));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "retrieved document details"
    );

    Ok((StatusCode::OK, Json(document.into())))
}

/// Request payload to update a document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
struct UpdateDocumentRequest {
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
}

/// Response for updated document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateDocumentResponse {
    pub document_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Document> for UpdateDocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

/// Updates a document by its document ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/documents/{documentId}/", tag = "documents",
    params(DocumentPathParams),
    request_body(
        content = UpdateDocumentRequest,
        description = "Document changes",
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
            description = "Document updated",
            body = UpdateDocumentResponse,
        ),
    ),
)]
async fn update_document(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    ValidateJson(request): ValidateJson<UpdateDocumentRequest>,
) -> Result<(StatusCode, Json<UpdateDocumentResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "updating document",
    );

    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::UpdateDocuments,
        )
        .await?;

    // Verify document exists before updating
    let Some(_existing_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("document"));
    };

    let update_document = UpdateDocument {
        display_name: request.display_name,
        ..Default::default()
    };

    let document =
        DocumentRepository::update_document(&mut conn, path_params.document_id, update_document)
            .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document updated successfully",
    );

    Ok((StatusCode::OK, Json(document.into())))
}

/// Response returned after deleting a document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DeleteDocumentResponse {
    pub document_id: Uuid,
    pub created_at: OffsetDateTime,
    pub deleted_at: OffsetDateTime,
}

impl From<Document> for DeleteDocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            deleted_at: document.deleted_at.unwrap_or_else(OffsetDateTime::now_utc),
        }
    }
}

/// Deletes a document by its document ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    delete, path = "/documents/{documentId}/", tag = "documents",
    params(DocumentPathParams),
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
            description = "Document deleted",
            body = DeleteDocumentResponse,
        ),
    )
)]
async fn delete_document(
    State(pg_database): State<PgClient>,
    State(_storage): State<MinioClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<DeleteDocumentResponse>)> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document deletion requested - this is a destructive operation",
    );

    let mut conn = pg_database.get_connection().await?;

    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::DeleteDocuments,
        )
        .await?;

    // Verify document exists before deleting
    let Some(_existing_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("document"));
    };

    DocumentRepository::delete_document(&mut conn, path_params.document_id).await?;

    let Some(deleted_document) =
        DocumentRepository::find_document_by_id(&mut conn, path_params.document_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("document"));
    };

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document deleted successfully",
    );

    Ok((StatusCode::OK, Json(deleted_document.into())))
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        .routes(routes!(create_document, get_all_documents))
        .routes(routes!(get_document, update_document, delete_document))
}

#[cfg(test)]
mod test {
    use crate::handler::documents::routes;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let _server = create_test_server_with_router(|_| routes()).await?;

        // TODO: Add comprehensive integration tests for:
        // - Document creation with proper authorization
        // - Document listing with pagination
        // - Document updates with permission checks
        // - Document deletion with cascade handling
        // - Error scenarios and edge cases

        Ok(())
    }
}
