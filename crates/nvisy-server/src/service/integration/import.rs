//! The import path: pulling files from a connection into the workspace store.
//!
//! [`Importer`] streams external bytes → hash → encrypt → first-party blob store,
//! records each imported file and its origin, and (for a whole-listing import)
//! reconciles source deletions. Stateless beyond its [`Infra`] and connector, so
//! it is cheap to clone.

use std::collections::{HashMap, HashSet};
use std::io;

use bytes::Bytes;
use futures::stream::{self, StreamExt};
use futures::{Stream, TryStreamExt};
use nvisy_postgres::model::{
    NewBlob, NewWorkspaceDocument, WorkspaceConnection, WorkspaceDocument,
};
use nvisy_postgres::query::{WorkspaceDocumentRepository, WorkspaceRepository};
use nvisy_postgres::types::{DocumentKind, SyncDeletionPolicy};
use nvisy_s3::{Bucket, DocumentKey};
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;
use uuid::Uuid;

use super::connector::Connector;
use super::file_source::{FileSource, SourceEntry};
use super::naming::{object_basename, object_extension};
use crate::response::Result;
use crate::service::{ConnectionConfig, HashingReader, Infra, Measurements};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::sync";

/// Imports files from a connection into the workspace file store.
#[derive(Clone)]
pub(super) struct Importer {
    infra: Infra,
    connector: Connector,
    /// Maximum objects imported concurrently within a single sync.
    import_concurrency: usize,
}

impl Importer {
    pub(super) fn new(infra: Infra, connector: Connector, import_concurrency: usize) -> Self {
        Self {
            infra,
            connector,
            import_concurrency: import_concurrency.max(1),
        }
    }

