//! Pipeline-run blob I/O.
//!
//! [`RunBlobStore`] reads and writes a run's document, redacted output, audit,
//! and enrichment intermediates in the platform's first-party S3-compatible blob
//! store (the `Documents`, `Audits`, and `Intermediates`
//! [`Bucket`](nvisy_s3::Bucket)s), handling per-workspace encryption and the
//! blob bookkeeping each one needs. It is distinct from
//! [`ExternalObjectStore`](crate::service::ExternalObjectStore), which bridges
//! external tenant object stores.
//!
//! Bytes are content-addressed: each `stage_*` method writes the (encrypted)
//! object and returns a [`NewBlob`] describing it, which the caller resolves
//! through [`find_or_create_blob`](nvisy_postgres::query::WorkspaceBlobRepository::find_or_create_blob)
//! (directly or via [`create_audit`](nvisy_postgres::query::WorkspaceAuditRepository::create_audit)
//! / [`create_workspace_document`](nvisy_postgres::query::WorkspaceDocumentRepository::create_workspace_document))
//! so identical content is stored once and shared.

use std::io::Cursor;
use std::str::FromStr;

use bytes::Bytes;
use elide_pipeline::file::Document;
use elide_pipeline::{ArtifactSet, Audit, Engine};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{
    Blob, NewBlob, WorkspaceDetection, WorkspaceDocument, WorkspacePipeline,
};
use nvisy_postgres::query::{ReclaimableBlob, WorkspaceAuditRepository, WorkspaceBlobRepository};
use nvisy_postgres::types::{RetentionScope, RetentionSettings};
use nvisy_s3::{AuditKey, Bucket, DocumentKey, IntermediateKey};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Result};
use crate::service::Infra;

/// Tracing target for blob-store operations.
const TRACING_TARGET: &str = "nvisy_server::service::run_blob_store";

/// Whether a reclaim step removed a blob's backing object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a Pending reclaim is not progress and must not be counted as one"]
pub enum PurgeOutcome {
    /// The object was removed and `reclaimed_at` was stamped.
    Purged,
    /// The object was not reclaimed — the blob gained a reference before it could
    /// be claimed, or the delete failed (store failure, bad key, or unknown
    /// bucket). A claimed-but-not-reclaimed blob is retried by the reconcile sweep.
    Pending,
}

/// Wraps a storage-key parse failure as an internal error.
fn invalid_key(err: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Invalid blob storage key")
        .with_context(err.to_string())
}

/// Reads and writes a pipeline run's blobs in the first-party blob store.
///
/// Cloneable and cheap to pass around: it holds the shared [`Infra`] clients
/// (all `Arc`-backed) and takes the per-request database connection as a method
/// argument. Not to be confused with
/// [`ExternalObjectStore`](crate::service::ExternalObjectStore), which bridges *external*
/// tenant object stores; this operates on the platform's own S3-compatible
/// store, routing to the `Documents`, `Audits`, and `Intermediates`
/// [`Bucket`](nvisy_s3::Bucket) prefixes.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct RunBlobStore {
    infra: Infra,
}

impl RunBlobStore {
    /// Creates a new [`RunBlobStore`] over the internal object store and crypto.
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Claims a due blob and reclaims its object.
    ///
    /// The claim (`purged_at`) is committed *before* the object is deleted, so the
    /// blob leaves the dedup set the instant it is claimed: `find_or_create_blob`
    /// can never hand out a reference to bytes that are about to be — or already —
    /// gone. If a reference was acquired since the sweep read the blob, the claim
    /// matches no row and the blob is skipped ([`PurgeOutcome::Pending`]). A claim
    /// whose object delete then fails stays claimed for the reconcile sweep to
    /// retry, so a transient store outage self-heals without ever resurrecting the
    /// bytes.
    pub async fn purge_blob(
        &self,
        conn: &mut PgConn,
        blob: &ReclaimableBlob,
    ) -> Result<PurgeOutcome> {
        // Claim first, in its own committed step: once purged_at is set the blob no
        // longer deduplicates, so deleting its object next cannot strand a live
        // reference even if this process crashes before the delete.
        if conn.claim_blob_for_purge(blob.id).await?.is_none() {
            return Ok(PurgeOutcome::Pending);
        }
        Ok(self.reclaim_claimed_object(conn, blob).await)
    }

