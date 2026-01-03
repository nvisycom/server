//! Workspace file upload and management handlers.
//!
//! This module provides comprehensive file management functionality for workspaces,
//! including upload, download, metadata management, and file operations. All
//! operations are secured with workspace-level authorization and include virus
//! scanning and content validation.

use std::collections::HashMap;
use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use nvisy_nats::NatsClient;
use nvisy_nats::object::{DocumentFileStore, DocumentLabel, InputFiles, ObjectKey};
use nvisy_postgres::model::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
use nvisy_postgres::query::DocumentFileRepository;
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Multipart, Path, Permission, PgPool, Query, ValidateJson,
};
use crate::handler::request::{
    DownloadArchivedFilesRequest, DownloadMultipleFilesRequest, FilePathParams, ListFilesQuery,
    Pagination, UpdateFile as UpdateFileRequest, WorkspacePathParams,
};
use crate::handler::response::{self, ErrorResponse, File, Files};
use crate::handler::{ErrorKind, Result};
use crate::service::{ArchiveFormat, ArchiveService, ServiceState};
use crate::utility::constants::MAX_FILE_SIZE;

/// Tracing target for workspace file operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_files";

/// Finds a file by ID or returns NotFound error.
async fn find_file(conn: &mut nvisy_postgres::PgConn, file_id: Uuid) -> Result<DocumentFile> {
    conn.find_document_file_by_id(file_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("File not found")
                .with_resource("file")
        })
}

/// Lists files in a workspace with optional filtering and sorting.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_files(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    Query(query): Query<ListFilesQuery>,
    Query(pagination): Query<Pagination>,
) -> Result<(StatusCode, Json<Files>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing files");

    auth_claims
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::ViewFiles)
        .await?;

    let files = conn
        .find_workspace_files_filtered(
            path_params.workspace_id,
            pagination.into(),
            query.to_sort(),
            query.to_filter(),
        )
        .await?;

    let response: Files = response::File::from_models(files);

    tracing::debug!(
        target: TRACING_TARGET,
        file_count = response.len(),
        "Files listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List files")
        .description(
            "Lists all files in a workspace with optional filtering by format and sorting.",
        )
        .response::<200, Json<Files>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Uploads input files to a workspace for processing.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn upload_file(
    PgPool(mut conn): PgPool,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Files>)> {
    tracing::info!(target: TRACING_TARGET, "Uploading files");

    let input_fs = nats_client.document_store::<InputFiles>().await?;

    auth_claims
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::UploadFiles)
        .await?;

    let mut uploaded_files = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read multipart field");
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        let filename = field
            .file_name()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("file_{}.bin", Uuid::now_v7()));

        // Validate and sanitize filename
        let filename = validate_filename(&filename)?;

        // Read file data with size limit to prevent DoS
        let mut data = Vec::new();
        let mut stream = field;

        while let Some(chunk) = stream.chunk().await.map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read file chunk");
            ErrorKind::BadRequest
                .with_message("Failed to read file data")
                .with_context(format!("Could not read file: {}", err))
        })? {
            // Check size before adding chunk to prevent memory exhaustion
            if data.len() + chunk.len() > MAX_FILE_SIZE {
                return Err(ErrorKind::BadRequest
                    .with_message("File too large")
                    .with_context(format!(
                        "File exceeds maximum size of {} MB",
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

        // Generate file ID and storage path
        let file_id = Uuid::now_v7();
        let object_key = input_fs.create_key(path_params.workspace_id, file_id);

        // Upload to NATS document store first (external system)
        // This order ensures we don't have orphaned DB records if storage fails
        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            size = file_size_bytes,
            "Uploading file to storage"
        );

        input_fs
            .put(&object_key, &content_data)
            .await
            .map_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "Failed to upload file to storage"
                );
                ErrorKind::InternalServerError
                    .with_message("Failed to upload file")
                    .with_context(format!("Storage upload failed: {}", err))
            })?;

        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            "File uploaded to storage"
        );

        // Create file record in database after successful storage upload
        let file_record = NewDocumentFile {
            workspace_id: path_params.workspace_id,
            document_id: None,
            account_id: auth_claims.account_id,
            display_name: Some(filename.clone()),
            original_filename: Some(filename.clone()),
            file_extension: Some(file_extension.clone()),
            file_size_bytes: Some(file_size_bytes),
            storage_path: object_key.as_str().to_string(),
            storage_bucket: Some(InputFiles::bucket_name().to_string()),
            file_hash_sha256: sha256_bytes,
            ..Default::default()
        };

        let created_file = match conn.create_document_file(file_record).await {
            Ok(file) => file,
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    file_id = %file_id,
                    "Failed to create file record, cleaning up storage"
                );

                // Best effort cleanup - delete the orphan storage object
                if let Err(cleanup_err) = input_fs.delete(&object_key).await {
                    tracing::error!(
                        target: TRACING_TARGET,
                        error = %cleanup_err,
                        file_id = %file_id,
                        "Failed to cleanup orphan storage object"
                    );
                }

                return Err(ErrorKind::InternalServerError
                    .with_message("Failed to save file metadata")
                    .with_context(format!("Database error: {}", err)));
            }
        };

        let uploaded_file = response::File::from_model(created_file.clone());

        // Publish file processing job to queue
        let job = nvisy_nats::stream::DocumentJob::new_file_processing(
            created_file.id,
            path_params.workspace_id,
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
                    "Failed to create document job publisher"
                );
                ErrorKind::InternalServerError.with_message("Failed to queue file for processing")
            })?;

        publisher.publish("pending", &job).await.map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                file_id = %file_id,
                "Failed to publish document job"
            );
            ErrorKind::InternalServerError.with_message("Failed to queue file for processing")
        })?;

        tracing::debug!(
            target: TRACING_TARGET,
            file_id = %file_id,
            job_id = %job.id,
            "Document job published"
        );

        uploaded_files.push(uploaded_file);
    }

    // Check if any files were uploaded
    if uploaded_files.is_empty() {
        return Err(ErrorKind::BadRequest.with_message("No files provided in multipart request"));
    }

    tracing::info!(
        target: TRACING_TARGET,
        file_count = uploaded_files.len(),
        "Files uploaded ",
    );

    Ok((StatusCode::CREATED, Json(uploaded_files)))
}

