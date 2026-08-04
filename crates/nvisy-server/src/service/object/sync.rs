//! Connection sync service: moves objects between a workspace's external
//! object-store connection and the internal NATS file store.
//!
//! Import pulls an object from the customer's connection and stores it as a
//! [`WorkspaceFile`]; export pushes a stored file back out to the connection.
//! Both directions stream end to end and keep files encrypted at rest in NATS.

use std::path::Path as StdPath;
use std::str::FromStr;

use nvisy_nats::NatsClient;
use nvisy_nats::object::{FileKey, FilesBucket};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{
    NewWorkspaceFile, UpdateWorkspaceConnectionRun, WorkspaceConnection, WorkspaceFile,
};
use nvisy_postgres::query::{WorkspaceConnectionRunRepository, WorkspaceFileRepository};
use nvisy_postgres::types::FileSource;
use uuid::Uuid;

use super::ObjectService;
use super::bridge::{reader_to_stream, stream_to_reader};
use crate::handler::{ErrorKind, Result};
use crate::service::{CryptoService, HashingReader, Measurements};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::object::sync";

/// Moves objects between an external connection and the internal file store.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ConnectionSyncService {
    postgres: PgClient,
    nats: NatsClient,
    crypto: CryptoService,
    object: ObjectService,
}

impl ConnectionSyncService {
    /// Creates a new [`ConnectionSyncService`].
    pub fn new(
        postgres: PgClient,
        nats: NatsClient,
        crypto: CryptoService,
        object: ObjectService,
    ) -> Self {
        Self {
            postgres,
            nats,
            crypto,
            object,
        }
    }

