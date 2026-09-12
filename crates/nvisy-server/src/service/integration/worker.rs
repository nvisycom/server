//! Connection sync worker: distributed scheduler + job consumer.
//!
//! Runs on every server instance and coordinates via NATS so scheduled syncs
//! fire exactly once across the fleet:
//!
//! - **Scheduler tick**: on an interval, one instance wins a KV compare-and-set
//!   lock (leader election for that tick) and enqueues due connections as jobs
//!   onto the [`ConnectionSyncStream`] work queue.
//! - **Consumer**: a durable pull consumer drains the work queue; JetStream
//!   delivers each job to a single instance, which runs the sync in its
//!   scheduled direction (import or redacted export) and records a `Scheduled`
//!   run.
//! - **Reaper**: on startup, runs left `Running` by a previous crashed process
//!   are failed so they do not appear stuck forever.

use std::collections::HashMap;
use std::time::Duration;

use jiff::{Span, Timestamp};
use nvisy_nats::kv::{SchedulerLockKey, SchedulerLocksBucket};
use nvisy_nats::stream::{ConnectionSyncStream, EventPublisher, EventSubscriber};
use nvisy_postgres::model::{
    NewWorkspaceConnectionSync, WorkspaceConnection, WorkspaceConnectionSync,
};
use nvisy_postgres::query::{
    ScheduledConnection, WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{SyncStatus, SyncTriggerType};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{ConnectionSyncService, StandardCronSchedule, TransferKind, TransferRequest};
use crate::response::{ErrorKind, Result};
use crate::service::{ConnectionConfig, CryptoService, Infra, Worker};

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
///
/// The direction is not carried on the job: the consumer re-reads the schedule
/// when it opens the run, so a direction change after enqueue is always honored.
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

/// The connection-sync JetStream stream, carrying [`ConnectionSyncJob`] payloads.
type SyncStream = ConnectionSyncStream<ConnectionSyncJob>;
type JobPublisher = EventPublisher<SyncStream>;
type JobSubscriber = EventSubscriber<SyncStream>;

/// Background worker driving scheduled connection syncs.
pub struct ConnectionSyncWorker {
    infra: Infra,
    crypto: CryptoService,
    sync: ConnectionSyncService,
}

impl Worker for ConnectionSyncWorker {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "connection_sync"
    }

    /// Runs the worker until cancelled, logging its lifecycle (start, stop,
    /// failure).
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting connection sync worker");

        let result = self.run_inner(cancel).await;

        match &result {
            Ok(()) => tracing::info!(target: TRACING_TARGET, "Connection sync worker stopped"),
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Connection sync worker failed")
            }
        }

        result
    }
}

impl ConnectionSyncWorker {
    /// Creates a new [`ConnectionSyncWorker`].
    pub fn new(infra: Infra, crypto: CryptoService, sync: ConnectionSyncService) -> Self {
        Self {
            infra,
            crypto,
            sync,
        }
    }

    /// Reaps stale runs, then drives the scheduler tick and the job consumer
    /// concurrently until cancelled.
    async fn run_inner(&self, cancel: CancellationToken) -> Result<()> {
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
        let mut conn = self.infra.postgres.get_connection().await?;
        let reaped = conn.fail_stale_running_syncs(cutoff.into()).await?;
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
        // instance's tick phase. The bucket TTL reclaims old period keys. Segments
        // are joined with `.`, which NATS KV keys allow (`[-/_=.a-zA-Z0-9]`).
        let period = now.as_second() / TICK_INTERVAL.as_secs() as i64;
        let lock_key = SchedulerLockKey::from(format!("{SCHEDULER_LOCK_KEY}.{period}"));
        let locks = self.infra.nats.kv_store::<SchedulerLocksBucket>().await?;
        let acquired = locks.create(&lock_key, &1).await?;
        if !acquired {
            tracing::debug!(target: TRACING_TARGET, "Another instance owns this scheduler period");
            return Ok(());
        }

        // Snapshot the scheduled connections (each with its cron) and every
        // candidate's latest sync in two queries, then release the DB connection
        // before publishing so it is not held across NATS round-trips.
        let due = {
            let mut conn = self.infra.postgres.get_connection().await?;
            let connections = conn.list_scheduled_connections().await?;
            let ids: Vec<Uuid> = connections.iter().map(|c| c.connection.id).collect();
            let latest: HashMap<Uuid, WorkspaceConnectionSync> = conn
                .find_latest_workspace_connection_syncs(&ids)
                .await?
                .into_iter()
                .map(|sync| (sync.connection_id, sync))
                .collect();

            let mut due = Vec::new();
            for ScheduledConnection {
                connection,
                schedule_cron,
            } in connections
            {
                let latest = latest.get(&connection.id);
                // A run already in progress means this connection is busy; skip it.
                if latest.is_some_and(|run| run.status.is_in_progress()) {
                    continue;
                }
                // Due-ness is measured from the last attempt (success or failure),
                // not the last success, so a persistently failing connection is not
                // re-enqueued every tick; retries are owned by maybe_retry.
                let last_attempt = latest.map(|run| run.started_at.into());
                if StandardCronSchedule.is_due(&schedule_cron, last_attempt, now) {
                    due.push((connection.workspace_id, connection.id));
                }
            }
            due
        };

        let publisher: JobPublisher = self.infra.nats.event_publisher().await?;
        for (workspace_id, connection_id) in due {
            let job = ConnectionSyncJob {
                workspace_id,
                connection_id,
                attempt: 1,
            };
            if let Err(err) = publisher.publish(&job).await {
                tracing::error!(
                    target: TRACING_TARGET,
                    %connection_id, error = %err,
                    "Failed to enqueue scheduled sync",
                );
            }
        }

        Ok(())
    }