fn upload_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Upload files")
        .description("Uploads one or more files to a document for processing. Files are validated, stored, and queued for processing.")
        .response::<201, Json<Files>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Updates file metadata.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn update_file(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateFileRequest>,
) -> Result<(StatusCode, Json<File>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating file");

    // Fetch the file first to get workspace context for authorization
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_claims
        .authorize_workspace(&mut conn, file.workspace_id, Permission::UpdateFiles)
        .await?;

    let updates = request.into_model();

    let updated_file = conn
        .update_document_file(path_params.file_id, updates)
        .await
        .map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to update file");
            ErrorKind::InternalServerError.with_message("Failed to update file")
        })?;

    tracing::info!(target: TRACING_TARGET, "File updated");

    Ok((
        StatusCode::OK,
        Json(response::File::from_model(updated_file)),
    ))
}

fn update_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update file")
        .description("Updates file metadata such as display name or processing priority.")
        .response::<200, Json<File>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Downloads a file with streaming support for large files.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn download_file(
    PgPool(mut conn): PgPool,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading file");

    let input_fs = nats_client.document_store::<InputFiles>().await?;

    // Fetch the file first to get workspace context for authorization
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_claims
        .authorize_workspace(&mut conn, file.workspace_id, Permission::DownloadFiles)
        .await?;

    // Create object key from storage path
    let object_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            storage_path = %file.storage_path,
            "Invalid storage path format"
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
                "Failed to retrieve file from storage"
            );
            ErrorKind::InternalServerError
                .with_message("Failed to retrieve file")
                .with_context(format!("Storage retrieval failed: {}", err))
        })?
        .ok_or_else(|| {
            tracing::warn!(
                target: TRACING_TARGET,
                file_id = %path_params.file_id,
                "File content not found in storage"
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

    headers.insert("content-type", "application/octet-stream".parse().unwrap());

    tracing::debug!(target: TRACING_TARGET, "File downloaded");

    // Stream the file content
    let bytes = content_data.into_bytes().to_vec();
    let body = Body::from(bytes);

    Ok((StatusCode::OK, headers, body))
}

fn download_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download file")
        .description("Downloads a file by ID. Returns the file content as a binary stream.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a file (soft delete).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn delete_file(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "File Deleting");

    // Fetch the file first to get workspace context for authorization
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_claims
        .authorize_workspace(&mut conn, file.workspace_id, Permission::DeleteFiles)
        .await?;

    // Soft delete by setting deleted_at timestamp
    let updates = UpdateDocumentFile {
        deleted_at: Some(Some(jiff::Timestamp::now().into())),
        ..Default::default()
    };

    conn.update_document_file(path_params.file_id, updates)
        .await
        .map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to soft delete file");
            ErrorKind::InternalServerError
                .with_message("Failed to delete file")
                .with_context(format!("Database error: {}", err))
        })?;

    tracing::info!(target: TRACING_TARGET, "File deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete file")
        .description("Soft deletes a file by setting a deleted timestamp. The file can be recovered within the retention period.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Downloads multiple files as a zip archive.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn download_multiple_files(
    PgPool(mut conn): PgPool,
    State(nats_client): State<NatsClient>,
    State(archive): State<ArchiveService>,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<DownloadMultipleFilesRequest>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading multiple files as archive");

    auth_claims
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::DownloadFiles,
        )
        .await?;

    let input_fs = nats_client.document_store::<InputFiles>().await?;

    // Batch fetch all requested files that belong to this workspace
    let files = conn.find_document_files_by_ids(&request.file_ids).await?;

    // Create a map for quick lookup and verify all files belong to workspace
    let files_map: HashMap<Uuid, DocumentFile> = files
        .into_iter()
        .filter(|f| f.workspace_id == path_params.workspace_id && f.deleted_at.is_none())
        .map(|f| (f.id, f))
        .collect();

    // Verify all requested files were found
    for file_id in &request.file_ids {
        if !files_map.contains_key(file_id) {
            tracing::warn!(target: TRACING_TARGET, %file_id, "File not found during batch download");
            return Err(ErrorKind::NotFound.with_message("One or more requested files not found"));
        }
    }

    // Fetch all file contents
    let mut files_data = Vec::new();

    for file_id in &request.file_ids {
        let file = files_map.get(file_id).unwrap(); // Safe - we verified above

        let object_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(format!("Parse error: {}", err))
        })?;

        let content_data = input_fs.get(&object_key).await?.ok_or_else(|| {
            tracing::error!(target: TRACING_TARGET, %file_id, "File content missing from storage");
            ErrorKind::NotFound.with_message("File content not found")
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
            "attachment; filename=\"workspace_{}_files.zip\"",
            path_params.workspace_id
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
        file_count = request.file_ids.len(),
        "Multiple files downloaded as archive",
    );

    Ok((StatusCode::OK, headers, archive_bytes))
}

