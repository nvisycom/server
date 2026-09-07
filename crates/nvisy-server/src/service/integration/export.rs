//! The export path: pushing workspace files out to a connection.
//!
//! [`Exporter`] streams a stored file from the first-party blob store, decrypts
//! it, and uploads it to the connection as a new provider file, recording each
//! export so a later run does not push it again. Stateless beyond its [`Infra`]
//! and connector, so it is cheap to clone.

use std::io;
use std::str::FromStr;

use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use nvisy_postgres::model::{WorkspaceConnection, WorkspaceFile};
use nvisy_postgres::query::WorkspaceFileRepository;
use nvisy_s3::FileKey;
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use super::connector::Connector;
use super::file_source::{ByteStream, FileSource};
use super::naming::{export_key, mime_from_extension};
use crate::handler::{ErrorKind, Result};
use crate::service::{ConnectionConfig, Infra};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::sync";

/// Object-store key prefix for scheduled redacted exports, keeping them apart
/// from imported originals.
const EXPORT_PREFIX_REDACTED: &str = "redacted/";

/// Object-store key prefix for manually exported files.
const EXPORT_PREFIX_SELECTED: &str = "exports/";

/// Exports workspace files out to a connection.
#[derive(Clone)]
pub(super) struct Exporter {
    infra: Infra,
    connector: Connector,
}

impl Exporter {
    pub(super) fn new(infra: Infra, connector: Connector) -> Self {
        Self { infra, connector }
    }

    /// Exports a caller-selected set of workspace files to the connection, each
    /// as a new provider file. Ids not found in the workspace are skipped.
    /// Returns the number exported. Backs the manual export endpoint.
    #[tracing::instrument(
        name = "sync.export_selected",
        skip_all,
        fields(connection_id = %connection.id, selected = file_ids.len()),
    )]
    pub(super) async fn export_selected(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
        file_ids: Vec<Uuid>,
    ) -> Result<u64> {
        tracing::debug!(target: TRACING_TARGET, "Exporting selected files to connection");

        let files = {
            let mut conn = self.infra.postgres.get_connection().await?;
            let mut files = Vec::with_capacity(file_ids.len());
            for file_id in file_ids {
                match conn
                    .find_file_in_workspace(connection.workspace_id, file_id)
                    .await?
                {
                    Some(file) => files.push(file),
                    None => tracing::warn!(
                        target: TRACING_TARGET,
                        %file_id, "Skipping export of file not found in workspace",
                    ),
                }
            }
            files
        };
        let exported = self
            .export_files(connection, config, files, EXPORT_PREFIX_SELECTED)
            .await?;

        tracing::debug!(target: TRACING_TARGET, exported, "Selected export complete");
        Ok(exported)
    }

    /// Exports every redacted output in the connection's workspace that has not
    /// yet been exported to this connection. Backs scheduled export syncs.
    ///
    /// Each redacted file is streamed out and its export recorded so a later run
    /// does not push it again. A single file failing is logged and skipped rather
    /// than aborting the run. Returns the number of files exported.
    #[tracing::instrument(
        name = "sync.export_redacted",
        skip_all,
        fields(connection_id = %connection.id),
    )]
    pub(super) async fn export_redacted(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
    ) -> Result<u64> {
        tracing::debug!(target: TRACING_TARGET, "Exporting redacted outputs to connection");

        let pending = {
            let mut conn = self.infra.postgres.get_connection().await?;
            conn.redacted_files_not_exported(connection.workspace_id, connection.id)
                .await?
        };
        let exported = self
            .export_files(connection, config, pending, EXPORT_PREFIX_REDACTED)
            .await?;

        tracing::debug!(target: TRACING_TARGET, exported, "Redacted export complete");
        Ok(exported)
    }

    /// Exports each file to the connection as a new provider file, recording the
    /// export so it is not pushed again. A file service creates a new file named
    /// after the output; an object store writes it under `object_prefix` so
    /// exports never overwrite imported originals. A single file failing is logged
    /// and skipped rather than aborting the run. Returns the number exported.
    ///
    /// The provider source is connected once for the whole batch (validating the
    /// endpoint and refreshing an OAuth token at most once), not per file. A
    /// connect failure is a batch-wide fault and is propagated, so the run is
    /// recorded as failed rather than silently reporting zero exports.
    async fn export_files(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
        files: Vec<WorkspaceFile>,
        object_prefix: &str,
    ) -> Result<u64> {
        let source = self.connector.file_source(connection, config).await?;
        let object_store = matches!(config, ConnectionConfig::ObjectStore(_));

        let mut exported = 0u64;
        for file in files {
            let remote_key = export_key(&file, object_store, object_prefix);
            match self
                .export_one(source.as_ref(), connection, &file, &remote_key)
                .await
            {
                Ok(()) => exported += 1,
                Err(err) => {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        file_id = %file.id, error = %err,
                        "Skipping file that failed to export",
                    );
                }
            }
        }
        Ok(exported)
    }

    /// Exports one stored workspace file to `remote_key` on an already-connected
    /// `source`.
    ///
    /// Streams the file's bytes from the files store, decrypts them, and uploads
    /// them to the external store. On success the export is recorded so a
    /// scheduled redacted export never re-pushes a file already exported here.
    #[tracing::instrument(
        name = "sync.export_one",
        skip_all,
        fields(connection_id = %connection.id, file_id = %file.id, key = %remote_key),
    )]
    async fn export_one(
        &self,
        source: &dyn FileSource,
        connection: &WorkspaceConnection,
        file: &WorkspaceFile,
        remote_key: &str,
    ) -> Result<()> {
        tracing::debug!(target: TRACING_TARGET, "Exporting file to connection");

        let file_key = FileKey::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(err.to_string())
        })?;
        let stored = self.infra.blobs.get(&file_key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("File content is missing from storage")
        })?;

        // Stored ciphertext reader -> decrypt -> external streaming upload.
        let plaintext = Box::pin(
            self.infra
                .crypto
                .decrypt_reader(connection.workspace_id, stored.into_reader()),
        );
        let body: ByteStream = Box::pin(reader_to_stream(plaintext).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to read stored file for export")
                .with_context(err.to_string())
        }));
        let content_type = mime_from_extension(&file.file_extension);
        source
            .put_stream(remote_key, content_type.as_str(), body)
            .await?;

        // Record the export (upsert) so both manual and scheduled paths dedupe:
        // a file exported here is not re-pushed by a later scheduled export.
        self.record_exported_file(file, connection, remote_key)
            .await?;

        tracing::debug!(target: TRACING_TARGET, "File exported");
        Ok(())
    }

    /// Records that a file was exported to a connection so a scheduled export
    /// does not push it again.
    async fn record_exported_file(
        &self,
        file: &WorkspaceFile,
        connection: &WorkspaceConnection,
        remote_key: &str,
    ) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        conn.record_exported_file(file.id, connection.id, remote_key.to_owned())
            .await?;
        Ok(())
    }
}

/// Adapts the decrypted-file [`AsyncRead`] into a byte stream for the provider
/// upload, surfacing read errors as [`io::Error`]; the caller maps that into its
/// own error type. Lets a stored file be piped blob store → decrypt → provider
/// without buffering the whole body in memory.
fn reader_to_stream<R>(reader: R) -> impl Stream<Item = Result<Bytes, io::Error>> + Unpin + Send
where
    R: AsyncRead + Unpin + Send,
{
    ReaderStream::new(reader)
}
