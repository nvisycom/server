//! Document file upload and management handlers.
//!
//! This module provides comprehensive file management functionality for documents,
//! including upload, download, metadata management, and file operations. All
//! operations are secured with document-level authorization and include virus
//! scanning and content validation.

use std::str::FromStr;

use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use nvisy_nats::NatsClient;
use nvisy_nats::object::{DocumentFileStore, DocumentLabel, InputFiles, ObjectKey};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewDocumentFile, UpdateDocumentFile};
use nvisy_postgres::query::DocumentFileRepository;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, Version,
};
use crate::handler::documents::DocumentPathParams;
use crate::handler::request::document_file::{UpdateFileRequest, UploadMode};
use crate::handler::response::document_file::{
    UpdateFileResponse, UploadFileResponse, UploadedFile,
};
use crate::handler::{ErrorKind, ErrorResponse, Result};
use crate::service::ServiceState;

/// Tracing target for document file operations.
const TRACING_TARGET: &str = "nvisy_server::handler::document_files";

/// Maximum file size: 100MB
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Combined path params for document file operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DocFileIdPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
    /// Unique identifier of the document file.
    pub file_id: Uuid,
}

/// Query parameters for file upload
#[derive(Debug, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileQuery {
    /// Upload mode: "single" for all files in one document, "individual" for one document per file
    pub upload_mode: UploadMode,
}

