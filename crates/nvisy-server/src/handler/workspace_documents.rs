//! Workspace document upload and management handlers.
//!
//! This module provides comprehensive document management functionality for
//! workspaces, including upload, download, metadata management, and document
//! operations. All operations are secured with workspace-level authorization.

use std::collections::BTreeSet;
use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::multipart::Field;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use futures::StreamExt;
use nvisy_postgres::model::{NewBlob, NewWorkspaceDocument, WorkspaceDocument as DocumentModel};
use nvisy_postgres::query::{
    DocumentWithBlob, WorkspaceBlobRepository, WorkspaceDocumentRepository,
};
use nvisy_postgres::types::{DocumentKind, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use nvisy_s3::{BlobStore, Bucket, DocumentKey};
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;

use crate::extract::{
    AuthState, Authorized, Json, Multipart, Path, Permission, Query, SecurityContext, ValidateJson,
    WorkspaceContext, markers,
};
use crate::handler::request::{
    CursorPagination, DeleteDocuments as DeleteDocumentsRequest, ListDocuments, UpdateDocument,
    WorkspaceDocumentPathParams,
};
use crate::handler::response::{self, Document, Documents, DocumentsPage};
use crate::handler::utility::{DownloadDocs, resolve_account_ref};
use crate::middleware::UploadConfig;
use crate::response::{Error, ErrorKind, ErrorResponse, Result, attachment_headers};
use crate::service::{
    CryptoService, DocumentCreated, DocumentDeleted, DocumentUpdated, EngineService, EventEmitter,
    EventOrigin, HashingReader, LimitedReader, ServiceState, WorkspaceEvent,
};

/// Tracing target for workspace document operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_documents";

/// Finds a document within a workspace or returns NotFound error.
async fn find_document(
    conn: &mut PgConn,
    workspace_id: Uuid,
    document_id: Uuid,
) -> Result<DocumentModel> {
    conn.find_document_in_workspace(workspace_id, document_id)
        .await?
        .ok_or_else(|| Error::not_found("document"))
}

/// Finds a document within a workspace, with its uploader's identity, or returns a
/// NotFound error.
async fn find_document_with_creator(
    conn: &mut PgConn,
    workspace_id: Uuid,
    document_id: Uuid,
) -> Result<WithAccountRef<DocumentWithBlob>> {
    conn.find_document_in_workspace_with_creator(workspace_id, document_id)
        .await?
        .ok_or_else(|| Error::not_found("document"))
}

/// Lists documents in a workspace with cursor-based pagination.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_documents(
    State(pg_client): State<PgClient>,
    State(engine): State<EngineService>,
    authz: Authorized<markers::ViewDocuments>,
    Query(documents_query): Query<ListDocuments>,
    Query(cursor_pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<DocumentsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing documents");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let filter = documents_query.to_filter(&engine).map_err(|err| {
        ErrorKind::BadRequest
            .with_message("Unknown document format filter")
            .with_context(err.to_string())
    })?;

    let page = conn
        .cursor_list_workspace_documents(workspace.id, cursor_pagination.into_cursor(), filter)
        .await?;

    let response = DocumentsPage::from_cursor_page(page, |wc| {
        Document::from_model(
            wc.item.document,
            &wc.item.blob,
            workspace.slug.clone(),
            wc.account.into(),
        )
    });

    tracing::debug!(
        target: TRACING_TARGET,
        document_count = response.items.len(),
        has_more = response.next_cursor.is_some(),
        "Documents listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_documents_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List documents")
        .description(
            "Lists documents in a workspace with cursor-based pagination. Use the `after` parameter with the `nextCursor` value from the response to fetch subsequent pages. Pass `hash` (a hex SHA-256) to find documents with identical content — a non-empty result means the document already exists, so an upload can be skipped.",
        )
        .response::<200, Json<DocumentsPage>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Context for processing a single document upload.
