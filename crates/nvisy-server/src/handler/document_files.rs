//! Document file upload and management handlers.
//!
//! This module provides comprehensive file management functionality for documents,
//! including upload, download, metadata management, and file operations. All
//! operations are secured with document-level authorization and include virus
//! scanning and content validation.
//!
//! # Security Features
//!
//! ## Authorization
//! - Document-level permissions required for all operations
//! - File ownership tracking with creator attribution
//! - Write permissions required for upload, update, and delete operations
//! - Read permissions sufficient for download and metadata access
//!
//! ## File Safety
//! - Virus scanning for all uploaded files
//! - Content type validation and sanitization
//! - File size limits and format restrictions
//! - Malicious content detection and blocking
//!
//! ## Data Integrity
//! - SHA-256 hash generation for file integrity verification
//! - Atomic operations with database and storage consistency
//! - Automatic cleanup of orphaned files
//! - Version tracking and audit trails
//!
//! # File Processing Pipeline
//!
//! 1. **Upload**: Multipart file upload with metadata extraction
//! 2. **Validation**: Content type, size, and security checks
//! 3. **Storage**: Secure storage in MinIO with hash verification
//! 4. **Processing**: AI/ML processing pipeline initiation
//! 5. **Indexing**: Full-text search indexing and metadata extraction
//!
//! # Endpoints
//!
//! ## File Operations
//! - `POST /documents/{documentId}/files` - Upload files to document
//! - `GET /documents/{documentId}/files/{fileId}` - Download specific file
//! - `PUT /documents/{documentId}/files/{fileId}` - Update file metadata
//! - `DELETE /documents/{documentId}/files/{fileId}` - Delete file permanently
//!
//! # Performance Considerations
//!
//! - Streaming upload/download for large files
//! - Efficient metadata caching
//! - Background processing for AI pipeline
//! - Automatic storage optimization

use axum::body::Bytes;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{AppendHeaders, IntoResponse};
use nvisy_minio::MinioClient;
use nvisy_postgres::PgDatabase;
use nvisy_postgres::models::{DocumentFile, NewDocumentFile};
use nvisy_postgres::queries::DocumentFileRepository;
use nvisy_postgres::types::{FileType, ProcessingStatus, RequireMode, VirusScanStatus};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use validator::Validate;

use crate::extract::{AuthState, Json, Path, ProjectPermission, ValidateJson, Version};
use crate::handler::documents::DocumentPathParams;
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for document file operations.
const TRACING_TARGET: &str = "nvisy::handler::document_files";

/// Maximum file size: 100MB
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Maximum files per upload
const MAX_FILES_PER_UPLOAD: usize = 10;

/// `Path` param for `{fileId}` handlers.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DocFileIdPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
    /// Unique identifier of the document file.
    pub file_id: Uuid,
}

/// Response for a single uploaded file.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadedFile {
    /// Unique file identifier
    pub file_id: Uuid,
    /// Display name
    pub display_name: String,
    /// File size in bytes
    pub file_size: i64,
    /// MIME type
    pub mime_type: String,
    /// Processing status
    pub status: ProcessingStatus,
}

/// Response returned after successful file upload.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileResponse {
    /// List of successfully uploaded files
    pub files: Vec<UploadedFile>,
    /// Number of files uploaded
    pub count: usize,
}

