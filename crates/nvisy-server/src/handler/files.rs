//! Workspace file upload and management handlers.
//!
//! This module provides comprehensive file management functionality for workspaces,
//! including upload, download, metadata management, and file operations. All
//! operations are secured with workspace-level authorization.

use std::collections::BTreeSet;
use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::multipart::Field;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use futures::StreamExt;
use nvisy_nats::NatsClient;
use nvisy_nats::object::{FileKey, FilesBucket, ObjectStore};
use nvisy_postgres::model::{NewWorkspaceFile, WorkspaceFile as FileModel};
use nvisy_postgres::query::WorkspaceFileRepository;
use nvisy_postgres::types::{FileKind, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Multipart, Path, Permission, Query, SecurityContext,
    ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    CursorPagination, DeleteFiles, ListFiles, UpdateFile, WorkspaceFilePathParams,
};
use crate::handler::response::{self, ErrorResponse, File, Files, FilesPage};
use crate::handler::utility::{DownloadResponseExt, attachment_headers, resolve_account_ref};
use crate::handler::{Error, ErrorKind, Result};
use crate::middleware::UploadConfig;
use crate::service::{
    CryptoService, EngineService, EventEmitter, EventOrigin, FileRef, HashingReader, LimitedReader,
    RunBlobStore, ServiceState, WorkspaceEvent,
};

/// Tracing target for workspace file operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_files";

/// Finds a file within a workspace or returns NotFound error.
async fn find_file(conn: &mut PgConn, workspace_id: Uuid, file_id: Uuid) -> Result<FileModel> {
    conn.find_file_in_workspace(workspace_id, file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))
}

/// Finds a file within a workspace, with its uploader's identity, or returns a
/// NotFound error.
async fn find_file_with_creator(
    conn: &mut PgConn,
    workspace_id: Uuid,
    file_id: Uuid,
) -> Result<WithAccountRef<FileModel>> {
    conn.find_file_in_workspace_with_creator(workspace_id, file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))
}

/// Lists files in a workspace with cursor-based pagination.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_files(
    State(pg_client): State<PgClient>,
    State(engine): State<EngineService>,
    WorkspaceContext(workspace): WorkspaceContext,
    AuthState(auth_claims): AuthState,
    Query(files_query): Query<ListFiles>,
    Query(cursor_pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<FilesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewFiles)
        .await?;

    let filter = files_query.to_filter(&engine).map_err(|err| {
        ErrorKind::BadRequest
            .with_message("Unknown file format filter")
            .with_context(err.to_string())
    })?;

    let page = conn
        .cursor_list_workspace_files(workspace.id, cursor_pagination.into(), filter)
        .await?;

    let response = FilesPage::from_cursor_page(page, |wc| {
        File::from_model(wc.item, workspace.slug.clone(), wc.account.into())
    });

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
    file_store: ObjectStore<FilesBucket>,
    crypto: CryptoService,
    /// Engine handle, used to reject upload of a format no codec can decode.
    engine: EngineService,
    /// Retention expiry for uploaded originals (`None` = keep indefinitely).
    expires_at: Option<jiff::Timestamp>,
    /// The effective per-file upload cap in bytes — the smaller of the workspace's
    /// soft cap and the server-wide hard limit. A file streaming past it is
    /// rejected before its excess reaches storage. This is a true per-file bound,
    /// unlike the request-body layer, which limits the whole multipart request.
    max_upload_bytes: u64,
}