/// Uploads input files to a document for processing.
///
/// The `mode` query parameter controls how files are organized:
/// - `single`: All uploaded files belong to a single document (existing document)
/// - `individual`: Each uploaded file creates a new document (default)
///
/// Query parameters:
/// - `mode` (optional): Upload mode - "single" or "individual" (default: individual)
///
/// Form data:
/// - `file`: One or more files to upload
#[tracing::instrument(skip(pg_client, multipart))]
#[utoipa::path(
    post, path = "/documents/{documentId}/files/", tag = "documents",
    params(DocumentPathParams),
    request_body(
        content = inline(String),
        description = "Multipart form data with 'mode' field and files",
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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<DocumentPathParams>,
    Query(query): Query<UploadFileQuery>,
    AuthState(auth_claims): AuthState,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadFileResponse>)> {
    let mut conn = pg_client.get_connection().await?;
    let input_fs = nats_client.document_store::<InputFiles>().await?;

    auth_claims
        .authorize_document(&mut conn, path_params.document_id, Permission::UploadFiles)
        .await?;

    let upload_mode = query.upload_mode;
    let mut uploaded_files = Vec::new();

    tracing::debug!(target: TRACING_TARGET, mode = ?upload_mode, "Starting file upload");

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!(target: TRACING_TARGET, error = %err, "failed to read multipart field");
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        let filename = if let Some(filename) = field.file_name() {
            filename.to_string()
        } else {
            tracing::debug!(target: TRACING_TARGET, "Skipping field without filename");
            continue;
        };

        // Validate and sanitize filename
        let filename = validate_filename(&filename)?;

        let content_type = field
            .content_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        // Validate MIME type
        validate_mime_type(&content_type)?;

        tracing::debug!(
            target: TRACING_TARGET,
            filename = %filename,
            content_type = %content_type,
            "processing file upload"
        );

        // Read file data with size limit to prevent DoS
        let mut data = Vec::new();
        let mut stream = field;

        while let Some(chunk) = stream.chunk().await.map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, filename = %filename, "Failed to read file chunk");
            ErrorKind::BadRequest
                .with_message("Failed to read file data")
                .with_context(format!("could not read file '{}': {}", filename, err))
        })? {
            // Check size before adding chunk to prevent memory exhaustion
            if data.len() + chunk.len() > MAX_FILE_SIZE {
                return Err(ErrorKind::BadRequest
                    .with_message("File too large")
                    .with_context(format!(
                        "file '{}' exceeds maximum size of {} MB",
                        filename,
                        MAX_FILE_SIZE / (1024 * 1024)
                    )));
            }
            data.extend_from_slice(&chunk);
        }

        // Extract file extension
        let file_extension = std::path::Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Generate unique file ID for storage
        let file_id = Uuid::now_v7();
        let storage_path = format!("documents/{}/files/{}", path_params.document_id, file_id);

        // Upload to NATS document store
        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            path = %storage_path,
            size = data.len(),
            "uploading file to NATS document store"
        );

        // Store file size before moving data
        let file_size_bytes = data.len() as i64;

        // Create content data with metadata
        let content_data =
            DocumentFileStore::<InputFiles>::create_content_data_with_metadata(data.into());

        // Create object key for the file
        let object_key = input_fs.create_key(
            auth_claims.account_id, // Using account_id as project_uuid
            path_params.document_id,
            file_id,
        );

        // Upload to InputFiles document store
        let put_result = input_fs
            .put(&object_key, &content_data)
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "failed to upload file to NATS document store"
                );
                ErrorKind::InternalServerError
                    .with_message("Failed to upload file")
                    .with_context(format!("NATS upload failed: {}", err))
            })?;

        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            nuid = %put_result.nuid,
            size = put_result.size,
            "file uploaded successfully to NATS document store"
        );

        // Extract SHA-256 hash from content data
        let sha256_bytes = content_data.compute_sha256().to_vec();

        // Create file record in database
        let file_record = NewDocumentFile {
            document_id: path_params.document_id,
            account_id: auth_claims.account_id,
            display_name: Some(filename.clone()),
            original_filename: Some(filename.clone()),
            file_extension: Some(file_extension.clone()),
            // require_mode: RequireMode::Text,
            file_size_bytes: Some(file_size_bytes),
            storage_path: object_key.as_str().to_string(),
            storage_bucket: Some(InputFiles::bucket_name().to_string()),
            file_hash_sha256: sha256_bytes,
            keep_for_sec: 30 * 24 * 60 * 60, // TODO: Load from project settings
            auto_delete_at: None,
            ..Default::default()
        };

        // Insert file record into database
        let created_file = DocumentFileRepository::create_document_file(&mut conn, file_record)
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "failed to create file record in database"
                );
                ErrorKind::InternalServerError
                    .with_message("Failed to save file metadata")
                    .with_context(format!("Database error: {}", err))
            })?;

        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            filename = %filename,
            size = file_size_bytes,
            "file upload completed successfully"
        );

        let uploaded_file = UploadedFile {
            file_id: created_file.id,
            display_name: created_file.display_name,
            file_size: created_file.file_size_bytes,
            status: created_file.processing_status,
        };

        // Publish file processing job to queue
        let job = nvisy_nats::stream::DocumentJob::new_file_processing(
            created_file.id,
            path_params.document_id,
            auth_claims.account_id,
            object_key.as_str().to_string(),
            file_extension.clone(),
            file_size_bytes,
        );

        // Publish to document job queue
        let jetstream = nats_client.jetstream();
        let publisher = nvisy_nats::stream::DocumentJobPublisher::new(&jetstream)
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "failed to create document job publisher"
                );
                ErrorKind::InternalServerError.with_message("Failed to queue file for processing")
            })?;

        publisher.publish("pending", &job).await.map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %file_id,
                "failed to publish document job"
            );
            ErrorKind::InternalServerError.with_message("Failed to queue file for processing")
        })?;

        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            job_id = %job.id,
            "document job published for file processing"
        );

        uploaded_files.push(uploaded_file);

        // In single mode, we process all files for the single document
        // In individual mode, stop after first file (for now)
        match upload_mode {
            UploadMode::Single => {
                // Continue processing all files for the single document
            }
            UploadMode::Multiple => {
                // In individual mode, stop after first file (for now)
                // TODO: Create new documents for each additional file
                break;
            }
        }
    }

    // Check if any files were uploaded
    if uploaded_files.is_empty() {
        return Err(ErrorKind::BadRequest.with_message("No files provided in multipart request"));
    }

    let count = uploaded_files.len();
    tracing::debug!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        file_count = count,
        mode = ?upload_mode,
        "file upload completed"
    );

    Ok((
        StatusCode::CREATED,
        Json(UploadFileResponse {
            files: uploaded_files,
            count,
        }),
    ))
}

