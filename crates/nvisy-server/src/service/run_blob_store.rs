//! Pipeline-run blob I/O.
//!
//! [`RunBlobStore`] reads and writes a run's document, redacted output, and audit
//! in the platform's internal object store ([`FilesBucket`](nvisy_nats::object::FilesBucket)
//! and [`AuditBucket`](nvisy_nats::object::AuditBucket)), handling per-workspace
//! encryption and the file-table bookkeeping each one needs. It is distinct from
//! [`ExternalObjectStore`](crate::service::ExternalObjectStore), which bridges external
//! tenant object stores.

use std::io::Cursor;
use std::str::FromStr;

use bytes::Bytes;
use elide_pipeline::file::Document;
use elide_pipeline::{Audit, Engine};
use nvisy_nats::object::{AuditBucket, AuditKey, FileKey, FilesBucket, ObjectBucket};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{
    NewWorkspaceFile, WorkspaceDetection, WorkspaceFile, WorkspacePipeline,
};
use nvisy_postgres::query::WorkspaceFileRepository;
use nvisy_postgres::types::{FileKind, RetentionScope, RetentionSettings};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::handler::{Error, ErrorKind, Result};
use crate::service::Infra;

/// Tracing target for blob-store operations.
const TRACING_TARGET: &str = "nvisy_server::service::run_blob_store";

/// Whether [`RunBlobStore::purge_file`] reclaimed a file's backing object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a Pending purge is not reclamation progress and must not be counted as one"]
pub enum PurgeOutcome {
    /// The object was removed and `purged_at` was stamped.
    Purged,
    /// The object was not reclaimed (store failure, bad key, or unknown bucket);
    /// the row stays soft-deleted with `purged_at` NULL for the reaper to retry.
    Pending,
}

/// Wraps a storage-key parse failure as an internal error.
fn invalid_key(err: impl std::fmt::Display) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Invalid file storage key")
        .with_context(err.to_string())
}

