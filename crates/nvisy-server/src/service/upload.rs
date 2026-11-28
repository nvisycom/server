//! Staging uploaded documents: streaming each one, encrypted, to object storage
//! and building its unsaved rows, ahead of the transaction that records them.

use std::io;
use std::path::Path;

use bytes::Bytes;
use futures::Stream;
use nvisy_postgres::model::{NewBlob, NewWorkspaceDocument, Workspace};
use nvisy_postgres::types::DocumentKind;
use nvisy_s3::{BlobStore, Bucket, DocumentKey};
use tokio_util::io::StreamReader;
use uuid::Uuid;

use crate::response::{ErrorKind, Result};
use crate::service::{CryptoService, EngineService, HashingReader, LimitedReader};

/// Tracing target for document staging.
const TRACING_TARGET: &str = "nvisy_server::service::upload";

/// Where an upload lands and the limits it is staged under.
#[derive(Debug, Clone, Copy)]
pub struct UploadTarget {
    /// The workspace receiving the documents.
    pub workspace_id: Uuid,
    /// The uploading account.
    pub account_id: Uuid,
    /// Retention expiry for the uploaded originals.
    pub expires_at: jiff::Timestamp,
    /// The effective per-document cap in bytes: the smaller of the workspace's
    /// soft cap and the server-wide hard limit. A document streaming past it is
    /// rejected before its excess reaches storage.
    pub max_upload_bytes: u64,
}

impl UploadTarget {
    /// Resolves the target for an upload into `workspace` by `account_id`, under
    /// the server-wide per-document `hard_limit`.
    #[must_use]
    pub fn new(workspace: &Workspace, account_id: Uuid, hard_limit: u64) -> Self {
        let settings = workspace.settings.or_default();
        Self {
            workspace_id: workspace.id,
            account_id,
            expires_at: settings
                .retention
                .original_documents
                .expires_at(jiff::Timestamp::now()),
            max_upload_bytes: settings.effective_max_upload_bytes(hard_limit),
        }
    }
}

/// A document streamed to object storage whose document and blob rows have not
/// been inserted yet. Pairs the object's key with its unsaved rows so a batch can
/// persist every row together, and reclaim every staged object if that fails.
#[derive(Debug)]
pub struct StagedDocument {
    /// The staged object's storage key.
    pub key: DocumentKey,
    /// The unsaved blob row; resolved (shared or created) at commit.
    pub blob: NewBlob,
    /// The unsaved document row, pointed at its blob at commit.
    pub document: NewWorkspaceDocument,
}

/// Streams uploaded documents, encrypted, to object storage, and reclaims staged
/// objects a commit does not keep.
#[derive(Clone)]
pub struct DocumentStager {
    blobs: BlobStore,
    crypto: CryptoService,
    /// Rejects a format no codec can decode before it is stored.
    engine: EngineService,
}

impl DocumentStager {
    /// Creates a [`DocumentStager`] over its clients.
    #[must_use]
    pub fn new(blobs: BlobStore, crypto: CryptoService, engine: EngineService) -> Self {
        Self {
            blobs,
            crypto,
            engine,
        }
    }

    /// Streams one document to storage and builds its unsaved rows.
    ///
    /// # Errors
    ///
    /// - `BadRequest` if no codec decodes the document's format.
    /// - `PayloadTooLarge` if the document exceeds the target's cap.
    /// - A storage error if the write fails.
    pub async fn stage(
        &self,
        target: &UploadTarget,
        filename: &str,
        body: impl Stream<Item = io::Result<Bytes>> + Send + Unpin,
    ) -> Result<StagedDocument> {
        let file_extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bin")
            .to_lowercase();

        // Reject a format no codec can decode before streaming it to storage: an
        // unprocessable document would only fail later at detection, after wasting an
        // encrypted upload.
        if !self.engine.supports_extension(&file_extension) {
            return Err(ErrorKind::BadRequest
                .with_message(format!("Unsupported document format: .{file_extension}"))
                .with_context(
                    "The redaction engine has no codec for this document type; upload a supported format.",
                ));
        }

        // Generate document key with a unique object ID for blob storage.
        let document_key = DocumentKey::generate(target.workspace_id);

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
        let cap = target.max_upload_bytes;
        let source = StreamReader::new(body);
        let (limited, limit_state) = LimitedReader::new(source, cap);
        let (measured, measurements) = HashingReader::new(limited);
        let encrypted = self.crypto.encrypt_reader(target.workspace_id, measured);

        if let Err(err) = self.blobs.put(&document_key, Box::pin(encrypted)).await {
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
            workspace_id: target.workspace_id,
            content_hash: measurements.sha256().to_vec(),
            file_size_bytes: i64::try_from(measurements.bytes()).unwrap_or(i64::MAX),
            storage_path: document_key.to_string(),
            storage_bucket: Bucket::Documents.name().to_owned(),
            audit_digest: None,
        };
        let document = NewWorkspaceDocument {
            workspace_id: target.workspace_id,
            account_id: target.account_id,
            // Resolved to the staged blob's id at commit time.
            blob_id: Uuid::nil(),
            kind: Some(DocumentKind::Original),
            display_name: Some(filename.to_owned()),
            original_filename: Some(filename.to_owned()),
            file_extension: Some(file_extension),
            metadata: None,
            expires_at: Some(target.expires_at.into()),
        };

        Ok(StagedDocument {
            key: document_key,
            blob,
            document,
        })
    }

    /// Removes staged objects best-effort, for a batch that does not commit. Each
    /// object was written before any row exists, so nothing else can reclaim it;
    /// a failed removal is logged and leaked rather than failing the response.
    pub async fn discard(&self, staged: &[StagedDocument]) {
        for document in staged {
            self.discard_object(&document.key).await;
        }
    }

    /// Removes one staged object best-effort, logging a failure.
    pub async fn discard_object(&self, key: &DocumentKey) {
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
