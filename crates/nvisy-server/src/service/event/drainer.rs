//! The event-outbox drainer: projects pending events onto their sinks.
//!
//! A background worker claims batches of due [`WorkspaceEventOutbox`] rows, decodes each
//! into a [`WorkspaceEvent`], and projects it onto the three sinks — the activity
//! log, the webhook stream, and notifications. This is the one place the event →
//! sink projection lives, off the request path.
//!
//! Each batch runs in one transaction: the claim (`FOR UPDATE SKIP LOCKED`), the
//! durable activity-log write, and the row's completion all commit together, so a
//! competing drainer never double-projects a row and a crash mid-batch rolls back
//! cleanly. The activity log is the durable sink and gates the row's completion;
//! a failed activity write defers the row with a backoff (so a bad row cannot spin
//! at the head of the queue) instead of blocking the batch. The webhook and
//! notification sinks are best-effort and fire after the transaction commits — off
//! the durable events — so their network work never holds a database transaction
//! open (webhook delivery has its own retry pipeline).

use std::time::Duration;

use nvisy_postgres::model::{NewWorkspaceActivity, WorkspaceEventOutbox};
use nvisy_postgres::query::{EventOutboxRepository, WorkspaceActivityRepository};
use nvisy_postgres::types::Json;
use nvisy_postgres::{AsyncConnection, PgConn};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::response::{Error, Result};
use crate::service::event::{Notification, NotifyTarget, WorkspaceEvent};
use crate::service::{Infra, NotificationEmitter, WebhookEmitter, Worker};

/// Tracing target for the outbox drainer.
const TRACING_TARGET: &str = "nvisy_server::service::event::drainer";

/// How often the drainer polls for due events. Short, since it is the delivery
/// latency for the activity log, webhooks, and notifications.
const TICK_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum events drained per tick, bounding the work (and lock hold) per pass.
const DRAIN_BATCH: i64 = 100;

/// Base unit of the retry backoff (seconds): a failed row's next attempt is
/// deferred by `RETRY_BACKOFF_BASE_SECS * attempts` (linear), capped at
/// [`RETRY_BACKOFF_MAX_SECS`].
const RETRY_BACKOFF_BASE_SECS: i64 = 30;

/// Ceiling on the retry backoff (seconds), so a long-failing row still retries
/// periodically rather than backing off unboundedly.
const RETRY_BACKOFF_MAX_SECS: i64 = 60 * 60;

/// How many failed attempts a row gets before the drainer gives up on it and
/// dead-letters it (stamps `failed_at`), so a poison event — one that can never
/// decode or project — stops consuming drain cycles instead of retrying forever.
const MAX_ATTEMPTS: i32 = 10;

/// Drains the event outbox, projecting each pending event onto its sinks.
pub struct EventOutboxDrainer {
    infra: Infra,
    webhook: WebhookEmitter,
    notification: NotificationEmitter,
}

/// The tally of one [`drain_batch`](EventOutboxDrainer::drain_batch) pass: of the
/// rows claimed, how many durably processed, how many were deferred for a later
/// retry (failing, backing off), and how many were dead-lettered (given up on).
/// A rising deferred count signals events are struggling to drain; a rising
/// dead-lettered count signals poison rows.
struct DrainPass {
    claimed: usize,
    processed: usize,
    deferred: usize,
    dead_lettered: usize,
}

impl Worker for EventOutboxDrainer {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "event_outbox_drainer"
    }

    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting event-outbox drainer");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => self.tick(&cancel).await,
            }
        }

        tracing::info!(target: TRACING_TARGET, "Event-outbox drainer stopped");
        Ok(())
    }
}

impl EventOutboxDrainer {
    /// Creates a new [`EventOutboxDrainer`].
    pub fn new(infra: Infra) -> Self {
        Self {
            webhook: WebhookEmitter::new(infra.clone()),
            notification: NotificationEmitter::new(infra.clone()),
            infra,
        }
    }

    /// One drain pass: claim and project batches until a short page signals the
    /// due set is drained, or until cancellation is requested.
    ///
    /// The cancellation check between batches keeps shutdown prompt even under a
    /// sustained backlog: a full due set would otherwise loop here indefinitely,
    /// starving the caller's `select!` of the chance to observe `cancel`.
    async fn tick(&self, cancel: &CancellationToken) {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            match self.drain_batch().await {
                Ok(pass) => {
                    // Deferred (retrying) and dead-lettered (given up on) are
                    // distinct failure signals — report them separately rather than
                    // lumping every non-processed row together.
                    if pass.deferred > 0 || pass.dead_lettered > 0 {
                        tracing::warn!(
                            target: TRACING_TARGET,
                            claimed = pass.claimed,
                            processed = pass.processed,
                            deferred = pass.deferred,
                            dead_lettered = pass.dead_lettered,
                            "Outbox drain pass had failing events",
                        );
                    } else if pass.claimed > 0 {
                        tracing::debug!(target: TRACING_TARGET, processed = pass.processed, "Outbox drain pass processed events");
                    }
                    if pass.claimed < DRAIN_BATCH as usize {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Outbox drain pass failed");
                    break;
                }
            }
        }
    }

