//! The event-outbox handler: projects pending events onto their sinks.
//!
//! Each due [`WorkspaceEventOutbox`] row is decoded into a [`WorkspaceEvent`] and
//! projected onto the three sinks — the activity log, the webhook stream, and
//! notifications. This is the one place the event → sink projection lives, off
//! the request path.
//!
//! The activity log is the durable sink: it is written in the drainer's batch
//! transaction and gates the row's completion, so a failed write defers the row.
//! The webhook and notification sinks are best-effort and fire after the batch
//! commits, so their network work never holds a database transaction open
//! (webhook delivery has its own retry pipeline).

use nvisy_postgres::model::{NewWorkspaceActivity, WorkspaceEventOutbox};
use nvisy_postgres::query::WorkspaceActivityRepository;
use nvisy_postgres::types::Json;
use nvisy_postgres::{AsyncConnection, PgConn};
use serde::Deserialize;
use uuid::Uuid;

use crate::service::event::{Notification, NotifyTarget, WorkspaceEvent};
use crate::service::{Infra, NotificationEmitter, WebhookEmitter};
use crate::worker::outbox::{Handled, OutboxDrainer, OutboxHandler};

/// Tracing target for the event projection.
const TRACING_TARGET: &str = "nvisy_server::worker::event";

/// Drains the event outbox, projecting each pending event onto its sinks.
pub type EventOutboxDrainer = OutboxDrainer<EventProjection>;

/// Projects workspace events onto the activity log (durably, in the batch
/// transaction), and onto webhooks and notifications (best-effort, after it
/// commits).
pub struct EventProjection {
    webhook: WebhookEmitter,
    notification: NotificationEmitter,
}

/// A committed event and its origin, handed to the post-commit side effects.
pub struct ProjectedEvent {
    workspace_id: Uuid,
    account_id: Uuid,
    event: WorkspaceEvent,
}

impl OutboxHandler for EventProjection {
    type Done = ProjectedEvent;
    type Row = WorkspaceEventOutbox;

    const NAME: &'static str = "event_outbox_drainer";

    async fn handle(
        &self,
        conn: &mut PgConn,
        row: &WorkspaceEventOutbox,
    ) -> Handled<ProjectedEvent> {
        match self.record_activity(conn, row).await {
            Ok(event) => Handled::Done(Some(ProjectedEvent {
                workspace_id: row.workspace_id,
                account_id: row.account_id,
                event,
            })),
            Err(()) => Handled::Failed,
        }
    }

    async fn after_commit(&self, done: Vec<ProjectedEvent>) {
        for projected in done {
            self.dispatch_side_effects(projected).await;
        }
    }
}

impl EventProjection {
    /// Creates a new [`EventProjection`].
    #[must_use]
    pub fn new(infra: &Infra) -> Self {
        Self {
            webhook: WebhookEmitter::new(infra.clone()),
            notification: NotificationEmitter::new(infra.clone()),
        }
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
        let event = WorkspaceEvent::deserialize(&row.event).map_err(|err| {
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
    async fn dispatch_side_effects(&self, projected: ProjectedEvent) {
        let ProjectedEvent {
            workspace_id,
            account_id: actor,
            event,
        } = projected;

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
        for notification in event.notification() {
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
