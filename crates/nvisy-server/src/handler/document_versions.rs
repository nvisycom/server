//! Document version management and output file download handlers.
//!
//! This module handles version history, output file downloads,
//! and version metadata management.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::DocumentVersionRepository;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Version};
use crate::handler::documents::DocumentPathParams;
use crate::handler::response::{Version as VersionResponse, Versions};
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for document version operations.
const TRACING_TARGET: &str = "nvisy_server::handler::document_versions";

/// Combined path params for document version operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DocVersionIdPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
    /// Unique identifier of the document version.
    pub version_id: Uuid,
}

/// Lists all versions of a document.
#[tracing::instrument(skip(pg_client))]
#[utoipa::path(
    get, path = "/documents/{documentId}/versions/", tag = "documents",
    params(
        DocumentPathParams,
        ("page" = Option<i64>, Query, description = "Page number (1-indexed)"),
        ("per_page" = Option<i64>, Query, description = "Items per page (max 100)"),
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Document not found",
            body = ErrorResponse,
        ),
        (
            status = UNAUTHORIZED,
            description = "Unauthorized",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Document versions",
            body = Versions,
        ),
    )
)]
async fn get_version_files(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<DocumentPathParams>,
    AuthState(auth_claims): AuthState,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Versions>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document access
    auth_claims
        .authorize_document(&mut conn, path_params.document_id, Permission::ViewFiles)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        page = ?pagination.offset,
        per_page = ?pagination.limit,
        "fetching document versions"
    );

    // Get versions with pagination
    let versions = DocumentVersionRepository::list_document_versions(
        &mut conn,
        path_params.document_id,
        pagination.into(),
    )
    .await
    .map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            document_id = %path_params.document_id,
            "failed to fetch versions"
        );
        ErrorKind::InternalServerError.with_message("Failed to fetch versions")
    })?;

    tracing::debug!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        count = versions.len(),
        "versions fetched successfully"
    );

    let response: Versions = versions.into_iter().map(VersionResponse::from).collect();

    Ok((StatusCode::OK, Json(response)))
}

/// Gets information about a specific version.
#[tracing::instrument(skip(pg_client))]
#[utoipa::path(
    get, path = "/documents/{documentId}/versions/{versionId}/info", tag = "documents",
    params(DocVersionIdPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Version not found",
            body = ErrorResponse,
        ),
        (
            status = UNAUTHORIZED,
            description = "Unauthorized",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Version information",
            body = VersionResponse,
        ),
    ),
)]
async fn get_version_info(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<VersionResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document access
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            Permission::ViewDocuments,
        )
        .await?;

    // Get version
    let Some(version) =
        DocumentVersionRepository::find_document_version_by_id(&mut conn, path_params.version_id)
            .await?
    else {
        return Err(ErrorKind::NotFound.with_message("Version not found"));
    };

    // Verify version belongs to document
    if version.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound.with_message("Version not found in document"));
    }

    tracing::debug!(
        target: TRACING_TARGET,
        version_id = %version.id,
        version_number = version.version_number,
        "version info retrieved"
    );

    Ok((StatusCode::OK, Json(VersionResponse::from(version))))
}