    /// Reclaims the object of a blob already claimed for purge (`purged_at` set),
    /// stamping `reclaimed_at` on success. Backs the reconcile sweep's retries.
    ///
    /// A failed delete leaves `reclaimed_at` NULL so the blob is retried; the blob
    /// is already out of the dedup set, so its bytes are never reused meanwhile.
    pub async fn reclaim_claimed_object(
        &self,
        conn: &mut PgConn,
        blob: &ReclaimableBlob,
    ) -> PurgeOutcome {
        if let Err(err) = self
            .delete_object(&blob.storage_bucket, &blob.storage_path)
            .await
        {
            tracing::error!(
                target: TRACING_TARGET,
                blob_id = %blob.id,
                error = %err,
                "Failed to delete claimed blob object; left for the reaper to retry",
            );
            return PurgeOutcome::Pending;
        }

        if let Err(err) = conn.mark_blob_reclaimed(blob.id).await {
            tracing::error!(
                target: TRACING_TARGET,
                blob_id = %blob.id,
                error = %err,
                "Deleted blob object but failed to mark it reclaimed; will retry",
            );
            return PurgeOutcome::Pending;
        }
        PurgeOutcome::Purged
    }

    /// Removes an object from whichever store its blob names. An unparseable
    /// storage key or an unknown store is an error, not a silent success: the
    /// object was not reclaimed, so the blob stays pending.
    async fn delete_object(&self, bucket: &str, storage_path: &str) -> Result<()> {
        let store = Bucket::from_name(bucket).ok_or_else(|| {
            ErrorKind::InternalServerError
                .with_message("Blob references an unknown storage bucket")
                .with_context(format!("bucket: {bucket}"))
        })?;

        // Each store's key type differs, so parse the key for the store this blob
        // names before deleting.
        match store {
            Bucket::Documents => {
                let key = DocumentKey::from_str(storage_path).map_err(invalid_key)?;
                self.infra.blobs.delete(&key).await?;
            }
            Bucket::Audits => {
                let key = AuditKey::from_str(storage_path).map_err(invalid_key)?;
                self.infra.blobs.delete(&key).await?;
            }
            Bucket::Intermediates => {
                let key = IntermediateKey::from_str(storage_path).map_err(invalid_key)?;
                self.infra.blobs.delete(&key).await?;
            }
            Bucket::AccountAvatars | Bucket::WorkspaceAvatars => {
                return Err(ErrorKind::InternalServerError
                    .with_message(
                        "Avatar objects are not reclaimed through the blob store's blob purge",
                    )
                    .with_context(format!("bucket: {bucket}")));
            }
        }
        Ok(())
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
    pub async fn build_document(
        &self,
        document: &WorkspaceDocument,
        blob: &Blob,
        correlation_id: Uuid,
    ) -> Result<Document> {
        let key = DocumentKey::from_str(&blob.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid blob storage path")
                .with_context(err.to_string())
        })?;

        let data = self.infra.blobs.get(&key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("Document content is missing from storage")
        })?;
        let mut reader = data.into_reader();
        let mut ciphertext = Vec::new();
        reader.read_to_end(&mut ciphertext).await.map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to read document content")
                .with_context(err.to_string())
        })?;

        let bytes = self
            .infra
            .crypto
            .decrypt(document.workspace_id, &ciphertext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decrypt document content")
                    .with_context(err.to_string())
            })?;

        Ok(Document::new(document.original_filename.clone(), bytes)
            .with_extension(document.file_extension.clone())
            .with_correlation_id(correlation_id))
    }

    /// Encrypts the analysis, writes it to the audit bucket, and builds the
    /// [`NewBlob`] that will hold it — but does not insert the blob.
    ///
    /// The analysis is the map of detected PII, so it is encrypted with the
    /// workspace key before it leaves the process. Its bytes live in the audit
    /// bucket; retention is stamped on the blob (`expires_at`), so the reaper's
    /// blob sweep expires it.
    ///
    /// The object write is not transactional, so it is kept out of the caller's
    /// database transaction: the bytes are written first, then the returned blob
    /// is resolved (with the run's other writes) atomically. A rollback therefore
    /// leaves at worst an orphan object in the bucket, never a blob that points at
    /// bytes that were never written; the caller reclaims that orphan via
    /// [`discard_staged_object`](Self::discard_staged_object).
    pub async fn stage_analyzed_document(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        analyzed: &Audit,
    ) -> Result<NewBlob> {
        self.stage_audit_blob(pipeline, workspace_settings, analyzed, "encrypt analysis")
            .await
    }

    /// Encrypts a redaction's review audit, writes it to the audit bucket, and
    /// builds the [`NewBlob`] that will hold it — without inserting the blob.
    ///
    /// The review audit is the post-redaction [`Audit`]: the detection's analysis
    /// with the reviewer's edits applied and the redaction outcome recorded per
    /// entity. It shares the audit-logs retention scope with the detection audit;
    /// the audit *row* (not a distinct blob kind) distinguishes it — a review
    /// audit sets `redaction_id` and `derived_from`.
    pub async fn stage_review_audit(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        reviewed: &Audit,
    ) -> Result<NewBlob> {
        self.stage_audit_blob(
            pipeline,
            workspace_settings,
            reviewed,
            "encrypt review audit",
        )
        .await
    }

    /// Shared audit-blob staging for the detection audit and the redaction review
    /// audit: both encrypt an [`Audit`] to the audit bucket under the audit-logs
    /// retention scope and return the blob to resolve.
    async fn stage_audit_blob(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        audit: &Audit,
        encrypt_step: &str,
    ) -> Result<NewBlob> {
        let workspace_id = pipeline.workspace_id;
        let plaintext = serde_json::to_vec(audit).map_err(analysis_serde_error)?;
        let hash = Sha256::digest(&plaintext).to_vec();
        let size = plaintext.len() as i64;
        let ciphertext = self
            .infra
            .crypto
            .encrypt(workspace_id, &plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message(format!("Failed to {encrypt_step}"))
                    .with_context(err.to_string())
            })?;

        let key = AuditKey::generate(workspace_id);
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;

        // Retention expiry for the audit scope (workspace baseline, pipeline
        // override if set).
        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::AuditLogs, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        Ok(NewBlob {
            workspace_id,
            content_hash: hash,
            file_size_bytes: size,
            storage_path: key.to_string(),
            storage_bucket: Bucket::Audits.name().to_owned(),
            expires_at: expires_at.map(Into::into),
        })
    }

    /// Serializes a detection's enrichment intermediates (OCR layout, transcript),
    /// encrypts them with the workspace key, writes them to the intermediates store,
    /// and builds the [`NewBlob`] that will hold them — without inserting the blob
    /// (staged like the analysis, reclaimed on rollback).
    ///
    /// The intermediates carry document content, so they are encrypted at rest and
    /// governed by their own retention scope, resolved here (workspace baseline,
    /// pipeline override if set).
    pub async fn stage_intermediates<T: Serialize>(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        artifacts: &T,
    ) -> Result<NewBlob> {
        let workspace_id = pipeline.workspace_id;
        let plaintext = serde_json::to_vec(artifacts).map_err(analysis_serde_error)?;
        let hash = Sha256::digest(&plaintext).to_vec();
        let size = plaintext.len() as i64;
        let ciphertext = self
            .infra
            .crypto
            .encrypt(workspace_id, &plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt intermediates")
                    .with_context(err.to_string())
            })?;

        let key = IntermediateKey::generate(workspace_id);
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;

        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::Intermediates, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        Ok(NewBlob {
            workspace_id,
            content_hash: hash,
            file_size_bytes: size,
            storage_path: key.to_string(),
            storage_bucket: Bucket::Intermediates.name().to_owned(),
            expires_at: expires_at.map(Into::into),
        })
    }

    /// Encrypts redacted bytes, writes them to the documents bucket, and builds the
    /// [`NewBlob`] that will hold them together with the redacted output's display
    /// name — without inserting the blob or the document.
    ///
    /// A redaction commits its output document, review-audit row, and redaction
    /// row together in one transaction, so the output is staged (object written,
    /// blob returned for the caller to resolve) rather than inserted here, and is
    /// reclaimed on rollback via
    /// [`discard_staged_object`](Self::discard_staged_object). The redacted
    /// document is a first-class document (a sibling of the source), downloadable
    /// through the normal document endpoints.
    pub async fn stage_redacted_document(
        &self,
        source_document: &WorkspaceDocument,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        bytes: Bytes,
    ) -> Result<(NewBlob, String)> {
        let workspace_id = source_document.workspace_id;
        let plaintext_size = bytes.len() as i64;
        let plaintext_hash = Sha256::digest(&bytes).to_vec();
        let ciphertext = self
            .infra
            .crypto
            .encrypt(workspace_id, &bytes)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt redacted document")
                    .with_context(err.to_string())
            })?;

        let key = DocumentKey::generate(workspace_id);
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;

        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::RedactedDocuments, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        let redacted_name = redacted_display_name(
            &source_document.display_name,
            &source_document.file_extension,
        );
        let blob = NewBlob {
            workspace_id,
            content_hash: plaintext_hash,
            file_size_bytes: plaintext_size,
            storage_path: key.to_string(),
            storage_bucket: Bucket::Documents.name().to_owned(),
            expires_at: expires_at.map(Into::into),
        };
        Ok((blob, redacted_name))
    }

    /// Deletes a staged object whose blob was never resolved.
    ///
    /// The `stage_*` methods write an object before its `workspace_blobs` row; if
    /// the committing transaction rolls back, the object has no blob and the
    /// blob-driven reaper can never find it. The caller invokes this on that path
    /// so the orphan is removed immediately instead of accumulating. It deletes
    /// from whichever bucket the staged blob names, so it reclaims a staged audit,
    /// review audit, intermediate, or redacted document alike. Best effort: a
    /// failure here only leaves the object for a later manual sweep, so callers log
    /// rather than propagate.
    pub async fn discard_staged_object(&self, staged: &NewBlob) -> Result<()> {
        self.delete_object(&staged.storage_bucket, &staged.storage_path)
            .await
    }

    /// Resolves the blob holding a detection's analysis (its base audit).
    ///
    /// The connection-bound step of loading an analysis; pair with
    /// [`load_audit`](Self::load_audit) to release the connection before the
    /// object-store round-trip. A detection with no base audit yet (409) and one
    /// whose audit blob has been reclaimed (404) map to distinct responses.
    pub async fn resolve_audit_blob(
        &self,
        conn: &mut PgConn,
        detection: &WorkspaceDetection,
    ) -> Result<Blob> {
        let audit = conn.find_base_audit(detection.id).await?.ok_or_else(|| {
            ErrorKind::Conflict
                .with_message("Detection has no analysis yet")
                .with_resource("detection")
        })?;
        self.blob_or_gone(
            conn,
            audit.blob_id,
            "The analysis for this detection has been deleted",
            "detection",
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
    /// back to concrete types.
    pub async fn load_audit(
        &self,
        engine: &Engine,
        workspace_id: Uuid,
        audit_blob: &Blob,
    ) -> Result<Audit> {
        let key = AuditKey::from_str(&audit_blob.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid audit storage key")
                .with_context(err.to_string())
        })?;

        let data = self.infra.blobs.get(&key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("Audit is missing from storage")
        })?;
        let mut reader = data.into_reader();
        let mut ciphertext = Vec::new();
        reader.read_to_end(&mut ciphertext).await.map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to read audit")
                .with_context(err.to_string())
        })?;

        let plaintext = self
            .infra
            .crypto
            .decrypt(workspace_id, &ciphertext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decrypt audit")
                    .with_context(err.to_string())
            })?;
        engine
            .deserialize_audit(&mut serde_json::Deserializer::from_slice(&plaintext))
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decode audit")
                    .with_context(err.to_string())
            })
    }

    /// Resolves the blob holding a detection's enrichment intermediates.
    ///
    /// The connection-bound step; pair with [`load_intermediates`](Self::load_intermediates)
    /// to release the connection before the object-store round-trip. A detection
    /// whose modality produced no enrichment (text, tabular) has none — a `None`
    /// reference maps to a 404, distinct from a reference to a reclaimed blob.
    pub async fn resolve_intermediates_blob(
        &self,
        conn: &mut PgConn,
        detection: &WorkspaceDetection,
    ) -> Result<Blob> {
        let blob_id = detection.intermediate_blob_id.ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Detection has no enrichment intermediates")
                .with_resource("detection")
        })?;
        self.blob_or_gone(
            conn,
            blob_id,
            "The intermediates for this detection have been deleted",
            "detection",
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
    pub async fn load_intermediates(
        &self,
        engine: &Engine,
        workspace_id: Uuid,
        intermediates_blob: &Blob,
    ) -> Result<ArtifactSet> {
        let key = IntermediateKey::from_str(&intermediates_blob.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid intermediates storage key")
                .with_context(err.to_string())
        })?;

        let data = self.infra.blobs.get(&key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("Intermediates are missing from storage")
        })?;
        let mut reader = data.into_reader();
        let mut ciphertext = Vec::new();
        reader.read_to_end(&mut ciphertext).await.map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to read intermediates")
                .with_context(err.to_string())
        })?;

        let plaintext = self
            .infra
            .crypto
            .decrypt(workspace_id, &ciphertext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decrypt intermediates")
                    .with_context(err.to_string())
            })?;
        engine
            .deserialize_artifacts(&mut serde_json::Deserializer::from_slice(&plaintext))
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decode intermediates")
                    .with_context(err.to_string())
            })
    }

    /// Resolves the blob holding a redaction's review audit.
    ///
    /// The connection-bound step of loading a review audit; pair with
    /// [`load_audit`](Self::load_audit) to release the connection before the
    /// object-store round-trip. Errors if the redaction has no review audit (409)
    /// or its blob has since been reclaimed (404).
    pub async fn resolve_review_blob(&self, conn: &mut PgConn, redaction_id: Uuid) -> Result<Blob> {
        let audit = conn
            .find_redaction_audit(redaction_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::Conflict
                    .with_message("Redaction has no review audit")
                    .with_resource("redaction")
            })?;
        self.blob_or_gone(
            conn,
            audit.blob_id,
            "The review audit for this redaction has been deleted",
            "redaction",
        )
        .await
    }

    /// Fetches a blob by id, mapping a reclaimed (absent) blob to a 404 with
    /// `gone_message` naming `resource`. Shared by the audit/intermediate/review
    /// resolvers, which differ in that message and the resource they name.
    async fn blob_or_gone(
        &self,
        conn: &mut PgConn,
        blob_id: Uuid,
        gone_message: &'static str,
        resource: &'static str,
    ) -> Result<Blob> {
        conn.find_blob_by_id(blob_id).await?.ok_or_else(|| {
            ErrorKind::NotFound
                .with_message(gone_message)
                .with_resource(resource)
        })
    }
}

