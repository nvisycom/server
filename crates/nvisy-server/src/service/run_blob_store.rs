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
use nvisy_engine::{Audit, Document};
use nvisy_nats::object::{AuditBucket, AuditKey, FileKey, FilesBucket};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{
    NewWorkspaceFile, WorkspaceFile, WorkspacePipeline, WorkspacePipelineRun,
};
use nvisy_postgres::query::WorkspaceFileRepository;
use nvisy_postgres::types::{FileKind, RetentionOverride, RetentionScope, RetentionSettings};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::handler::{Error, ErrorKind, Result};
use crate::service::Infra;

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
        let over = RetentionOverride::from_pipeline_metadata(&pipeline.metadata);
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

    /// Encrypts an [`Audit`], stores it in the audit bucket, and records it as an
    /// `audit`-kind [`WorkspaceFile`], returning that file's id.
    ///
    /// The analysis is the map of detected PII, so it is encrypted with the
    /// workspace key before it leaves the process. Modeling it as a file lets
    /// data-retention expire it with the same `expires_at` sweep as documents;
    /// its bytes live in the audit bucket, not the files bucket.
    pub async fn store_analyzed_document(
        &self,
        conn: &mut PgConn,
        pipeline: &WorkspacePipeline,
        workspace_settings: &RetentionSettings,
        account_id: Uuid,
        analyzed: &Audit,
    ) -> Result<Uuid> {
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
        let over = RetentionOverride::from_pipeline_metadata(&pipeline.metadata);
        let expires_at = workspace_settings
            .resolve(RetentionScope::AuditLogs, over.as_ref())
            .expires_at(jiff::Timestamp::now());

        let new_file = NewWorkspaceFile {
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
        };
        let file = conn.create_workspace_file(new_file).await?;
        Ok(file.id)
    }

    /// Fetches and decrypts a run's stored [`Audit`].
    ///
    /// Errors if the run has not been analyzed yet or the stored object is
    /// missing.
    pub async fn load_analyzed_document(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        run: &WorkspacePipelineRun,
    ) -> Result<Audit> {
        let audit_file_id = run.audit_file_id.ok_or_else(|| {
            ErrorKind::Conflict
                .with_message("Run has no analysis yet")
                .with_resource("pipeline_run")
        })?;
        let audit_file = conn
            .find_file_in_workspace(workspace_id, audit_file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::InternalServerError.with_message("Analysis is missing from storage")
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