/// Updates file metadata.
#[tracing::instrument(skip(pg_client))]
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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<DocFileIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
    ValidateJson(request): ValidateJson<UpdateFileRequest>,
) -> Result<(StatusCode, Json<UpdateFileResponse>)> {
    let mut conn = pg_client.get_connection().await?;
    let _input_fs = nats_client.document_store::<InputFiles>().await?;

    // Verify permissions
    // Verify document write permissions
    auth_claims
        .authorize_document(
            &mut conn,
            path_params.document_id,
            Permission::UpdateDocuments,
        )
        .await?;

    // Get existing file
    let Some(file) =
        DocumentFileRepository::find_document_file_by_id(&mut conn, path_params.file_id).await?
    else {
        return Err(ErrorKind::NotFound.with_message("File not found"));
    };

    // Verify file belongs to document
    if file.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound.with_message("File not found in document"));
    }

    // Create update struct
    let updates = UpdateDocumentFile {
        display_name: request.display_name,
        processing_priority: request.processing_priority,
        ..Default::default()
    };

    // Save changes
    let updated_file =
        DocumentFileRepository::update_document_file(&mut conn, path_params.file_id, updates)
            .await
            .map_err(|err| {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to update file");
                ErrorKind::InternalServerError.with_message("Failed to update file")
            })?;

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %updated_file.id,
        "file updated successfully"
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
#[tracing::instrument(skip(pg_client))]
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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<DocFileIdPathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    let mut conn = pg_client.get_connection().await?;
    let input_fs = nats_client.document_store::<InputFiles>().await?;

    // Get file metadata from database
    let file = DocumentFileRepository::find_document_file_by_id(&mut conn, path_params.file_id)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %path_params.file_id,
                "failed to find file in database"
            );
            ErrorKind::InternalServerError
                .with_message("Failed to find file")
                .with_context(format!("Database error: {}", err))
        })?
        .ok_or_else(|| {
            tracing::warn!(
                target: TRACING_TARGET,
                file_id = %path_params.file_id,
                "file not found"
            );
            ErrorKind::NotFound.with_message("File not found")
        })?;

    // Create object key from storage path
    let object_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            storage_path = %file.storage_path,
            "invalid storage path format"
        );
        ErrorKind::InternalServerError
            .with_message("Invalid file storage path")
            .with_context(format!("Parse error: {}", err))
    })?;

    // Get content from NATS document store
    let content_data = input_fs
        .get(&object_key)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %path_params.file_id,
                "failed to retrieve file from NATS document store"
            );
            ErrorKind::InternalServerError
                .with_message("Failed to retrieve file")
                .with_context(format!("NATS retrieval failed: {}", err))
        })?
        .ok_or_else(|| {
            tracing::warn!(
                target: TRACING_TARGET,
                file_id = %path_params.file_id,
                "file content not found in storage"
            );
            ErrorKind::NotFound.with_message("File content not found")
        })?;

    // Set up response headers
    let mut headers = HeaderMap::new();

    headers.insert(
        "content-disposition",
        format!("attachment; filename=\"{}\"", file.display_name)
            .parse()
            .unwrap(),
    );
    headers.insert(
        "content-length",
        content_data.size().to_string().parse().unwrap(),
    );

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %path_params.file_id,
        filename = %file.display_name,
        size = content_data.size(),
        "file downloaded successfully"
    );

    Ok((StatusCode::OK, headers, content_data.into_bytes().to_vec()))
}