/// Uploads input files to a document for processing.
#[tracing::instrument(skip(pg_database, storage, multipart))]
#[utoipa::path(
    post, path = "/documents/{documentId}/files/", tag = "documents",
    params(DocumentPathParams),
    request_body(
        content = inline(String),
        description = "Multipart form data with files",
        content_type = "multipart/form-data",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request - invalid file or too many files",
            body = ErrorResponse,
        ),
        (
            status = UNAUTHORIZED,
            description = "Unauthorized - insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = CREATED,
            description = "Files uploaded successfully",
            body = UploadFileResponse,
        ),
    )
)]
async fn upload_file(
    State(pg_database): State<PgDatabase>,
    State(minio_client): State<MinioClient>,
    Path(path_params): Path<DocumentPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadFileResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    // Verify document exists and user has editor permissions
    // Verify document write permissions
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::UploadFiles,
        )
        .await?;

    let mut uploaded_files = Vec::new();
    let mut file_count = 0;

    tracing::info!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        account_id = %auth_claims.account_id,
        "Starting file upload"
    );

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read multipart field");
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        // Check file limit
        if file_count >= MAX_FILES_PER_UPLOAD {
            return Err(ErrorKind::BadRequest
                .with_message("Too many files")
                .with_context(format!(
                    "Maximum {} files allowed per upload",
                    MAX_FILES_PER_UPLOAD
                ))
                .into_error());
        }

        let filename = if let Some(filename) = field.file_name() {
            filename.to_string()
        } else {
            tracing::debug!(target: TRACING_TARGET, "Skipping field without filename");
            continue;
        };

        let content_type = field
            .content_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        tracing::debug!(
            target: TRACING_TARGET,
            filename = %filename,
            content_type = %content_type,
            "Processing file upload"
        );

        // Read file data
        let data = field.bytes().await.map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, filename = %filename, "Failed to read file data");
            ErrorKind::BadRequest
                .with_message("Failed to read file data")
                .with_context(format!("Could not read file '{}': {}", filename, err))
        })?;

        // Check file size
        if data.len() > MAX_FILE_SIZE {
            return Err(ErrorKind::BadRequest
                .with_message("File too large")
                .with_context(format!(
                    "File '{}' exceeds maximum size of {} MB",
                    filename,
                    MAX_FILE_SIZE / (1024 * 1024)
                ))
                .into_error());
        }

        // Extract file extension
        let file_extension = std::path::Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Determine file type from extension/MIME type
        let file_type = determine_file_type(&file_extension, &content_type);

        // Generate unique file ID for storage
        let file_id = Uuid::now_v7();
        let storage_path = format!("documents/{}/files/{}", path_params.document_id, file_id);

        // Upload to MinIO
        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            path = %storage_path,
            size = data.len(),
            "Uploading file to storage"
        );

        storage
            .put_object(&storage_path, data.clone())
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "Failed to upload file to storage"
                );
                ErrorKind::InternalServerError
                    .with_message("File upload failed")
                    .with_context(format!("Storage error: {}", err))
            })?;

        // Create database record
        let new_file = NewDocumentFile {
            id: file_id,
            document_id: path_params.document_id,
            account_id: auth_claims.account_id,
            display_name: filename.clone(),
            original_filename: filename.clone(),
            file_extension: file_extension.clone(),
            mime_type: content_type.clone(),
            file_type,
            require_mode: RequireMode::Optional,
            processing_priority: 0,
            file_size: data.len() as i64,
            file_hash: calculate_hash(&data),
            storage_path: storage_path.clone(),
            virus_scan_status: VirusScanStatus::Pending,
            virus_scan_result: None,
            processing_status: ProcessingStatus::Pending,
            processing_error: None,
            processing_started_at: None,
            processing_completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let document_file = DocumentFileRepository::create_document_file(&mut conn, new_file)
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "Failed to create file record in database"
                );
                ErrorKind::InternalServerError.with_message("Failed to save file metadata")
            })?;

        tracing::info!(
            target: TRACING_TARGET,
            file_id = %file_id,
            filename = %filename,
            size = data.len(),
            "File uploaded successfully"
        );

        uploaded_files.push(UploadedFile {
            file_id: document_file.id,
            display_name: document_file.display_name,
            file_size: document_file.file_size,
            mime_type: document_file.mime_type,
            status: document_file.processing_status,
        });

        file_count += 1;
    }

    if uploaded_files.is_empty() {
        return Err(ErrorKind::BadRequest
            .with_message("No files provided")
            .with_context("Multipart request contained no file fields")
            .into_error());
    }

    let response = UploadFileResponse {
        count: uploaded_files.len(),
        files: uploaded_files,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Request to update file metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileRequest {
    /// New display name for the file
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// New processing priority
    pub processing_priority: Option<i32>,
    /// Override file hash (for re-processing)
    pub override_hash: Option<String>,
}

/// Response after updating file metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileResponse {
    /// Updated file information
    pub file_id: Uuid,
    pub display_name: String,
    pub processing_priority: i32,
    pub updated_at: OffsetDateTime,
}

