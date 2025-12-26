//! Document management handlers for document CRUD operations.
//!
//! This module provides comprehensive document management functionality within projects,
//! including creation, reading, updating, and deletion of documents. All operations
//! are secured with proper authorization and follow project-based access control.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::DocumentRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::request::{
    CreateDocument, DocumentPathParams, Pagination, ProjectPathParams, UpdateDocument,
};
use crate::handler::response::{Document, Documents};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for document operations.
const TRACING_TARGET: &str = "nvisy_server::handler::documents";

/// Creates a new document.
///
/// Creates a document container for organizing files. Requires `CreateDocuments` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn create_document(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::info!(target: TRACING_TARGET, "Creating new document");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::CreateDocuments,
        )
        .await?;

    let new_document = request.into_model(path_params.project_id, auth_state.account_id);
    let document = pg_client.create_document(new_document).await?;

    tracing::info!(
        target: TRACING_TARGET,
        document_id = %document.id,
        "Document created successfully",
    );

    Ok((StatusCode::CREATED, Json(document.into())))
}

/// Returns all documents for a project.
///
/// Lists documents with pagination. Requires `ViewDocuments` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn get_all_documents(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Documents>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project documents");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewDocuments,
        )
        .await?;

    let documents = pg_client
        .find_documents_by_project(path_params.project_id, pagination.into())
        .await?;

    let response: Documents = documents.into_iter().map(Document::from).collect();

    tracing::debug!(
        target: TRACING_TARGET,
        document_count = response.len(),
        "Project documents listed successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Gets a document by its document ID.
///
/// Returns document details. Requires `ViewDocuments` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        document_id = %path_params.document_id,
    )
)]
async fn get_document(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading document");

    auth_state
        .authorize_document(
            &pg_client,
            path_params.document_id,
            Permission::ViewDocuments,
        )
        .await?;

    let document = find_document(&pg_client, path_params.document_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Document retrieved successfully");

    Ok((StatusCode::OK, Json(document.into())))
}

/// Updates a document by its document ID.
///
/// Updates document metadata. Requires `UpdateDocuments` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        document_id = %path_params.document_id,
    )
)]
async fn update_document(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    ValidateJson(request): ValidateJson<UpdateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::info!(target: TRACING_TARGET, "Updating document");

    auth_state
        .authorize_document(
            &pg_client,
            path_params.document_id,
            Permission::UpdateDocuments,
        )
        .await?;

    // Verify document exists
    let _ = find_document(&pg_client, path_params.document_id).await?;

    let update_data = request.into_model();
    let document = pg_client
        .update_document(path_params.document_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Document updated successfully");

    Ok((StatusCode::OK, Json(document.into())))
}

/// Deletes a document by its document ID.
///
/// Soft-deletes the document and associated files. Requires `DeleteDocuments` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        document_id = %path_params.document_id,
    )
)]
async fn delete_document(
    State(pg_client): State<PgClient>,
    State(_nats_client): State<NatsClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Document deletion requested");

    auth_state
        .authorize_document(
            &pg_client,
            path_params.document_id,
            Permission::DeleteDocuments,
        )
        .await?;

    // Verify document exists
    let _ = find_document(&pg_client, path_params.document_id).await?;

    pg_client.delete_document(path_params.document_id).await?;

    tracing::warn!(target: TRACING_TARGET, "Document deleted successfully");

    Ok(StatusCode::OK)
}

/// Finds a document by ID or returns NotFound error.
async fn find_document(
    pg_client: &PgClient,
    document_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::Document> {
    pg_client
        .find_document_by_id(document_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Document not found")
                .with_resource("document")
        })
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/projects/:project_id/documents", post(create_document))
        .api_route("/projects/:project_id/documents", get(get_all_documents))
        .api_route("/documents/:document_id", get(get_document))
        .api_route("/documents/:document_id", patch(update_document))
        .api_route("/documents/:document_id", delete(delete_document))
}
