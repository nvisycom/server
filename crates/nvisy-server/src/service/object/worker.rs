//! Connection sync worker: distributed scheduler + job consumer.
//!
//! Runs on every server instance and coordinates via NATS so scheduled syncs
//! fire exactly once across the fleet:
//!
//! - **Scheduler tick**: on an interval, one instance wins a KV compare-and-set
//!   lock (leader election for that tick) and enqueues due connections as jobs
//!   onto the [`ConnectionSyncStream`] work queue.
//! - **Consumer**: a durable pull consumer drains the work queue; JetStream
//!   delivers each job to a single instance, which runs the import and records a
//!   `Scheduled` run.
//! - **Reaper**: on startup, runs left `Running` by a previous crashed process
//!   are failed so they do not appear stuck forever.

use std::time::Duration;

use jiff::{Span, Timestamp};
use nvisy_nats::NatsClient;
use nvisy_nats::kv::{LockKey, SchedulerLocksBucket};
use nvisy_nats::stream::{ConnectionSyncStream, EventPublisher, EventSubscriber};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewWorkspaceConnectionRun, WorkspaceConnection};
use nvisy_postgres::query::{WorkspaceConnectionRepository, WorkspaceConnectionRunRepository};
use nvisy_postgres::types::{SyncStatus, SyncTriggerType};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{ConnectionSyncService, is_cron_due};
use crate::handler::Result;
use crate::service::CryptoService;

/// Tracing target for the connection sync worker.
const TRACING_TARGET: &str = "nvisy_server::worker::connection_sync";

/// How often the scheduler tick runs.
const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Runs older than this that are still `Running` at startup are reaped.
const STALE_RUN_AGE_HOURS: i64 = 6;

/// Fixed KV key for the per-tick scheduler leader-election lock.
const SCHEDULER_LOCK_KEY: &str = "connection-sync-scheduler";

/// Maximum number of attempts for a scheduled sync before it is left failed.
const MAX_SYNC_ATTEMPTS: i32 = 3;

/// Base backoff before re-enqueueing a failed scheduled sync. Each attempt
/// waits this multiplied by the attempt number (linear backoff).
const RETRY_BACKOFF: Duration = Duration::from_secs(30);

/// A scheduled sync job: enqueued by the scheduler, consumed by any instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSyncJob {
    /// Workspace owning the connection.
    pub workspace_id: Uuid,
    /// Connection to sync.
    pub connection_id: Uuid,
    /// 1-based attempt number; a failed run re-enqueues with this incremented,
    /// up to a bounded maximum.
    #[serde(default = "first_attempt")]
    pub attempt: i32,
}

/// Default attempt for jobs enqueued before this field existed, and for the
/// first attempt of a new job.
fn first_attempt() -> i32 {
    1
}

type JobPublisher = EventPublisher<ConnectionSyncJob, ConnectionSyncStream>;
type JobSubscriber = EventSubscriber<ConnectionSyncJob, ConnectionSyncStream>;

/// Background worker driving scheduled connection syncs.
pub struct ConnectionSyncWorker {
    postgres: PgClient,
    nats: NatsClient,
    crypto: CryptoService,
    sync: ConnectionSyncService,
}

impl ConnectionSyncWorker {
    /// Creates a new [`ConnectionSyncWorker`].
    pub fn new(
        postgres: PgClient,
        nats: NatsClient,
        crypto: CryptoService,
        sync: ConnectionSyncService,
    ) -> Self {
        Self {
            postgres,
            nats,
            crypto,
            sync,
        }
    }

    /// Runs the worker until cancelled: reaps stale runs, then drives the
    /// scheduler tick and the job consumer concurrently.
    pub async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting connection sync worker");

