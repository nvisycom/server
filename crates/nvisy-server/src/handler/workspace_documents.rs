//! Workspace document upload and management handlers.
//!
//! Metadata operations (list, read, update, delete) delegate to
//! [`WorkspaceDocumentService`]; the byte-I/O actions — upload (multipart
//! streaming to storage) and download (streaming decrypt to the client) — stay
//! here, loading a document through the service. All operations are secured with
//! workspace-level authorization.

// Axum handlers take their dependencies as typed extractor arguments, so a
// dependency-heavy handler exceeds the arg limit by construction; the signature
// is fixed by the framework and cannot be bundled into a struct.
#![allow(clippy::too_many_arguments)]

use std::io;
use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use futures::StreamExt;
use nvisy_postgres::types::DocumentKind;
use nvisy_s3::{BlobStore, DocumentKey};
use tokio_util::io::ReaderStream;

use crate::domain;
use crate::extract::{Authorized, Json, Multipart, Path, Permission, Query, ValidateJson, markers};
use crate::handler::request::{
    CursorPagination, DeleteWorkspaceDocuments as DeleteDocumentsRequest, ListWorkspaceDocuments,
    UpdateWorkspaceDocument, WorkspaceDocumentPathParams,
};
use crate::handler::response::{
    self, WorkspaceDocument, WorkspaceDocumentDetails, WorkspaceDocuments, WorkspaceDocumentsPage,
    WorkspaceRef,
};
use crate::handler::utility::DownloadDocs;
use crate::middleware::UploadConfig;
use crate::response::{ErrorKind, ErrorResponse, Result, attachment_headers};
use crate::service::{CryptoService, DocumentStager, ServiceState, StagedDocument, UploadTarget};

/// Tracing target for workspace document operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_documents";

/// Lists documents in a workspace with cursor-based pagination.
#[tracing::instrument(skip_all)]
async fn list_documents(
    State(documents): State<domain::WorkspaceDocumentService>,
    authz: Authorized<markers::ViewDocuments>,
    Query(documents_query): Query<ListWorkspaceDocuments>,
    Query(cursor_pagination): Query<CursorPagination>,
) -> Result<Json<WorkspaceDocumentsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing documents");

    let workspace = &authz.workspace;
    let page = documents
        .list(
            workspace.id,
            cursor_pagination.into_cursor(),
            documents_query.into(),
        )
        .await?;

    let response = WorkspaceDocumentsPage::from_cursor_page(page, |wc| {
        WorkspaceDocument::from_model(
            wc.item.document,
            &wc.item.blob,
            WorkspaceRef::from(workspace),
            wc.account.into(),
        )
    });

    tracing::debug!(
        target: TRACING_TARGET,
        document_count = response.items.len(),
        has_more = response.next_cursor.is_some(),
        "Documents listed"
    );

    Ok(Json(response))
}

fn list_documents_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List documents")
        .description(
            "Lists documents in a workspace with cursor-based pagination. Use the `after` parameter with the `nextCursor` value from the response to fetch subsequent pages. Use `kind=original` or `kind=redacted` to filter by role, and `search` to find a display name or original filename. Pass `hash` (a hex SHA-256) to find documents with identical content — a non-empty result means the document already exists, so an upload can be skipped.",
        )
        .response::<200, Json<WorkspaceDocumentsPage>>()
}

/// Streams every document field of the multipart body to storage. Non-document
/// fields are skipped. On any error the objects staged so far are reclaimed, so a
/// failed batch never leaves an object with no row to reclaim it.
async fn stage_multipart(
    stager: &DocumentStager,
    target: &UploadTarget,
    multipart: &mut Multipart,
) -> Result<Vec<StagedDocument>> {
    let mut uploads = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(err) => {
                stager.discard(&uploads).await;
                return Err(ErrorKind::BadRequest
                    .with_message("Invalid multipart data")
                    .with_context(format!("Failed to parse multipart form: {err}")));
            }
        };

        let Some(filename) = field.file_name().map(ToOwned::to_owned) else {
            tracing::debug!(
                target: TRACING_TARGET,
                name = ?field.name(),
                "Skipping non-document multipart field"
            );
            continue;
        };

        let body = field.map(|chunk| chunk.map_err(io::Error::other));
        match stager.stage(target, &filename, body).await {
            Ok(document) => uploads.push(document),
            Err(err) => {
                stager.discard(&uploads).await;
                return Err(err);
            }
        }
    }
    Ok(uploads)
}