/// Streams one multipart file to storage and builds its unsaved row.
///
/// Returns the object's storage key alongside the row (rather than inserting it),
/// so the caller can persist the row and record its creation event in one
/// transaction, and reclaim the already-stored object if that transaction fails.
async fn process_single_file(
    ctx: &FileUploadContext,
    field: Field<'_>,
) -> Result<(FileKey, NewWorkspaceFile)> {
    let filename = field
        .file_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("file_{}.bin", Uuid::now_v7()));

    let file_extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("bin")
        .to_lowercase();

    // Reject a format no codec can decode before streaming it to storage: an
    // unprocessable file would only fail later at detection, after wasting an
    // encrypted upload.
    if !ctx.engine.supports_extension(&file_extension) {
        return Err(ErrorKind::BadRequest
            .with_message(format!("Unsupported file format: .{file_extension}"))
            .with_context(
                "The redaction engine has no codec for this file type; upload a supported format.",
            ));
    }

    // Generate file key with unique object ID for NATS storage
    let file_key = FileKey::generate(ctx.workspace_id);

    tracing::debug!(
        target: TRACING_TARGET,
        object_id = %file_key.object_id,
        "Streaming file to storage"
    );

    // Step 1: Encrypt the plaintext as it streams to NATS. The limited reader
    // aborts an oversized upload before its excess is encrypted and stored,
    // enforcing the effective per-file cap directly (the request-body layer only
    // bounds the whole multipart request). The measured reader captures the
    // plaintext size and hash (NATS only sees ciphertext).
    let cap = ctx.max_upload_bytes;
    let source = StreamReader::new(field.map(|result| result.map_err(std::io::Error::other)));
    let (limited, limit_state) = LimitedReader::new(source, cap);
    let (measured, measurements) = HashingReader::new(limited);
    let encrypted = ctx.crypto.encrypt_reader(ctx.workspace_id, measured);

    if let Err(err) = ctx.file_store.put(&file_key, Box::pin(encrypted)).await {
        // The limited reader aborts the stream, which fails the `put`. When that
        // is why it failed, report the size limit (413) rather than a storage
        // error; the reader's error is stringified in transit, so consult the
        // shared state instead of inspecting the error.
        if limit_state.is_exceeded() {
            return Err(ErrorKind::PayloadTooLarge
                .with_message(format!("File exceeds the {cap}-byte upload limit")));
        }
        return Err(err.into());
    }

    tracing::debug!(
        target: TRACING_TARGET,
        object_id = %file_key.object_id,
        size = measurements.bytes(),
        "File encrypted and streamed to storage"
    );

    // Step 2: Create DB record with all storage info (Postgres generates its own id)
    let file_record = NewWorkspaceFile {
        workspace_id: ctx.workspace_id,
        account_id: ctx.account_id,
        display_name: Some(filename.clone()),
        original_filename: Some(filename),
        file_extension: Some(file_extension),
        file_kind: Some(FileKind::Original),
        file_size_bytes: measurements.bytes() as i64,
        file_hash_sha256: measurements.sha256().to_vec(),
        storage_path: file_key.to_string(),
        storage_bucket: ctx.file_store.bucket().to_owned(),
        expires_at: ctx.expires_at.map(Into::into),
        ..Default::default()
    };

    Ok((file_key, file_record))
}

/// Uploads input files to a workspace for processing.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn upload_file(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    State(upload): State<UploadConfig>,
    WorkspaceContext(workspace): WorkspaceContext,
    AuthState(auth_claims): AuthState,
    security: SecurityContext,
    Multipart(mut multipart): Multipart,
) -> Result<(StatusCode, Json<Files>)> {
    tracing::info!(target: TRACING_TARGET, "Uploading files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::UploadFiles)
        .await?;

    let file_store = nats_client.object_store::<FilesBucket>().await?;

    // The uploader is the caller; resolve their identity once for every file below.
    let uploaded_by = resolve_account_ref(&mut conn, auth_claims.account_id).await?;

    // Read workspace settings once for the whole batch: the retention expiry for
    // uploaded originals, and the effective per-file upload cap (the workspace's
    // soft cap clamped to the server-wide hard limit).
    let settings = workspace.settings.or_default();
    let expires_at = settings
        .retention
        .original_documents
        .expires_at(jiff::Timestamp::now());
    let max_upload_bytes = settings.effective_max_upload_bytes(upload.max_file_bytes());

    let ctx = FileUploadContext {
        workspace_id: workspace.id,
        account_id: auth_claims.account_id,
        file_store,
        crypto,
        engine,
        expires_at,
        max_upload_bytes,
    };

    let mut uploaded_files = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read multipart field");
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        if field.file_name().is_none() {
            tracing::debug!(
                target: TRACING_TARGET,
                name = ?field.name(),
                "Skipping non-file multipart field"
            );
            continue;
        }

        let (file_key, file_record) = process_single_file(&ctx, field).await?;

        // Persist the row and record its creation event in one transaction, so
        // the event is never lost, nor recorded for a row that rolled back.
        let created_file = match conn
            .transaction(async |conn| {
                let created_file = conn.create_workspace_file(file_record).await?;
                conn.emit_event(
                    EventOrigin {
                        workspace_id: workspace.id,
                        account_id: auth_claims.account_id,
                        security: &security,
                    },
                    WorkspaceEvent::FileCreated {
                        file: FileRef {
                            file_id: created_file.id,
                            file_name: created_file.display_name.clone(),
                        },
                        file_size_bytes: created_file.file_size_bytes,
                    },
                )
                .await?;
                Ok::<_, Error>(created_file)
            })
            .await
        {
            Ok(created_file) => created_file,
            Err(err) => {
                // The object was streamed to storage before this transaction, so a
                // rollback leaves it with no row and nothing can reclaim it later
                // (the reaper works from file rows). Remove it best-effort before
                // surfacing the error.
                if let Err(cleanup) = ctx.file_store.delete(&file_key).await {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        error = %cleanup,
                        object_id = %file_key.object_id,
                        "Failed to remove orphaned object after rolled-back file insert",
                    );
                }
                return Err(err);
            }
        };

        uploaded_files.push(response::File::from_model(
            created_file,
            workspace.slug.clone(),
            uploaded_by.clone(),
        ));
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
        .description("Uploads one or more files to a workspace. Each file is encrypted, streamed to storage, and recorded.")
        .response::<201, Json<Files>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<413, Json<ErrorResponse>>()
}

