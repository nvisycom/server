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
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::projects::ProjectPathParams;
use crate::handler::request::{CreateDocument, UpdateDocument};
use crate::handler::response::{Document, Documents};
use crate::handler::{ErrorKind, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for document operations.
const TRACING_TARGET: &str = "nvisy_server::handler::documents";

/// `Path` param for `{documentId}` handlers.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
}

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn test_create_document_success() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = CreateDocument {
            display_name: "Test Document".to_string(),
            description: Some("Test description".to_string()),
            ..Default::default()
        };

        let project_id = Uuid::new_v4();
        let response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;
        response.assert_status(StatusCode::CREATED);

        let body: Document = response.json();
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
        let create_request = CreateDocument {
            display_name: "Test Document".to_string(),
            description: Some("Updated description".to_string()),
            ..Default::default()
        };
        server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&create_request)
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
        let create_request = CreateDocument {
            display_name: "Original Name".to_string(),
            description: Some("Test description".to_string()),
            ..Default::default()
        };
        let create_response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&create_request)
            .await;
        let created: Document = create_response.json();

        // Update the document
        let update_request = UpdateDocument {
            display_name: Some("Updated Name".to_string()),
            description: None,
            ..Default::default()
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
        let create_request = CreateDocument {
            display_name: "Test Document".to_string(),
            description: Some("Test description".to_string()),
            ..Default::default()
        };
        let create_response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&create_request)
            .await;
        let created: Document = create_response.json();

        // Get the document
        let response = server
            .get(&format!("/documents/{}/", created.document_id))
            .await;
        response.assert_status_ok();

        let body: Document = response.json();
        assert_eq!(body.document_id, created.document_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_document() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;
        let project_id = Uuid::new_v4();

        // Create a document
        let request = CreateDocument {
            display_name: "Delete Test".to_string(),
            description: Some("Test description".to_string()),
            ..Default::default()
        };
        let create_response = server
            .post(&format!("/projects/{}/documents/", project_id))
            .json(&request)
            .await;
        let created: Document = create_response.json();

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
