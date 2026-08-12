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

use futures::stream::{self, StreamExt};
use nvisy_nats::object::{FileKey, FilesBucket, ObjectBucket};
use nvisy_object::client::ObjectStoreClient;
use nvisy_object::providers::StorageConfig;
use nvisy_postgres::model::{NewWorkspaceFile, WorkspaceConnection, WorkspaceFile};
use nvisy_postgres::query::{
    WorkspaceConnectionSyncRepository, WorkspaceFileRepository, WorkspaceRepository,
};
use nvisy_postgres::types::{
    ConnectionSyncCompletedParams, ConnectionSyncFailedParams, FileKind, NotificationPayload,
    SyncDeletionPolicy, WorkspaceSettings,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::SyncConfig;
use super::bridge::{reader_to_stream, stream_to_reader};
use crate::handler::{ErrorKind, Result};
use crate::service::{
    ExternalObjectStore, HashingReader, Infra, Measurements, NotificationEmitter, WebhookEmitter,
};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::sync";

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
    infra: Infra,
    object: ExternalObjectStore,
    webhook: WebhookEmitter,
    notification: NotificationEmitter,
    /// Maximum objects imported concurrently within a single sync.
    import_concurrency: usize,
    // Cancellation tokens for transfers running in this process, keyed by run id.
    // Cancellation is best-effort and process-local: it aborts a transfer only on
    // the instance running it. Cross-instance runs are stopped by the DB status
    // flip plus the status-guarded finalizers.
    running: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl ConnectionSyncService {
    /// Creates a new [`ConnectionSyncService`].
    pub fn new(
        infra: Infra,
        object: ExternalObjectStore,
        webhook: WebhookEmitter,
        notification: NotificationEmitter,
        config: SyncConfig,
    ) -> Self {
        Self {
            infra,
            object,
            webhook,
            notification,
            import_concurrency: config.import_concurrency.max(1),
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
        config: &StorageConfig,
        deletion_policy: SyncDeletionPolicy,
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

        let mut conn = self.infra.postgres.get_connection().await?;
        let already_imported: HashSet<String> = conn
            .imported_keys_for_connection(connection.id)
            .await?
            .into_iter()
            .collect();
        // Imported files are original documents; the retention expiry is the same
        // for every object in this sync, so resolve it once here rather than
        // re-querying the workspace per file.
        let expires_at = conn
            .find_workspace_by_id(connection.workspace_id)
            .await?
            .and_then(|workspace| {
                WorkspaceSettings::from_value(&workspace.settings)
                    .retention
                    .original_documents
                    .expires_at(jiff::Timestamp::now())
            });
        drop(conn);

        // Import the not-yet-imported objects with bounded concurrency: up to
        // `import_concurrency` fetch/decrypt/store pipelines run at once. A single
        // bad object is logged and skipped rather than aborting the whole sync.
        // `client` and `self` are borrowed by every task, so the async blocks
        // capture shared references (only `key` is per-task).
        // Collect the keys to import into owned values first, so each task's
        // future captures no borrow of `remote_keys`.
        let to_import: Vec<String> = remote_keys
            .iter()
            .filter(|key| !already_imported.contains(*key))
            .cloned()
            .collect();
        let imported = stream::iter(to_import)
            .map(|key| {
                // `ObjectStoreClient` is an `Arc` handle, so cloning per task is
                // cheap and keeps each future free of borrowed locals.
                let client = client.clone();
                async move {
                    match self
                        .import_one(&client, connection, account_id, &key, expires_at)
                        .await
                    {
                        Ok(_) => 1u64,
                        Err(err) => {
                            tracing::warn!(
                                target: TRACING_TARGET,
                                key = %key, error = %err,
                                "Skipping object that failed to import",
                            );
                            0
                        }
                    }
                }
            })
            .buffer_unordered(self.import_concurrency)
            .fold(0u64, |total, imported| std::future::ready(total + imported))
            .await;

        self.reconcile_deletions(connection, deletion_policy, &remote_keys)
            .await;

        tracing::debug!(target: TRACING_TARGET, imported, "Import sync complete");
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
            match conn.imported_files_for_connection(connection.id).await {
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
            if let Err(err) = self
                .remove_vanished_file(file.file_id, &file.storage_path)
                .await
            {
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

    /// Deletes a single file whose source object is gone.
    ///
    /// The file row is soft-deleted first so it stops being readable, then its
    /// stored object is removed to reclaim storage. Object removal is
    /// best-effort: the row is already tombstoned, so a failure only leaves an
    /// orphaned object, which is logged.
    async fn remove_vanished_file(&self, file_id: Uuid, storage_path: &str) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        conn.delete_workspace_file(file_id).await?;

        if let Ok(file_key) = FileKey::from_str(storage_path) {
            let store = self.infra.nats.object_store::<FilesBucket>().await?;
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
    /// the workspace key, writes them to the NATS files bucket, and records an
    /// original-kind file along with its import origin (connection and remote
    /// key). If recording fails, the just-written object is deleted so none is
    /// orphaned.
    async fn import_one(
        &self,
        client: &ObjectStoreClient,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        remote_key: &str,
        expires_at: Option<jiff::Timestamp>,
    ) -> Result<WorkspaceFile> {
        // Stream external bytes -> hash+measure -> encrypt -> NATS files bucket.
        let source = client.get_stream(remote_key).await?;
        let (measured, measurements) = HashingReader::new(stream_to_reader(source));
        let ciphertext = Box::pin(
            self.infra
                .crypto
                .encrypt_reader(connection.workspace_id, measured),
        );

        let store = self.infra.nats.object_store::<FilesBucket>().await?;
        let file_key = FileKey::generate(connection.workspace_id);
        store.put(&file_key, ciphertext).await?;

        // The object now exists in storage; if recording it in the database
        // fails, delete it so no orphaned object is left behind.
        match self
            .record_imported_file(
                connection,
                account_id,
                remote_key,
                &file_key,
                &measurements,
                expires_at,
            )
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

    /// Inserts the [`WorkspaceFile`] row for a freshly imported object. The
    /// retention `expires_at` is resolved once per sync by the caller and passed
    /// in, so this only holds a pooled connection for the insert itself.
    async fn record_imported_file(
        &self,
        connection: &WorkspaceConnection,
        account_id: Uuid,
        remote_key: &str,
        file_key: &FileKey,
        measurements: &Measurements,
        expires_at: Option<jiff::Timestamp>,
    ) -> Result<WorkspaceFile> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let filename = object_basename(remote_key);
        let extension = object_extension(remote_key);

        let new_file = NewWorkspaceFile {
            workspace_id: connection.workspace_id,
            account_id,
            display_name: Some(filename.clone()),
            original_filename: Some(filename),
            file_extension: extension,
            file_kind: Some(FileKind::Original),
            file_size_bytes: measurements.bytes() as i64,
            file_hash_sha256: measurements.sha256().to_vec(),
            storage_path: file_key.to_string(),
            storage_bucket: FilesBucket::NAME.to_owned(),
            expires_at: expires_at.map(Into::into),
            ..Default::default()
        };
        Ok(conn
            .record_imported_file(new_file, connection.id, remote_key.to_owned())
            .await?)
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
        config: &StorageConfig,
        file: &WorkspaceFile,
        remote_key: &str,
    ) -> Result<()> {
        tracing::debug!(target: TRACING_TARGET, "Exporting file to connection");

        let client = self.object.connect(config).await?;

        let store = self.infra.nats.object_store::<FilesBucket>().await?;
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
            self.infra
                .crypto
                .decrypt_reader(connection.workspace_id, stored.into_reader()),
        );
        let body = reader_to_stream(plaintext);
        let content_type = mime_from_extension(&file.file_extension);
        client
            .put_multipart(remote_key, Some(content_type.as_str()), body)
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
        config: StorageConfig,
        deletion_policy: SyncDeletionPolicy,
        account_id: Uuid,
        export: Option<(WorkspaceFile, String)>,
    ) {
        let token = CancellationToken::new();
        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .insert(run_id, token.clone());

        let workspace_id = connection.workspace_id;
        let connection_id = connection.id;

        let _ = self
            .webhook
            .emit_connection_sync_started(
                workspace_id,
                run_id,
                Some(account_id),
                Some(serde_json::json!({ "connection_id": connection_id })),
            )
            .await;

        let transfer = self.clone();
        let mut work = tokio::spawn(async move {
            match export {
                None => {
                    transfer
                        .import_new(&connection, &config, deletion_policy, account_id)
                        .await
                }
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
            Outcome::Finished(result) => {
                let sync_event = match &result {
                    Ok(records_synced) => (
                        true,
                        serde_json::json!({
                            "connection_id": connection_id,
                            "records_synced": records_synced,
                        }),
                    ),
                    Err(err) => (
                        false,
                        serde_json::json!({
                            "connection_id": connection_id,
                            "error": err.message().unwrap_or("Sync failed"),
                        }),
                    ),
                };

                self.finish_run(run_id, result.clone()).await;

                let (succeeded, data) = sync_event;
                let _ = if succeeded {
                    self.webhook
                        .emit_connection_sync_completed(
                            workspace_id,
                            run_id,
                            Some(account_id),
                            Some(data),
                        )
                        .await
                } else {
                    self.webhook
                        .emit_connection_sync_failed(
                            workspace_id,
                            run_id,
                            Some(account_id),
                            Some(data),
                        )
                        .await
                };

                // Notify the connection owner (best-effort).
                let payload = match &result {
                    Ok(records_synced) => NotificationPayload::ConnectionSyncCompleted(
                        ConnectionSyncCompletedParams {
                            connection_id,
                            records_synced: Some(*records_synced as i64),
                        },
                    ),
                    Err(err) => {
                        NotificationPayload::ConnectionSyncFailed(ConnectionSyncFailedParams {
                            connection_id,
                            error: Some(err.message().unwrap_or("Sync failed").to_owned()),
                        })
                    }
                };
                if let Err(err) = self
                    .notification
                    .notify_account(workspace_id, account_id, payload)
                    .await
                {
                    tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to create sync notification");
                }
            }
            Outcome::Cancelled => self.cancel_run(run_id).await,
        }
    }

    /// Records the outcome of a background sync run: completes it (one object
    /// transferred) on success, or marks it failed with a safe error message.
    ///
    /// Errors updating the run are logged rather than propagated, since this
    /// runs after the response has already been sent.
    pub async fn finish_run(&self, run_id: Uuid, result: Result<u64>) {
        let mut conn = match self.infra.postgres.get_connection().await {
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
                conn.complete_workspace_connection_sync(run_id, records_synced as i64)
                    .await
            }
            Err(err) => {
                // Log the full error (may include backend URLs/details) but
                // persist only the safe summary: the stored message is exposed
                // to clients via the sync's `error_message`.
                tracing::warn!(target: TRACING_TARGET, %run_id, error = %err, "Sync failed");
                let safe_message = err.message().unwrap_or("Sync failed").to_owned();
                conn.fail_workspace_connection_sync(run_id, &safe_message)
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
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record cancellation: no connection");
                return;
            }
        };
        if let Err(err) = conn.cancel_workspace_connection_sync(run_id).await {
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

/// Guesses the MIME type for a file extension, falling back to
/// `application/octet-stream` when unknown. Used to set the export
/// Content-Type, since files store only their extension, not a MIME type.
fn mime_from_extension(extension: &str) -> String {
    mime_guess::from_ext(extension.trim_start_matches('.'))
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
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
