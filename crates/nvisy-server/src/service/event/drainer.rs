//! The event-outbox drainer: projects pending events onto their sinks.
//!
//! A background worker claims batches of pending [`EventOutbox`] rows, decodes
//! each into a [`WorkspaceEvent`], and fans it out to the three sinks — the
//! activity log, the webhook stream, and notifications — then marks the row
//! processed. A sink failure leaves the row pending for a later retry. This is
//! the one place the event → sink projection lives, off the request path.

use std::time::Duration;

use nvisy_postgres::PgConn;
use nvisy_postgres::model::{EventOutbox, NewWorkspaceActivity};
use nvisy_postgres::query::{EventOutboxRepository, WorkspaceActivityRepository};
use nvisy_postgres::types::{
    ActivityPayload, ConnectionActivityParams, ConnectionSyncCompletedParams,
    ConnectionSyncFailedParams, FileActivityParams, Handle, InviteActivityParams, Json,
    MemberActivityParams, NotificationPayload, PipelineActivityParams, PipelineRunActivityParams,
    PipelineRunAnalyzedParams, PipelineRunCompletedParams, PipelineRunFailedParams,
    PolicyActivityParams, WebhookActivityParams, WebhookEvent, WorkspaceActivityParams,
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::handler::Result;
use crate::service::event::{FileRef, PipelineRunRef, WorkspaceEvent};
use crate::service::{Infra, NotificationEmitter, WebhookEmitter, Worker};

/// Tracing target for the outbox drainer.
const TRACING_TARGET: &str = "nvisy_server::service::event::drainer";

/// How often the drainer polls for pending events. Short, since it is the
/// delivery latency for the activity log, webhooks, and notifications.
const TICK_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum events drained per tick, bounding the work (and lock hold) per pass.
const DRAIN_BATCH: i64 = 100;

/// Drains the event outbox, projecting each pending event onto its sinks.
pub struct EventOutboxDrainer {
    infra: Infra,
    webhook: WebhookEmitter,
    notification: NotificationEmitter,
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
                _ = ticker.tick() => self.tick().await,
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
    /// pending set is drained.
    async fn tick(&self) {
        loop {
            match self.drain_batch().await {
                Ok(count) if count < DRAIN_BATCH as usize => break,
                Ok(_) => {}
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Outbox drain pass failed");
                    break;
                }
            }
        }
    }

    /// Claims one batch and projects each event, returning how many rows it
    /// claimed. Each row is marked processed on success, or has its attempt
    /// recorded (staying pending) on a decode/sink failure.
    async fn drain_batch(&self) -> Result<usize> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let batch = conn.claim_outbox_batch(DRAIN_BATCH).await?;
        let count = batch.len();

        for row in batch {
            let id = row.id;
            let outcome = match serde_json::from_value::<WorkspaceEvent>(row.event.clone()) {
                Ok(event) => self.deliver(&mut conn, &row, event).await,
                // A row that will never decode is poison; treat it as a failure so
                // it stays pending and surfaces rather than blocking silently.
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, %id, "Failed to decode outbox event");
                    Err(())
                }
            };

            let marked = match outcome {
                Ok(()) => conn.mark_outbox_processed(id).await,
                Err(()) => conn.record_outbox_failure(id).await,
            };
            if let Err(err) = marked {
                tracing::warn!(target: TRACING_TARGET, error = %err, %id, "Failed to update outbox row after delivery");
            }
        }

        Ok(count)
    }

    /// Projects one event onto its sinks, returning `Ok` once the durable sink —
    /// the activity log — is written; the row is then marked processed. A failed
    /// activity write returns `Err`, leaving the row pending for a later retry.
    /// The webhook and notification sinks are best-effort: a failure there is
    /// logged and does not hold up the row (delivery to them is at-most-once).
    async fn deliver(
        &self,
        conn: &mut PgConn,
        row: &EventOutbox,
        event: WorkspaceEvent,
    ) -> std::result::Result<(), ()> {
        let workspace_id = row.workspace_id;
        let actor = row.account_id;

        // Activity log — the durable sink. Every event produces one entry, stamped
        // with the stored security context.
        let activity = activity_of(&event);
        let activity_row = NewWorkspaceActivity {
            workspace_id,
            account_id: actor,
            activity_type: activity.activity_type(),
            params: Json::encode(&activity),
            ip_address: row.ip_address,
            user_agent: row.user_agent.clone(),
        };
        if let Err(err) = conn.log_activity(activity_row).await {
            tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, "Failed to record activity; leaving event pending");
            return Err(());
        }

        // Webhook — only the events the webhook vocabulary carries.
        if let Some((webhook_event, data)) = webhook_of(&event) {
            let resource_id = resource_id_of(&event);
            if let Err(err) = self
                .webhook
                .emit(workspace_id, webhook_event, resource_id, Some(actor), data)
                .await
            {
                tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, "Failed to emit webhook event");
            }
        }

        // Notification — only the events that raise one.
        if let Some((recipient, payload)) = notification_of(event)
            && let Err(err) = self
                .notification
                .notify_account(workspace_id, recipient, payload)
                .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, %workspace_id, %recipient, "Failed to notify");
        }

        Ok(())
    }
}

