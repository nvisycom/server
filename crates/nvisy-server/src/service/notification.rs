//! In-app notification emitter.

use nvisy_nats::stream::BroadcastStream;
use nvisy_postgres::PgConn;
use nvisy_postgres::model::NewAccountNotification;
use nvisy_postgres::query::{AccountNotificationRepository, WorkspaceMemberRepository};
use nvisy_postgres::types::{NotificationPayload, WorkspaceRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Result;
use crate::service::Infra;

/// Tracing target for notification emission.
const TRACING_TARGET: &str = "nvisy_server::service::notification";

/// An account's current unread-notification count, broadcast on the account's
/// core-NATS unread subject.
///
/// Fan-out to any watching SSE connections so a badge updates live; the stored
/// rows in Postgres remain the source of truth, so a missed broadcast is
/// recoverable by re-reading the count.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadCountEvent {
    /// The account's current number of unread notifications.
    pub unread_count: i64,
}

/// The core-NATS subject an account's unread-count changes are broadcast on.
#[must_use]
fn unread_subject(account_id: Uuid) -> String {
    format!("notifications.accounts.{account_id}.unread")
}

/// Creates in-app notifications for domain events.
///
/// Cheaply cloneable (holds the shared [`Infra`] clients, all `Arc`-backed).
/// Each notification is a stored row of `notify_type` + typed params (the client
/// localizes the copy); a member's `notification_events_app` preferences decide
/// whether the event reaches them.
#[derive(Clone)]
#[must_use = "the emitter does nothing unless you notify with it"]
pub struct NotificationEmitter {
    infra: Infra,
}

impl NotificationEmitter {
    /// Creates a new [`NotificationEmitter`].
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Subscribes to an account's unread-count broadcasts, yielding each
    /// [`UnreadCountEvent`].
    ///
    /// Used by the unread SSE endpoint to push a live badge count to a watching
    /// client.
    pub async fn subscribe_unread(
        &self,
        account_id: Uuid,
    ) -> crate::handler::Result<BroadcastStream<UnreadCountEvent>> {
        let stream = self
            .infra
            .nats
            .subscribe_broadcast::<UnreadCountEvent>(unread_subject(account_id))
            .await?;
        Ok(stream)
    }

    /// Recomputes an account's unread count and broadcasts it on the account's
    /// core-NATS subject (best-effort; the stored rows are authoritative, so a
    /// dropped broadcast is recoverable by re-reading the count).
    ///
    /// Takes the caller's open connection so the count read shares the same
    /// connection as the insert or update that triggered it.
    pub async fn broadcast_unread(&self, conn: &mut PgConn, account_id: Uuid) {
        let unread_count = match conn.count_unread_account_notifications(account_id).await {
            Ok(count) => count,
            Err(err) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    error = %err,
                    %account_id,
                    "Failed to read unread count for broadcast",
                );
                return;
            }
        };

        let event = UnreadCountEvent { unread_count };
        if let Err(err) = self
            .infra
            .nats
            .publish_broadcast(unread_subject(account_id), &event)
            .await
        {
            tracing::debug!(
                target: TRACING_TARGET,
                error = %err,
                %account_id,
                "Failed to broadcast unread count",
            );
        }
    }

    /// Notifies a single account unconditionally, without a workspace membership
    /// or preference check.
    ///
    /// For events that target someone who is not (yet) a workspace member — e.g.
    /// `member:invited`, where the recipient is being invited *to* the workspace.
    /// Best-effort: callers log-and-continue on error.
    pub async fn notify_account_direct(
        &self,
        account_id: Uuid,
        payload: NotificationPayload,
    ) -> Result<()> {
        let (event, params) = payload.into_stored();
        let mut conn = self.infra.postgres.get_connection().await?;
        conn.create_account_notification(NewAccountNotification {
            account_id,
            notify_type: event,
            params,
            expires_at: None,
        })
        .await?;
        tracing::debug!(target: TRACING_TARGET, %account_id, event = %event, "Notification created");

        self.broadcast_unread(&mut conn, account_id).await;
        Ok(())
    }

    /// Notifies a single account of an event, honoring the recipient's in-app
    /// notification preferences within `workspace_id`.
    ///
    /// The notification is skipped (returning `false`) when the account is not a
    /// member of the workspace, or has narrowed its `notification_events_app` to
    /// exclude this event. A member with the default (empty is treated as "all",
    /// as is any list that contains the event) receives it.
    ///
    /// Best-effort by contract: callers log-and-continue on error so a failed
    /// notification never fails the operation that triggered it.
    pub async fn notify_account(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        payload: NotificationPayload,
    ) -> Result<bool> {
        let (event, params) = payload.into_stored();

        let mut conn = self.infra.postgres.get_connection().await?;

        // Respect the member's in-app preferences. An empty preference list means
        // "all events" (the column defaults to every event), so only a member who
        // has narrowed the set and excluded this event is skipped.
        let member = conn.find_workspace_member(workspace_id, account_id).await?;
        let deliver = match member {
            Some(member) => {
                let prefs = member.app_notification_events();
                prefs.is_empty() || prefs.contains(&event)
            }
            None => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    %account_id,
                    %workspace_id,
                    "Account is not a workspace member; skipping notification",
                );
                false
            }
        };

        if !deliver {
            return Ok(false);
        }

        conn.create_account_notification(NewAccountNotification {
            account_id,
            notify_type: event,
            params,
            expires_at: None,
        })
        .await?;

        tracing::debug!(target: TRACING_TARGET, %account_id, event = %event, "Notification created");

        self.broadcast_unread(&mut conn, account_id).await;
        Ok(true)
    }

    /// Notifies every member of `workspace_id` holding one of `roles` and
    /// accepting the event in-app, skipping `exclude` (e.g. the actor who
    /// triggered it). Returns how many notifications were created.
    ///
    /// Two queries total regardless of recipient count: one resolves the
    /// preference-filtered recipients, one batch-inserts their rows.
    pub async fn notify_workspace_roles(
        &self,
        workspace_id: Uuid,
        roles: &[WorkspaceRole],
        exclude: Option<Uuid>,
        payload: NotificationPayload,
    ) -> Result<usize> {
        let (event, params) = payload.into_stored();

        let mut conn = self.infra.postgres.get_connection().await?;
        let recipients = conn
            .notification_recipients_by_roles(workspace_id, roles, event)
            .await?;

        let account_ids: Vec<Uuid> = recipients
            .into_iter()
            .filter(|account_id| Some(*account_id) != exclude)
            .collect();

        let rows: Vec<NewAccountNotification> = account_ids
            .iter()
            .map(|&account_id| NewAccountNotification {
                account_id,
                notify_type: event,
                params: params.clone(),
                expires_at: None,
            })
            .collect();

        let created = conn.create_account_notifications(rows).await?;
        tracing::debug!(target: TRACING_TARGET, event = %event, created, "Broadcast notifications created");

        // Push each recipient's fresh unread count so their badge updates live.
        for account_id in account_ids {
            self.broadcast_unread(&mut conn, account_id).await;
        }

        Ok(created)
    }
}
