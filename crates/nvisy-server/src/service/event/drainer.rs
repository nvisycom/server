//! The event-outbox drainer: projects pending events onto their sinks.
//!
//! A background worker claims batches of due [`EventOutbox`] rows, decodes each
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

use nvisy_postgres::model::{EventOutbox, NewWorkspaceActivity};
use nvisy_postgres::query::{EventOutboxRepository, WorkspaceActivityRepository};
use nvisy_postgres::types::{
    ActivityPayload, ConnectionActivityParams, ConnectionId, ConnectionSyncCompletedParams,
    ConnectionSyncFailedParams, DetectionActivityParams, DetectionCompletedParams,
    DetectionFailedParams, DetectionId, FileActivityParams, Handle, InviteActivityParams, Json,
    MemberActivityParams, NotificationPayload, PipelineActivityParams, PolicyActivityParams,
    RedactionActivityParams, RedactionCreatedParams, RedactionId, WebhookActivityParams,
    WebhookEvent, WebhookId, WorkspaceActivityParams,
};
use nvisy_postgres::{AsyncConnection, PgConn};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::handler::{Error, Result};
use crate::service::event::{
    ConnectionRef, DetectionRef, FileRef, InviteRef, MemberRef, PolicyRef, WebhookRef,
    WorkspaceEvent, WorkspaceRef,
};
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
        row: &EventOutbox,
    ) -> std::result::Result<WorkspaceEvent, ()> {
        let event = serde_json::from_value::<WorkspaceEvent>(row.event.clone()).map_err(|err| {
            tracing::error!(target: TRACING_TARGET, error = %err, id = %row.id, "Failed to decode outbox event");
        })?;

        let activity = activity_of(&event);
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
    async fn dispatch_side_effects(&self, row: &EventOutbox, event: &WorkspaceEvent) {
        let workspace_id = row.workspace_id;
        let actor = row.account_id;

        // Webhook — only the events the webhook vocabulary carries.
        if let Some((webhook_event, data)) = webhook_of(event) {
            let resource_id = resource_id_of(event);
            if let Err(err) = self
                .webhook
                .emit(workspace_id, webhook_event, resource_id, Some(actor), data)
                .await
            {
                tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, "Failed to emit webhook event");
            }
        }

        // Notification — only the events that raise one.
        if let Some((recipient, payload)) = notification_of(event.clone())
            && let Err(err) = self
                .notification
                .notify_account(workspace_id, recipient, payload)
                .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, %recipient, "Failed to notify");
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