#[derive(Clone)]
struct DocumentUploadContext {
    workspace_id: Uuid,
    account_id: Uuid,
    blobs: BlobStore,
    crypto: CryptoService,
    /// Engine handle, used to reject upload of a format no codec can decode.
    engine: EngineService,
    /// Retention expiry for uploaded originals (`None` = keep indefinitely).
    expires_at: Option<jiff::Timestamp>,
    /// The effective per-document upload cap in bytes — the smaller of the workspace's
    /// soft cap and the server-wide hard limit. A document streaming past it is
    /// rejected before its excess reaches storage. This is a true per-document bound,
    /// unlike the request-body layer, which limits the whole multipart request.
    max_upload_bytes: u64,
}

/// A document streamed to object storage whose document/blob rows have not yet been
/// inserted.
///
/// Pairs the object's storage key with its unsaved blob and document so the batch
/// can persist every row together, and reclaim every staged object if that fails.
struct StagedDocument {
    key: DocumentKey,
    blob: NewBlob,
    document: NewWorkspaceDocument,
}

/// Streams one multipart document to storage and builds its unsaved rows.
async fn stage_document(ctx: &DocumentUploadContext, field: Field<'_>) -> Result<StagedDocument> {
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
    // unprocessable document would only fail later at detection, after wasting an
    // encrypted upload.
    if !ctx.engine.supports_extension(&file_extension) {
        return Err(ErrorKind::BadRequest
            .with_message(format!("Unsupported document format: .{file_extension}"))
            .with_context(
                "The redaction engine has no codec for this document type; upload a supported format.",
            ));
    }

    // Generate document key with a unique object ID for blob storage.
    let document_key = DocumentKey::generate(ctx.workspace_id);

    tracing::debug!(
        target: TRACING_TARGET,
        object_id = %document_key.object_id,
        "Streaming document to storage"
    );

    // Step 1: Encrypt the plaintext as it streams to the blob store. The limited
    // reader aborts an oversized upload before its excess is encrypted and stored,
    // enforcing the effective per-document cap directly (the request-body layer only
    // bounds the whole multipart request). The measured reader captures the
    // plaintext size and hash (the store only sees ciphertext).
    let cap = ctx.max_upload_bytes;
    let source = StreamReader::new(field.map(|result| result.map_err(std::io::Error::other)));
    let (limited, limit_state) = LimitedReader::new(source, cap);
    let (measured, measurements) = HashingReader::new(limited);
    let encrypted = ctx.crypto.encrypt_reader(ctx.workspace_id, measured);

    if let Err(err) = ctx.blobs.put(&document_key, Box::pin(encrypted)).await {
        // The limited reader aborts the stream, which fails the `put`. When that
        // is why it failed, report the size limit (413) rather than a storage
        // error; the reader's error is stringified in transit, so consult the
        // shared state instead of inspecting the error.
        if limit_state.is_exceeded() {
            return Err(ErrorKind::PayloadTooLarge
                .with_message(format!("Document exceeds the {cap}-byte upload limit")));
        }
        return Err(err.into());
    }

    tracing::debug!(
        target: TRACING_TARGET,
        object_id = %document_key.object_id,
        size = measurements.bytes(),
        "Document encrypted and streamed to storage"
    );

    // Step 2: Build the unsaved blob (content-addressed bytes + retention) and the
    // unsaved document that will point at it (Postgres generates each row's own id
    // on insert; the blob is resolved — shared or created — at commit time). The
    // content hash and size live on the blob; the human-facing name, extension, and
    // creator live on the document.
    let blob = NewBlob {
        workspace_id: ctx.workspace_id,
        content_hash: measurements.sha256().to_vec(),
        file_size_bytes: measurements.bytes() as i64,
        storage_path: document_key.to_string(),
        storage_bucket: Bucket::Documents.name().to_owned(),
        expires_at: ctx.expires_at.map(Into::into),
    };
    let document = NewWorkspaceDocument {
        workspace_id: ctx.workspace_id,
        account_id: ctx.account_id,
        // Resolved to the staged blob's id at commit time.
        blob_id: Uuid::nil(),
        kind: Some(DocumentKind::Original),
        display_name: Some(filename.clone()),
        original_filename: Some(filename),
        file_extension: Some(file_extension),
        metadata: None,
    };

    Ok(StagedDocument {
        key: document_key,
        blob,
        document,
    })
}

