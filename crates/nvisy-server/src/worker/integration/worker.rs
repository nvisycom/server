//! Connection sync worker: distributed scheduler + job consumer.
//!
//! Runs on every server instance and coordinates via NATS so scheduled syncs
//! fire exactly once across the fleet:
//!
//! - **Scheduler tick**: on an interval, one instance wins a KV compare-and-set
//!   lock (leader election for that tick) and enqueues due connections as jobs
//!   onto the [`ConnectionSyncStream`] work queue.
//! - **Consumer**: a durable pull consumer drains the work queue; `JetStream`
//!   delivers each job to a single instance, which runs the sync in its
//!   scheduled direction (import or redacted export) and records a `Scheduled`
//!   run.
//! - **Reaper**: on startup, runs left `Running` by a previous crashed process
//!   are failed so they do not appear stuck forever.

use std::collections::HashMap;
use std::time::Duration;

use jiff::{Span, Timestamp};
use nvisy_nats::kv::{SchedulerLockKey, SchedulerLocksBucket};
use nvisy_nats::stream::{ConnectionSyncStream, Delivery, EventPublisher, Settle};
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

use crate::extract::SecurityContext;
use crate::response::{Error, ErrorKind, Result};
use crate::service::{
    ConnectionConfig, ConnectionSyncService, CryptoService, Infra, StandardCronSchedule,
    TransferKind, TransferRequest, event,
};
use crate::worker::{Worker, every};

/// Tracing target for the connection sync worker.
const TRACING_TARGET: &str = "nvisy_server::worker::integration";

/// How often the scheduler tick runs.
const TICK_INTERVAL: Duration = Duration::from_mins(1);

/// Runs older than this that are still `Running` at startup are reaped.
const STALE_RUN_AGE_HOURS: i64 = 6;

/// Fixed KV key for the per-tick scheduler leader-election lock.
const SCHEDULER_LOCK_KEY: &str = "connection-sync-scheduler";

/// Base backoff before a failed scheduled sync is redelivered. Each attempt
/// waits this multiplied by the attempt number (linear backoff); the stream's
/// `MAX_DELIVER` bounds the attempts.
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
}

/// The connection-sync `JetStream` stream, carrying [`ConnectionSyncJob`] payloads.
pub type SyncStream = ConnectionSyncStream<ConnectionSyncJob>;
type JobPublisher = EventPublisher<SyncStream>;

/// Background worker driving scheduled connection syncs.
#[derive(Clone)]
pub struct ConnectionSyncWorker {
    infra: Infra,
    crypto: CryptoService,
    sync: ConnectionSyncService,
}

impl Worker for ConnectionSyncWorker {
    type Error = Error;

    const NAME: &'static str = "connection_sync";

    /// Reaps stale runs, then drives the scheduler tick and the job consumer
    /// concurrently until cancelled.
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        if let Err(err) = self.reap_stale_runs().await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to reap stale runs");
        }

        let scheduler = every(TICK_INTERVAL, &cancel, None, async || {
            if let Err(err) = self.schedule_due().await {
                tracing::error!(target: TRACING_TARGET, error = %err, "Scheduler tick failed");
            }
        });
        let worker = self.clone();
        // One sync at a time: a run holds a provider session and streams files.
        // At-least-once: a job settles after its run; both directions are
        // idempotent (already-imported keys and already-exported files are
        // skipped), so a redelivery is safe.
        let subscriber = self.infra.nats.event_subscriber::<SyncStream>();
        let consumer = subscriber.consume(1, &cancel, move |job, delivery| {
            let worker = worker.clone();
            async move { worker.run_job(job, delivery).await }
        });
        let ((), consumer) = tokio::join!(scheduler, consumer);
        consumer?;
        Ok(())
    }
}

impl ConnectionSyncWorker {
    /// Creates a new [`ConnectionSyncWorker`].
    #[must_use]
    pub fn new(infra: Infra, crypto: CryptoService, sync: ConnectionSyncService) -> Self {
        Self {
            infra,
            crypto,
            sync,
        }
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

    /// If this instance wins the leader election for the current period,
    /// enqueue every due connection.
    async fn schedule_due(&self) -> Result<()> {
        let now = Timestamp::now();

        // Leader election for this wall-clock period: the lock key is the period
        // itself, so exactly one instance wins per period regardless of each
        // instance's tick phase. The bucket TTL reclaims old period keys. Segments
        // are joined with `.`, which NATS KV keys allow (`[-/_=.a-zA-Z0-9]`).
        let period = now.as_second() / i64::try_from(TICK_INTERVAL.as_secs()).unwrap_or(i64::MAX);
        let lock_key: SchedulerLockKey = format!("{SCHEDULER_LOCK_KEY}.{period}").parse()?;
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
                // re-enqueued every tick; retries are the stream's redeliveries.
                let last_attempt = latest.map(|run| run.started_at.into());
                if StandardCronSchedule.is_due(&schedule_cron, last_attempt, now) {
                    due.push((connection.workspace_id, connection.id));
                }
            }
            due
        };