    /// Imports a single object from the connection into the workspace file store.
    ///
    /// Streams the object's bytes from the external store, encrypts them with
    /// the workspace key, and writes them to the NATS files bucket, then records
    /// a [`WorkspaceFile`] with [`FileSource::Imported`]. `remote_key` is
    /// resolved relative to the connection's configured root path.
    #[tracing::instrument(
        name = "sync.import_object",
        skip_all,
        fields(connection_id = %connection.id, key = %remote_key),
    )]
    pub async fn import_object(
        &self,
        connection: &WorkspaceConnection,
        credentials: serde_json::Value,
        account_id: Uuid,
        remote_key: &str,
    ) -> Result<WorkspaceFile> {
        tracing::debug!(target: TRACING_TARGET, "Importing object from connection");

        let client = self
            .object
            .connect(&connection.provider, credentials)
            .await?;

        // Stream external bytes -> hash+measure -> encrypt -> NATS files bucket.
        let source = client.get_stream(remote_key).await?;
        let (measured, measurements) = HashingReader::new(stream_to_reader(source));
        let ciphertext = Box::pin(
            self.crypto
                .encrypt_reader(connection.workspace_id, measured),
        );

        let store = self.nats.object_store::<FilesBucket>().await?;
        let file_key = FileKey::generate(connection.workspace_id);
        store.put(&file_key, ciphertext).await?;

        // The object now exists in storage; if recording it in the database
        // fails, delete it so no orphaned object is left behind.
        let file = match self
            .record_imported_file(connection, account_id, remote_key, &file_key, &measurements)
            .await
        {
            Ok(file) => file,
            Err(err) => {
                if let Err(cleanup) = store.delete(&file_key).await {
                    tracing::error!(
                        target: TRACING_TARGET,
                        error = %cleanup,
                        "Failed to delete orphaned object after import bookkeeping failure",
                    );
                }
                return Err(err);
            }
        };

        tracing::info!(target: TRACING_TARGET, file_id = %file.id, "Object imported");
        Ok(file)
    }

    /// Inserts the [`WorkspaceFile`] row for a freshly imported object.
    async fn record_imported_file(
        &self,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        remote_key: &str,
        file_key: &FileKey,
        measurements: &Measurements,
    ) -> Result<WorkspaceFile> {
        let store = self.nats.object_store::<FilesBucket>().await?;
        let mut conn = self.postgres.get_connection().await?;
        let filename = object_basename(remote_key);
        let extension = object_extension(remote_key);
        let new_file = NewWorkspaceFile {
            workspace_id: connection.workspace_id,
            account_id,
            display_name: Some(filename.clone()),
            original_filename: Some(filename),
            file_extension: extension,
            source: Some(FileSource::Imported),
            file_size_bytes: measurements.bytes() as i64,
            file_hash_sha256: measurements.sha256().to_vec(),
            storage_path: file_key.to_string(),
            storage_bucket: store.bucket().to_owned(),
            ..Default::default()
        };
        Ok(conn.create_workspace_file(new_file).await?)
    }

    /// Exports a stored workspace file back out to the connection at `remote_key`.
    ///
    /// Streams the file's bytes from the NATS files bucket, decrypts them, and
    /// uploads them to the external store. `remote_key` is resolved relative to
    /// the connection's configured root path.
    #[tracing::instrument(
        name = "sync.export_file",
        skip_all,
        fields(connection_id = %connection.id, file_id = %file.id, key = %remote_key),
    )]
    pub async fn export_file(
        &self,
        connection: &WorkspaceConnection,
        credentials: serde_json::Value,
        file: &WorkspaceFile,
        remote_key: &str,
    ) -> Result<()> {
        tracing::debug!(target: TRACING_TARGET, "Exporting file to connection");

        let client = self
            .object
            .connect(&connection.provider, credentials)
            .await?;

        let store = self.nats.object_store::<FilesBucket>().await?;
        let file_key = FileKey::from_str(&file.storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid file storage path")
                .with_context(err.to_string())
        })?;
        let stored = store.get(&file_key).await?.ok_or_else(|| {
            ErrorKind::InternalServerError.with_message("File content is missing from storage")
        })?;

        // NATS ciphertext reader -> decrypt -> external multipart upload.
        let plaintext = Box::pin(
            self.crypto
                .decrypt_reader(connection.workspace_id, stored.into_reader()),
        );
        let body = reader_to_stream(plaintext);
        client
            .put_multipart(remote_key, file.mime_type.as_deref(), body)
            .await?;

        tracing::info!(target: TRACING_TARGET, "File exported");
        Ok(())
    }

    /// Records the outcome of a background sync run: completes it (one object
    /// transferred) on success, or marks it failed with a safe error message.
    ///
    /// Errors updating the run are logged rather than propagated, since this
    /// runs after the response has already been sent.
    pub async fn finish_run(&self, run_id: Uuid, result: Result<()>) {
        let mut conn = match self.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    %run_id, error = %err,
                    "Failed to record sync outcome: no connection",
                );
                return;
            }
        };

        let outcome = match result {
            Ok(()) => {
                // A sync transfers exactly one object today (single-key import
                // or single-file export).
                let updates = UpdateWorkspaceConnectionRun {
                    records_synced: Some(1),
                    ..Default::default()
                };
                if let Err(err) = conn.update_workspace_connection_run(run_id, updates).await {
                    tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record sync count");
                }
                conn.complete_workspace_connection_run(run_id).await
            }
            Err(err) => {
                // Log the full error (may include backend URLs/details) but
                // persist only the safe summary: the stored message is exposed
                // to clients via the sync's `error_message`.
                tracing::warn!(target: TRACING_TARGET, %run_id, error = %err, "Sync failed");
                let safe_message = err.message().unwrap_or("Sync failed").to_owned();
                conn.fail_workspace_connection_run(run_id, &safe_message)
                    .await
            }
        };

        if let Err(err) = outcome {
            tracing::error!(
                target: TRACING_TARGET,
                %run_id, error = %err,
                "Failed to finalize sync run",
            );
        }
    }
}

/// The final path segment of a key (its file name), or the whole key when it
/// contains no separator.
fn object_basename(key: &str) -> String {
    StdPath::new(key)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(key)
        .to_string()
}

/// The lowercased extension of a key, if any.
fn object_extension(key: &str) -> Option<String> {
    StdPath::new(key)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::{object_basename, object_extension};

    #[test]
    fn basename_and_extension() {
        assert_eq!(object_basename("incoming/report.pdf"), "report.pdf");
        assert_eq!(
            object_extension("incoming/report.PDF").as_deref(),
            Some("pdf")
        );
        assert_eq!(object_basename("flat"), "flat");
        assert_eq!(object_extension("flat"), None);
    }
}