impl DocumentUploadContext {
    /// Streams every document field in the multipart body to storage, returning the
    /// staged documents (object key + unsaved row). Non-document fields are skipped.
    ///
    /// On any error, the objects staged so far are removed before returning, so a
    /// failed batch never leaves an object behind with no row to reclaim it.
    async fn stage_all(&self, multipart: &mut Multipart) -> Result<Vec<StagedDocument>> {
        let mut staged: Vec<StagedDocument> = Vec::new();
        loop {
            let field = match multipart.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => break,
                Err(err) => {
                    self.discard_staged(&staged).await;
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read multipart field");
                    return Err(ErrorKind::BadRequest
                        .with_message("Invalid multipart data")
                        .with_context(format!("Failed to parse multipart form: {err}")));
                }
            };

            if field.file_name().is_none() {
                tracing::debug!(
                    target: TRACING_TARGET,
                    name = ?field.name(),
                    "Skipping non-document multipart field"
                );
                continue;
            }

            match stage_document(self, field).await {
                Ok(document) => staged.push(document),
                Err(err) => {
                    self.discard_staged(&staged).await;
                    return Err(err);
                }
            }
        }
        Ok(staged)
    }

    /// Removes staged objects best-effort, for when the batch does not commit.
    /// Each object was written before any row exists, so nothing else can reclaim
    /// it; a failed removal is logged and left for no one — an acceptable leak in
    /// the rare storage-error case, not worth failing the response over.
    async fn discard_staged(&self, staged: &[StagedDocument]) {
        for document in staged {
            self.discard_staged_object(&document.key).await;
        }
    }

    /// Removes one staged object best-effort, logging a failure rather than
    /// failing the response.
    async fn discard_staged_object(&self, key: &DocumentKey) {
        if let Err(err) = self.blobs.delete(key).await {
            tracing::warn!(
                target: TRACING_TARGET,
                error = %err,
                object_id = %key.object_id,
                "Failed to remove staged object",
            );
        }
    }
}

/// Uploads input documents to a workspace for processing.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn upload_document(
    State(pg_client): State<PgClient>,
    State(blobs): State<BlobStore>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    State(upload): State<UploadConfig>,
    authz: Authorized<markers::UploadDocuments>,
    security: SecurityContext,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Documents>)> {
    tracing::info!(target: TRACING_TARGET, "Uploading documents");

    let workspace = authz.workspace;

    // Do the quick pre-flight DB work under a connection, then release it:
    // resolve the uploader's identity, and read the workspace's upload settings.
    // Holding a pooled connection across the streaming below would pin it for the
    // whole upload and starve the pool under load, so this scope drops it before
    // streaming begins.
    let (uploaded_by, expires_at, max_upload_bytes) = {
        let mut conn = pg_client.get_connection().await?;

        let uploaded_by = resolve_account_ref(&mut conn, authz.account_id).await?;

        let settings = workspace.settings.or_default();
        let expires_at = settings
            .retention
            .original_documents
            .expires_at(jiff::Timestamp::now());
        let max_upload_bytes = settings.effective_max_upload_bytes(upload.max_file_bytes());

        (uploaded_by, expires_at, max_upload_bytes)
    };

    let ctx = DocumentUploadContext {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        blobs,
        crypto,
        engine,
        expires_at,
        max_upload_bytes,
    };

    // Stream every document to storage first (no DB connection held), then persist all
    // their rows and events in one transaction. The upload is atomic: it either
    // records every document or, on any failure, records none and reclaims every
    // staged object — never a partial batch, and never an object left behind with
    // no row.
    let staged = ctx.stage_all(&mut multipart).await?;

    if staged.is_empty() {
        return Err(
            ErrorKind::BadRequest.with_message("No documents provided in multipart request")
        );
    }

    // Re-acquire a connection only for the final commit, so the pool is free
    // during the streaming above.
    let mut conn = pg_client.get_connection().await?;
    let origin = EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };
    let created = match conn
        .transaction(async |conn| {
            let mut created: Vec<DocumentWithBlob> = Vec::with_capacity(staged.len());
            for staged_document in &staged {
                let document = conn
                    .create_workspace_document(
                        staged_document.document.clone(),
                        staged_document.blob.clone(),
                    )
                    .await?;
                // Resolve the blob the document was pointed at (shared or freshly
                // created), for the response's content-addressed fields.
                let blob = conn
                    .find_blob_by_id(document.blob_id)
                    .await?
                    .ok_or_else(|| {
                        ErrorKind::InternalServerError.with_message("Staged blob not found")
                    })?;
                conn.emit_event(
                    origin,
                    WorkspaceEvent::DocumentCreated(DocumentCreated {
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                        document_size_bytes: blob.file_size_bytes,
                    }),
                )
                .await?;
                created.push(DocumentWithBlob { document, blob });
            }
            Ok::<_, Error>(created)
        })
        .await
    {
        Ok(created) => created,
        Err(err) => {
            // The objects were streamed before this transaction, so a rollback
            // leaves them with no rows and nothing to reclaim them later (the
            // reaper works from blob rows). Remove them best-effort first.
            ctx.discard_staged(&staged).await;
            return Err(err);
        }
    };

    // A document whose content deduplicated onto an existing blob is pointed at
    // that blob's stored object, orphaning the object staged for it here. Remove
    // the redundant staged object best-effort now that the batch has committed.
    for (staged_document, entry) in staged.iter().zip(&created) {
        if entry.blob.storage_path != staged_document.blob.storage_path {
            ctx.discard_staged_object(&staged_document.key).await;
        }
    }

    let mut uploaded_documents: Documents = Vec::with_capacity(created.len());
    for entry in created {
        uploaded_documents.push(response::Document::from_model(
            entry.document,
            &entry.blob,
            workspace.slug.clone(),
            uploaded_by.clone(),
        ));
    }

    tracing::info!(
        target: TRACING_TARGET,
        document_count = uploaded_documents.len(),
        "Documents uploaded",
    );

    Ok((StatusCode::CREATED, Json(uploaded_documents)))
}