        let publisher: JobPublisher = self.infra.nats.event_publisher();
        for (workspace_id, connection_id) in due {
            let job = ConnectionSyncJob {
                workspace_id,
                connection_id,
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

    /// Runs one scheduled sync job: loads the connection, opens a `Scheduled`
    /// run, and performs the sync in its scheduled direction. A failed run, or a
    /// transient failure before it opens, is redelivered with a linear backoff
    /// until the stream's attempts run out; everything else is acked.
    async fn run_job(&self, job: ConnectionSyncJob, delivery: Delivery) -> Settle {
        let attempt = i32::try_from(delivery.attempt).unwrap_or(i32::MAX);
        let connection = match self.load_connection(&job).await {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                tracing::warn!(target: TRACING_TARGET, connection_id = %job.connection_id, "Scheduled connection not found");
                return Settle::Ack;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, connection_id = %job.connection_id, error = %err, "Failed to load scheduled connection");
                return retry(delivery);
            }
        };

        if !connection.is_active {
            return Settle::Ack;
        }

        // A job may be delivered more than once (at-least-once), or a duplicate
        // may have been enqueued. If a run is already in progress for this
        // connection, skip rather than starting a second concurrent run. The
        // one-active-run unique index is the authoritative guard; this check just
        // avoids the noisy index violation on the common redelivery case.
        match self.load_latest_run(&connection).await {
            Ok(Some(run)) if run.status.is_in_progress() => {
                tracing::debug!(target: TRACING_TARGET, connection_id = %connection.id, "Sync already in progress; skipping duplicate job");
                return Settle::Ack;
            }
            Ok(_) => {}
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, connection_id = %connection.id, error = %err, "Failed to check in-progress run");
                return retry(delivery);
            }
        }

        let connection_id = connection.id;
        let request = match self.begin_run(connection, attempt).await {
            Ok(Some(request)) => request,
            // The connection is no longer scheduled (schedule removed or cron
            // cleared after enqueue); drop the job quietly.
            Ok(None) => return Settle::Ack,
            // A lost race on the one-active-run index is benign and expected
            // (at-least-once delivery). A transient failure (an exhausted pool, a
            // lost connection) is retried; anything else — a decrypt failure or a mis-scheduled
            // non-sync connection — is a real fault that a retry cannot fix.
            Err(err) if err.kind() == ErrorKind::Conflict => {
                tracing::debug!(target: TRACING_TARGET, connection_id = %connection_id, "Skipping scheduled run: already active");
                return Settle::Ack;
            }
            Err(err) if err.kind() == ErrorKind::ServiceUnavailable => {
                tracing::warn!(target: TRACING_TARGET, connection_id = %connection_id, error = %err, "Failed to open scheduled run; retrying");
                return retry(delivery);
            }
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, connection_id = %connection_id, error = %err, "Failed to open scheduled run");
                return Settle::Ack;
            }
        };

        // Capture the identifiers needed after the transfer takes ownership of
        // the request (and its connection). Runs through the shared transfer path.
        let run_id = request.run_id;
        self.sync.run_transfer(request).await;

        if self.run_failed(run_id).await {
            tracing::info!(target: TRACING_TARGET, %connection_id, attempt, "Scheduled sync failed");
            return retry(delivery);
        }
        Settle::Ack
    }

    /// Whether a run ended failed (a cancelled or completed run is not retried).
    async fn run_failed(&self, run_id: Uuid) -> bool {
        let found = match self.infra.postgres.get_connection().await {
            Ok(mut conn) => conn.find_workspace_connection_sync_by_id(run_id).await,
            Err(err) => Err(err),
        };
        match found {
            Ok(run) => run.is_some_and(|run| run.status.is_failed()),
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to check run outcome for retry");
                false
            }
        }
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
        // Create the run and record its start event atomically. A background run
        // has no request, so its origin carries no client IP or user agent.
        let origin = event::EventOrigin {
            workspace_id: connection.workspace_id,
            account_id,
            security: &SecurityContext::default(),
        };
        let run = self
            .sync
            .create_run(&mut conn, origin, new_run, &connection)
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

/// Redelivers a job after a linear backoff, or acks it on its final attempt (a
/// retry would be dropped by the stream anyway).
fn retry(delivery: Delivery) -> Settle {
    if delivery.is_final {
        return Settle::Ack;
    }
    let attempt = u32::try_from(delivery.attempt).unwrap_or(u32::MAX);
    Settle::RetryAfter(RETRY_BACKOFF.saturating_mul(attempt))
}
