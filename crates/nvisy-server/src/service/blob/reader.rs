//! Read side of the pipeline-run blob store: loading a run's staged outputs.

use elide_pipeline::entity::AuditHash;
use elide_pipeline::file::Document;
use elide_pipeline::{ArtifactSet, Audit, Engine};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{Blob, WorkspaceDetection, WorkspaceDocument, WorkspaceRedaction};
use nvisy_postgres::query::WorkspaceBlobRepository;
use nvisy_s3::{AuditKey, DocumentKey, IntermediateKey, ObjectKey};
use uuid::Uuid;

use crate::response::{ErrorKind, Result};
use crate::service::{CryptoService, Infra};

/// Loads a pipeline run's staged outputs from the first-party blob store: reads
/// and decrypts audits and enrichment intermediates, and reconstructs them
/// through the engine. Its write counterpart is [`ArtifactWriter`].
///
/// Cloneable and cheap to pass around: it holds the shared [`Infra`] clients
/// (all `Arc`-backed) and takes the per-request database connection as a method
/// argument.
///
/// [`ArtifactWriter`]: crate::service::ArtifactWriter
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ArtifactReader {
    infra: Infra,
    crypto: CryptoService,
}

impl ArtifactReader {
    /// Creates a new [`ArtifactReader`] over the internal object store and crypto.
    pub fn new(infra: Infra, crypto: CryptoService) -> Self {
        Self { infra, crypto }
    }

    /// Resolves the blob holding a detection's analysis (its base audit).
    ///
    /// The connection-bound step of loading an analysis; pair with [`load_audit`]
    /// to release the connection before the object-store round-trip. A detection
    /// with no base audit yet (409) and one whose audit blob has been reclaimed
    /// (404) map to distinct responses.
    ///
    /// [`load_audit`]: Self::load_audit
    ///
    /// # Errors
    ///
    /// - `Conflict` if the detection has no base audit yet.
    /// - `NotFound` if the audit's blob has been reclaimed.
    /// - A database error if either lookup fails.
    pub async fn resolve_audit_blob(
        &self,
        conn: &mut PgConn,
        detection: &WorkspaceDetection,
    ) -> Result<Blob> {
        // The base analysis is the detection's `audit_blob_id`. A NULL pointer is
        // two distinct cases the status disambiguates: on a completed detection the
        // bytes were reclaimed on retention (404), otherwise the analysis has not
        // been produced yet (409).
        if detection.audit_blob_id.is_none() && !detection.status.is_complete() {
            return Err(ErrorKind::Conflict.with_message("Detection has no analysis yet"));
        }
        self.blob_or_gone(
            conn,
            detection.audit_blob_id,
            "The analysis for this detection has been deleted",
        )
        .await
    }