/// Maps an analysis (de)serialization failure to an internal error.
fn analysis_serde_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process analysis")
        .with_context(error.to_string())
}

/// Builds the redacted document's display name by inserting a `redacted` marker
/// before the extension: `report.pdf` becomes `report.redacted.pdf`.
///
/// The stem is taken by stripping a trailing `.{extension}` (case-insensitive)
/// from the display name; a name that does not end in its own extension (or has
/// none) simply gains a `.redacted` suffix.
fn redacted_display_name(display_name: &str, extension: &str) -> String {
    // Split off a trailing `.{extension}`, matched case-insensitively on both
    // sides so an upper- or mixed-case extension (`Report.PDF`) still has the
    // marker inserted before it, not appended after.
    let suffix = format!(".{extension}");
    let stem = display_name
        .len()
        .checked_sub(suffix.len())
        .filter(|_| !extension.is_empty())
        .filter(|&at| display_name.is_char_boundary(at))
        .map(|at| display_name.split_at(at))
        .filter(|(_, tail)| tail.eq_ignore_ascii_case(&suffix))
        .map(|(stem, _)| stem);
    match stem {
        Some(stem) => format!("{stem}.redacted.{extension}"),
        None => format!("{display_name}.redacted"),
    }
}

#[cfg(test)]
mod tests {
    use super::redacted_display_name;

