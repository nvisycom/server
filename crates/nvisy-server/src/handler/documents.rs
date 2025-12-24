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
use nvisy_postgres::model::NewDocument;
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
#[tracing::instrument(skip_all)]
async fn create_document(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        display_name = %request.display_name,
        "creating new document",
    );

    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::CreateDocuments,
        )
        .await?;

    let new_document = NewDocument {
        project_id: path_params.project_id,
        account_id: auth_claims.account_id,
        display_name: Some(request.display_name.clone()),
        ..Default::default()
    };

    let document = pg_client.create_document(new_document).await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        document_id = document.id.to_string(),
        "document created successfully",
    );

    Ok((StatusCode::CREATED, Json(document.into())))
}

/// Returns all documents for a project.
#[tracing::instrument(skip_all)]
async fn get_all_documents(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Documents>)> {
    auth_claims
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
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        document_count = response.len(),
        "listed project documents"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Gets a document by its document ID.
#[tracing::instrument(skip_all)]
async fn get_document(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<Document>)> {
    auth_claims
        .authorize_document(
            &pg_client,
            path_params.document_id,
            Permission::ViewDocuments,
        )
        .await?;

    let Some(document) = pg_client
        .find_document_by_id(path_params.document_id)
        .await?
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

/// Updates a document by its document ID.
#[tracing::instrument(skip_all)]
async fn update_document(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    ValidateJson(request): ValidateJson<UpdateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "updating document",
    );

    auth_claims
        .authorize_document(
            &pg_client,
            path_params.document_id,
            Permission::UpdateDocuments,
        )
        .await?;

    // Verify document exists before updating
    let Some(_existing_document) = pg_client
        .find_document_by_id(path_params.document_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("document"));
    };

    let update_data = nvisy_postgres::model::UpdateDocument {
        display_name: request.display_name,
        ..Default::default()
    };

    let document = pg_client
        .update_document(path_params.document_id, update_data)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document updated successfully",
    );

    Ok((StatusCode::OK, Json(document.into())))
}

/// Deletes a document by its document ID.
#[tracing::instrument(skip_all)]
async fn delete_document(
    State(pg_client): State<PgClient>,
    State(_nats_client): State<NatsClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document deletion requested - this is a destructive operation",
    );

    auth_claims
        .authorize_document(
            &pg_client,
            path_params.document_id,
            Permission::DeleteDocuments,
        )
        .await?;

    // Verify document exists before deleting
    let Some(_existing_document) = pg_client
        .find_document_by_id(path_params.document_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("document"));
    };

    pg_client.delete_document(path_params.document_id).await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        document_id = path_params.document_id.to_string(),
        "document deleted successfully",
    );

    Ok(StatusCode::OK)
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