    /// Consumer loop: drain the work queue, running each job as a scheduled sync.
    async fn run_consumer(&self, cancel: CancellationToken) -> Result<()> {
        let subscriber: JobSubscriber = self.infra.nats.event_subscriber().await?;
        let mut stream = subscriber.subscribe().await?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(target: TRACING_TARGET, "Connection sync worker shutdown requested");
                    break;
                }
                result = stream.next_with_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(Some(mut message)) => {
                            let job = message.payload().clone();
                            // At-least-once: run first, then ack. A crash before
                            // ack redelivers the job; both directions are
                            // idempotent (already-imported keys and already-
                            // exported files are skipped), so a redelivery is safe.
                            self.run_job(job, &cancel).await;
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

    /// Runs one scheduled sync job: loads the connection, opens a `Scheduled`
    /// run, and performs the sync in its scheduled direction.
    async fn run_job(&self, job: ConnectionSyncJob, cancel: &CancellationToken) {
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

        // A job may be delivered more than once (at-least-once), or a duplicate
        // may have been enqueued. If a run is already in progress for this
        // connection, skip rather than starting a second concurrent run. The
        // one-active-run unique index is the authoritative guard; this check just
        // avoids the noisy index violation on the common redelivery case.
        match self.load_latest_run(&connection).await {
            Ok(Some(run)) if run.status.is_in_progress() => {
                tracing::debug!(target: TRACING_TARGET, connection_id = %connection.id, "Sync already in progress; skipping duplicate job");
                return;
            }
            Ok(_) => {}
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, connection_id = %connection.id, error = %err, "Failed to check in-progress run");
                return;
            }
        }