/// The activity-log payload for an event. Total: every event is recorded.
fn activity_of(event: &WorkspaceEvent) -> ActivityPayload {
    use WorkspaceEvent as E;

    let workspace = |workspace_slug: &Handle| WorkspaceActivityParams {
        workspace_slug: workspace_slug.clone(),
    };
    let member = |member_username: &Handle| MemberActivityParams {
        member_username: member_username.clone(),
    };
    let invite = |invite_id: Uuid, email: &str| InviteActivityParams {
        invite_id,
        email: email.to_owned(),
    };
    let connection = |connection_id: Uuid| ConnectionActivityParams { connection_id };
    let webhook = |webhook_id: Uuid| WebhookActivityParams { webhook_id };
    let file_params = |file: &FileRef| FileActivityParams {
        file_id: file.file_id,
        file_name: file.file_name.clone(),
    };
    let pipeline = |pipeline_slug: &Handle| PipelineActivityParams {
        pipeline_slug: pipeline_slug.clone(),
    };
    let run = |run: &PipelineRunRef| PipelineRunActivityParams {
        pipeline_slug: run.pipeline_slug.clone(),
        run_id: run.run_id,
    };
    let policy = |policy_id: Uuid| PolicyActivityParams { policy_id };

    match event {
        E::WorkspaceCreated { workspace_slug } => {
            ActivityPayload::WorkspaceCreated(workspace(workspace_slug))
        }
        E::WorkspaceUpdated { workspace_slug } => {
            ActivityPayload::WorkspaceUpdated(workspace(workspace_slug))
        }
        E::WorkspaceDeleted { workspace_slug, .. } => {
            ActivityPayload::WorkspaceDeleted(workspace(workspace_slug))
        }
        E::MemberAdded { member_username } => ActivityPayload::MemberAdded(member(member_username)),
        E::MemberUpdated(m) => ActivityPayload::MemberUpdated(member(&m.member_username)),
        E::MemberDeleted(m) => ActivityPayload::MemberDeleted(member(&m.member_username)),
        E::InviteCreated(i) => ActivityPayload::InviteCreated(invite(i.invite_id, &i.email)),
        E::InviteAccepted(i) => ActivityPayload::InviteAccepted(invite(i.invite_id, &i.email)),
        E::InviteDeclined(i) => ActivityPayload::InviteDeclined(invite(i.invite_id, &i.email)),
        E::InviteCanceled(i) => ActivityPayload::InviteCanceled(invite(i.invite_id, &i.email)),
        E::ConnectionCreated(c) => ActivityPayload::ConnectionCreated(connection(c.connection_id)),
        E::ConnectionUpdated(c) => ActivityPayload::ConnectionUpdated(connection(c.connection_id)),
        E::ConnectionDeleted(c) => ActivityPayload::ConnectionDeleted(connection(c.connection_id)),
        E::ConnectionSyncCompleted { connection_id, .. } => {
            ActivityPayload::ConnectionSyncCompleted(connection(*connection_id))
        }
        E::ConnectionSyncFailed { connection_id, .. } => {
            ActivityPayload::ConnectionSyncFailed(connection(*connection_id))
        }
        E::WebhookCreated(w) => ActivityPayload::WebhookCreated(webhook(w.webhook_id)),
        E::WebhookUpdated(w) => ActivityPayload::WebhookUpdated(webhook(w.webhook_id)),
        E::WebhookDeleted(w) => ActivityPayload::WebhookDeleted(webhook(w.webhook_id)),
        E::FileCreated { file, .. } => ActivityPayload::FileCreated(file_params(file)),
        E::FileUpdated(f) => ActivityPayload::FileUpdated(file_params(f)),
        E::FileDeleted(f) => ActivityPayload::FileDeleted(file_params(f)),
        E::PipelineCreated(p) => ActivityPayload::PipelineCreated(pipeline(&p.pipeline_slug)),
        E::PipelineUpdated(p) => ActivityPayload::PipelineUpdated(pipeline(&p.pipeline_slug)),
        E::PipelineDeleted(p) => ActivityPayload::PipelineDeleted(pipeline(&p.pipeline_slug)),
        E::PipelineRunStarted(r) => ActivityPayload::PipelineRunStarted(run(r)),
        E::PipelineRunAnalyzed { run: r, .. } => ActivityPayload::PipelineRunAnalyzed(run(r)),
        E::PipelineRunCompleted { run: r, .. } => ActivityPayload::PipelineRunCompleted(run(r)),
        E::PipelineRunFailed { run: r, .. } => ActivityPayload::PipelineRunFailed(run(r)),
        E::PolicyCreated(p) => ActivityPayload::PolicyCreated(policy(p.policy_id)),
        E::PolicyUpdated(p) => ActivityPayload::PolicyUpdated(policy(p.policy_id)),
        E::PolicyDeleted(p) => ActivityPayload::PolicyDeleted(policy(p.policy_id)),
    }
}

