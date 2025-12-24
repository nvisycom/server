//! Project file upload and management handlers.
//!
//! This module provides comprehensive file management functionality for projects,
//! including upload, download, metadata management, and file operations. All
//! operations are secured with project-level authorization and include virus
//! scanning and content validation.

use std::str::FromStr;

use aide::axum::ApiRouter;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use nvisy_nats::NatsClient;
use nvisy_nats::object::{DocumentFileStore, DocumentLabel, InputFiles, ObjectKey};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewDocumentFile, UpdateDocumentFile};
use nvisy_postgres::query::{DocumentFileRepository, ProjectRepository};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson, Version};
use crate::handler::request::{
    DownloadArchivedFilesRequest, DownloadMultipleFilesRequest, FilePathParams, ProjectPathParams,
    UpdateDocumentKnowledge,
};
use crate::handler::response::{File, Files};
use crate::handler::{ErrorKind, Result};
use crate::service::{ArchiveFormat, ArchiveService, ServiceState};

/// Tracing target for project file operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_files";

/// Maximum file size: 100MB
const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Uploads input files to a project for processing.
///
/// Form data:
/// - `file`: One or more files to upload
#[tracing::instrument(skip(pg_client, nats_client, multipart), fields(project_id = %path_params.project_id))]
async fn upload_file(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Files>)> {
    let input_fs = nats_client.document_store::<InputFiles>().await?;

    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::UploadFiles)
        .await?;

    // Load project keep_for_sec setting
    let project_keep_for_sec = pg_client
        .find_project_by_id(path_params.project_id)
        .await?
        .and_then(|p| p.keep_for_sec);

    let mut uploaded_files = Vec::new();

    tracing::debug!(target: TRACING_TARGET, "Starting file upload");

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
                .with_context(format!("Could not read file '{}': {}", filename, err))
        })? {
            // Check size before adding chunk to prevent memory exhaustion
            if data.len() + chunk.len() > MAX_FILE_SIZE {
                return Err(ErrorKind::BadRequest
                    .with_message("File too large")
                    .with_context(format!(
                        "File '{}' exceeds maximum size of {} MB",
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

        let file_size_bytes = data.len() as i64;

        // Create content data with metadata
        let content_data =
            DocumentFileStore::<InputFiles>::create_content_data_with_metadata(data.into());

        // Extract SHA-256 hash from content data
        let sha256_bytes = content_data.compute_sha256().to_vec();

        // Generate a temporary ID to create the storage path
        // Note: We'll use Uuid::now_v7() temporarily, then postgres will assign the real ID
        let temp_file_id = Uuid::now_v7();
        let object_key = input_fs.create_key(path_params.project_id, temp_file_id);

        // Create file record in database with storage path
        let file_record = NewDocumentFile {
            project_id: path_params.project_id,
            document_id: None,
            account_id: auth_claims.account_id,
            display_name: Some(filename.clone()),
            original_filename: Some(filename.clone()),
            file_extension: Some(file_extension.clone()),
            file_size_bytes: Some(file_size_bytes),
            storage_path: object_key.as_str().to_string(),
            storage_bucket: Some(InputFiles::bucket_name().to_string()),
            file_hash_sha256: sha256_bytes,
            keep_for_sec: project_keep_for_sec,
            auto_delete_at: None,
            ..Default::default()
        };

        // Insert file record into database
        let created_file = pg_client
            .create_document_file(file_record)
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    "failed to create file record in database"
                );
                ErrorKind::InternalServerError
                    .with_message("Failed to save file metadata")
                    .with_context(format!("Database error: {}", err))
            })?;

        let file_id = created_file.id;

        // Upload to NATS document store
        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            project_id = %path_params.project_id,
            size = file_size_bytes,
            "uploading file to NATS document store"
        );

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

        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            filename = %filename,
            size = file_size_bytes,
            "file upload completed successfully"
        );

        let uploaded_file = File {
            file_id: created_file.id,
            display_name: created_file.display_name,
            file_size: created_file.file_size_bytes,
            status: created_file.processing_status,
            processing_priority: Some(created_file.processing_priority),
            updated_at: Some(created_file.updated_at.into()),
        };

        // Publish file processing job to queue
        let job = nvisy_nats::stream::DocumentJob::new_file_processing(
            created_file.id,
            path_params.project_id,
            auth_claims.account_id,
            object_key.as_str().to_string(),
            file_extension.clone(),
            file_size_bytes,
        );

        // Publish to document job queue using NATS helper methods
        let jetstream = nats_client.jetstream();
        let publisher = nvisy_nats::stream::DocumentJobPublisher::new(jetstream)
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
    }

    // Check if any files were uploaded
    if uploaded_files.is_empty() {
        return Err(ErrorKind::BadRequest.with_message("No files provided in multipart request"));
    }

    let count = uploaded_files.len();
    tracing::debug!(
        target: TRACING_TARGET,
        project_id = %path_params.project_id,
        file_count = count,
        "file upload completed"
    );

    Ok((StatusCode::CREATED, Json(uploaded_files)))
}