fn upload_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Upload documents")
        .description("Uploads one or more documents to a workspace. Each document is encrypted and streamed to storage. The batch is atomic: either every document is recorded, or on any failure none are and the request fails.")
        .response::<201, Json<Documents>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<413, Json<ErrorResponse>>()
}

/// Gets document metadata without downloading the content.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn read_document(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading document metadata");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let found =
        find_document_with_creator(&mut conn, workspace.id, path_params.document_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Document metadata retrieved");

    Ok((
        StatusCode::OK,
        Json(Document::from_model(
            found.item.document,
            &found.item.blob,
            workspace.slug,
            found.account.into(),
        )),
    ))
}

fn read_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get document metadata")
        .description("Returns document metadata without downloading the document content.")
        .response::<200, Json<Document>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates document metadata.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn update_document(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::UpdateDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateDocument>,
) -> Result<(StatusCode, Json<Document>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating document");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Confirm the document exists in this workspace before mutating.
    find_document(&mut conn, workspace.id, path_params.document_id).await?;

    let updates = request.into_model();

    // Update the document and record its update event in one transaction, so the
    // event is never lost, nor recorded for an update that rolled back.
    conn.transaction(async |conn| {
        let updated_document = conn
            .update_workspace_document(path_params.document_id, updates)
            .await
            .map_err(|err| {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to update document");
                ErrorKind::InternalServerError.with_message("Failed to update document")
            })?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            WorkspaceEvent::DocumentUpdated(DocumentUpdated {
                document_id: path_params.document_id,
                document_name: updated_document.display_name.clone(),
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    let found =
        find_document_with_creator(&mut conn, workspace.id, path_params.document_id).await?;
    let uploaded_by = found.account;

    tracing::info!(target: TRACING_TARGET, "Document updated");

    Ok((
        StatusCode::OK,
        Json(response::Document::from_model(
            found.item.document,
            &found.item.blob,
            workspace.slug,
            uploaded_by.into(),
        )),
    ))
}

fn update_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update document")
        .description("Updates document metadata such as display name, tags, or metadata.")
        .response::<200, Json<Document>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Downloads a document with streaming support for large documents.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth.account_id,
        workspace_id = %workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn download_document(
    State(pg_client): State<PgClient>,
    State(blobs): State<BlobStore>,
    State(crypto): State<CryptoService>,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    auth: AuthState,
) -> Result<(StatusCode, HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading document");

    let mut conn = pg_client.get_connection().await?;

    // Gate on workspace document access before resolving the document, so a caller who
    // cannot see documents cannot distinguish a missing document from a forbidden one.
    // Every kind-specific download permission below already requires at least the
    // role this check does, so it never rejects an otherwise-authorized caller.
    auth.authorize_workspace(&mut conn, workspace.id, Permission::ViewDocuments)
        .await?;

    // The permission a download requires depends on the document's kind, so the
    // raw original bytes and the redacted output are gated separately. Audit and
    // intermediate bytes are not documents; they download through the
    // detection-audit endpoints under DownloadAudit.
    let document = find_document(&mut conn, workspace.id, path_params.document_id).await?;
    let blob = conn
        .find_blob_by_id(document.blob_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_message("Document content not found"))?;

    let permission = match document.kind {
        DocumentKind::Original => Permission::DownloadOriginalDocuments,
        DocumentKind::Redacted => Permission::DownloadRedactedDocuments,
    };

    auth.authorize_workspace(&mut conn, workspace.id, permission)
        .await?;

    let document_key = DocumentKey::from_str(&blob.storage_path).map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            storage_path = %blob.storage_path,
            "Invalid storage path format"
        );
        ErrorKind::InternalServerError
            .with_message("Invalid document storage path")
            .with_context(format!("Parse error: {}", err))
    })?;

    // Get streaming content from the blob store.
    let get_result = blobs
        .get(&document_key)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                document_id = %path_params.document_id,
                "Failed to retrieve document from storage"
            );
            ErrorKind::InternalServerError
                .with_message("Failed to retrieve document")
                .with_context(format!("Storage retrieval failed: {}", err))
        })?
        .ok_or_else(|| {
            tracing::warn!(
                target: TRACING_TARGET,
                document_id = %path_params.document_id,
                "Document content not found in storage"
            );
            ErrorKind::NotFound.with_message("Document content not found")
        })?;

    // `attachment_headers` handles the user-controlled name safely (escapes it,
    // and carries a non-ASCII name via RFC 6266 `filename*`), so it is passed
    // through as-is. Content-length is the plaintext size from the record;
    // storage holds the larger ciphertext, which the decrypting reader unwraps as
    // it streams.
    let headers = attachment_headers(
        &document.display_name,
        HeaderValue::from_static("application/octet-stream"),
        blob.file_size_bytes as u64,
    );

    tracing::debug!(
        target: TRACING_TARGET,
        document_id = %path_params.document_id,
        size = blob.file_size_bytes,
        "Streaming document download"
    );

    // Decrypt the stored ciphertext as it streams to the client.
    let decrypted = crypto.decrypt_reader(document.workspace_id, get_result.into_reader());
    let stream = ReaderStream::new(Box::pin(decrypted));
    let body = Body::from_stream(stream);

    Ok((StatusCode::OK, headers, body))
}