    /// Drains one batch: claims due rows and writes their durable activity-log
    /// entries in a single transaction, then dispatches the best-effort side
    /// effects for the rows that committed. Returns the [`DrainPass`] tally.
    ///
    /// The transaction holds the claim's `FOR UPDATE SKIP LOCKED` locks through
    /// completion, so the claim, each activity write, and each row's state
    /// transition commit atomically and no other drainer can take the same rows. A
    /// row whose event cannot decode or whose activity write fails is deferred with
    /// a backoff rather than blocking the batch.
    async fn drain_batch(&self) -> Result<DrainPass> {
        let mut conn = self.infra.postgres.get_connection().await?;

        // The transaction returns the pass tally and the committed events, so the
        // side effects below run only for rows that durably landed.
        let (mut pass, committed) = conn
            .transaction(async |conn| {
                let batch = conn.claim_outbox_batch(DRAIN_BATCH).await?;
                let mut pass = DrainPass {
                    claimed: batch.len(),
                    processed: 0,
                    deferred: 0,
                    dead_lettered: 0,
                };
                let mut committed = Vec::with_capacity(pass.claimed);

                for row in batch {
                    match self.record_activity(conn, &row).await {
                        Ok(event) => {
                            conn.mark_outbox_processed(row.id).await?;
                            committed.push((row, event));
                        }
                        // `attempts` counts prior failures; this attempt makes it
                        // `attempts + 1`. Once that reaches the cap, give up on the
                        // row (dead-letter) instead of deferring it forever.
                        Err(()) if row.attempts + 1 >= MAX_ATTEMPTS => {
                            tracing::error!(target: TRACING_TARGET, id = %row.id, attempts = row.attempts + 1, "Dead-lettering outbox event after too many failed attempts");
                            conn.mark_outbox_failed(row.id).await?;
                            pass.dead_lettered += 1;
                        }
                        Err(()) => {
                            conn.defer_outbox_attempt(row.id, retry_backoff(row.attempts))
                                .await?;
                            pass.deferred += 1;
                        }
                    }
                }

                Ok::<_, Error>((pass, committed))
            })
            .await?;

        pass.processed = committed.len();
        for (row, event) in committed {
            self.dispatch_side_effects(&row, &event).await;
        }

        Ok(pass)
    }

    /// Writes the durable activity-log entry for one row, returning its decoded
    /// event so the caller can drive the side effects. Returns `Err` if the row
    /// cannot decode or the activity write fails, so the caller defers it.
    ///
    /// Runs inside the batch transaction. The activity write itself runs in a
    /// nested transaction (a savepoint): a Postgres statement error there — which
    /// would otherwise poison the whole batch transaction and abort every later
    /// `mark_*`/`defer_*` — is contained to the savepoint, so the outer transaction
    /// stays healthy and the caller can still defer or dead-letter this row.
    async fn record_activity(
        &self,
        conn: &mut PgConn,
        row: &WorkspaceEventOutbox,
    ) -> std::result::Result<WorkspaceEvent, ()> {
        let event = serde_json::from_value::<WorkspaceEvent>(row.event.clone()).map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, id = %row.id, "Failed to decode outbox event");
        })?;

        let activity = event.activity();
        let activity_row = NewWorkspaceActivity {
            workspace_id: row.workspace_id,
            account_id: row.account_id,
            activity_type: activity.activity_type(),
            params: Json::encode(&activity),
            ip_address: row.ip_address,
            user_agent: row.user_agent.clone(),
        };
        conn.transaction(async |conn| conn.log_activity(activity_row).await)
            .await
            .map_err(|err| {
                tracing::warn!(target: TRACING_TARGET, error = %err, id = %row.id, "Failed to record activity; deferring event");
            })?;

        Ok(event)
    }

    /// Dispatches the best-effort side effects for a committed event: the webhook
    /// stream and in-app notifications. Runs after the batch transaction commits,
    /// so these never hold a database transaction open across their network work;
    /// each is at-most-once (webhook delivery has its own retry pipeline).
    async fn dispatch_side_effects(&self, row: &WorkspaceEventOutbox, event: &WorkspaceEvent) {
        let workspace_id = row.workspace_id;
        let actor = row.account_id;

        // Webhook — only the events the webhook vocabulary carries.
        if let Some(delivery) = event.webhook() {
            let resource_id = event.resource_id();
            if let Err(err) = self
                .webhook
                .emit(
                    workspace_id,
                    delivery.event,
                    resource_id,
                    Some(actor),
                    delivery.body,
                )
                .await
            {
                tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, "Failed to emit webhook event");
            }
        }

        // Notifications — an event may raise several, each to its own audience.
        for notification in event.clone().notification() {
            self.dispatch_notification(workspace_id, notification).await;
        }
    }

    /// Delivers one notification to its target audience, honoring recipient
    /// preferences. Best-effort: a failure is logged, never propagated.
    async fn dispatch_notification(&self, workspace_id: Uuid, notification: Notification) {
        let Notification { target, payload } = notification;
        let result = match target {
            NotifyTarget::Account(recipient) => self
                .notification
                .notify_account(workspace_id, recipient, payload)
                .await
                .map(|_delivered| ()),
            NotifyTarget::Roles { roles, exclude } => self
                .notification
                .notify_workspace_roles(workspace_id, &roles, exclude, payload)
                .await
                .map(|_count| ()),
        };
        if let Err(err) = result {
            tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, "Failed to deliver notification");
        }
    }
}

/// The delay in seconds before a failed row's next attempt: linear in `attempts`
/// (the count before this failure), capped at [`RETRY_BACKOFF_MAX_SECS`], so a
/// transient failure retries soon while a persistently failing row backs off the
/// queue head.
fn retry_backoff(attempts: i32) -> i64 {
    let steps = i64::from(attempts.max(0)) + 1;
    RETRY_BACKOFF_BASE_SECS
        .saturating_mul(steps)
        .min(RETRY_BACKOFF_MAX_SECS)
}