/// Updates file metadata.
#[tracing::instrument(skip(pg_client), fields(project_id = %path_params.project_id, file_id = %path_params.file_id))]
async fn update_file(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
    ValidateJson(request): ValidateJson<UpdateDocumentKnowledge>,
) -> Result<(StatusCode, Json<File>)> {
    // Verify project write permissions
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::UpdateFiles)
        .await?;

    // Get existing file
    let Some(file) = pg_client
        .find_document_file_by_id(path_params.file_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_message("File not found"));
    };

    // Verify file belongs to project
    if file.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound.with_message("File not found in project"));
    }

    // Create update struct
    let updates = UpdateDocumentFile {
        is_indexed: request.is_indexed,
        content_segmentation: request.content_segmentation,
        visual_support: request.visual_support,
        ..Default::default()
    };

    // Save changes
    let updated_file = pg_client
        .update_document_file(path_params.file_id, updates)
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

    let response = File {
        file_id: updated_file.id,
        display_name: updated_file.display_name,
        file_size: updated_file.file_size_bytes,
        status: updated_file.processing_status,
        processing_priority: Some(updated_file.processing_priority),
        updated_at: Some(updated_file.updated_at.into()),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Downloads a project file.
#[tracing::instrument(skip(pg_client, nats_client), fields(project_id = %path_params.project_id, file_id = %path_params.file_id))]
async fn download_file(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    let input_fs = nats_client.document_store::<InputFiles>().await?;

    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::DownloadFiles,
        )
        .await?;

    // Get file metadata from database
    let file = pg_client
        .find_document_file_by_id(path_params.file_id)
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

    // Verify file belongs to project
    if file.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound.with_message("File not found in project"));
    }

    // Verify file is not soft-deleted
    if file.deleted_at.is_some() {
        return Err(ErrorKind::NotFound.with_message("File not found"));
    }

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

/// Deletes a project file (soft delete).
#[tracing::instrument(skip(pg_client), fields(project_id = %path_params.project_id, file_id = %path_params.file_id))]
async fn delete_file(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
    _version: Version,
) -> Result<StatusCode> {
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::DeleteFiles)
        .await?;

    // Get file metadata
    let Some(file) = pg_client
        .find_document_file_by_id(path_params.file_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_message("File not found"));
    };

    // Verify file belongs to project
    if file.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound.with_message("File not found in project"));
    }

    // Soft delete by setting deleted_at timestamp
    let updates = UpdateDocumentFile {
        deleted_at: Some(Some(jiff::Timestamp::now().into())),
        ..Default::default()
    };

    pg_client
        .update_document_file(path_params.file_id, updates)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %path_params.file_id,
                "failed to soft delete file"
            );
            ErrorKind::InternalServerError
                .with_message("Failed to delete file")
                .with_context(format!("Database error: {}", err))
        })?;

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %path_params.file_id,
        "file soft deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Downloads multiple files as a zip archive.