/// Uploads input documents to a workspace for processing.
#[tracing::instrument(skip_all)]
async fn upload_document(
    State(documents): State<domain::WorkspaceDocumentService>,
    State(stager): State<DocumentStager>,
    State(upload): State<UploadConfig>,
    authz: Authorized<markers::UploadDocuments>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<WorkspaceDocuments>)> {
    tracing::info!(target: TRACING_TARGET, "Uploading documents");

    // Stream every document to storage with no connection held, then record
    // them all in one transaction: the batch records every document or none.
    let target = UploadTarget::new(&authz.workspace, authz.account_id, upload.max_file_bytes());
    let uploads = stage_multipart(&stager, &target, &mut multipart).await?;
    let created = documents.commit_uploads(authz.origin(), uploads).await?;

    let uploaded_documents = created
        .into_iter()
        .map(|entry| {
            response::WorkspaceDocument::from_model(
                entry.document,
                &entry.blob,
                WorkspaceRef::from(&authz.workspace),
                authz.account.clone().into(),
            )
        })
        .collect();
    Ok((StatusCode::CREATED, Json(uploaded_documents)))
}

fn upload_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Upload documents")
        .description("Uploads one or more documents to a workspace. Each document is encrypted and streamed to storage. The batch is atomic: either every document is recorded, or on any failure none are and the request fails.")
        .response::<201, Json<WorkspaceDocuments>>()
        .response::<413, Json<ErrorResponse>>()
}

/// Gets document metadata without downloading the content.
#[tracing::instrument(
    skip_all,
    fields(
        document_id = %path_params.document_id,
    )
)]
async fn read_document(
    State(documents): State<domain::WorkspaceDocumentService>,
    authz: Authorized<markers::ViewDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
) -> Result<Json<WorkspaceDocumentDetails>> {
    tracing::debug!(target: TRACING_TARGET, "Reading document metadata");

    let workspace = &authz.workspace;
    let found = documents
        .find(workspace.id, path_params.document_id)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Document metadata retrieved");

    Ok(Json(WorkspaceDocumentDetails::from_model(
        found.item.document,
        &found.item.blob,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )))
}

fn read_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get document metadata")
        .description("Returns document metadata without downloading the document content.")
        .response::<200, Json<WorkspaceDocumentDetails>>()
}

/// Updates document metadata.
#[tracing::instrument(
    skip_all,
    fields(
        document_id = %path_params.document_id,
    )
)]
async fn update_document(
    State(documents): State<domain::WorkspaceDocumentService>,
    authz: Authorized<markers::UpdateDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspaceDocument>,
) -> Result<Json<WorkspaceDocumentDetails>> {
    tracing::debug!(target: TRACING_TARGET, "Updating document");

    let workspace = &authz.workspace;
    let origin = authz.origin();
    let found = documents
        .update(origin, path_params.document_id, request.into())
        .await?;

    tracing::info!(target: TRACING_TARGET, "Document updated");

    Ok(Json(response::WorkspaceDocumentDetails::from_model(
        found.item.document,
        &found.item.blob,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )))
}

fn update_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update document")
        .description(
            "Updates the display name or replaces custom metadata; returns the updated metadata.",
        )
        .response::<200, Json<WorkspaceDocumentDetails>>()
}

