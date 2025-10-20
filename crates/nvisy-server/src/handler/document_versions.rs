//! Document version management and output file download handlers.
//!
//! This module handles version history, output file downloads,
//! and version metadata management.

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use nvisy_minio::MinioClient;
use nvisy_postgres::PgDatabase;
use nvisy_postgres::models::DocumentVersion;
use nvisy_postgres::queries::DocumentVersionRepository;
use nvisy_postgres::types::FileType;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::auth::AuthProvider;
use crate::extract::{AuthState, Json, Path, ProjectPermission, Version};
use crate::handler::documents::DocumentPathParams;
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for document version operations.
const TRACING_TARGET: &str = "nvisy::handler::document_versions";

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

/// Document version information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct VersionInfo {
    /// Version unique ID
    pub version_id: Uuid,
    /// Version number (incremental)
    pub version_number: i32,
    /// Display name
    pub display_name: String,
    /// File extension
    pub file_extension: String,
    /// MIME type
    pub mime_type: String,
    /// File type
    pub file_type: FileType,
    /// File size in bytes
    pub file_size: i64,
    /// Processing credits used
    pub processing_credits: i32,
    /// Processing duration in milliseconds
    pub processing_duration_ms: i32,
    /// Creation timestamp
    pub created_at: OffsetDateTime,
}

impl From<DocumentVersion> for VersionInfo {
    fn from(version: DocumentVersion) -> Self {
        Self {
            version_id: version.id,
            version_number: version.version_number,
            display_name: version.display_name,
            file_extension: version.file_extension,
            mime_type: version.mime_type,
            file_type: version.file_type,
            file_size: version.file_size_bytes,
            processing_credits: version.processing_credits,
            processing_duration_ms: version.processing_duration_ms,
            created_at: version.created_at,
        }
    }
}

/// Response containing document versions list.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReadAllVersionsResponse {
    /// List of versions
    pub versions: Vec<VersionInfo>,
    /// Total number of versions
    pub total: usize,
    /// Pagination information
    pub page: i64,
    pub per_page: i64,
}

/// Lists all versions of a document.
#[tracing::instrument(skip(pg_database))]
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
    State(pg_database): State<PgDatabase>,
    Path(path_params): Path<DocumentPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ReadAllVersionsResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    // Verify document access
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::ViewFiles,
        )
        .await?;

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
#[tracing::instrument(skip(pg_database))]
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
    State(pg_database): State<PgDatabase>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<(StatusCode, Json<VersionInfo>)> {
    let mut conn = pg_database.get_connection().await?;

    // Verify document access
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::ViewDocuments,
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

    tracing::info!(
        target: TRACING_TARGET,
        version_id = %version.id,
        version_number = version.version_number,
        "Version info retrieved"
    );

    Ok((StatusCode::OK, Json(VersionInfo::from(version))))
}

/// Downloads a document version output file.
#[tracing::instrument(skip(pg_database, minio_client))]
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
    State(pg_database): State<PgDatabase>,
    State(minio_client): State<MinioClient>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    let mut conn = pg_database.get_connection().await?;

    // Verify document access
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::ViewFiles,
        )
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        version_id = %path_params.version_id,
        document_id = %path_params.document_id,
        "Fetching version for download"
    );

    // Get version metadata
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
        path = %version.storage_path,
        "Downloading version file from storage"
    );

    // Download from MinIO
    let (file_data, _download_result) = minio_client
        .object_operations()
        .download_file("documents", &version.storage_path)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %version.id,
                path = %version.storage_path,
                "Failed to download version file from storage"
            );
            ErrorKind::InternalServerError.with_message("Failed to retrieve version file")
        })?;

    // Build response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&version.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    let filename = format!("attachment; filename=\"{}\"", version.display_name);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&filename).unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );

    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from(version.file_size_bytes as u64),
    );

    // Add version metadata headers
    headers.insert(
        "X-Version-Number",
        HeaderValue::from_str(&version.version_number.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    tracing::info!(
        target: TRACING_TARGET,
        version_id = %version.id,
        filename = %version.display_name,
        size = version.file_size_bytes,
        "Version downloaded successfully"
    );

    Ok((StatusCode::OK, headers, file_data.to_vec()))
}

/// Deletes a document version.
#[tracing::instrument(skip(pg_database, minio_client))]
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
    State(pg_database): State<PgDatabase>,
    State(minio_client): State<MinioClient>,
    Path(path_params): Path<DocVersionIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<StatusCode> {
    let mut conn = pg_database.get_connection().await?;

    // Verify permissions (need editor role to delete)
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::DeleteDocuments,
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

    // Delete from storage
    minio_client
        .object_operations()
        .delete_object("documents", &version.storage_path)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %version.id,
                "Failed to delete version from storage"
            );
            ErrorKind::InternalServerError.with_message("Failed to delete version from storage")
        })?;

    // Delete from database
    DocumentVersionRepository::delete_document_version(&mut conn, version.id)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                version_id = %version.id,
                "Failed to delete version from database"
            );
            ErrorKind::InternalServerError.with_message("Failed to delete version record")
        })?;

    tracing::info!(
        target: TRACING_TARGET,
        version_id = %version.id,
        version_number = version.version_number,
        "Version deleted successfully"
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
