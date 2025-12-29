//! Document management handlers for document CRUD operations.
//!
//! This module provides comprehensive document management functionality within projects,
//! including creation, reading, updating, and deletion of documents. All operations
//! are secured with proper authorization and follow project-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_nats::NatsClient;
use nvisy_postgres::query::DocumentRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{
    CreateDocument, DocumentPathParams, Pagination, ProjectPathParams, UpdateDocument,
};
use crate::handler::response::{Document, Documents, ErrorResponse};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for document operations.
const TRACING_TARGET: &str = "nvisy_server::handler::documents";

/// Creates a new document.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn create_document(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating document");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::CreateDocuments,
        )
        .await?;

    let new_document = request.into_model(path_params.project_id, auth_state.account_id);
    let document = conn.create_document(new_document).await?;

    tracing::info!(
        target: TRACING_TARGET,
        document_id = %document.id,
        "Document created",
    );

    Ok((StatusCode::CREATED, Json(document.into())))
}

fn create_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create document")
        .description("Creates a new document container for organizing files.")
        .response::<201, Json<Document>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Returns all documents for a project.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn get_all_documents(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Documents>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing documents");

    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::ViewDocuments)
        .await?;

    let documents = conn
        .find_documents_by_project(path_params.project_id, pagination.into())
        .await?;

    let response: Documents = documents.into_iter().map(Document::from).collect();

    tracing::debug!(
        target: TRACING_TARGET,
        document_count = response.len(),
        "Documents listed",
    );

    Ok((StatusCode::OK, Json(response)))
}

fn get_all_documents_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List documents")
        .description("Lists all documents in a project with pagination.")
        .response::<200, Json<Documents>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Gets a document by its document ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        document_id = %path_params.document_id,
    )
)]
async fn get_document(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading document");

    auth_state
        .authorize_document(
            &mut conn,
            path_params.document_id,
            Permission::ViewDocuments,
        )
        .await?;

    let document = find_document(&mut conn, path_params.document_id).await?;

    tracing::info!(target: TRACING_TARGET, "Document read");

    Ok((StatusCode::OK, Json(document.into())))
}

fn get_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get document")
        .description("Returns document details by ID.")
        .response::<200, Json<Document>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a document by its document ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        document_id = %path_params.document_id,
    )
)]
async fn update_document(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<DocumentPathParams>,
    ValidateJson(request): ValidateJson<UpdateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating document");

    auth_state
        .authorize_document(
            &mut conn,
            path_params.document_id,
            Permission::UpdateDocuments,
        )
        .await?;

    // Verify document exists
    let _ = find_document(&mut conn, path_params.document_id).await?;

    let update_data = request.into_model();
    let document = conn
        .update_document(path_params.document_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Document updated");

    Ok((StatusCode::OK, Json(document.into())))
}

fn update_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update document")
        .description("Updates document metadata.")
        .response::<200, Json<Document>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a document by its document ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        document_id = %path_params.document_id,
    )
)]
async fn delete_document(
    PgPool(mut conn): PgPool,
    State(_nats_client): State<NatsClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<DocumentPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting document");

    auth_state
        .authorize_document(
            &mut conn,
            path_params.document_id,
            Permission::DeleteDocuments,
        )
        .await?;

    // Verify document exists
    let _ = find_document(&mut conn, path_params.document_id).await?;

    conn.delete_document(path_params.document_id).await?;

    tracing::info!(target: TRACING_TARGET, "Document deleted");

    Ok(StatusCode::OK)
}

fn delete_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete document")
        .description("Soft-deletes the document and associated files.")
        .response_with::<200, (), _>(|res| res.description("Document deleted."))
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a document by ID or returns NotFound error.
async fn find_document(
    conn: &mut nvisy_postgres::PgConn,
    document_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::Document> {
    conn.find_document_by_id(document_id).await?.ok_or_else(|| {
        ErrorKind::NotFound
            .with_message("Document not found.")
            .with_resource("document")
    })
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/{project_id}/documents",
            post_with(create_document, create_document_docs)
                .get_with(get_all_documents, get_all_documents_docs),
        )
        .api_route(
            "/documents/{document_id}",
            get_with(get_document, get_document_docs)
                .patch_with(update_document, update_document_docs)
                .delete_with(delete_document, delete_document_docs),
        )
        .with_path_items(|item| item.tag("Documents"))
}