/// The webhook event (and any extra body) for an event, or `None` for events the
/// webhook vocabulary does not carry (workspace lifecycle, invites, webhook CRUD).
fn webhook_of(event: &WorkspaceEvent) -> Option<(WebhookEvent, Option<Value>)> {
    use WorkspaceEvent as E;
    let webhook = match event {
        E::MemberAdded { .. } => (WebhookEvent::MemberAdded, None),
        E::MemberUpdated(..) => (WebhookEvent::MemberUpdated, None),
        E::MemberDeleted(..) => (WebhookEvent::MemberDeleted, None),
        E::ConnectionCreated(..) => (WebhookEvent::ConnectionCreated, None),
        E::ConnectionUpdated(..) => (WebhookEvent::ConnectionUpdated, None),
        E::ConnectionDeleted(..) => (WebhookEvent::ConnectionDeleted, None),
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
        E::PipelineRunStarted(..) => (WebhookEvent::PipelineRunStarted, None),
        E::PipelineRunAnalyzed { .. } => (WebhookEvent::PipelineRunAnalyzed, None),
        E::PipelineRunCompleted { .. } => (WebhookEvent::PipelineRunCompleted, None),
        E::PipelineRunFailed { .. } => (WebhookEvent::PipelineRunFailed, None),
        E::PolicyCreated(..) => (WebhookEvent::PolicyCreated, None),
        E::PolicyUpdated(..) => (WebhookEvent::PolicyUpdated, None),
        E::PolicyDeleted(..) => (WebhookEvent::PolicyDeleted, None),
        E::WorkspaceCreated { .. }
        | E::WorkspaceUpdated { .. }
        | E::WorkspaceDeleted { .. }
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
            records_synced,
            notify,
        } => notify.map(|to| {
            (
                to,
                NotificationPayload::ConnectionSyncCompleted(ConnectionSyncCompletedParams {
                    connection_id,
                    records_synced,
                }),
            )
        }),
        E::ConnectionSyncFailed {
            connection_id,
            error,
            notify,
        } => notify.map(|to| {
            (
                to,
                NotificationPayload::ConnectionSyncFailed(ConnectionSyncFailedParams {
                    connection_id,
                    error,
                }),
            )
        }),
        E::PipelineRunAnalyzed {
            run,
            input_file_name,
            notify,
        } => Some((
            notify,
            NotificationPayload::PipelineRunAnalyzed(PipelineRunAnalyzedParams {
                run_id: run.run_id,
                pipeline_slug: run.pipeline_slug.to_string(),
                input_file_name,
            }),
        )),
        E::PipelineRunCompleted {
            run,
            input_file_name,
            notify,
        } => Some((
            notify,
            NotificationPayload::PipelineRunCompleted(PipelineRunCompletedParams {
                run_id: run.run_id,
                pipeline_slug: run.pipeline_slug.to_string(),
                input_file_name,
            }),
        )),
        E::PipelineRunFailed {
            run,
            input_file_name,
            error,
            notify,
        } => Some((
            notify,
            NotificationPayload::PipelineRunFailed(PipelineRunFailedParams {
                run_id: run.run_id,
                pipeline_slug: run.pipeline_slug.to_string(),
                input_file_name,
                error,
            }),
        )),
        _ => None,
    }
}

/// The affected resource's id, for the webhook payload. Meaningful only for
/// events that raise a webhook (see [`webhook_of`]).
fn resource_id_of(event: &WorkspaceEvent) -> Uuid {
    use WorkspaceEvent as E;
    match event {
        E::WorkspaceDeleted { workspace_id, .. } => *workspace_id,
        E::MemberUpdated(m) | E::MemberDeleted(m) => m.member_id,
        E::InviteCreated(i)
        | E::InviteAccepted(i)
        | E::InviteDeclined(i)
        | E::InviteCanceled(i) => i.invite_id,
        E::ConnectionCreated(c) | E::ConnectionUpdated(c) | E::ConnectionDeleted(c) => {
            c.connection_id
        }
        E::ConnectionSyncCompleted { connection_id, .. }
        | E::ConnectionSyncFailed { connection_id, .. } => *connection_id,
        E::WebhookCreated(w) | E::WebhookUpdated(w) | E::WebhookDeleted(w) => w.webhook_id,
        E::FileCreated { file, .. } => file.file_id,
        E::FileUpdated(f) | E::FileDeleted(f) => f.file_id,
        E::PipelineCreated(p) | E::PipelineUpdated(p) | E::PipelineDeleted(p) => p.pipeline_id,
        E::PipelineRunStarted(r) => r.run_id,
        E::PipelineRunAnalyzed { run, .. }
        | E::PipelineRunCompleted { run, .. }
        | E::PipelineRunFailed { run, .. } => run.run_id,
        E::PolicyCreated(p) | E::PolicyUpdated(p) | E::PolicyDeleted(p) => p.policy_id,
        E::WorkspaceCreated { .. } | E::WorkspaceUpdated { .. } | E::MemberAdded { .. } => {
            Uuid::nil()
        }
    }
}