/// Gets file metadata without downloading the content.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn read_file(
    State(pg_client): State<PgClient>,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WorkspaceFilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<File>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading file metadata");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewFiles)
        .await?;

    let found = find_file_with_creator(&mut conn, workspace.id, path_params.file_id).await?;

    tracing::debug!(target: TRACING_TARGET, "File metadata retrieved");

    Ok((
        StatusCode::OK,
        Json(File::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
        )),
    ))
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
        workspace_id = %workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn update_file(
    State(pg_client): State<PgClient>,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WorkspaceFilePathParams>,
    AuthState(auth_claims): AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateFile>,
) -> Result<(StatusCode, Json<File>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating file");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::UpdateFiles)
        .await?;

    // Confirm the file exists in this workspace before mutating.
    find_file(&mut conn, workspace.id, path_params.file_id).await?;

    let updates = request.into_model();

    // Update the file and record its update event in one transaction, so the
    // event is never lost, nor recorded for an update that rolled back.
    conn.transaction(async |conn| {
        let updated_file = conn
            .update_workspace_file(path_params.file_id, updates)
            .await
            .map_err(|err| {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to update file");
                ErrorKind::InternalServerError.with_message("Failed to update file")
            })?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_claims.account_id,
                security: &security,
            },
            WorkspaceEvent::FileUpdated(FileRef {
                file_id: path_params.file_id,
                file_name: updated_file.display_name.clone(),
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    let found = find_file_with_creator(&mut conn, workspace.id, path_params.file_id).await?;
    let updated_file = found.item;
    let uploaded_by = found.account;

    tracing::info!(target: TRACING_TARGET, "File updated");

    Ok((
        StatusCode::OK,
        Json(response::File::from_model(
            updated_file,
            workspace.slug,
            uploaded_by.into(),
        )),
    ))
}

fn update_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update file")
        .description("Updates file metadata such as display name, tags, or metadata.")
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
        workspace_id = %workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn download_file(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(crypto): State<CryptoService>,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WorkspaceFilePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading file");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::DownloadFiles)
        .await?;

    let file = find_file(&mut conn, workspace.id, path_params.file_id).await?;

    let file_store = nats_client
        .object_store::<FilesBucket>()
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                "Failed to create file store"
            );
            ErrorKind::InternalServerError.with_message("Failed to initialize file storage")
        })?;

    let file_key = FileKey::from_str(&file.storage_path).map_err(|err| {
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

    // Get streaming content from NATS file store
    let get_result = file_store
        .get(&file_key)
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

    // The display name is user-controlled, so strip characters that are invalid
    // in a quoted header value to avoid header injection and a failed parse
    // before it goes into the (server-trusting) attachment header.
    let safe_name: String = file
        .display_name
        .chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
        .collect();
    // Content-length is the plaintext size from the record; storage holds the
    // larger ciphertext, which the decrypting reader unwraps as it streams.
    let headers = attachment_headers(
        &safe_name,
        HeaderValue::from_static("application/octet-stream"),
        file.file_size_bytes as u64,
    );

    tracing::debug!(
        target: TRACING_TARGET,
        file_id = %path_params.file_id,
        size = file.file_size_bytes,
        "Streaming file download"
    );

    // Decrypt the stored ciphertext as it streams to the client.
    let decrypted = crypto.decrypt_reader(file.workspace_id, get_result.into_reader());
    let stream = ReaderStream::new(Box::pin(decrypted));
    let body = Body::from_stream(stream);

    Ok((StatusCode::OK, headers, body))
}

fn download_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download file")
        .description("Downloads a file by ID. Returns the file content as a binary stream.")
        .download_response("The file content.", &["application/octet-stream"])
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a file (soft delete).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn delete_file(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WorkspaceFilePathParams>,
    AuthState(auth_claims): AuthState,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting file");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::DeleteFiles)
        .await?;

    // Confirm the file exists in this workspace before deleting.
    let file = find_file(&mut conn, workspace.id, path_params.file_id).await?;

    // Soft-delete the row and record the deletion event in one transaction, so
    // the event is never lost, nor recorded for a delete that rolled back.
    conn.transaction(async |conn| {
        conn.delete_workspace_file(file.id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_claims.account_id,
                security: &security,
            },
            WorkspaceEvent::FileDeleted(FileRef {
                file_id: path_params.file_id,
                file_name: file.display_name.clone(),
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    // Purge the object, reclaiming storage the same way retention expiry does.
    // The soft-delete already committed above; `purge_file` re-runs it
    // idempotently, then removes the object. Runs keep their reference to the file
    // as history; a reader resolves it to "gone". The purge outcome is not
    // surfaced to the caller: a pending object purge is the reaper's to retry.
    let _ = blob
        .purge_file(&mut conn, file.id, &file.storage_path, &file.storage_bucket)
        .await?;

    tracing::info!(target: TRACING_TARGET, "File deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_file_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete file")
        .description("Deletes a file: the record is retired and its stored content is removed. This is permanent — the file's content cannot be recovered.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes several files in one call (soft delete).
///
/// Idempotent: each requested id that resolves to a live file in the workspace
/// is deleted; ids that are unknown, already deleted, or in another workspace are
/// reported as skipped rather than failing the request.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        workspace_id = %workspace.id,
        requested = request.file_ids.len(),
    )
)]
async fn bulk_delete_files(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    WorkspaceContext(workspace): WorkspaceContext,
    AuthState(auth_claims): AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<DeleteFiles>,
) -> Result<(StatusCode, Json<response::DeletedFiles>)> {
    tracing::debug!(target: TRACING_TARGET, "Bulk-deleting files");

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_workspace(&mut conn, workspace.id, Permission::DeleteFiles)
        .await?;

    // De-duplicate the requested ids.
    let requested: BTreeSet<Uuid> = request.file_ids.into_iter().collect();
    let requested: Vec<Uuid> = requested.into_iter().collect();

    // Atomically soft-delete the live files among them and record a deletion event
    // for each in one transaction: the delete resolves and transitions the rows in
    // a single guarded statement (so a row a concurrent request already deleted is
    // never double-reported), and the events commit with it — none lost, none
    // recorded for a delete that rolled back. `files` holds exactly the rows this
    // request deleted.
    let files = conn
        .transaction(async |conn| {
            let files = conn
                .delete_files_in_workspace(workspace.id, &requested)
                .await?;
            for file in &files {
                conn.emit_event(
                    EventOrigin {
                        workspace_id: workspace.id,
                        account_id: auth_claims.account_id,
                        security: &security,
                    },
                    WorkspaceEvent::FileDeleted(FileRef {
                        file_id: file.id,
                        file_name: file.display_name.clone(),
                    }),
                )
                .await?;
            }
            Ok::<_, Error>(files)
        })
        .await?;

    // Whatever was not deleted is skipped: unknown, already deleted, or another
    // workspace's — the delete is idempotent.
    let deleted_ids: BTreeSet<Uuid> = files.iter().map(|file| file.id).collect();
    let skipped: Vec<Uuid> = requested
        .into_iter()
        .filter(|id| !deleted_ids.contains(id))
        .collect();

    // Purge each object, reclaiming storage the same way retention expiry does.
    // The soft-deletes already committed above; `purge_file` re-runs each
    // idempotently, then removes the object. The deletion is the committed result,
    // so a purge failure must not fail the response (that would make a retry see
    // these ids as already-gone `skipped`): log it and leave the object for the
    // reaper to reclaim.
    for file in &files {
        if let Err(err) = blob
            .purge_file(&mut conn, file.id, &file.storage_path, &file.storage_bucket)
            .await
        {
            tracing::error!(
                target: TRACING_TARGET,
                file_id = %file.id,
                error = %err,
                "Failed to purge a bulk-deleted file's object; left for the reaper to retry",
            );
        }
    }

    let deleted: Vec<Uuid> = deleted_ids.into_iter().collect();
    tracing::info!(
        target: TRACING_TARGET,
        deleted = deleted.len(),
        skipped = skipped.len(),
        "Files bulk-deleted",
    );

    Ok((
        StatusCode::OK,
        Json(response::DeletedFiles { deleted, skipped }),
    ))
}

fn bulk_delete_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete files")
        .description("Deletes several files in one call. Idempotent: ids that resolve to live files in the workspace are removed and returned in `deleted`; ids that are unknown, already deleted, or in another workspace are returned in `skipped`. Deletion is permanent — the files' content cannot be recovered.")
        .response::<200, Json<response::DeletedFiles>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(max_file_body_bytes: usize) -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspaceSlug}/files/",
            post_with(upload_file, upload_file_docs)
                // Raise this route's default body limit to the upload ceiling; the
                // global `RequestBodyLimitLayer` still caps every route at the same
                // hard limit.
                .layer(DefaultBodyLimit::max(max_file_body_bytes))
                .get_with(list_files, list_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/files/delete/",
            post_with(bulk_delete_files, bulk_delete_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/files/{fileId}/",
            get_with(read_file, read_file_docs)
                .patch_with(update_file, update_file_docs)
                .delete_with(delete_file, delete_file_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/files/{fileId}/content/",
            get_with(download_file, download_file_docs),
        )
        .with_path_items(|item| item.tag("Files"))
}
