//! Connection sync service: moves objects between a workspace's external
//! object-store connection and the internal NATS file store.
//!
//! Import pulls an object from the customer's connection and stores it as a
//! [`WorkspaceFile`]; export pushes a stored file back out to the connection.
//! Both directions stream end to end and keep files encrypted at rest in NATS.

use std::collections::{HashMap, HashSet};
use std::path::Path as StdPath;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use nvisy_nats::NatsClient;
use nvisy_nats::object::{FileKey, FilesBucket};
use nvisy_object::client::ObjectStoreClient;
use nvisy_object::providers::ConnectionConfig;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewWorkspaceFile, WorkspaceConnection, WorkspaceFile};
use nvisy_postgres::query::{WorkspaceConnectionRunRepository, WorkspaceFileRepository};
use nvisy_postgres::types::{FileSource, SyncDeletionPolicy};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::ObjectService;
use super::bridge::{reader_to_stream, stream_to_reader};
use crate::handler::{ErrorKind, Result};
use crate::service::{CryptoService, HashingReader, Measurements, WebhookEmitter};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::object::sync";

/// Maximum wall-clock time for a single sync transfer before it is failed.
const SYNC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// How a transfer ended: either it ran to a result/timeout, or it was cancelled.
enum Outcome {
    Finished(Result<u64>),
    Cancelled,
}

