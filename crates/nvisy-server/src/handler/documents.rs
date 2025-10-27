//! Document management handlers for document CRUD operations.
//!
//! This module provides comprehensive document management functionality within projects,
//! including creation, reading, updating, and deletion of documents. All operations
//! are secured with proper authorization and follow project-based access control.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{Document, NewDocument, UpdateDocument};
use nvisy_postgres::query::DocumentRepository;
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
const TRACING_TARGET: &str = "nvisy_server::handler::documents";

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
#[schema(example = json!({
    "displayName": "Q4 Financial Report"
}))]
struct CreateDocumentRequest {
    /// Display name of the document.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
}

/// Response returned when a document is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateDocumentResponse {
    /// ID of the document.
    pub document_id: Uuid,
    /// Timestamp when the document was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated.
    pub updated_at: OffsetDateTime,
}

impl CreateDocumentResponse {
    /// Creates a new instance of [`CreateDocumentResponse`].
    pub fn new(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

impl From<Document> for CreateDocumentResponse {
    fn from(document: Document) -> Self {
        Self::new(document)
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
    State(pg_client): State<PgClient>,
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

    let mut conn = pg_client.get_connection().await?;

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
    /// ID of the document.
    pub document_id: Uuid,
    /// ID of the account that owns the document.
    pub account_id: Uuid,
    /// Display name of the document.
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListDocumentsResponse>)> {
    let mut conn = pg_client.get_connection().await?;

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
        pagination.into(),
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
    /// ID of the document.
    pub id: Uuid,
    /// ID of the project that the document belongs to.
    pub project_id: Uuid,
    /// ID of the account that owns the document.
    pub account_id: Uuid,
    /// Display name of the document.
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<GetDocumentResponse>)> {
    let mut conn = pg_client.get_connection().await?;

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
#[schema(example = json!({
    "displayName": "Updated Report Name"
}))]
struct UpdateDocumentRequest {
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
}

/// Response for updated document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateDocumentResponse {
    /// ID of the updated document.
    pub document_id: Uuid,
    /// Timestamp when the document was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated.
    pub updated_at: OffsetDateTime,
}

impl UpdateDocumentResponse {
    /// Creates a new instance of `UpdateDocumentResponse`.
    pub fn new(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

impl From<Document> for UpdateDocumentResponse {
    fn from(document: Document) -> Self {
        Self::new(document)
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    ValidateJson(request): ValidateJson<UpdateDocumentRequest>,
) -> Result<(StatusCode, Json<UpdateDocumentResponse>)> {
    let mut conn = pg_client.get_connection().await?;

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
    State(pg_client): State<PgClient>,
    // State(_storage): State<MinioClient>, // TODO: Replace with NATS object store
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<DeleteDocumentResponse>)> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document deletion requested - this is a destructive operation",
    );

    let mut conn = pg_client.get_connection().await?;

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
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn test_create_document_success() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = CreateDocumentRequest {
            display_name: "Test Document".to_string(),
        };

        let project_id = Uuid::new_v4();
        let response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;
        response.assert_status(StatusCode::CREATED);

        let body: CreateDocumentResponse = response.json();
        assert!(!body.document_id.is_nil());

        Ok(())
    }

    #[tokio::test]
    async fn test_create_document_empty_name() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = serde_json::json!({
            "displayName": ""
        });

        let project_id = Uuid::new_v4();
        let response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;
        response.assert_status_bad_request();

        Ok(())
    }

    #[tokio::test]
    async fn test_list_documents() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let project_id = Uuid::new_v4();

        // Create a document first
        let request = CreateDocumentRequest {
            display_name: "List Test Document".to_string(),
        };
        server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;

        // List documents
        let pagination = Pagination {
            offset: Some(0),
            limit: Some(10),
        };
        let response = server
            .get(&format!("/projects/{}/documents/", project_id))
            .json(&pagination)
            .await;
        response.assert_status_ok();

        Ok(())
    }

    #[tokio::test]
    async fn test_update_document() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let project_id = Uuid::new_v4();

        // Create a document
        let create_request = CreateDocumentRequest {
            display_name: "Original Name".to_string(),
        };
        let create_response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&create_request)
            .await;
        let created: CreateDocumentResponse = create_response.json();

        // Update the document
        let update_request = UpdateDocumentRequest {
            display_name: Some("Updated Name".to_string()),
        };

        let response = server
            .patch(&format!("/documents/{}/", created.document_id))
            .json(&update_request)
            .await;
        response.assert_status_ok();

        Ok(())
    }

    #[tokio::test]
    async fn test_get_document() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let project_id = Uuid::new_v4();

        // Create a document
        let request = CreateDocumentRequest {
            display_name: "Get Test".to_string(),
        };
        let create_response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;
        let created: CreateDocumentResponse = create_response.json();

        // Get the document
        let response = server
            .get(&format!("/documents/{}/", created.document_id))
            .await;
        response.assert_status_ok();

        let body: GetDocumentResponse = response.json();
        assert_eq!(body.id, created.document_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_document() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let project_id = Uuid::new_v4();

        // Create a document
        let request = CreateDocumentRequest {
            display_name: "Delete Test".to_string(),
        };
        let create_response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;
        let created: CreateDocumentResponse = create_response.json();

        // Delete the document
        let response = server
            .delete(&format!("/documents/{}/", created.document_id))
            .await;
        response.assert_status_ok();

        Ok(())
    }

    #[tokio::test]
    async fn test_get_nonexistent_document() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let fake_id = Uuid::new_v4();
        let response = server.get(&format!("/documents/{}/", fake_id)).await;
        response.assert_status_not_found();

        Ok(())
    }
}
