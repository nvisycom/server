//! Write side of the pipeline-run blob store: staging a run's outputs.

use std::io::Cursor;
use std::str::FromStr;

use bytes::Bytes;
use elide_pipeline::Audit;
use elide_pipeline::file::Document;
use nvisy_postgres::model::{Blob, NewBlob, WorkspaceDocument};
use nvisy_postgres::types::{RetentionOverride, RetentionScope, RetentionSettings};
use nvisy_s3::{AuditKey, Bucket, DocumentKey, IntermediateKey};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Result};
use crate::service::{CryptoService, Infra};

/// Stages a pipeline run's outputs into the first-party blob store: encrypts and
/// writes documents, audits, and enrichment intermediates, returning the
/// [`NewBlob`] the caller resolves into a row. Its read counterpart is
/// [`ArtifactReader`].
///
/// Cloneable and cheap to pass around: it holds the shared [`Infra`] clients
/// (all `Arc`-backed) and takes the per-request database connection as a method
/// argument. Not to be confused with [`ExternalObjectStore`], which bridges
/// *external* tenant object stores; this operates on the platform's own
/// S3-compatible store, routing to the `Documents`, `Audits`, and
/// `Intermediates` [`Bucket`] prefixes.
///
/// [`ArtifactReader`]: crate::service::ArtifactReader
/// [`ExternalObjectStore`]: crate::service::ExternalObjectStore
/// [`NewBlob`]: nvisy_postgres::model::NewBlob
/// [`Bucket`]: nvisy_s3::Bucket
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ArtifactWriter {
    pub(super) infra: Infra,
    pub(super) crypto: CryptoService,
}

impl ArtifactWriter {
    /// Creates a new [`ArtifactWriter`] over the internal object store and crypto.
    pub fn new(infra: Infra, crypto: CryptoService) -> Self {
        Self { infra, crypto }
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
    /// - `InternalServerError` if the blob's storage path is not a valid document
    ///   key, if the object is missing from storage, if reading its bytes fails,
    ///   or if decrypting them with the workspace key fails.
    /// - A storage error if the object store rejects the read.
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
    /// [`discard_staged_object`].
    ///
    /// [`discard_staged_object`]: Self::discard_staged_object
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if serializing or encrypting the audit fails.
    /// - A storage error if the object write fails.
    pub async fn stage_analyzed_document(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        analyzed: &Audit,
    ) -> Result<NewBlob> {
        self.stage_audit_blob(
            workspace_id,
            retention_override,
            workspace_settings,
            analyzed,
            "encrypt analysis",
        )
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
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if serializing or encrypting the audit fails.
    /// - A storage error if the object write fails.
    pub async fn stage_review_audit(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        reviewed: &Audit,
    ) -> Result<NewBlob> {
        self.stage_audit_blob(
            workspace_id,
            retention_override,
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
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        audit: &Audit,
        encrypt_step: &str,
    ) -> Result<NewBlob> {
        let plaintext = serde_json::to_vec(audit).map_err(|error| analysis_serde_error(&error))?;
        let hash = Sha256::digest(&plaintext).to_vec();
        let size = i64::try_from(plaintext.len()).unwrap_or(i64::MAX);
        let ciphertext = self
            .crypto
            .encrypt(workspace_id, &plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message(format!("Failed to {encrypt_step}"))
                    .with_context(err.to_string())
            })?;

        let key = AuditKey::generate(workspace_id);
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;

        // Retention expiry for the audit scope: the detection's snapshotted
        // override if any, else the workspace baseline.
        let expires_at = workspace_settings
            .resolve(RetentionScope::AuditLogs, retention_override)
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
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if serializing `artifacts` or encrypting them with
    ///   the workspace key fails.
    /// - A storage error if writing the object fails.
    pub async fn stage_intermediates<T: Serialize>(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        artifacts: &T,
    ) -> Result<NewBlob> {
        let plaintext =
            serde_json::to_vec(artifacts).map_err(|error| analysis_serde_error(&error))?;
        let hash = Sha256::digest(&plaintext).to_vec();
        let size = i64::try_from(plaintext.len()).unwrap_or(i64::MAX);
        let ciphertext = self
            .crypto
            .encrypt(workspace_id, &plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt intermediates")
                    .with_context(err.to_string())
            })?;

        let key = IntermediateKey::generate(workspace_id);
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;

        let expires_at = workspace_settings
            .resolve(RetentionScope::Intermediates, retention_override)
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
    /// reclaimed on rollback via [`discard_staged_object`]. The redacted document
    /// is a first-class document (a sibling of the source), downloadable through
    /// the normal document endpoints.
    ///
    /// [`discard_staged_object`]: Self::discard_staged_object
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if encrypting the redacted bytes with the
    ///   workspace key fails.
    /// - A storage error if writing the object fails.
    pub async fn stage_redacted_document(
        &self,
        source_document: &WorkspaceDocument,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        bytes: Bytes,
    ) -> Result<(NewBlob, String)> {
        let workspace_id = source_document.workspace_id;
        let plaintext_size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        let plaintext_hash = Sha256::digest(&bytes).to_vec();
        let ciphertext = self.crypto.encrypt(workspace_id, &bytes).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to encrypt redacted document")
                .with_context(err.to_string())
        })?;

        let key = DocumentKey::generate(workspace_id);
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;

        let expires_at = workspace_settings
            .resolve(RetentionScope::RedactedDocuments, retention_override)
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
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if the staged blob names an unknown or non-purgeable
    ///   bucket, or its storage key does not parse.
    /// - A storage error if the object delete fails.
    pub async fn discard_staged_object(&self, staged: &NewBlob) -> Result<()> {
        self.delete_object(&staged.storage_bucket, &staged.storage_path)
            .await
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
            Bucket::Avatars => {
                return Err(ErrorKind::InternalServerError
                    .with_message(
                        "Avatar objects are not reclaimed through the blob store's blob purge",
                    )
                    .with_context(format!("bucket: {bucket}")));
            }
        }
        Ok(())
    }
}

/// Wraps a storage-key parse failure as an internal error.
fn invalid_key(err: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Invalid blob storage key")
        .with_context(err.to_string())
}

/// Maps an analysis (de)serialization failure to an internal error.
fn analysis_serde_error(error: &serde_json::Error) -> Error<'static> {
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