/// The activity-log payload for an event. Total: every event is recorded.
fn activity_of(event: &WorkspaceEvent) -> ActivityPayload {
    use WorkspaceEvent as E;

    let workspace = |w: &WorkspaceRef| WorkspaceActivityParams {
        workspace_slug: w.workspace_slug.clone(),
    };
    let member = |m: &MemberRef| MemberActivityParams {
        member_username: m.member_username.clone(),
    };
    let invite = |i: &InviteRef| InviteActivityParams {
        invite_id: i.invite_id,
        email: i.email.clone(),
    };
    let connection = |c: &ConnectionRef| ConnectionActivityParams {
        connection_id: ConnectionId::from_uuid(c.connection_id),
        connection_name: c.connection_name.clone(),
    };
    let connection_sync = |connection_id: Uuid, connection_name: &str| ConnectionActivityParams {
        connection_id: ConnectionId::from_uuid(connection_id),
        connection_name: connection_name.to_owned(),
    };
    let webhook = |w: &WebhookRef| WebhookActivityParams {
        webhook_id: WebhookId::from_uuid(w.webhook_id),
        webhook_name: w.webhook_name.clone(),
    };
    let file_params = |file: &FileRef| FileActivityParams {
        file_id: file.file_id,
        file_name: file.file_name.clone(),
    };
    let pipeline = |pipeline_slug: &Handle| PipelineActivityParams {
        pipeline_slug: pipeline_slug.clone(),
    };
    let detection = |detection: &DetectionRef| DetectionActivityParams {
        pipeline_slug: detection.pipeline_slug.clone(),
        detection_id: DetectionId::from_uuid(detection.detection_id),
    };
    let redaction = |detection: &DetectionRef, redaction_id: Uuid| RedactionActivityParams {
        pipeline_slug: detection.pipeline_slug.clone(),
        redaction_id: RedactionId::from_uuid(redaction_id),
    };
    let policy = |p: &PolicyRef| PolicyActivityParams {
        policy_id: p.policy_id,
        policy_slug: p.policy_slug.clone(),
    };

    match event {
        E::WorkspaceCreated(w) => ActivityPayload::WorkspaceCreated(workspace(w)),
        E::WorkspaceUpdated(w) => ActivityPayload::WorkspaceUpdated(workspace(w)),
        E::WorkspaceDeleted(w) => ActivityPayload::WorkspaceDeleted(workspace(w)),
        E::MemberAdded(m) => ActivityPayload::MemberAdded(member(m)),
        E::MemberUpdated(m) => ActivityPayload::MemberUpdated(member(m)),
        E::MemberDeleted(m) => ActivityPayload::MemberDeleted(member(m)),
        E::InviteCreated(i) => ActivityPayload::InviteCreated(invite(i)),
        E::InviteAccepted(i) => ActivityPayload::InviteAccepted(invite(i)),
        E::InviteDeclined(i) => ActivityPayload::InviteDeclined(invite(i)),
        E::InviteCanceled(i) => ActivityPayload::InviteCanceled(invite(i)),
        E::ConnectionCreated(c) => ActivityPayload::ConnectionCreated(connection(c)),
        E::ConnectionUpdated(c) => ActivityPayload::ConnectionUpdated(connection(c)),
        E::ConnectionDeleted(c) => ActivityPayload::ConnectionDeleted(connection(c)),
        E::ConnectionSyncStarted(c) => ActivityPayload::ConnectionSyncStarted(connection(c)),
        E::ConnectionSyncCompleted {
            connection_id,
            connection_name,
            ..
        } => ActivityPayload::ConnectionSyncCompleted(connection_sync(
            *connection_id,
            connection_name,
        )),
        E::ConnectionSyncFailed {
            connection_id,
            connection_name,
            ..
        } => {
            ActivityPayload::ConnectionSyncFailed(connection_sync(*connection_id, connection_name))
        }
        E::WebhookCreated(w) => ActivityPayload::WebhookCreated(webhook(w)),
        E::WebhookUpdated(w) => ActivityPayload::WebhookUpdated(webhook(w)),
        E::WebhookDeleted(w) => ActivityPayload::WebhookDeleted(webhook(w)),
        E::FileCreated { file, .. } => ActivityPayload::FileCreated(file_params(file)),
        E::FileUpdated(f) => ActivityPayload::FileUpdated(file_params(f)),
        E::FileDeleted(f) => ActivityPayload::FileDeleted(file_params(f)),
        E::PipelineCreated(p) => ActivityPayload::PipelineCreated(pipeline(&p.pipeline_slug)),
        E::PipelineUpdated(p) => ActivityPayload::PipelineUpdated(pipeline(&p.pipeline_slug)),
        E::PipelineDeleted(p) => ActivityPayload::PipelineDeleted(pipeline(&p.pipeline_slug)),
        E::DetectionStarted(d) => ActivityPayload::DetectionStarted(detection(d)),
        E::DetectionCompleted { detection: d, .. } => {
            ActivityPayload::DetectionCompleted(detection(d))
        }
        E::DetectionFailed { detection: d, .. } => ActivityPayload::DetectionFailed(detection(d)),
        E::RedactionCreated {
            detection: d,
            redaction_id,
            ..
        } => ActivityPayload::RedactionCreated(redaction(d, *redaction_id)),
        E::PolicyCreated(p) => ActivityPayload::PolicyCreated(policy(p)),
        E::PolicyUpdated(p) => ActivityPayload::PolicyUpdated(policy(p)),
        E::PolicyDeleted(p) => ActivityPayload::PolicyDeleted(policy(p)),
    }
}