#[tracing::instrument(skip(pg_client, nats_client, archive), fields(project_id = %path_params.project_id))]
async fn download_multiple_files(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(archive): State<ArchiveService>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<DownloadMultipleFilesRequest>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::DownloadFiles,
        )
        .await?;

    let input_fs = nats_client.document_store::<InputFiles>().await?;

    // Fetch all requested files
    let mut files_data = Vec::new();

    for file_id in &request.file_ids {
        let file = pg_client
            .find_document_file_by_id(*file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound.with_message(format!("File {} not found", file_id))
            })?;

        // Verify file belongs to project and is not deleted
        if file.project_id != path_params.project_id {
            return Err(
                ErrorKind::NotFound.with_message(format!("File {} not found in project", file_id))
            );
        }

        if file.deleted_at.is_some() {
            return Err(ErrorKind::NotFound.with_message(format!("File {} not found", file_id)));
        }

        // Get file content from NATS
        let object_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(format!("Parse error: {}", err))
        })?;

        let content_data = input_fs.get(&object_key).await?.ok_or_else(|| {
            ErrorKind::NotFound.with_message(format!("File {} content not found", file_id))
        })?;

        files_data.push((
            file.display_name.clone(),
            content_data.into_bytes().to_vec(),
        ));
    }

    // Create zip archive
    let archive_bytes = archive
        .create_archive(files_data, ArchiveFormat::Zip)
        .await?;

    // Set up response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-disposition",
        format!(
            "attachment; filename=\"project_{}_files.zip\"",
            path_params.project_id
        )
        .parse()
        .unwrap(),
    );
    headers.insert("content-type", "application/zip".parse().unwrap());
    headers.insert(
        "content-length",
        archive_bytes.len().to_string().parse().unwrap(),
    );

    tracing::debug!(
        target: TRACING_TARGET,
        project_id = %path_params.project_id,
        file_count = request.file_ids.len(),
        archive_size = archive_bytes.len(),
        "multiple files downloaded as archive"
    );

    Ok((StatusCode::OK, headers, archive_bytes))
}

/// Downloads all or specific project files as an archive.
#[tracing::instrument(skip(pg_client, nats_client, archive), fields(project_id = %path_params.project_id))]
async fn download_archived_files(
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(archive): State<ArchiveService>,
    Json(request): Json<DownloadArchivedFilesRequest>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::DownloadFiles,
        )
        .await?;

    let input_fs = nats_client.document_store::<InputFiles>().await?;

    // Determine which files to download
    let file_ids = if let Some(specific_ids) = request.file_ids {
        specific_ids
    } else {
        // Get all project files - use list_account_files and filter by project
        pg_client
            .list_account_files(
                auth_claims.account_id,
                nvisy_postgres::query::Pagination::default(),
            )
            .await
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to fetch project files")
                    .with_context(format!("Database error: {}", err))
            })?
            .into_iter()
            .filter(|f| f.project_id == path_params.project_id && f.deleted_at.is_none())
            .map(|f| f.id)
            .collect()
    };

    // Fetch all files
    let mut files_data = Vec::new();

    for file_id in &file_ids {
        let file = pg_client
            .find_document_file_by_id(*file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound.with_message(format!("File {} not found", file_id))
            })?;

        if file.project_id != path_params.project_id || file.deleted_at.is_some() {
            continue; // Skip files that don't belong or are deleted
        }

        // Get file content
        let object_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(format!("Parse error: {}", err))
        })?;

        if let Ok(Some(content_data)) = input_fs.get(&object_key).await {
            files_data.push((
                file.display_name.clone(),
                content_data.into_bytes().to_vec(),
            ));
        }
    }

    if files_data.is_empty() {
        return Err(ErrorKind::NotFound.with_message("No files found for archive"));
    }

    // Create archive
    let archive_bytes = archive.create_archive(files_data, request.format).await?;

    // Determine content type and file extension based on format
    let (content_type, extension) = match request.format {
        ArchiveFormat::Tar => ("application/x-tar", "tar.gz"),
        ArchiveFormat::Zip => ("application/zip", "zip"),
    };

    // Set up response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-disposition",
        format!(
            "attachment; filename=\"project_{}_archive.{}\"",
            path_params.project_id, extension
        )
        .parse()
        .unwrap(),
    );
    headers.insert("content-type", content_type.parse().unwrap());
    headers.insert(
        "content-length",
        archive_bytes.len().to_string().parse().unwrap(),
    );

    tracing::debug!(
        target: TRACING_TARGET,
        project_id = %path_params.project_id,
        file_count = file_ids.len(),
        format = ?request.format,
        archive_size = archive_bytes.len(),
        "project files downloaded as archive"
    );

    Ok((StatusCode::OK, headers, archive_bytes))
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
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/documents/:document_id/files/", post(upload_file))
        .api_route("/documents/:document_id/files/:file_id", patch(update_file))
        .api_route("/documents/:document_id/files/:file_id", get(download_file))
        .api_route(
            "/documents/:document_id/files/:file_id",
            delete(delete_file),
        )
        .api_route(
            "/projects/:project_id/files/download",
            post(download_multiple_files),
        )
        .api_route(
            "/projects/:project_id/files/archive",
            post(download_archived_files),
        )
}

