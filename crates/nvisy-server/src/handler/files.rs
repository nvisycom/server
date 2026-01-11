//! Workspace file upload and management handlers.
//!
//! This module provides comprehensive file management functionality for workspaces,
//! including upload, download, metadata management, and file operations. All
//! operations are secured with workspace-level authorization and include virus
//! scanning and content validation.

use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use futures::StreamExt;
use nvisy_nats::NatsClient;
use nvisy_nats::object::{DocumentKey, DocumentStore, Files as FilesBucket};
use nvisy_nats::stream::{DocumentJobPublisher, PreprocessingData};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{DocumentFile, NewDocumentFile};
use nvisy_postgres::query::DocumentFileRepository;
use nvisy_postgres::types::ProcessingStatus;
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Multipart, Path, Permission, Query, ValidateJson,
};
use crate::handler::request::{
    CursorPagination, DeleteFiles, DownloadFiles, FilePathParams, ListFiles, UpdateFile,
    WorkspacePathParams,
};
use crate::handler::response::{self, ErrorResponse, File, Files, FilesPage};
use crate::handler::{ErrorKind, Result};
use crate::middleware::DEFAULT_MAX_FILE_BODY_SIZE;
use crate::service::{ArchiveFormat, ArchiveService, ServiceState};

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

/// Lists files in a workspace with cursor-based pagination.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_files(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    Query(files_query): Query<ListFiles>,
    Query(cursor_pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<FilesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::ViewFiles)
        .await?;

    let page = conn
        .cursor_list_workspace_files(
            path_params.workspace_id,
            cursor_pagination.into(),
            files_query.to_filter(),
        )
        .await?;

    let response = FilesPage::from_cursor_page(page, File::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        file_count = response.items.len(),
        has_more = response.next_cursor.is_some(),
        "Files listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List files")
        .description(
            "Lists files in a workspace with cursor-based pagination. Use the `after` parameter with the `nextCursor` value from the response to fetch subsequent pages.",
        )
        .response::<200, Json<FilesPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Context for processing a single file upload.
#[derive(Clone)]
struct FileUploadContext {
    workspace_id: Uuid,
    account_id: Uuid,
    document_store: DocumentStore<FilesBucket>,
    publisher: DocumentJobPublisher<PreprocessingData>,
}

/// Processes a single file from a multipart upload using streaming.
async fn process_single_file(
    conn: &mut nvisy_postgres::PgConn,
    ctx: &FileUploadContext,
    field: axum::extract::multipart::Field<'_>,
) -> Result<DocumentFile> {
    let filename = field
        .file_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("file_{}.bin", Uuid::now_v7()));

    let file_extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("bin")
        .to_lowercase();

    // Generate document key with unique object ID for NATS storage
    let document_key = DocumentKey::generate(ctx.workspace_id);

    tracing::debug!(
        target: TRACING_TARGET,
        object_id = %document_key.object_id(),
        "Streaming file to storage"
    );

    // Step 1: Stream upload to NATS (computes SHA-256 on-the-fly)
    let reader = tokio_util::io::StreamReader::new(
        field.map(|result| result.map_err(std::io::Error::other)),
    );

    let put_result = ctx.document_store.put(&document_key, reader).await?;

    tracing::debug!(
        target: TRACING_TARGET,
        object_id = %document_key.object_id(),
        size = put_result.size(),
        sha256 = %put_result.sha256_hex(),
        "File streamed to storage"
    );

    // Step 2: Create DB record with all storage info (Postgres generates its own id)
    let file_record = NewDocumentFile {
        workspace_id: ctx.workspace_id,
        account_id: ctx.account_id,
        display_name: Some(filename.clone()),
        original_filename: Some(filename),
        file_extension: Some(file_extension.clone()),
        file_size_bytes: put_result.size() as i64,
        file_hash_sha256: put_result.sha256().to_vec(),
        storage_path: document_key.to_string(),
        storage_bucket: ctx.document_store.bucket().to_owned(),
        processing_status: Some(ProcessingStatus::Pending),
        ..Default::default()
    };

    let created_file = conn.create_document_file(file_record).await?;

    // Step 3: Publish job to queue (use Postgres-generated file ID)
    let job = nvisy_nats::stream::DocumentJob::new(
        created_file.id,
        document_key.to_string(),
        file_extension,
        PreprocessingData::default(),
    );

    ctx.publisher.publish_job(&job).await.map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            file_id = %created_file.id,
            "Failed to publish document job"
        );
        ErrorKind::InternalServerError.with_message("Failed to queue file for processing")
    })?;

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %created_file.id,
        job_id = %job.id,
        "Document job published"
    );

    Ok(created_file)
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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    Multipart(mut multipart): Multipart,
) -> Result<(StatusCode, Json<Files>)> {
    tracing::info!(target: TRACING_TARGET, "Uploading files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::UploadFiles)
        .await?;

    let document_store = nats_client.document_store::<FilesBucket>().await?;

    let publisher = nats_client
        .document_job_publisher::<PreprocessingData>()
        .await?;

    let ctx = FileUploadContext {
        workspace_id: path_params.workspace_id,
        account_id: auth_claims.account_id,
        document_store,
        publisher,
    };

    let mut uploaded_files = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read multipart field");
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        let created_file = process_single_file(&mut conn, &ctx, field).await?;
        uploaded_files.push(response::File::from_model(created_file));
    }

    if uploaded_files.is_empty() {
        return Err(ErrorKind::BadRequest.with_message("No files provided in multipart request"));
    }

    tracing::info!(
        target: TRACING_TARGET,
        file_count = uploaded_files.len(),
        "Files uploaded",
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

/// Gets file metadata without downloading the content.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn read_file(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<File>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading file metadata");

    let mut conn = pg_client.get_connection().await?;

    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_claims
        .authorize_workspace(&mut conn, file.workspace_id, Permission::ViewFiles)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "File metadata retrieved");

    Ok((StatusCode::OK, Json(File::from_model(file))))
}