/// The webhook event (and any extra body) for an event, or `None` for events the
/// webhook vocabulary does not carry (workspace lifecycle, invites, webhook CRUD).
fn webhook_of(event: &WorkspaceEvent) -> Option<(WebhookEvent, Option<Value>)> {
    use WorkspaceEvent as E;
    let webhook = match event {
        E::MemberAdded(..) => (WebhookEvent::MemberAdded, None),
        E::MemberUpdated(..) => (WebhookEvent::MemberUpdated, None),
        E::MemberDeleted(..) => (WebhookEvent::MemberDeleted, None),
        E::ConnectionCreated(..) => (WebhookEvent::ConnectionCreated, None),
        E::ConnectionUpdated(..) => (WebhookEvent::ConnectionUpdated, None),
        E::ConnectionDeleted(..) => (WebhookEvent::ConnectionDeleted, None),
        E::ConnectionSyncStarted(..) => (WebhookEvent::ConnectionSyncStarted, None),
        E::ConnectionSyncCompleted { .. } => (WebhookEvent::ConnectionSyncCompleted, None),
        E::ConnectionSyncFailed { .. } => (WebhookEvent::ConnectionSyncFailed, None),
        E::FileCreated {
            file,
            file_size_bytes,
        } => (
            WebhookEvent::FileCreated,
            Some(
                serde_json::json!({ "displayName": file.file_name, "fileSizeBytes": file_size_bytes }),
            ),
        ),
        E::FileUpdated(f) => (
            WebhookEvent::FileUpdated,
            Some(serde_json::json!({ "displayName": f.file_name })),
        ),
        E::FileDeleted(f) => (
            WebhookEvent::FileDeleted,
            Some(serde_json::json!({ "displayName": f.file_name })),
        ),
        E::PipelineCreated(..) => (WebhookEvent::PipelineCreated, None),
        E::PipelineUpdated(..) => (WebhookEvent::PipelineUpdated, None),
        E::PipelineDeleted(..) => (WebhookEvent::PipelineDeleted, None),
        E::DetectionStarted(..) => (WebhookEvent::DetectionStarted, None),
        E::DetectionCompleted { .. } => (WebhookEvent::DetectionCompleted, None),
        E::DetectionFailed { .. } => (WebhookEvent::DetectionFailed, None),
        E::RedactionCreated { .. } => (WebhookEvent::RedactionCreated, None),
        E::PolicyCreated(..) => (WebhookEvent::PolicyCreated, None),
        E::PolicyUpdated(..) => (WebhookEvent::PolicyUpdated, None),
        E::PolicyDeleted(..) => (WebhookEvent::PolicyDeleted, None),
        E::WorkspaceCreated(..)
        | E::WorkspaceUpdated(..)
        | E::WorkspaceDeleted(..)
        | E::InviteCreated(..)
        | E::InviteAccepted(..)
        | E::InviteDeclined(..)
        | E::InviteCanceled(..)
        | E::WebhookCreated(..)
        | E::WebhookUpdated(..)
        | E::WebhookDeleted(..) => return None,
    };
    Some(webhook)
}