    /// Loads and decodes an analysis from its already-resolved audit blob. Holds
    /// no database connection: only object-store I/O and decryption, so a caller
    /// can run it after releasing its connection.
    ///
    /// The `engine` reconstructs the audit's report from its serialized form: an
    /// [`Audit`] serializes but does not `Deserialize`, since its report tags each
    /// entity group by modality name and only the engine's registry can map those
    /// back to concrete types. The decoded audit is checked against the digest
    /// recorded on the blob when it was written.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if the blob's storage path is not a valid audit
    ///   key, if the blob has no valid recorded digest, if the object is missing
    ///   from storage, if reading its bytes fails, if decrypting them fails, or if
    ///   the engine cannot decode the audit or it does not match its digest.
    /// - A storage error if the object store rejects the read.
    pub async fn load_audit(
        &self,
        engine: &Engine,
        workspace_id: Uuid,
        audit_blob: &Blob,
    ) -> Result<Audit> {
        let expected = recorded_digest(audit_blob)?;
        let plaintext = self
            .read_decrypted::<AuditKey>(workspace_id, &audit_blob.storage_path, "audit")
            .await?
            .ok_or_else(|| {
                ErrorKind::InternalServerError.with_message("Audit is missing from storage")
            })?;
        engine
            .deserialize_audit(
                &expected,
                &mut serde_json::Deserializer::from_slice(&plaintext),
            )
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decode audit")
                    .with_context(err.to_string())
            })
    }

    /// Resolves the blob holding a detection's enrichment intermediates.
    ///
    /// The connection-bound step; pair with [`load_intermediates`] to release the
    /// connection before the object-store round-trip. A detection whose modality
    /// produced no enrichment (text, tabular) has none — a `None` reference maps
    /// to a 404, distinct from a reference to a reclaimed blob.
    ///
    /// [`load_intermediates`]: Self::load_intermediates
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection has no intermediates reference, or its blob
    ///   has been reclaimed.
    /// - A database error if the blob lookup fails.
    pub async fn resolve_intermediates_blob(
        &self,
        conn: &mut PgConn,
        detection: &WorkspaceDetection,
    ) -> Result<Blob> {
        self.blob_or_gone(
            conn,
            detection.intermediate_blob_id,
            "The intermediates for this detection are not available",
        )
        .await
    }

    /// Loads and decodes a detection's enrichment intermediates from its
    /// already-resolved blob. Holds no database connection: only object-store I/O
    /// and decryption.
    ///
    /// The `engine` reconstructs the [`ArtifactSet`] from its serialized form: it
    /// serializes but does not `Deserialize`, since each group is tagged by
    /// modality name and only the engine's registry can map those back to concrete
    /// artifact types.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if the blob's storage path is not a valid
    ///   intermediates key, if the blob has no valid recorded digest, if the
    ///   object is missing from storage, if reading its bytes fails, if decrypting
    ///   them fails, or if the engine cannot decode the artifacts or they do not
    ///   match their digest.
    /// - A storage error if the object store rejects the read.
    pub async fn load_intermediates(
        &self,
        engine: &Engine,
        workspace_id: Uuid,
        intermediates_blob: &Blob,
    ) -> Result<ArtifactSet> {
        let expected = recorded_digest(intermediates_blob)?;
        let plaintext = self
            .read_decrypted::<IntermediateKey>(
                workspace_id,
                &intermediates_blob.storage_path,
                "intermediates",
            )
            .await?
            .ok_or_else(|| {
                ErrorKind::InternalServerError
                    .with_message("Intermediates are missing from storage")
            })?;
        engine
            .deserialize_artifacts(
                &expected,
                &mut serde_json::Deserializer::from_slice(&plaintext),
            )
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decode intermediates")
                    .with_context(err.to_string())
            })
    }

    /// Resolves the blob holding a redaction's review audit.
    ///
    /// The connection-bound step of loading a review audit; pair with
    /// [`load_audit`] to release the connection before the object-store
    /// round-trip. A redaction always produces a review audit, so a NULL pointer
    /// means its blob has since been reclaimed (404).
    ///
    /// [`load_audit`]: Self::load_audit
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review audit's blob has been reclaimed.
    /// - A database error if the lookup fails.
    pub async fn resolve_review_blob(
        &self,
        conn: &mut PgConn,
        redaction: &WorkspaceRedaction,
    ) -> Result<Blob> {
        self.blob_or_gone(
            conn,
            redaction.review_audit_blob_id,
            "The review audit for this redaction has been deleted",
        )
        .await
    }

    /// Reads a document's bytes from its blob and builds an engine [`Document`],
    /// stamping the run's id as the correlation id.
    ///
    /// The document's name is its original filename, and its format is resolved
    /// from the document's `file_extension` (the trusted, `NOT NULL` column the
    /// codec dispatches on — the filename may lie about the type). The name is the
    /// engine's identity for the document: it roots every part path and is how
    /// `anonymize` matches an audit back to its document, so redaction would
    /// silently no-op if it differed between passes. Both detect and redact build
    /// from the same document through here, so the name is identical by
    /// construction.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the object is missing from storage (its bytes were purged
    ///   after the blob was looked up).
    /// - `InternalServerError` if the blob's storage path is not a valid document
    ///   key, if reading its bytes fails, or if decrypting them with the workspace
    ///   key fails.
    /// - A storage error if the object store rejects the read.
    pub async fn build_document(
        &self,
        document: &WorkspaceDocument,
        blob: &Blob,
        correlation_id: Uuid,
    ) -> Result<Document> {
        let bytes = self
            .read_decrypted::<DocumentKey>(document.workspace_id, &blob.storage_path, "document")
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound.with_message("Document content is missing from storage")
            })?;
        Ok(Document::new(document.original_filename.clone(), bytes)
            .with_extension(document.file_extension.clone())
            .with_correlation_id(correlation_id))
    }

    /// Reads and decrypts the object at `storage_path` in `K`'s store, or `None`
    /// if it is missing. `what` names the content in error messages.
    async fn read_decrypted<K: ObjectKey>(
        &self,
        workspace_id: Uuid,
        storage_path: &str,
        what: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = K::from_str(storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message(format!("Invalid {what} storage key"))
                .with_context(err.to_string())
        })?;
        let Some(ciphertext) = self.infra.blobs.get_bytes(&key).await? else {
            return Ok(None);
        };
        let plaintext = self
            .crypto
            .decrypt(workspace_id, &ciphertext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message(format!("Failed to decrypt {what}"))
                    .with_context(err.to_string())
            })?;
        Ok(Some(plaintext))
    }

    /// Fetches a blob by its optional id, mapping a reclaimed blob to a 404 with
    /// `gone_message`. A `None` id is the reclaimed case too: once the bytes pass
    /// retention the referrer's pointer is nulled (the row is kept), so an absent
    /// pointer and an absent blob row are the same "gone" outcome. Shared by the
    /// audit/intermediate/review resolvers, which differ only in that message.
    async fn blob_or_gone(
        &self,
        conn: &mut PgConn,
        blob_id: Option<Uuid>,
        gone_message: &'static str,
    ) -> Result<Blob> {
        let blob_id = blob_id.ok_or_else(|| ErrorKind::NotFound.with_message(gone_message))?;
        conn.find_blob_by_id(blob_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message(gone_message))
    }
}

/// The engine digest recorded on an audit or intermediates blob when it was
/// written, which the decoded content must match.
///
/// # Errors
///
/// `InternalServerError` if the blob has no digest or it is not a valid one.
fn recorded_digest(blob: &Blob) -> Result<AuditHash> {
    let digest = blob.audit_digest.as_deref().ok_or_else(|| {
        ErrorKind::InternalServerError.with_message("Stored analysis has no recorded digest")
    })?;
    let bytes = <[u8; 32]>::try_from(digest).map_err(|_| {
        ErrorKind::InternalServerError
            .with_message("Stored analysis has an invalid recorded digest")
            .with_context(format!("expected 32 bytes, found {}", digest.len()))
    })?;
    Ok(AuditHash::from_bytes(bytes))
}
