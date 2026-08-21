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
use elide_pipeline::{Audit, Document};
use nvisy_nats::object::{AuditBucket, AuditKey, FileKey, FilesBucket, ObjectBucket};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{
    NewWorkspaceFile, WorkspaceFile, WorkspacePipeline, WorkspacePipelineRun,
};
use nvisy_postgres::query::WorkspaceFileRepository;
use nvisy_postgres::types::{FileKind, RetentionScope, RetentionSettings};
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

    /// Stores redacted bytes as a new workspace file (the run's output).
    ///
    /// The redacted file is a first-class file — a sibling of the source — so it
    /// is downloadable through the normal file endpoints.
    pub async fn store_redacted_file(
        &self,
        conn: &mut PgConn,
        source: &WorkspaceFile,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        account_id: Uuid,
        bytes: Bytes,
    ) -> Result<WorkspaceFile> {
        // Record the plaintext size and hash before encrypting; storage holds
        // only the ciphertext.
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

        // Retention expiry for the redacted-documents scope (workspace baseline,
        // pipeline override if set).
        let over = pipeline.metadata.or_default().retention;
        let expires_at = workspace_settings
            .resolve(RetentionScope::RedactedDocuments, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        let redacted_name = format!("{}.redacted", source.display_name);
        let new_file = NewWorkspaceFile {
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
        };

        Ok(conn.create_workspace_file(new_file).await?)
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
    /// [`discard_staged_audit`](Self::discard_staged_audit).
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

    /// Deletes a staged audit object whose file row was never committed.
    ///
    /// [`stage_analyzed_document`](Self::stage_analyzed_document) writes the object
    /// before its `workspace_files` row; if the committing transaction rolls back,
    /// the object has no row and the row-driven reaper can never find it. The
    /// caller invokes this on that path so the orphan is removed immediately
    /// instead of accumulating. Best effort: a failure here only leaves the object
    /// for a later manual sweep, so callers log rather than propagate.
    pub async fn discard_staged_audit(&self, staged: &NewWorkspaceFile) -> Result<()> {
        self.delete_object(&staged.storage_bucket, &staged.storage_path)
            .await
    }

    /// Fetches and decrypts a run's stored [`Audit`].
    ///
    /// Errors if the run was never analyzed (409) or its analysis has since been
    /// deleted (404).
    pub async fn load_analyzed_document(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        run: &WorkspacePipelineRun,
    ) -> Result<Audit> {
        // A NULL reference means the run never produced an analysis; a reference
        // to a now-deleted file means it did, but the analysis has been removed.
        // These are distinct states, so they map to distinct responses.
        let audit_file_id = run.audit_file_id.ok_or_else(|| {
            ErrorKind::Conflict
                .with_message("Run has no analysis yet")
                .with_resource("pipeline_run")
        })?;
        let audit_file = conn
            .find_file_in_workspace(workspace_id, audit_file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound
                    .with_message("The analysis for this run has been deleted")
                    .with_resource("pipeline_run")
            })?;
        let key = AuditKey::from_str(&audit_file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid analysis storage key")
                .with_context(err.to_string())
        })?;

        let store = self.infra.nats.object_store::<AuditBucket>().await?;
        let data = store.get(&key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("Analysis is missing from storage")
        })?;
        let mut reader = data.into_reader();
        let mut ciphertext = Vec::new();
        reader.read_to_end(&mut ciphertext).await.map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to read analysis")
                .with_context(err.to_string())
        })?;

        let plaintext = self
            .infra
            .crypto
            .decrypt(workspace_id, &ciphertext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decrypt analysis")
                    .with_context(err.to_string())
            })?;
        serde_json::from_slice(&plaintext).map_err(analysis_serde_error)
    }
}

/// Maps an analysis (de)serialization failure to an internal error.
fn analysis_serde_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process analysis")
        .with_context(error.to_string())
}