fn download_multiple_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download multiple files")
        .description("Downloads multiple files as a zip archive. Provide a list of file IDs to include in the archive.")
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Downloads all or specific workspace files as an archive.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn download_archived_files(
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    PgPool(mut conn): PgPool,
    State(nats_client): State<NatsClient>,
    State(archive): State<ArchiveService>,
    Json(request): Json<DownloadArchivedFilesRequest>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading archived files");

    auth_claims
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::DownloadFiles,
        )
        .await?;

    let input_fs = nats_client.document_store::<InputFiles>().await?;

    // Determine which files to download
    let files = if let Some(specific_ids) = request.file_ids {
        // Batch fetch specific files
        conn.find_document_files_by_ids(&specific_ids).await?
    } else {
        // Get all workspace files using the workspace-scoped query
        conn.find_document_files_by_workspace(
            path_params.workspace_id,
            Pagination::default().into(),
        )
        .await?
    };

    // Filter to only files belonging to this workspace and not deleted
    let valid_files: Vec<_> = files
        .into_iter()
        .filter(|f| f.workspace_id == path_params.workspace_id && f.deleted_at.is_none())
        .collect();

    if valid_files.is_empty() {
        return Err(ErrorKind::NotFound.with_message("No files found for archive"));
    }

    // Fetch all file contents
    let mut files_data = Vec::new();

    for file in &valid_files {
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
            "attachment; filename=\"workspace_{}_archive.{}\"",
            path_params.workspace_id, extension
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
        file_count = valid_files.len(),
        "Workspace files downloaded as archive",
    );

    Ok((StatusCode::OK, headers, archive_bytes))
}

fn download_archived_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download archived files")
        .description("Downloads all or specific workspace files as a compressed archive. Supports zip and tar.gz formats.")
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Validates file name to prevent path traversal and other attacks.
fn validate_filename(filename: &str) -> Result<String> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(ErrorKind::BadRequest
            .with_message("Invalid filename")
            .with_context("Filename contains path traversal characters"));
    }

    if filename.starts_with('.') {
        return Err(ErrorKind::BadRequest
            .with_message("Invalid filename")
            .with_context("Filename cannot start with a dot"));
    }

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
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspace_id}/files/",
            get_with(list_files, list_files_docs).post_with(upload_file, upload_file_docs),
        )
        .api_route(
            "/workspaces/{workspace_id}/files/download",
            post_with(download_multiple_files, download_multiple_files_docs),
        )
        .api_route(
            "/workspaces/{workspace_id}/files/archive",
            post_with(download_archived_files, download_archived_files_docs),
        )
        // File-specific routes (file ID is globally unique)
        .api_route(
            "/files/{file_id}",
            patch_with(update_file, update_file_docs)
                .get_with(download_file, download_file_docs)
                .delete_with(delete_file, delete_file_docs),
        )
        .with_path_items(|item| item.tag("Files"))
}