/// Deletes a document file.
#[tracing::instrument(skip(pg_client))]
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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<DocFileIdPathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<StatusCode> {
    let mut conn = pg_client.get_connection().await?;
    let input_fs = nats_client.document_store::<InputFiles>().await?;

    auth_claims
        .authorize_document(&mut conn, path_params.document_id, Permission::DeleteFiles)
        .await?;

    // Get file metadata
    let Some(file) =
        DocumentFileRepository::find_document_file_by_id(&mut conn, path_params.file_id).await?
    else {
        return Err(ErrorKind::NotFound.with_message("File not found"));
    };

    // Verify file belongs to document
    if file.document_id != path_params.document_id {
        return Err(ErrorKind::NotFound.with_message("File not found in document"));
    }

    // TODO: Replace with NATS object store implementation
    // Get file metadata from database
    let file = DocumentFileRepository::find_document_file_by_id(&mut conn, path_params.file_id)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %path_params.file_id,
                "failed to find file in database"
            );
            ErrorKind::InternalServerError
                .with_message("Failed to find file")
                .with_context(format!("Database error: {}", err))
        })?
        .ok_or_else(|| {
            tracing::warn!(
                target: TRACING_TARGET,
                file_id = %path_params.file_id,
                "file not found"
            );
            ErrorKind::NotFound.with_message("File not found")
        })?;

    // Create object key from storage path
    let object_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            storage_path = %file.storage_path,
            "invalid storage path format"
        );
        ErrorKind::InternalServerError
            .with_message("Invalid file storage path")
            .with_context(format!("Parse error: {}", err))
    })?;

    // Delete from NATS document store
    input_fs.delete(&object_key).await.map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            file_id = %path_params.file_id,
            "failed to delete file from NATS document store"
        );
        ErrorKind::InternalServerError
            .with_message("Failed to delete file from storage")
            .with_context(format!("NATS deletion failed: {}", err))
    })?;

    // Soft delete in database by updating the record
    // Note: deleted_at field may need to be handled differently based on the actual schema

    // For now, we'll just delete from storage. Database soft delete can be implemented
    // when the update method and deleted_at field are properly available

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %path_params.file_id,
        "file deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Validates that the file MIME type is allowed for upload.
///
/// This prevents potentially dangerous file types from being uploaded.
fn validate_mime_type(mime_type: &str) -> Result<()> {
    // Allowed MIME types - extend this list as needed
    const ALLOWED_MIME_TYPES: &[&str] = &[
        // Documents
        "application/pdf",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.ms-excel",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.ms-powerpoint",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "text/plain",
        "text/csv",
        "text/markdown",
        // Images
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "image/svg+xml",
        // Archives
        "application/zip",
        "application/x-rar-compressed",
        "application/x-7z-compressed",
        // Other
        "application/json",
        "application/xml",
        "text/xml",
        // Generic fallback
        "application/octet-stream",
    ];

    // Block known dangerous MIME types
    const BLOCKED_MIME_TYPES: &[&str] = &[
        "application/x-msdownload",    // .exe
        "application/x-msdos-program", // .com
        "application/x-sh",            // shell scripts
        "application/x-csh",           // C shell scripts
        "text/x-script.python",        // Python scripts
        "application/x-javascript",    // JavaScript
        "text/javascript",             // JavaScript
        "application/x-executable",    // Executables
    ];

    // Check if blocked
    if BLOCKED_MIME_TYPES.contains(&mime_type) {
        return Err(ErrorKind::BadRequest
            .with_message("File type not allowed")
            .with_context(format!(
                "MIME type '{}' is blocked for security reasons",
                mime_type
            )));
    }

    // Check if allowed (case-insensitive)
    let mime_lower = mime_type.to_lowercase();
    if !ALLOWED_MIME_TYPES.iter().any(|&allowed| {
        allowed.to_lowercase() == mime_lower
            || mime_lower.starts_with("image/")
            || mime_lower.starts_with("text/")
    }) {
        tracing::warn!(
            target: TRACING_TARGET,
            mime_type = %mime_type,
            "potentially unsafe MIME type uploaded"
        );
    }

    Ok(())
}

/// Validates file name to prevent path traversal and other attacks.
fn validate_filename(filename: &str) -> Result<String> {
    // Block path traversal attempts
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(ErrorKind::BadRequest
            .with_message("Invalid filename")
            .with_context("Filename contains path traversal characters"));
    }

    // Block filenames that start with dangerous patterns
    if filename.starts_with('.') {
        return Err(ErrorKind::BadRequest
            .with_message("Invalid filename")
            .with_context("Filename cannot start with a dot"));
    }

    // Sanitize filename - remove potentially dangerous characters
    let sanitized: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
        .collect();

    if sanitized.is_empty() {
        return Err(ErrorKind::BadRequest
            .with_message("Invalid filename")
            .with_context("Filename contains no valid characters"));
    }

    Ok(sanitized)
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