        let connection_id = connection.id;
        let request = match self.begin_run(connection, job.attempt).await {
            Ok(Some(request)) => request,
            // The connection is no longer scheduled (schedule removed or cron
            // cleared after enqueue); drop the job quietly.
            Ok(None) => return,
            // A lost race on the one-active-run index is benign and expected
            // (at-least-once delivery); anything else — a decrypt failure or a
            // mis-scheduled non-sync connection — is a real fault worth surfacing.
            Err(err) if err.kind() == ErrorKind::Conflict => {
                tracing::debug!(target: TRACING_TARGET, connection_id = %connection_id, "Skipping scheduled run: already active");
                return;
            }
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, connection_id = %connection_id, error = %err, "Failed to open scheduled run");
                return;
            }
        };

        // Capture the identifiers needed after the transfer takes ownership of
        // the request (and its connection). Runs through the shared transfer path.
        let run_id = request.run_id;
        let workspace_id = request.connection.workspace_id;
        self.sync.run_transfer(request).await;

        self.maybe_retry(workspace_id, connection_id, run_id, job.attempt, cancel)
            .await;
    }

    /// Re-enqueues a scheduled job after a failed run, up to
    /// [`MAX_SYNC_ATTEMPTS`]. Reads the run's final status; a run that was
    /// cancelled or completed is not retried.
    ///
    /// The linear backoff runs in a detached task so the consumer can ack and
    /// move on immediately rather than blocking the whole queue during the wait.
    /// The task also stops on the worker's cancellation token so a shutdown does
    /// not leave a sleeping re-enqueue behind.
    async fn maybe_retry(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        run_id: Uuid,
        attempt: i32,
        cancel: &CancellationToken,
    ) {
        if attempt >= MAX_SYNC_ATTEMPTS {
            return;
        }

        let failed = {
            let mut conn = match self.infra.postgres.get_connection().await {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to check run outcome for retry");
                    return;
                }
            };
            match conn.find_workspace_connection_sync_by_id(run_id).await {
                Ok(Some(run)) => run.status.is_failed(),
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
        let nats = self.infra.nats.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            // Wait out the backoff, but abandon the retry if the worker is
            // shutting down so no sleeping task outlives the process.
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(backoff) => {}
            }

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
        let mut conn = self.infra.postgres.get_connection().await?;
        Ok(conn
            .find_connection_in_workspace(job.workspace_id, job.connection_id)
            .await?)
    }

    /// Loads the most recent run for a connection (its current sync state).
    async fn load_latest_run(
        &self,
        connection: &WorkspaceConnection,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        let mut conn = self.infra.postgres.get_connection().await?;
        Ok(conn
            .find_latest_workspace_connection_sync(connection.id)
            .await?)
    }

    /// Decrypts the connection config and opens a `Scheduled` run for the
    /// connection at the given attempt number.
    ///
    /// Revalidates the schedule against the live row rather than trusting the
    /// job: the schedule must still exist and still carry a cron (the connection
    /// is still scheduled), and the *current* [`SyncMode`] — not the job's, which
    /// may be stale if the direction was flipped after enqueue — selects the
    /// transfer. Import pulls the whole listing (reconciling deletions per the
    /// schedule's policy); export pushes every redacted output not yet exported.
    /// Returns `Ok(None)` when the connection is no longer scheduled, so the job
    /// is dropped quietly rather than treated as a fault.
    async fn begin_run(
        &self,
        connection: WorkspaceConnection,
        attempt: i32,
    ) -> Result<Option<TransferRequest>> {
        let config = self.crypto.decrypt_json::<ConnectionConfig>(
            connection.workspace_id,
            &connection.encrypted_data,
        )?;

        let mut conn = self.infra.postgres.get_connection().await?;
        // Re-read the schedule: the direction/cron may have changed (or the
        // schedule been removed) since this job was enqueued.
        let Some(schedule) = conn.find_connection_schedule(connection.id).await? else {
            tracing::debug!(target: TRACING_TARGET, connection_id = %connection.id, "Connection no longer has a schedule; dropping job");
            return Ok(None);
        };
        if schedule.schedule_cron.is_none() {
            tracing::debug!(target: TRACING_TARGET, connection_id = %connection.id, "Connection is no longer scheduled; dropping job");
            return Ok(None);
        }

        // Only object stores are scheduled (a file service is request-time only
        // and has no schedule row, so it is never listed as due), and an object
        // store is enumerable in either direction. A non-object-store here would
        // mean a schedule row on a connection that cannot have one — a scheduling
        // bug, not a runtime input — so it is rejected rather than dispatched.
        if !matches!(config, ConnectionConfig::ObjectStore(_)) {
            return Err(ErrorKind::InternalServerError
                .with_message("scheduled sync for a non-object-store connection"));
        }
        let kind = if schedule.sync_mode.is_export() {
            TransferKind::ExportRedacted
        } else {
            TransferKind::ImportAll {
                deletion_policy: schedule.deletion_policy,
            }
        };
        // A scheduled run is attributed to whoever created the connection.
        let account_id = connection.account_id;
        let new_run = NewWorkspaceConnectionSync {
            connection_id: connection.id,
            account_id,
            trigger_type: Some(SyncTriggerType::Scheduled),
            status: Some(SyncStatus::Running),
            records_synced: Some(0),
            attempt: Some(attempt),
            metadata: None,
        };
        // Create the run and record its start event atomically.
        let run = self
            .sync
            .create_run(&mut conn, new_run, &connection)
            .await?;
        Ok(Some(TransferRequest {
            run_id: run.id,
            connection,
            config,
            account_id,
            kind,
        }))
    }
}
