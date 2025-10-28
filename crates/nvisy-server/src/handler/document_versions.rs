//! Document version management and output file download handlers.
//!
//! This module handles version history, output file downloads,
//! and version metadata management.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::DocumentVersionRepository;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::authorize;
use crate::extract::auth::AuthProvider;
use crate::extract::{AuthState, Json, Path, Permission, Version};
use crate::handler::documents::DocumentPathParams;
use crate::handler::response::document_version::{ReadAllVersionsResponse, VersionInfo};
use crate::handler::{ErrorKind, ErrorResponse, PaginationRequest, Result};
use crate::service::ServiceState;

/// Tracing target for document version operations.
const TRACING_TARGET: &str = "nvisy_server::handler::document_versions";

/// `Path` param for `{versionId}` handlers.
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
            body = ReadAllVersionsResponse,
        ),
    )
)]
async fn get_version_files(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<DocumentPathParams>,
    AuthState(auth_claims): AuthState,
    Json(pagination): Json<PaginationRequest>,
) -> Result<(StatusCode, Json<ReadAllVersionsResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document access
    authorize!(
        document: path_params.document_id,
        auth_claims, &mut conn,
        Permission::ViewFiles,
    );

    tracing::debug!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        page = ?pagination.offset,
        per_page = ?pagination.limit,
        "Fetching document versions"
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
            "Failed to fetch versions"
        );
        ErrorKind::InternalServerError.with_message("Failed to fetch versions")
    })?;

    tracing::info!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        count = versions.len(),
        "Versions fetched successfully"
    );

    let version_infos: Vec<VersionInfo> = versions.into_iter().map(VersionInfo::from).collect();
    let total =
        DocumentVersionRepository::count_document_versions(&mut conn, path_params.document_id)
            .await? as usize;

    let response = ReadAllVersionsResponse {
        total,
        versions: version_infos,
        page: ((pagination.offset() / pagination.limit()) + 1) as i64,
        per_page: pagination.limit() as i64,
    };

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
            body = VersionInfo,
        ),
    ),
)]
async fn get_version_info(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<(StatusCode, Json<VersionInfo>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify document access
    authorize!(
        document: path_params.document_id,
        auth_claims, &mut conn,
        Permission::ViewDocuments,
    );

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

    tracing::info!(
        target: TRACING_TARGET,
        version_id = %version.id,
        version_number = version.version_number,
        "Version info retrieved"
    );

    Ok((StatusCode::OK, Json(VersionInfo::from(version))))
}

/// Downloads a document version output file.
#[tracing::instrument(skip(_pg_client))]
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
    State(_pg_client): State<PgClient>,
    // State(minio_client): State<MinioClient>, // TODO: Replace with NATS object store
    Path(_path_params): Path<DocVersionIdPathParams>,
    AuthState(_auth_claims): AuthState,
    _version: Version,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    // TODO: Replace with NATS object store implementation
    Err(ErrorKind::NotImplemented.with_message(
        "Version file download not implemented - MinIO removed, use NATS object store",
    ))
}

/// Deletes a document version.
#[tracing::instrument(skip(pg_client))]
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
    // State(minio_client): State<MinioClient>, // TODO: Replace with NATS object store
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<StatusCode> {
    let mut conn = pg_client.get_connection().await?;

    // Verify permissions (need editor role to delete)
    authorize!(
        document: path_params.document_id,
        auth_claims, &mut conn,
        Permission::DeleteDocuments,
    );

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

    // Get latest version number
    let stats =
        DocumentVersionRepository::get_document_version_stats(&mut conn, path_params.document_id)
            .await?;

    // Prevent deleting the latest version
    if version.version_number == stats.latest_version_number {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot delete the latest version")
            .with_context("Delete older versions or create a new version first"));
    }

    // TODO: Replace with NATS object store implementation
    Err(ErrorKind::NotImplemented
        .with_message("Version deletion not implemented - MinIO removed, use NATS object store"))
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