/// Downloads a document with streaming support for large documents.
#[tracing::instrument(
    skip_all,
    fields(
        document_id = %path_params.document_id,
    )
)]
async fn download_document(
    State(documents): State<domain::WorkspaceDocumentService>,
    State(blobs): State<BlobStore>,
    State(crypto): State<CryptoService>,
    // Gate on document access before resolving the document, so a caller who
    // cannot see documents cannot distinguish a missing document from a
    // forbidden one. Every kind-specific download permission below requires at
    // least this role, so it never rejects an otherwise-authorized caller.
    authz: Authorized<markers::ViewDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
) -> Result<(HeaderMap, Body)> {
    tracing::debug!(target: TRACING_TARGET, "Downloading document");
    let workspace = &authz.workspace;

    // The permission a download requires depends on the document's kind, so the
    // raw original bytes and the redacted output are gated separately. Audit and
    // intermediate bytes are not documents; they download through the
    // detection-audit endpoints under DownloadAudit.
    let found = documents
        .find(workspace.id, path_params.document_id)
        .await?;
    let document = found.item.document;
    let blob = found.item.blob;

    let permission = match document.kind {
        DocumentKind::Original => Permission::DownloadOriginalDocuments,
        DocumentKind::Redacted => Permission::DownloadRedactedDocuments,
    };

    authz.require(permission)?;

    let document_key = DocumentKey::from_str(&blob.storage_path).map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET,
            error = %err,
            storage_path = %blob.storage_path,
            "Invalid storage path format"
        );
        ErrorKind::InternalServerError
            .with_message("Invalid document storage path")
            .with_context(format!("Parse error: {err}"))
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
                .with_context(format!("Storage retrieval failed: {err}"))
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
        u64::try_from(blob.file_size_bytes).unwrap_or(0),
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

    Ok((headers, body))
}

fn download_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Download document")
        .description(
            "Downloads a document by ID. Returns the document content as a binary stream. The required \
             permission depends on the document's kind: an original document needs \
             DownloadOriginalDocuments, and a redacted output needs DownloadRedactedDocuments.",
        )
        .download_response("The document content.", &["application/octet-stream"])
        .response::<403, Json<ErrorResponse>>()
}

/// Deletes a document (soft delete).
#[tracing::instrument(
    skip_all,
    fields(
        document_id = %path_params.document_id,
    )
)]
async fn delete_document(
    State(documents): State<domain::WorkspaceDocumentService>,
    authz: Authorized<markers::DeleteDocuments>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting document");

    let origin = authz.origin();
    documents.delete(origin, path_params.document_id).await?;

    tracing::info!(target: TRACING_TARGET, "Document deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_document_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete document")
        .description("Deletes a document: the record is retired and its blob reference dropped. The stored content is reclaimed later, once no other document references it and its retention window has passed. This is permanent — the document cannot be restored.")
        .response::<204, ()>()
}

/// Deletes several documents in one call (soft delete).
///
/// Idempotent: each requested id that resolves to a live document in the workspace
/// is deleted; ids that are unknown, already deleted, or in another workspace are
/// reported as skipped rather than failing the request.
#[tracing::instrument(
    skip_all,
    fields(
        requested = request.document_ids.len(),
    )
)]
async fn bulk_delete_documents(
    State(documents): State<domain::WorkspaceDocumentService>,
    authz: Authorized<markers::DeleteDocuments>,
    ValidateJson(request): ValidateJson<DeleteDocumentsRequest>,
) -> Result<Json<response::WorkspaceDeletedDocuments>> {
    tracing::debug!(target: TRACING_TARGET, "Bulk-deleting documents");

    let origin = authz.origin();
    let outcome = documents.bulk_delete(origin, request.document_ids).await?;

    tracing::info!(
        target: TRACING_TARGET,
        deleted = outcome.deleted.len(),
        skipped = outcome.skipped.len(),
        "Documents bulk-deleted",
    );

    Ok(Json(response::WorkspaceDeletedDocuments {
        deleted: outcome.deleted,
        skipped: outcome.skipped,
    }))
}

fn bulk_delete_documents_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete documents")
        .description("Deletes several documents in one call. Idempotent: ids that resolve to live documents in the workspace are removed and returned in `deleted`; ids that are unknown, already deleted, or in another workspace are returned in `skipped`. Deletion is permanent — the documents' content cannot be recovered.")
        .response::<200, Json<response::WorkspaceDeletedDocuments>>()
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(max_file_body_bytes: usize) -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspaceId}/documents",
            post_with(upload_document, upload_document_docs)
                // Raise this route's default body limit to the upload ceiling; the
                // global `RequestBodyLimitLayer` still caps every route at the same
                // hard limit.
                .layer(DefaultBodyLimit::max(max_file_body_bytes))
                .get_with(list_documents, list_documents_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/documents/delete",
            post_with(bulk_delete_documents, bulk_delete_documents_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/documents/{documentId}",
            get_with(read_document, read_document_docs)
                .patch_with(update_document, update_document_docs)
                .delete_with(delete_document, delete_document_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/documents/{documentId}/content",
            get_with(download_document, download_document_docs),
        )
        .with_path_items(|item| item.tag("Documents"))
}