fn read_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get file metadata")
        .description("Returns file metadata without downloading the file content.")
        .response::<200, Json<File>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
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
    State(pg_client): State<PgClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateFile>,
) -> Result<(StatusCode, Json<File>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating file");

    let mut conn = pg_client.get_connection().await?;

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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading file");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the file first to get workspace context for authorization
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_claims
        .authorize_workspace(&mut conn, file.workspace_id, Permission::DownloadFiles)
        .await?;

    let document_store = nats_client
        .document_store::<FilesBucket>()
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                "Failed to create document store"
            );
            ErrorKind::InternalServerError.with_message("Failed to initialize file storage")
        })?;

    let document_key = DocumentKey::from_str(&file.storage_path).map_err(|err| {
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

    // Get streaming content from NATS document store
    let get_result = document_store
        .get(&document_key)
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
        get_result.size().to_string().parse().unwrap(),
    );
    headers.insert("content-type", "application/octet-stream".parse().unwrap());

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %path_params.file_id,
        size = get_result.size(),
        "Streaming file download"
    );

    // Stream the file content using ReaderStream
    let stream = tokio_util::io::ReaderStream::new(get_result.into_reader());
    let body = Body::from_stream(stream);

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
    State(pg_client): State<PgClient>,
    Path(path_params): Path<FilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "File Deleting");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the file first to get workspace context for authorization
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_claims
        .authorize_workspace(&mut conn, file.workspace_id, Permission::DeleteFiles)
        .await?;

    conn.delete_document_file(path_params.file_id)
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

/// Deletes multiple files (soft delete).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn delete_multiple_files(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<DeleteFiles>,
) -> Result<StatusCode> {
    tracing::info!(target: TRACING_TARGET, file_count = request.file_ids.len(), "Deleting multiple files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::DeleteFiles)
        .await?;

    // Soft delete all files in a single query
    let deleted_count = conn
        .delete_document_files(path_params.workspace_id, &request.file_ids)
        .await?;

    // Check if all requested files were deleted
    if deleted_count != request.file_ids.len() {
        tracing::warn!(
            target: TRACING_TARGET,
            requested = request.file_ids.len(),
            deleted = deleted_count,
            "Some files were not found or already deleted"
        );
        return Err(ErrorKind::NotFound
            .with_message("One or more files not found")
            .with_resource("file"));
    }

    tracing::info!(target: TRACING_TARGET, file_count = deleted_count, "Files deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_multiple_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete multiple files")
        .description("Soft deletes multiple files by setting deleted timestamps. Files can be recovered within the retention period.")
        .response::<204, ()>()
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
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(archive): State<ArchiveService>,
    Path(path_params): Path<WorkspacePathParams>,
    AuthState(auth_claims): AuthState,
    Json(request): Json<DownloadFiles>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading archived files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::DownloadFiles,
        )
        .await?;

    let document_store = nats_client
        .document_store::<FilesBucket>()
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                "Failed to create document store"
            );
            ErrorKind::InternalServerError.with_message("Failed to initialize file storage")
        })?;

    // Determine which files to download
    let files = if let Some(specific_ids) = request.file_ids {
        // Batch fetch specific files
        conn.find_document_files_by_ids(&specific_ids).await?
    } else {
        // Get all workspace files using the workspace-scoped query
        conn.cursor_list_workspace_files(
            path_params.workspace_id,
            Default::default(),
            Default::default(),
        )
        .await?
        .items
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
        let document_key = DocumentKey::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(format!("Parse error: {}", err))
        })?;

        if let Ok(Some(mut get_result)) = document_store.get(&document_key).await {
            let mut buffer = Vec::with_capacity(get_result.size());
            if tokio::io::AsyncReadExt::read_to_end(get_result.reader(), &mut buffer)
                .await
                .is_ok()
            {
                files_data.push((file.display_name.clone(), buffer));
            }
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

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspaceId}/files/",
            post_with(upload_file, upload_file_docs)
                .layer(DefaultBodyLimit::max(DEFAULT_MAX_FILE_BODY_SIZE))
                .get_with(list_files, list_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/files/batch",
            get_with(download_archived_files, download_archived_files_docs)
                .delete_with(delete_multiple_files, delete_multiple_files_docs),
        )
        // File-specific routes (file ID is globally unique)
        .api_route(
            "/files/{fileId}",
            get_with(read_file, read_file_docs)
                .patch_with(update_file, update_file_docs)
                .delete_with(delete_file, delete_file_docs),
        )
        .api_route(
            "/files/{fileId}/content",
            get_with(download_file, download_file_docs),
        )
        .with_path_items(|item| item.tag("Files"))
}