/// Moves objects between an external connection and the internal file store.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ConnectionSyncService {
    postgres: PgClient,
    nats: NatsClient,
    crypto: CryptoService,
    object: ObjectService,
    webhook: WebhookEmitter,
    // Cancellation tokens for transfers running in this process, keyed by run id.
    // Cancellation is best-effort and process-local: it aborts a transfer only on
    // the instance running it. Cross-instance runs are stopped by the DB status
    // flip plus the status-guarded finalizers.
    running: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl ConnectionSyncService {
    /// Creates a new [`ConnectionSyncService`].
    pub fn new(
        postgres: PgClient,
        nats: NatsClient,
        crypto: CryptoService,
        object: ObjectService,
        webhook: WebhookEmitter,
    ) -> Self {
        Self {
            postgres,
            nats,
            crypto,
            object,
            webhook,
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Signals a locally-running transfer to stop, if this instance is running
    /// it. Returns whether a token was found and cancelled. This is best-effort:
    /// a run executing on another instance is not reached here and relies on the
    /// DB status flip instead.
    pub fn cancel_local(&self, run_id: Uuid) -> bool {
        let guard = self.running.lock().expect("sync cancel registry poisoned");
        if let Some(token) = guard.get(&run_id) {
            token.cancel();
            true
        } else {
            false
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
    pub async fn import_new(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
        account_id: Uuid,
    ) -> Result<u64> {
        tracing::debug!(target: TRACING_TARGET, "Importing new objects from connection");

        let client = self.object.connect(config).await?;

        // List the source once; keys are already scoped to the connection's root
        // path by the provider's PrefixStore. The listing is reused both to
        // import new objects and to detect ones that have been deleted.
        let objects = client.list("").await?;
        let remote_keys: HashSet<String> = objects
            .iter()
            .map(|o| o.location.as_ref().to_owned())
            .collect();

        let mut conn = self.postgres.get_connection().await?;
        let already_imported: HashSet<String> = conn
            .imported_keys_for_connection(connection.id)
            .await?
            .into_iter()
            .collect();
        drop(conn);

        let mut imported = 0u64;
        for key in &remote_keys {
            if already_imported.contains(key) {
                continue;
            }
            match self.import_one(&client, connection, account_id, key).await {
                Ok(_) => imported += 1,
                Err(err) => {
                    // Tolerate a single bad object; the rest of the sync proceeds.
                    tracing::warn!(
                        target: TRACING_TARGET,
                        key = %key, error = %err,
                        "Skipping object that failed to import",
                    );
                }
            }
        }

        self.reconcile_deletions(connection, account_id, &remote_keys)
            .await;

        tracing::debug!(target: TRACING_TARGET, imported, "Import sync complete");
        Ok(imported)
    }

    /// Removes imported files whose source object no longer exists, per the
    /// connection's [`SyncDeletionPolicy`].
    ///
    /// `Ignore` leaves everything untouched. `Delete` soft-deletes each vanished
    /// file and removes its stored object, emitting a single `ConnectionDesynced`
    /// event when anything changed. Failures on individual files are logged and
    /// skipped so one bad file does not abort reconciliation.
    async fn reconcile_deletions(
        &self,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        remote_keys: &HashSet<String>,
    ) {
        if connection.deletion_policy == SyncDeletionPolicy::Ignore {
            return;
        }

        let imported = {
            let mut conn = match self.postgres.get_connection().await {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to list imported files for reconciliation");
                    return;
                }
            };
            match conn.imported_files_for_connection(connection.id).await {
                Ok(files) => files,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to list imported files for reconciliation");
                    return;
                }
            }
        };

        let mut removed = 0u64;
        for (source_key, file_id, storage_path) in imported {
            if remote_keys.contains(&source_key) {
                continue;
            }
            if let Err(err) = self.remove_vanished_file(file_id, &storage_path).await {
                tracing::warn!(
                    target: TRACING_TARGET,
                    key = %source_key, error = %err,
                    "Failed to reconcile deleted source object",
                );
            } else {
                removed += 1;
            }
        }

        if removed > 0 {
            tracing::info!(target: TRACING_TARGET, removed, "Reconciled deleted source objects");
            let _ = self
                .webhook
                .emit_connection_desynced(
                    connection.workspace_id,
                    connection.id,
                    Some(account_id),
                    Some(serde_json::json!({ "removed": removed })),
                )
                .await;
        }
    }

    /// Deletes a single file whose source object is gone.
    ///
    /// The file row is soft-deleted first so it stops being readable, then its
    /// stored object is removed to reclaim storage. Object removal is
    /// best-effort: the row is already tombstoned, so a failure only leaves an
    /// orphaned object, which is logged.
    async fn remove_vanished_file(&self, file_id: Uuid, storage_path: &str) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        conn.delete_workspace_file(file_id).await?;

        if let Ok(file_key) = FileKey::from_str(storage_path) {
            let store = self.nats.object_store::<FilesBucket>().await?;
            if let Err(err) = store.delete(&file_key).await {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    "Failed to delete stored object for vanished source object",
                );
            }
        }
        Ok(())
    }

    /// Imports one object at `remote_key` using an already-connected `client`.
    ///
    /// Streams the object's bytes from the external store, encrypts them with
    /// the workspace key, writes them to the NATS files bucket, and records a
    /// [`WorkspaceFile`] with [`FileSource::Imported`] and its source origin. If
    /// recording fails, the just-written object is deleted so none is orphaned.
    async fn import_one(
        &self,
        client: &ObjectStoreClient,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        remote_key: &str,
    ) -> Result<WorkspaceFile> {
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
        match self
            .record_imported_file(connection, account_id, remote_key, &file_key, &measurements)
            .await
        {
            Ok(file) => Ok(file),
            Err(err) => {
                if let Err(cleanup) = store.delete(&file_key).await {
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
            source_connection_id: Some(connection.id),
            source_key: Some(remote_key.to_owned()),
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
        config: &ConnectionConfig,
        file: &WorkspaceFile,
        remote_key: &str,
    ) -> Result<()> {
        tracing::debug!(target: TRACING_TARGET, "Exporting file to connection");

        let client = self.object.connect(config).await?;

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

        tracing::debug!(target: TRACING_TARGET, "File exported");
        Ok(())
    }

    /// Runs a sync transfer to completion and records the run's outcome.
    ///
    /// The transfer runs in an inner task bounded by a fixed timeout: a panic
    /// surfaces as a join error and a hung backend as a timeout, both recorded
    /// as a failed run rather than leaving it stuck `Running`. `export` selects
    /// the direction: `None` imports all new objects, `Some((file, key))` pushes
    /// a file out. A cancel signal (see [`cancel_local`]) aborts the transfer and
    /// records the run as cancelled. Shared by the manual endpoint and the
    /// scheduled worker.
    ///
    /// [`cancel_local`]: Self::cancel_local
    pub async fn run_transfer(
        &self,
        run_id: Uuid,
        connection: WorkspaceConnection,
        config: ConnectionConfig,
        account_id: Uuid,
        export: Option<(WorkspaceFile, String)>,
    ) {
        let token = CancellationToken::new();
        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .insert(run_id, token.clone());

        let transfer = self.clone();
        let mut work = tokio::spawn(async move {
            match export {
                None => transfer.import_new(&connection, &config, account_id).await,
                Some((file, key)) => transfer
                    .export_file(&connection, &config, &file, &key)
                    .await
                    .map(|()| 1),
            }
        });

        // Race the transfer against a cancel signal and a hard timeout. The join
        // handle is polled by mutable reference so it can still be aborted in the
        // cancel/timeout branches.
        let outcome = tokio::select! {
            _ = token.cancelled() => {
                work.abort();
                Outcome::Cancelled
            }
            _ = tokio::time::sleep(SYNC_TIMEOUT) => {
                work.abort();
                Outcome::Finished(Err(ErrorKind::InternalServerError.with_message("Sync timed out")))
            }
            joined = &mut work => match joined {
                Ok(result) => Outcome::Finished(result),
                Err(join_err) => Outcome::Finished(Err(ErrorKind::InternalServerError
                    .with_message("Sync task terminated unexpectedly")
                    .with_context(join_err.to_string()))),
            },
        };

        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .remove(&run_id);

        match outcome {
            Outcome::Finished(result) => self.finish_run(run_id, result).await,
            Outcome::Cancelled => self.cancel_run(run_id).await,
        }
    }

    /// Records the outcome of a background sync run: completes it (one object
    /// transferred) on success, or marks it failed with a safe error message.
    ///
    /// Errors updating the run are logged rather than propagated, since this
    /// runs after the response has already been sent.
    pub async fn finish_run(&self, run_id: Uuid, result: Result<u64>) {
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
            Ok(records_synced) => {
                conn.complete_workspace_connection_run(run_id, records_synced as i64)
                    .await
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

    /// Marks a cancelled run's row as cancelled. The status transition is
    /// guarded, so if the cancel handler already flipped the row (or it reached
    /// another terminal state first) this is a harmless no-op.
    async fn cancel_run(&self, run_id: Uuid) {
        let mut conn = match self.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record cancellation: no connection");
                return;
            }
        };
        if let Err(err) = conn.cancel_workspace_connection_run(run_id).await {
            tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record sync cancellation");
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