/// Downloads a document version output file.
#[tracing::instrument(skip(pg_client, nats_client))]
#[utoipa::path(
    get, path = "/documents/{documentId}/versions/{versionId}/download", tag = "documents",
    params(DocVersionIdPathParams),
    responses(
        (
            status = NOT_FOUND,
            description = "Version not found",
            body = ErrorResponse,
        ),
        (
            status = UNAUTHORIZED,
            description = "Unauthorized",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Document version file download",
            content_type = "application/octet-stream",
        ),
    ),
)]
async fn download_version_file(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    use std::str::FromStr;

    use nvisy_nats::object::{ObjectKey, OutputFiles};

    let mut conn = pg_client.get_connection().await?;

    // Verify document access
    auth_claims
        .authorize_document(&mut conn, path_params.document_id, Permission::ViewFiles)
        .await?;

    // Get version
    let Some(version) =
        DocumentVersionRepository::find_document_version_by_id(&mut conn, path_params.version_id)
            .await?
    else {
        return Err(ErrorKind::NotFound.with_message("Version not found"));
    };

    // Verify version belongs to document
    if version.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound.with_message("Version not found in document"));
    }

    // Check if version has output file
    let storage_path = &version.storage_path;
    if storage_path.is_empty() {
        return Err(ErrorKind::NotFound.with_message("Version has no output file"));
    }

    // Get output file store
    let output_fs = nats_client
        .document_store::<OutputFiles>()
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %path_params.version_id,
                "failed to get output file store"
            );
            ErrorKind::InternalServerError.with_message("Failed to access file storage")
        })?;

    // Parse storage path to object key
    let object_key = ObjectKey::<OutputFiles>::from_str(storage_path).map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            storage_path = %storage_path,
            "invalid storage path format"
        );
        ErrorKind::InternalServerError.with_message("Invalid file storage path")
    })?;

    // Get content from NATS object store
    let content_data = output_fs
        .get(&object_key)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %path_params.version_id,
                "failed to retrieve version file from NATS object store"
            );
            ErrorKind::InternalServerError.with_message("Failed to retrieve file")
        })?
        .ok_or_else(|| {
            tracing::warn!(
                target: TRACING_TARGET,
                version_id = %path_params.version_id,
                "version output file not found in storage"
            );
            ErrorKind::NotFound.with_message("Version output file not found")
        })?;

    // Set up response headers
    let mut headers = HeaderMap::new();
    let filename = format!("document_v{}.pdf", version.version_number);
    headers.insert(
        "content-disposition",
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );
    headers.insert(
        "content-length",
        content_data.size().to_string().parse().unwrap(),
    );

    tracing::debug!(
        target: TRACING_TARGET,
        version_id = %path_params.version_id,
        version_number = version.version_number,
        size = content_data.size(),
        "version file downloaded successfully"
    );

    Ok((StatusCode::OK, headers, content_data.into_bytes().to_vec()))
}

/// Deletes a document version.
#[tracing::instrument(skip(pg_client, nats_client))]
#[utoipa::path(
    delete, path = "/documents/{documentId}/versions/{versionId}", tag = "documents",
    params(DocVersionIdPathParams),
    responses(
        (
            status = NOT_FOUND,
            description = "Version not found",
            body = ErrorResponse,
        ),
        (
            status = UNAUTHORIZED,
            description = "Unauthorized",
            body = ErrorResponse,
        ),
        (
            status = BAD_REQUEST,
            description = "Cannot delete latest version",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = NO_CONTENT,
            description = "Version deleted successfully",
        ),
    )
)]
async fn delete_version(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<StatusCode> {
    use std::str::FromStr;

    use nvisy_nats::object::{ObjectKey, OutputFiles};

    let mut conn = pg_client.get_connection().await?;

    // Verify permissions (need editor role to delete)
    auth_claims
        .authorize_document(&mut conn, path_params.document_id, Permission::DeleteFiles)
        .await?;

    // Get version
    let Some(version) =
        DocumentVersionRepository::find_document_version_by_id(&mut conn, path_params.version_id)
            .await?
    else {
        return Err(ErrorKind::NotFound.with_message("Version not found"));
    };

    // Verify version belongs to document
    if version.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound.with_message("Version not found in document"));
    }

    // Delete output file from NATS object store if it exists
    let storage_path = &version.storage_path;
    if !storage_path.is_empty() {
        let output_fs = nats_client
            .document_store::<OutputFiles>()
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    version_id = %path_params.version_id,
                    "failed to get output file store"
                );
                ErrorKind::InternalServerError.with_message("Failed to access file storage")
            })?;

        // Parse storage path to object key
        let object_key = ObjectKey::<OutputFiles>::from_str(storage_path).map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                storage_path = %storage_path,
                "invalid storage path format"
            );
            ErrorKind::InternalServerError.with_message("Invalid file storage path")
        })?;

        // Delete from NATS object store
        output_fs.delete(&object_key).await.map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %path_params.version_id,
                "failed to delete version file from NATS object store"
            );
            ErrorKind::InternalServerError.with_message("Failed to delete file from storage")
        })?;

        tracing::debug!(
            target: TRACING_TARGET,
            version_id = %path_params.version_id,
            "version output file deleted from storage"
        );
    }

    // Soft delete version in database
    DocumentVersionRepository::delete_document_version(&mut conn, path_params.version_id)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %path_params.version_id,
                "failed to delete version from database"
            );
            ErrorKind::InternalServerError.with_message("Failed to delete version")
        })?;

    tracing::debug!(
        target: TRACING_TARGET,
        version_id = %path_params.version_id,
        version_number = version.version_number,
        "version deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(
        get_version_files,
        get_version_info,
        download_version_file,
        delete_version
    ))
}

#[cfg(test)]
mod test {
    use crate::handler::document_versions::routes;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let _server = create_test_server_with_router(|_| routes()).await?;
        Ok(())
    }
}