/// Reads and writes a pipeline run's blobs in the internal object store.
///
/// Cloneable and cheap to pass around: it holds the shared [`Infra`] clients
/// (all `Arc`-backed) and takes the per-request database connection as a method
/// argument. Not to be confused with
/// [`ExternalObjectStore`](crate::service::ExternalObjectStore), which bridges *external*
/// tenant object stores; this operates on the platform's own NATS buckets
/// ([`FilesBucket`], [`AuditBucket`]).
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

    /// Tears a stored file down: soft-deletes the row (stopping reads) and tries
    /// to purge its backing object from the bucket the row names.
    ///
    /// Returns whether the object was reclaimed. Removal is best-effort — an
    /// object-store failure, an unparseable key, or an unknown bucket all leave
    /// the row soft-deleted with `purged_at` NULL and return
    /// [`PurgeOutcome::Pending`], so the reaper's reconcile sweep retries it.
    /// `purged_at` is stamped only on a confirmed removal
    /// ([`PurgeOutcome::Purged`]) — the caller observes the row as deleted either
    /// way, and a `Pending` result must not be counted as reclamation progress.
    ///
    /// Runs keep their references to a deleted file as append-only history; a
    /// reader resolving one to a soft-deleted row gets "gone", distinct from a
    /// NULL reference.
    ///
    /// Shared by the file reaper (expiring/reconciling files) and the manual
    /// file-delete handler, so both paths reclaim storage identically.
    pub async fn purge_file(
        &self,
        conn: &mut PgConn,
        file_id: Uuid,
        storage_path: &str,
        storage_bucket: &str,
    ) -> Result<PurgeOutcome> {
        conn.delete_workspace_file(file_id).await?;

        if let Err(err) = self.delete_object(storage_bucket, storage_path).await {
            tracing::error!(
                target: TRACING_TARGET,
                file_id = %file_id,
                error = %err,
                "Failed to purge file object; left for the reaper to retry",
            );
            return Ok(PurgeOutcome::Pending);
        }

        conn.mark_file_purged(file_id).await?;
        Ok(PurgeOutcome::Purged)
    }

    /// Removes an object from whichever internal bucket its row names. An
    /// unparseable storage key or an unknown bucket is an error, not a silent
    /// success: the object was not reclaimed, so the row stays pending.
    async fn delete_object(&self, bucket: &str, storage_path: &str) -> Result<()> {
        match bucket {
            b if b == FilesBucket::NAME => {
                let key = FileKey::from_str(storage_path).map_err(invalid_key)?;
                self.infra
                    .nats
                    .object_store::<FilesBucket>()
                    .await?
                    .delete(&key)
                    .await?;
            }
            b if b == AuditBucket::NAME => {
                let key = AuditKey::from_str(storage_path).map_err(invalid_key)?;
                self.infra
                    .nats
                    .object_store::<AuditBucket>()
                    .await?
                    .delete(&key)
                    .await?;
            }
            other => {
                return Err(ErrorKind::InternalServerError
                    .with_message("File references an unknown storage bucket")
                    .with_context(format!("bucket: {other}")));
            }
        }
        Ok(())
    }

    /// Reads a workspace file's bytes from object storage and builds an engine
    /// [`Document`], stamping the run's id as the correlation id.
    pub async fn build_document(
        &self,
        file: &WorkspaceFile,
        correlation_id: Uuid,
    ) -> Result<Document> {
        let store = self.infra.nats.object_store::<FilesBucket>().await?;
        let key = FileKey::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(err.to_string())
        })?;

        let data = store.get(&key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("File content is missing from storage")
        })?;
        let mut reader = data.into_reader();
        let mut ciphertext = Vec::new();
        reader.read_to_end(&mut ciphertext).await.map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to read file content")
                .with_context(err.to_string())
        })?;

        let bytes = self
            .infra
            .crypto
            .decrypt(file.workspace_id, &ciphertext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decrypt file content")
                    .with_context(err.to_string())
            })?;

        Ok(Document::new(bytes, file.file_extension.clone()).with_correlation_id(correlation_id))
    }

    /// Encrypts the analysis, writes it to the audit bucket, and builds the
    /// `audit`-kind [`WorkspaceFile`] row that will point at it — but does not
    /// insert the row.
    ///
    /// The analysis is the map of detected PII, so it is encrypted with the
    /// workspace key before it leaves the process. Modeling it as a file lets
    /// data-retention expire it with the same `expires_at` sweep as documents;
    /// its bytes live in the audit bucket, not the files bucket.
    ///
    /// The object write is not transactional, so it is kept out of the caller's
    /// database transaction: the bytes are written first, then the returned row is
    /// inserted (with the run's other writes) atomically. A rollback therefore
    /// leaves at worst an orphan object in the bucket, never a file row that points
    /// at bytes that were never written; the caller reclaims that orphan via
    /// [`discard_staged_object`](Self::discard_staged_object).
    pub async fn stage_analyzed_document(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        account_id: Uuid,
        analyzed: &Audit,
    ) -> Result<NewWorkspaceFile> {
        let workspace_id = pipeline.workspace_id;
        let plaintext = serde_json::to_vec(analyzed).map_err(analysis_serde_error)?;
        let hash = Sha256::digest(&plaintext).to_vec();
        let size = plaintext.len() as i64;
        let ciphertext = self
            .infra
            .crypto
            .encrypt(workspace_id, &plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt analysis")
                    .with_context(err.to_string())
            })?;

        let store = self.infra.nats.object_store::<AuditBucket>().await?;
        let key = AuditKey::generate(workspace_id);
        store.put(&key, Cursor::new(ciphertext)).await?;

        // Retention expiry for the audit scope (workspace baseline, pipeline
        // override if set).
        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::AuditLogs, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        Ok(NewWorkspaceFile {
            workspace_id,
            account_id,
            display_name: Some("analysis.audit".to_owned()),
            original_filename: Some("analysis.audit".to_owned()),
            file_extension: Some("json".to_owned()),
            file_kind: Some(FileKind::Audit),
            file_size_bytes: size,
            file_hash_sha256: hash,
            storage_path: key.to_string(),
            storage_bucket: store.bucket().to_owned(),
            expires_at: expires_at.map(Into::into),
            ..Default::default()
        })
    }

    /// Serializes a detection's enrichment intermediates (OCR layout, transcript),
    /// encrypts them with the workspace key, writes them to the audit bucket, and
    /// builds the `intermediate`-kind [`WorkspaceFile`] row that will point at it —
    /// without inserting the row (staged like the analysis, reclaimed on rollback).
    ///
    /// The intermediates carry document content, so they are encrypted at rest and
    /// governed by their own retention scope, resolved here (workspace baseline,
    /// pipeline override if set).
    pub async fn stage_intermediates<T: Serialize>(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        account_id: Uuid,
        artifacts: &T,
    ) -> Result<NewWorkspaceFile> {
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

        let store = self.infra.nats.object_store::<AuditBucket>().await?;
        let key = AuditKey::generate(workspace_id);
        store.put(&key, Cursor::new(ciphertext)).await?;

        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::Intermediates, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        Ok(NewWorkspaceFile {
            workspace_id,
            account_id,
            display_name: Some("analysis.intermediates".to_owned()),
            original_filename: Some("analysis.intermediates".to_owned()),
            file_extension: Some("json".to_owned()),
            file_kind: Some(FileKind::Intermediate),
            file_size_bytes: size,
            file_hash_sha256: hash,
            storage_path: key.to_string(),
            storage_bucket: store.bucket().to_owned(),
            expires_at: expires_at.map(Into::into),
            ..Default::default()
        })
    }

    /// Deletes a staged object whose file row was never committed.
    ///
    /// The `stage_*` methods write an object before its `workspace_files` row; if
    /// the committing transaction rolls back, the object has no row and the
    /// row-driven reaper can never find it. The caller invokes this on that path
    /// so the orphan is removed immediately instead of accumulating. It deletes
    /// from whichever bucket the staged row names, so it reclaims a staged audit,
    /// review audit, or redacted document alike. Best effort: a failure here only
    /// leaves the object for a later manual sweep, so callers log rather than
    /// propagate.
    pub async fn discard_staged_object(&self, staged: &NewWorkspaceFile) -> Result<()> {
        self.delete_object(&staged.storage_bucket, &staged.storage_path)
            .await
    }

    /// Resolves the `workspace_files` row holding a detection's analysis blob.
    ///
    /// This is the only connection-bound step of loading an analysis, so a caller
    /// can resolve the row under a connection, release it, and then load the
    /// object with [`load_audit`](Self::load_audit) — keeping the pooled
    /// connection off the object-store round-trip.
    pub async fn resolve_audit_file(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        detection: &WorkspaceDetection,
    ) -> Result<WorkspaceFile> {
        // A NULL reference means the detection never produced an analysis; a
        // reference to a now-deleted file means it did, but the analysis has been
        // removed. These are distinct states, so they map to distinct responses.
        let audit_file_id = detection.audit_file_id.ok_or_else(|| {
            ErrorKind::Conflict
                .with_message("Detection has no analysis yet")
                .with_resource("detection")
        })?;
        conn.find_file_in_workspace(workspace_id, audit_file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound
                    .with_message("The analysis for this detection has been deleted")
                    .with_resource("detection")
            })
    }

    /// Loads and decodes a detection's analysis blob from its already-resolved
    /// audit file row. Holds no database connection: only object-store I/O and
    /// decryption, so a caller can run it after releasing its connection.
    ///
    /// The `engine` reconstructs the audit's report from its serialized form: an
    /// [`Audit`] serializes but does not `Deserialize`, since its report tags each
    /// entity group by modality name and only the engine's registry can map those
    /// back to concrete types.
    pub async fn load_audit(
        &self,
        engine: &Engine,
        workspace_id: Uuid,
        audit_file: &WorkspaceFile,
    ) -> Result<Audit> {
        let key = AuditKey::from_str(&audit_file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid audit storage key")
                .with_context(err.to_string())
        })?;

        let store = self.infra.nats.object_store::<AuditBucket>().await?;
        let data = store.get(&key).await?.ok_or_else(|| {
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

    /// Resolves the `workspace_files` row holding a detection's enrichment
    /// intermediates.
    ///
    /// The connection-bound step; pair with [`load_intermediates`](Self::load_intermediates)
    /// to release the connection before the object-store round-trip. A detection
    /// whose modality produced no enrichment (text, tabular) has none — a `None`
    /// reference maps to a 404, distinct from a reference to a since-deleted file.
    pub async fn resolve_intermediates_file(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        detection: &WorkspaceDetection,
    ) -> Result<WorkspaceFile> {
        let file_id = detection.intermediates_file_id.ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Detection has no enrichment intermediates")
                .with_resource("detection")
        })?;
        conn.find_file_in_workspace(workspace_id, file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound
                    .with_message("The intermediates for this detection have been deleted")
                    .with_resource("detection")
            })
    }

    /// Loads a detection's enrichment intermediates from its already-resolved file
    /// row, as the JSON value they were stored as. Holds no database connection:
    /// only object-store I/O and decryption.
    ///
    /// Served to the client verbatim (the OCR layout / transcript), so it is
    /// returned as an opaque [`serde_json::Value`] rather than reconstructed into
    /// the engine's artifact types.
    pub async fn load_intermediates(
        &self,
        workspace_id: Uuid,
        intermediates_file: &WorkspaceFile,
    ) -> Result<serde_json::Value> {
        let key = AuditKey::from_str(&intermediates_file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid intermediates storage key")
                .with_context(err.to_string())
        })?;

        let store = self.infra.nats.object_store::<AuditBucket>().await?;
        let data = store.get(&key).await?.ok_or_else(|| {
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
        serde_json::from_slice(&plaintext).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to decode intermediates")
                .with_context(err.to_string())
        })
    }

    /// Encrypts a redaction's review audit, writes it to the audit bucket, and
    /// builds the `review`-kind [`WorkspaceFile`] row that will point at it —
    /// without inserting the row.
    ///
    /// The review audit is the post-redaction [`Audit`]: the detection's analysis
    /// with the reviewer's edits applied and the redaction outcome recorded per
    /// entity. It is the redaction's counterpart to
    /// [`stage_analyzed_document`](Self::stage_analyzed_document) — same staged
    /// object-then-row protocol, reclaimed on rollback via
    /// [`discard_staged_object`](Self::discard_staged_object) — but a distinct
    /// file kind so it is never confused with the immutable detection audit.
    pub async fn stage_review_audit(
        &self,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        account_id: Uuid,
        reviewed: &Audit,
    ) -> Result<NewWorkspaceFile> {
        let workspace_id = pipeline.workspace_id;
        let plaintext = serde_json::to_vec(reviewed).map_err(analysis_serde_error)?;
        let hash = Sha256::digest(&plaintext).to_vec();
        let size = plaintext.len() as i64;
        let ciphertext = self
            .infra
            .crypto
            .encrypt(workspace_id, &plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt review audit")
                    .with_context(err.to_string())
            })?;

        let store = self.infra.nats.object_store::<AuditBucket>().await?;
        let key = AuditKey::generate(workspace_id);
        store.put(&key, Cursor::new(ciphertext)).await?;

        // A review audit shares the audit-logs retention scope with the detection
        // audit (workspace baseline, pipeline override if set).
        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::AuditLogs, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        Ok(NewWorkspaceFile {
            workspace_id,
            account_id,
            display_name: Some("review.audit".to_owned()),
            original_filename: Some("review.audit".to_owned()),
            file_extension: Some("json".to_owned()),
            file_kind: Some(FileKind::Review),
            file_size_bytes: size,
            file_hash_sha256: hash,
            storage_path: key.to_string(),
            storage_bucket: store.bucket().to_owned(),
            expires_at: expires_at.map(Into::into),
            ..Default::default()
        })
    }

    /// Encrypts redacted bytes, writes them to the files bucket, and builds the
    /// `redacted`-kind [`WorkspaceFile`] row that will point at them — without
    /// inserting the row.
    ///
    /// A redaction commits its output row, review-audit row, and redaction row
    /// together in one transaction, so the output file is staged (object written,
    /// row returned for the caller to insert) rather than inserted here, and is
    /// reclaimed on rollback via
    /// [`discard_staged_object`](Self::discard_staged_object). The redacted file
    /// is a first-class file (a sibling of the source), downloadable
    /// through the normal file endpoints.
    pub async fn stage_redacted_file(
        &self,
        source: &WorkspaceFile,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        account_id: Uuid,
        bytes: Bytes,
    ) -> Result<NewWorkspaceFile> {
        let plaintext_size = bytes.len() as i64;
        let plaintext_hash = Sha256::digest(&bytes).to_vec();
        let ciphertext = self
            .infra
            .crypto
            .encrypt(source.workspace_id, &bytes)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt redacted file")
                    .with_context(err.to_string())
            })?;

        let store = self.infra.nats.object_store::<FilesBucket>().await?;
        let key = FileKey::generate(source.workspace_id);
        store.put(&key, Cursor::new(ciphertext)).await?;

        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::RedactedDocuments, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        let redacted_name = redacted_display_name(&source.display_name, &source.file_extension);
        Ok(NewWorkspaceFile {
            workspace_id: source.workspace_id,
            account_id,
            parent_id: Some(source.id),
            display_name: Some(redacted_name),
            original_filename: Some(source.original_filename.clone()),
            file_extension: Some(source.file_extension.clone()),
            file_kind: Some(FileKind::Redacted),
            file_size_bytes: plaintext_size,
            file_hash_sha256: plaintext_hash,
            storage_path: key.to_string(),
            storage_bucket: store.bucket().to_owned(),
            expires_at: expires_at.map(Into::into),
            ..Default::default()
        })
    }

    /// Resolves the `workspace_files` row holding a redaction's review audit blob.
    ///
    /// The connection-bound step of loading a review audit; pair with
    /// [`load_audit`](Self::load_audit) to release the connection before the
    /// object-store round-trip. Errors if the redaction has no review audit (409)
    /// or it has since been deleted (404).
    pub async fn resolve_review_file(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        review_file_id: Option<Uuid>,
    ) -> Result<WorkspaceFile> {
        let review_file_id = review_file_id.ok_or_else(|| {
            ErrorKind::Conflict
                .with_message("Redaction has no review audit")
                .with_resource("redaction")
        })?;
        conn.find_file_in_workspace(workspace_id, review_file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound
                    .with_message("The review audit for this redaction has been deleted")
                    .with_resource("redaction")
            })
    }
}

/// Maps an analysis (de)serialization failure to an internal error.
fn analysis_serde_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process analysis")
        .with_context(error.to_string())
}

/// Builds the redacted file's display name by inserting a `redacted` marker
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