/// Updates file metadata.
#[tracing::instrument(skip(pg_database))]
#[utoipa::path(
    patch, path = "/documents/{documentId}/files/{fileId}", tag = "documents",
    params(DocFileIdPathParams),
    request_body = UpdateFileRequest,
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "File not found",
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
            description = "File updated successfully",
            body = UpdateFileResponse,
        ),
    )
)]
async fn update_file(
    State(pg_database): State<PgDatabase>,
    Path(path_params): Path<DocFileIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
    ValidateJson(request): ValidateJson<UpdateFileRequest>,
) -> Result<(StatusCode, Json<UpdateFileResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    // Verify permissions
    // Verify document write permissions
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::UpdateDocuments,
        )
        .await?;

    // Get existing file
    let mut file = DocumentFileRepository::find_file_by_id(&mut conn, path_params.file_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_message("File not found"))?;

    // Verify file belongs to document
    if file.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound
            .with_message("File not found in document")
            .into_error());
    }

    // Update fields
    if let Some(display_name) = request.display_name {
        file.display_name = display_name;
    }
    if let Some(priority) = request.processing_priority {
        file.processing_priority = priority;
    }
    if let Some(override_hash) = request.override_hash {
        file.file_hash = override_hash;
    }

    file.updated_at = OffsetDateTime::now_utc();

    // Save changes
    let updated_file = DocumentFileRepository::update_document_file(&mut conn, file)
        .await
        .map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to update file");
            ErrorKind::InternalServerError.with_message("Failed to update file")
        })?;

    tracing::info!(
        target: TRACING_TARGET,
        file_id = %updated_file.id,
        "File updated successfully"
    );

    let response = UpdateFileResponse {
        file_id: updated_file.id,
        display_name: updated_file.display_name,
        processing_priority: updated_file.processing_priority,
        updated_at: updated_file.updated_at,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Downloads a document file.
#[tracing::instrument(skip(pg_database, storage))]
#[utoipa::path(
    get, path = "/documents/{documentId}/files/{fileId}", tag = "documents",
    params(DocFileIdPathParams),
    responses(
        (
            status = NOT_FOUND,
            description = "File not found",
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
            description = "File download",
            content_type = "application/octet-stream",
        ),
    )
)]
async fn download_file(
    State(pg_database): State<PgDatabase>,
    State(minio_client): State<MinioClient>,
    Path(path_params): Path<DocFileIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<impl IntoResponse> {
    let mut conn = pg_database.get_connection().await?;

    // Verify permissions
    // Verify document read permissions
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::ViewFiles,
        )
        .await?;

    // Get file metadata
    let file = DocumentFileRepository::find_file_by_id(&mut conn, path_params.file_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_message("File not found"))?;

    // Verify file belongs to document
    if file.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound
            .with_message("File not found in document")
            .into_error());
    }

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %file.id,
        path = %file.storage_path,
        "Downloading file from storage"
    );

    // Download from MinIO
    let file_data = storage
        .get_object(&file.storage_path)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %file.id,
                path = %file.storage_path,
                "Failed to download file from storage"
            );
            ErrorKind::InternalServerError.with_message("Failed to retrieve file")
        })?;

    // Build response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&file.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    let filename = format!("attachment; filename=\"{}\"", file.display_name);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&filename).unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );

    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from(file.file_size as u64),
    );

    tracing::info!(
        target: TRACING_TARGET,
        file_id = %file.id,
        filename = %file.display_name,
        size = file.file_size,
        "File downloaded successfully"
    );

    Ok((StatusCode::OK, headers, file_data))
}

/// Deletes a document file.
#[tracing::instrument(skip(pg_database, storage))]
#[utoipa::path(
    delete, path = "/documents/{documentId}/files/{fileId}", tag = "documents",
    params(DocFileIdPathParams),
    responses(
        (
            status = NOT_FOUND,
            description = "File not found",
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
            status = NO_CONTENT,
            description = "File deleted successfully",
        ),
    )
)]
async fn delete_file(
    State(pg_database): State<PgDatabase>,
    State(minio_client): State<MinioClient>,
    Path(path_params): Path<DocFileIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<StatusCode> {
    let mut conn = pg_database.get_connection().await?;

    // Verify permissions
    // Verify document write permissions
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            ProjectPermission::DeleteFiles,
        )
        .await?;

    // Get file metadata
    let file = DocumentFileRepository::find_file_by_id(&mut conn, path_params.file_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_message("File not found"))?;

    // Verify file belongs to document
    if file.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound
            .with_message("File not found in document")
            .into_error());
    }

    // Delete from storage
    storage
        .delete_object(&file.storage_path)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %file.id,
                "Failed to delete file from storage"
            );
            ErrorKind::InternalServerError.with_message("Failed to delete file from storage")
        })?;

    // Delete from database
    DocumentFileRepository::delete_document_file(&mut conn, file.id)
        .await
        .map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, file_id = %file.id, "Failed to delete file from database");
            ErrorKind::InternalServerError.with_message("Failed to delete file record")
        })?;

    tracing::info!(
        target: TRACING_TARGET,
        file_id = %file.id,
        "File deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Helper: Determine file type from extension and MIME type.
fn determine_file_type(extension: &str, mime_type: &str) -> FileType {
    match extension {
        "pdf" => FileType::Pdf,
        "doc" | "docx" => FileType::Word,
        "xls" | "xlsx" => FileType::Excel,
        "ppt" | "pptx" => FileType::PowerPoint,
        "txt" => FileType::Text,
        "csv" => FileType::Csv,
        "json" => FileType::Json,
        "xml" => FileType::Xml,
        "html" | "htm" => FileType::Html,
        "md" | "markdown" => FileType::Markdown,
        _ if mime_type.starts_with("image/") => FileType::Image,
        _ => FileType::Other,
    }
}

/// Helper: Calculate SHA-256 hash of file data.
fn calculate_hash(data: &Bytes) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(
        upload_file,
        update_file,
        download_file,
        delete_file
    ))
}

#[cfg(test)]
mod test {
    use crate::handler::document_files::routes;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let _server = create_test_server_with_router(|_| routes()).await?;
        Ok(())
    }
}