        if let Err(err) = self.reap_stale_runs().await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to reap stale runs");
        }

        let scheduler = self.run_scheduler(cancel.clone());
        let consumer = self.run_consumer(cancel);
        let (scheduler, consumer) = tokio::join!(scheduler, consumer);

        scheduler.and(consumer)
    }

    /// Fails any runs left `Running` by a previously crashed process.
    async fn reap_stale_runs(&self) -> Result<()> {
        let cutoff = Timestamp::now() - Span::new().hours(STALE_RUN_AGE_HOURS);
        let mut conn = self.postgres.get_connection().await?;
        let reaped = conn.fail_stale_running_runs(cutoff.into()).await?;
        if reaped > 0 {
            tracing::warn!(target: TRACING_TARGET, reaped, "Reaped stale sync runs");
        }
        Ok(())
    }

    /// Scheduler loop: each tick, try to win the leader lock and, if won,
    /// enqueue due connections.
    async fn run_scheduler(&self, cancel: CancellationToken) -> Result<()> {
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => {
                    if let Err(err) = self.schedule_due().await {
                        tracing::error!(target: TRACING_TARGET, error = %err, "Scheduler tick failed");
                    }
                }
            }
        }
        Ok(())
    }

    /// If this instance wins the leader election for the current period,
    /// enqueue every due connection.
    async fn schedule_due(&self) -> Result<()> {
        let now = Timestamp::now();

        // Leader election for this wall-clock period: the lock key is the period
        // itself, so exactly one instance wins per period regardless of each
        // instance's tick phase. The bucket TTL reclaims old period keys.
        let period = now.as_second() / TICK_INTERVAL.as_secs() as i64;
        let lock_key = LockKey::from(format!("{SCHEDULER_LOCK_KEY}:{period}").as_str());
        let locks = self
            .nats
            .kv_store::<LockKey, u64, SchedulerLocksBucket>()
            .await?;
        let acquired = locks.create(&lock_key, &1).await?;
        if !acquired {
            tracing::debug!(target: TRACING_TARGET, "Another instance owns this scheduler period");
            return Ok(());
        }

        let mut conn = self.postgres.get_connection().await?;
        let connections = conn.list_scheduled_connections().await?;

        let publisher: JobPublisher = self.nats.event_publisher().await?;
        for connection in connections {
            let Some(cron) = &connection.schedule_cron else {
                continue;
            };
            let last_sync = conn
                .last_successful_sync_at(&[connection.id])
                .await?
                .into_iter()
                .next()
                .map(|(_, ts)| ts.into());
            if !is_cron_due(cron, last_sync, now) {
                continue;
            }
            // Skip when a run is already in progress for this connection.
            if let Some(latest) = conn
                .find_latest_workspace_connection_run(connection.id)
                .await?
                && latest.is_in_progress()
            {
                continue;
            }

            let job = ConnectionSyncJob {
                workspace_id: connection.workspace_id,
                connection_id: connection.id,
                attempt: 1,
            };
            if let Err(err) = publisher.publish(&job).await {
                tracing::error!(
                    target: TRACING_TARGET,
                    connection_id = %connection.id, error = %err,
                    "Failed to enqueue scheduled sync",
                );
            }
        }

        Ok(())
    }

    /// Consumer loop: drain the work queue, running each job as a scheduled sync.
    async fn run_consumer(&self, cancel: CancellationToken) -> Result<()> {
        let subscriber: JobSubscriber = self.nats.event_subscriber().await?;
        let mut stream = subscriber.subscribe().await?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                result = stream.next_with_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(Some(mut message)) => {
                            let job = message.payload().clone();
                            // At-least-once: run first, then ack. A crash before
                            // ack redelivers the job; the import is idempotent
                            // (already-imported keys are skipped), so a redelivery
                            // is safe.
                            self.run_job(job).await;
                            if let Err(err) = message.ack().await {
                                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to ack job");
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::error!(target: TRACING_TARGET, error = %err, "Error receiving job");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Runs one scheduled import job: loads the connection, opens a `Scheduled`
    /// run, and performs the import.
    async fn run_job(&self, job: ConnectionSyncJob) {
        let connection = match self.load_connection(&job).await {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                tracing::warn!(target: TRACING_TARGET, connection_id = %job.connection_id, "Scheduled connection not found");
                return;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, connection_id = %job.connection_id, error = %err, "Failed to load scheduled connection");
                return;
            }
        };

        if !connection.is_active {
            return;
        }

        let (credentials, run_id) = match self.begin_run(&connection, job.attempt).await {
            Ok(started) => started,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, connection_id = %connection.id, error = %err, "Failed to start scheduled run");
                return;
            }
        };

        // Scheduled syncs are import-only; imported files are attributed to the
        // connection's creator. Runs through the shared transfer path.
        let account_id = connection.account_id;
        let connection_id = connection.id;
        let workspace_id = connection.workspace_id;
        self.sync
            .run_transfer(run_id, connection, credentials, account_id, None)
            .await;

        self.maybe_retry(workspace_id, connection_id, run_id, job.attempt)
            .await;
    }

    /// Re-enqueues a scheduled job after a failed run, up to
    /// [`MAX_SYNC_ATTEMPTS`]. Reads the run's final status; a run that was
    /// cancelled or completed is not retried.
    ///
    /// The linear backoff runs in a detached task so the consumer can ack and
    /// move on immediately rather than blocking the whole queue during the wait.
    async fn maybe_retry(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        run_id: Uuid,
        attempt: i32,
    ) {
        if attempt >= MAX_SYNC_ATTEMPTS {
            return;
        }

        let failed = {
            let mut conn = match self.postgres.get_connection().await {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to check run outcome for retry");
                    return;
                }
            };
            match conn.find_workspace_connection_run_by_id(run_id).await {
                Ok(Some(run)) => run.is_failed(),
                Ok(None) => false,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to check run outcome for retry");
                    return;
                }
            }
        };
        if !failed {
            return;
        }

        let next_attempt = attempt + 1;
        let backoff = RETRY_BACKOFF * attempt as u32;
        let nats = self.nats.clone();
        tokio::spawn(async move {
            tokio::time::sleep(backoff).await;

            let job = ConnectionSyncJob {
                workspace_id,
                connection_id,
                attempt: next_attempt,
            };
            let publisher: JobPublisher = match nats.event_publisher().await {
                Ok(publisher) => publisher,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, %connection_id, error = %err, "Failed to build publisher for retry");
                    return;
                }
            };
            if let Err(err) = publisher.publish(&job).await {
                tracing::error!(target: TRACING_TARGET, %connection_id, error = %err, "Failed to re-enqueue failed sync");
            } else {
                tracing::info!(target: TRACING_TARGET, %connection_id, attempt = next_attempt, "Re-enqueued failed scheduled sync");
            }
        });
    }

    /// Loads the connection for a job, scoped to its workspace.
    async fn load_connection(
        &self,
        job: &ConnectionSyncJob,
    ) -> Result<Option<WorkspaceConnection>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .find_connection_in_workspace(job.workspace_id, job.connection_id)
            .await?)
    }

    /// Decrypts credentials and opens a `Scheduled` run for the connection at the
    /// given attempt number.
    async fn begin_run(
        &self,
        connection: &WorkspaceConnection,
        attempt: i32,
    ) -> Result<(serde_json::Value, Uuid)> {
        let credentials = self
            .crypto
            .decrypt_json(connection.workspace_id, &connection.encrypted_data)?;

        let mut conn = self.postgres.get_connection().await?;
        let new_run = NewWorkspaceConnectionRun {
            connection_id: connection.id,
            account_id: None,
            trigger_type: Some(SyncTriggerType::Scheduled),
            status: Some(SyncStatus::Running),
            records_synced: Some(0),
            attempt: Some(attempt),
            metadata: None,
        };
        let run = conn.create_workspace_connection_run(new_run).await?;
        Ok((credentials, run.id))
    }
}