fn download_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download document")
        .description(
            "Downloads a document by ID. Returns the document content as a binary stream. The required \
             permission depends on the document's kind: an original document needs \
             DownloadOriginalDocuments, and a redacted output needs DownloadRedactedDocuments.",
        )
        .download_response("The document content.", &["application/octet-stream"])
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a document (soft delete).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn delete_document(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::DeleteDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting document");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Confirm the document exists in this workspace before deleting.
    let document = find_document(&mut conn, workspace.id, path_params.document_id).await?;

    // Soft-delete the document (which drops its blob reference) and record the
    // deletion event in one transaction, so the event is never lost, nor recorded
    // for a delete that rolled back. The backing object is not removed here: its
    // bytes may be shared with another document, so the reaper reclaims the blob
    // once its last reference is gone and its retention window has passed.
    conn.transaction(async |conn| {
        conn.delete_workspace_document(document.id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            WorkspaceEvent::DocumentDeleted(DocumentDeleted {
                document_id: path_params.document_id,
                document_name: document.display_name.clone(),
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "Document deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete document")
        .description("Deletes a document: the record is retired and its blob reference dropped. The stored content is reclaimed later, once no other document references it and its retention window has passed. This is permanent — the document cannot be restored.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes several documents in one call (soft delete).
///
/// Idempotent: each requested id that resolves to a live document in the workspace
/// is deleted; ids that are unknown, already deleted, or in another workspace are
/// reported as skipped rather than failing the request.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        requested = request.document_ids.len(),
    )
)]
async fn bulk_delete_documents(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::DeleteDocuments>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<DeleteDocumentsRequest>,
) -> Result<(StatusCode, Json<response::DeletedDocuments>)> {
    tracing::debug!(target: TRACING_TARGET, "Bulk-deleting documents");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // De-duplicate the requested ids.
    let requested: BTreeSet<Uuid> = request.document_ids.into_iter().collect();
    let requested: Vec<Uuid> = requested.into_iter().collect();

    // Atomically soft-delete the live documents among them and record a deletion event
    // for each in one transaction: the delete resolves and transitions the rows in
    // a single guarded statement (so a row a concurrent request already deleted is
    // never double-reported), and the events commit with it — none lost, none
    // recorded for a delete that rolled back. `documents` holds exactly the rows
    // this request deleted.
    let documents = conn
        .transaction(async |conn| {
            let documents = conn
                .delete_documents_in_workspace(workspace.id, &requested)
                .await?;
            for document in &documents {
                conn.emit_event(
                    EventOrigin {
                        workspace_id: workspace.id,
                        account_id: authz.account_id,
                        security: &security,
                    },
                    WorkspaceEvent::DocumentDeleted(DocumentDeleted {
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    }),
                )
                .await?;
            }
            Ok::<_, Error>(documents)
        })
        .await?;

    // Whatever was not deleted is skipped: unknown, already deleted, another
    // workspace's, or held by an in-progress detection — the delete is idempotent.
    let deleted_ids: BTreeSet<Uuid> = documents.iter().map(|document| document.id).collect();
    let skipped: Vec<Uuid> = requested
        .into_iter()
        .filter(|id| !deleted_ids.contains(id))
        .collect();

    // The backing objects are not removed here: each deleted document dropped its
    // blob reference in the transaction above, and the reaper reclaims a blob once
    // its last reference is gone and its retention window has passed. This keeps
    // deletion cheap and correct when bytes are shared between documents.

    let deleted: Vec<Uuid> = deleted_ids.into_iter().collect();
    tracing::info!(
        target: TRACING_TARGET,
        deleted = deleted.len(),
        skipped = skipped.len(),
        "Documents bulk-deleted",
    );

    Ok((
        StatusCode::OK,
        Json(response::DeletedDocuments { deleted, skipped }),
    ))
}

fn bulk_delete_documents_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete documents")
        .description("Deletes several documents in one call. Idempotent: ids that resolve to live documents in the workspace are removed and returned in `deleted`; ids that are unknown, already deleted, or in another workspace are returned in `skipped`. Deletion is permanent — the documents' content cannot be recovered.")
        .response::<200, Json<response::DeletedDocuments>>()
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
            "/workspaces/{workspaceSlug}/documents/",
            post_with(upload_document, upload_document_docs)
                // Raise this route's default body limit to the upload ceiling; the
                // global `RequestBodyLimitLayer` still caps every route at the same
                // hard limit.
                .layer(DefaultBodyLimit::max(max_file_body_bytes))
                .get_with(list_documents, list_documents_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/documents/delete/",
            post_with(bulk_delete_documents, bulk_delete_documents_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/documents/{documentId}/",
            get_with(read_document, read_document_docs)
                .patch_with(update_document, update_document_docs)
                .delete_with(delete_document, delete_document_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/documents/{documentId}/content/",
            get_with(download_document, download_document_docs),
        )
        .with_path_items(|item| item.tag("Documents"))
}