    /// Imports every not-yet-imported object from the connection and reconciles
    /// deletions according to the connection's deletion policy.
    ///
    /// Lists the objects under the connection's root path, skips those already
    /// imported (by remote key), and streams the rest into the workspace file
    /// store. A single object failing is logged and skipped rather than aborting
    /// the whole sync. Files whose source object is no longer present are then
    /// reconciled per the connection's policy (removed and desynced, or left
    /// untouched). Returns the number of objects imported.
    #[tracing::instrument(
        name = "sync.import_new",
        skip_all,
        fields(connection_id = %connection.id),
    )]
    pub(super) async fn import_new(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
        deletion_policy: SyncDeletionPolicy,
        account_id: Uuid,
    ) -> Result<u64> {
        tracing::debug!(target: TRACING_TARGET, "Importing new objects from connection");

        let source = self.connector.object_source(config).await?;

        // List the source once; entries are already scoped to the connection's
        // root. The listing is reused both to import new objects and to detect
        // ones that have been deleted. Entries are keyed by the object path.
        let entries = source.list().await?;
        let entries_by_key: HashMap<String, SourceEntry> = entries
            .into_iter()
            .map(|entry| (entry.key.clone(), entry))
            .collect();
        let remote_keys: HashSet<String> = entries_by_key.keys().cloned().collect();

        let already_imported = self.already_imported_keys(connection).await?;

        // Import the not-yet-imported entries.
        let to_import: Vec<SourceEntry> = remote_keys
            .iter()
            .filter(|key| !already_imported.contains(*key))
            .filter_map(|key| entries_by_key.get(key).cloned())
            .collect();
        let imported = self
            .import_entries(&source, connection, account_id, to_import)
            .await?;

        // Reconcile deletions against the full listing (import-all only).
        self.reconcile_deletions(connection, deletion_policy, &remote_keys)
            .await;

        tracing::debug!(target: TRACING_TARGET, imported, "Import sync complete");
        Ok(imported)
    }

    /// Imports a caller-selected set of provider files (the file-service picker),
    /// skipping the full listing. Already-imported ids are filtered out so a
    /// re-selection of the same file is a no-op. Returns the number imported.
    #[tracing::instrument(
        name = "sync.import_selected",
        skip_all,
        fields(connection_id = %connection.id, selected = entries.len()),
    )]
    pub(super) async fn import_selected(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
        account_id: Uuid,
        entries: Vec<SourceEntry>,
    ) -> Result<u64> {
        tracing::debug!(target: TRACING_TARGET, "Importing selected files from connection");

        let source = self.connector.file_source(connection, config).await?;
        let already_imported = self.already_imported_keys(connection).await?;

        // Skip any selection that is already imported (idempotent re-selection).
        let to_import: Vec<SourceEntry> = entries
            .into_iter()
            .filter(|entry| !already_imported.contains(&entry.key))
            .collect();
        let imported = self
            .import_entries(source.as_ref(), connection, account_id, to_import)
            .await?;

        tracing::debug!(target: TRACING_TARGET, imported, "Selected import complete");
        Ok(imported)
    }

    /// The provider keys already imported for `connection`.
    async fn already_imported_keys(
        &self,
        connection: &WorkspaceConnection,
    ) -> Result<HashSet<String>> {
        let mut conn = self.infra.postgres.get_connection().await?;
        Ok(conn
            .imported_keys_for_connection(connection.id)
            .await?
            .into_iter()
            .collect())
    }

    /// Resolves the retention expiry for imported originals once for a whole
    /// import (the same for every file in it).
    async fn resolve_import_expiry(&self, workspace_id: Uuid) -> Result<Option<jiff::Timestamp>> {
        let mut conn = self.infra.postgres.get_connection().await?;
        Ok(conn
            .find_workspace_by_id(workspace_id)
            .await?
            .and_then(|workspace| {
                workspace
                    .settings
                    .or_default()
                    .retention
                    .original_documents
                    .expires_at(jiff::Timestamp::now())
            }))
    }

    /// Runs the fetch → hash → encrypt → store pipeline for `entries` with bounded
    /// concurrency (up to `import_concurrency` at once). A file that fails is
    /// logged and skipped rather than aborting the whole import. Returns the count
    /// actually imported.
    async fn import_entries(
        &self,
        source: &dyn FileSource,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        entries: Vec<SourceEntry>,
    ) -> Result<u64> {
        let expires_at = self.resolve_import_expiry(connection.workspace_id).await?;
        let imported = stream::iter(entries)
            .map(|entry| async move {
                match self
                    .import_one(source, connection, account_id, &entry, expires_at)
                    .await
                {
                    Ok(_) => 1u64,
                    Err(err) => {
                        tracing::warn!(
                            target: TRACING_TARGET,
                            key = %entry.key, error = %err,
                            "Skipping file that failed to import",
                        );
                        0
                    }
                }
            })
            .buffer_unordered(self.import_concurrency)
            .fold(0u64, |total, imported| std::future::ready(total + imported))
            .await;
        Ok(imported)
    }

    /// Removes imported files whose source object no longer exists, per the
    /// connection's [`SyncDeletionPolicy`].
    ///
    /// `Ignore` leaves everything untouched. `Delete` soft-deletes each vanished
    /// file and removes its stored object. Failures on individual files are logged
    /// and skipped so one bad file does not abort reconciliation.
    async fn reconcile_deletions(
        &self,
        connection: &WorkspaceConnection,
        deletion_policy: SyncDeletionPolicy,
        remote_keys: &HashSet<String>,
    ) {
        if deletion_policy == SyncDeletionPolicy::Ignore {
            return;
        }

        let imported = {
            let mut conn = match self.infra.postgres.get_connection().await {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to list imported files for reconciliation");
                    return;
                }
            };
            match conn.imported_documents_for_connection(connection.id).await {
                Ok(files) => files,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to list imported files for reconciliation");
                    return;
                }
            }
        };

        let mut removed = 0u64;
        for file in imported {
            if remote_keys.contains(&file.source_key) {
                continue;
            }
            if let Err(err) = self.remove_vanished_file(file.document_id).await {
                tracing::warn!(
                    target: TRACING_TARGET,
                    key = %file.source_key, error = %err,
                    "Failed to reconcile deleted source object",
                );
            } else {
                removed += 1;
            }
        }

        if removed > 0 {
            tracing::info!(target: TRACING_TARGET, removed, "Reconciled deleted source objects");
        }
    }

    /// Deletes a single document whose source object is gone.
    ///
    /// The document is soft-deleted, which drops its blob reference; the backing
    /// object is not removed here, since its bytes may be shared with another
    /// document. The reaper reclaims the blob once its last reference is gone and
    /// its retention window has passed.
    async fn remove_vanished_file(&self, document_id: Uuid) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        conn.delete_workspace_document(document_id).await?;
        Ok(())
    }

    /// Imports one object at `entry.key` using an already-connected `source`.
    ///
    /// Streams the object's bytes from the external store, encrypts them with the
    /// workspace key, writes them to the files store, and records an original-kind
    /// file along with its import origin (connection and remote key). If recording
    /// fails, the just-written object is deleted so none is orphaned.
    async fn import_one(
        &self,
        source: &dyn FileSource,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        entry: &SourceEntry,
        expires_at: Option<jiff::Timestamp>,
    ) -> Result<WorkspaceDocument> {
        // Stream external bytes -> hash+measure -> encrypt -> files store.
        let bytes = source.get_stream(&entry.key).await?;
        let (measured, measurements) = HashingReader::new(stream_to_reader(bytes));
        let ciphertext = Box::pin(
            self.infra
                .crypto
                .encrypt_reader(connection.workspace_id, measured),
        );

        let file_key = DocumentKey::generate(connection.workspace_id);
        self.infra.blobs.put(&file_key, ciphertext).await?;

        // The object now exists in storage; if recording it in the database
        // fails, delete it so no orphaned object is left behind.
        match self
            .record_imported_file(
                connection,
                account_id,
                entry,
                &file_key,
                &measurements,
                expires_at,
            )
            .await
        {
            Ok(file) => Ok(file),
            Err(err) => {
                if let Err(cleanup) = self.infra.blobs.delete(&file_key).await {
                    tracing::error!(
                        target: TRACING_TARGET,
                        error = %cleanup,
                        "Failed to delete orphaned object after import bookkeeping failure",
                    );
                }
                Err(err)
            }
        }
    }

    /// Inserts the document and blob rows for a freshly imported object. The
    /// retention `expires_at` is resolved once per sync by the caller and passed
    /// in, so this only holds a pooled connection for the insert itself.
    async fn record_imported_file(
        &self,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        entry: &SourceEntry,
        file_key: &DocumentKey,
        measurements: &Measurements,
        expires_at: Option<jiff::Timestamp>,
    ) -> Result<WorkspaceDocument> {
        let mut conn = self.infra.postgres.get_connection().await?;
        // The display name and extension come from the entry's name; the
        // import-origin key (recorded below) is the entry's provider key, which
        // for a file service is an opaque id rather than a path.
        let filename = object_basename(&entry.name);
        let extension = object_extension(&entry.name);

        // The content-addressed fields (size, hash) and retention live on the blob;
        // the human-facing name, extension, and creator live on the document.
        let new_blob = NewBlob {
            workspace_id: connection.workspace_id,
            content_hash: measurements.sha256().to_vec(),
            file_size_bytes: measurements.bytes() as i64,
            storage_path: file_key.to_string(),
            storage_bucket: Bucket::Documents.name().to_owned(),
            expires_at: expires_at.map(Into::into),
        };
        let new_document = NewWorkspaceDocument {
            workspace_id: connection.workspace_id,
            account_id,
            blob_id: Uuid::nil(),
            kind: Some(DocumentKind::Original),
            display_name: Some(filename.clone()),
            original_filename: Some(filename),
            file_extension: extension,
            metadata: None,
        };
        Ok(conn
            .record_imported_document(new_document, new_blob, connection.id, entry.key.clone())
            .await?)
    }
}

/// Adapts a provider byte stream into an [`AsyncRead`], so an imported object can
/// be piped source → hash → encrypt → blob store without buffering the whole body
/// in memory. Stream errors surface as [`io::Error`], as [`StreamReader`]
/// requires; the stream's error type only needs to be convertible into a boxed
/// error.
fn stream_to_reader<S, E>(stream: S) -> impl AsyncRead + Unpin + Send
where
    S: Stream<Item = Result<Bytes, E>> + Unpin + Send,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    StreamReader::new(stream.map_err(io::Error::other))
}