    #[test]
    fn inserts_marker_before_the_extension() {
        assert_eq!(
            redacted_display_name("report.pdf", "pdf"),
            "report.redacted.pdf"
        );
    }

    #[test]
    fn matches_the_extension_case_insensitively() {
        // The name's extension case differs from the passed extension...
        assert_eq!(
            redacted_display_name("Report.PDF", "pdf"),
            "Report.redacted.pdf"
        );
        // ...and the passed extension itself may be upper- or mixed-case; the
        // marker still lands before it, and the original extension text is kept.
        assert_eq!(
            redacted_display_name("Report.PDF", "PDF"),
            "Report.redacted.PDF"
        );
        assert_eq!(
            redacted_display_name("report.pdf", "PDF"),
            "report.redacted.PDF"
        );
    }

    #[test]
    fn preserves_a_multi_dot_stem() {
        assert_eq!(
            redacted_display_name("2026.q1.report.pdf", "pdf"),
            "2026.q1.report.redacted.pdf"
        );
    }

    #[test]
    fn appends_when_the_name_lacks_its_extension() {
        // A display name that does not end in `.{extension}` just gains the
        // marker, so no extension is fabricated.
        assert_eq!(redacted_display_name("report", "pdf"), "report.redacted");
        assert_eq!(
            redacted_display_name("report.txt", "pdf"),
            "report.txt.redacted"
        );
    }
}