/// The in-app notification for an event — recipient and payload — or `None` for
/// events that raise none. Consumes the event, moving its facts into the payload.
fn notification_of(event: WorkspaceEvent) -> Option<(Uuid, NotificationPayload)> {
    use WorkspaceEvent as E;
    match event {
        E::ConnectionSyncCompleted {
            connection_id,
            connection_name,
            records_synced,
            notify,
        } => notify.map(|to| {
            (
                to,
                NotificationPayload::ConnectionSyncCompleted(ConnectionSyncCompletedParams {
                    connection_id: ConnectionId::from_uuid(connection_id),
                    connection_name,
                    records_synced,
                }),
            )
        }),
        E::ConnectionSyncFailed {
            connection_id,
            connection_name,
            error,
            notify,
        } => notify.map(|to| {
            (
                to,
                NotificationPayload::ConnectionSyncFailed(ConnectionSyncFailedParams {
                    connection_id: ConnectionId::from_uuid(connection_id),
                    connection_name,
                    error,
                }),
            )
        }),
        E::DetectionCompleted {
            detection,
            input_file_name,
            notify,
        } => Some((
            notify,
            NotificationPayload::DetectionCompleted(DetectionCompletedParams {
                detection_id: DetectionId::from_uuid(detection.detection_id),
                pipeline_slug: detection.pipeline_slug,
                input_file_name,
            }),
        )),
        E::RedactionCreated {
            detection,
            redaction_id,
            input_file_name,
            notify,
        } => Some((
            notify,
            NotificationPayload::RedactionCreated(RedactionCreatedParams {
                redaction_id: RedactionId::from_uuid(redaction_id),
                detection_id: DetectionId::from_uuid(detection.detection_id),
                pipeline_slug: detection.pipeline_slug,
                input_file_name,
            }),
        )),
        E::DetectionFailed {
            detection,
            input_file_name,
            error,
            notify,
        } => Some((
            notify,
            NotificationPayload::DetectionFailed(DetectionFailedParams {
                detection_id: DetectionId::from_uuid(detection.detection_id),
                pipeline_slug: detection.pipeline_slug,
                input_file_name,
                error,
            }),
        )),
        // Events that raise no in-app notification. Listed explicitly (no wildcard)
        // so a new event forces a deliberate notify / no-notify decision here.
        E::WorkspaceCreated(_)
        | E::WorkspaceUpdated(_)
        | E::WorkspaceDeleted(_)
        | E::MemberAdded(_)
        | E::MemberUpdated(_)
        | E::MemberDeleted(_)
        | E::InviteCreated(_)
        | E::InviteAccepted(_)
        | E::InviteDeclined(_)
        | E::InviteCanceled(_)
        | E::ConnectionCreated(_)
        | E::ConnectionUpdated(_)
        | E::ConnectionDeleted(_)
        | E::ConnectionSyncStarted(_)
        | E::WebhookCreated(_)
        | E::WebhookUpdated(_)
        | E::WebhookDeleted(_)
        | E::FileCreated { .. }
        | E::FileUpdated(_)
        | E::FileDeleted(_)
        | E::PipelineCreated(_)
        | E::PipelineUpdated(_)
        | E::PipelineDeleted(_)
        | E::DetectionStarted(_)
        | E::PolicyCreated(_)
        | E::PolicyUpdated(_)
        | E::PolicyDeleted(_) => None,
    }
}

/// The affected resource's id, for the webhook payload. Every event carries its
/// resource's id, so consumers always receive a real identifier.
fn resource_id_of(event: &WorkspaceEvent) -> Uuid {
    use WorkspaceEvent as E;
    match event {
        E::WorkspaceCreated(w) | E::WorkspaceUpdated(w) | E::WorkspaceDeleted(w) => w.workspace_id,
        E::MemberAdded(m) | E::MemberUpdated(m) | E::MemberDeleted(m) => m.member_id,
        E::InviteCreated(i)
        | E::InviteAccepted(i)
        | E::InviteDeclined(i)
        | E::InviteCanceled(i) => i.invite_id,
        E::ConnectionCreated(c)
        | E::ConnectionUpdated(c)
        | E::ConnectionDeleted(c)
        | E::ConnectionSyncStarted(c) => c.connection_id,
        E::ConnectionSyncCompleted { connection_id, .. }
        | E::ConnectionSyncFailed { connection_id, .. } => *connection_id,
        E::WebhookCreated(w) | E::WebhookUpdated(w) | E::WebhookDeleted(w) => w.webhook_id,
        E::FileCreated { file, .. } => file.file_id,
        E::FileUpdated(f) | E::FileDeleted(f) => f.file_id,
        E::PipelineCreated(p) | E::PipelineUpdated(p) | E::PipelineDeleted(p) => p.pipeline_id,
        E::DetectionStarted(d) => d.detection_id,
        E::DetectionCompleted { detection, .. }
        | E::DetectionFailed { detection, .. }
        | E::RedactionCreated { detection, .. } => detection.detection_id,
        E::PolicyCreated(p) | E::PolicyUpdated(p) | E::PolicyDeleted(p) => p.policy_id,
    }
}
