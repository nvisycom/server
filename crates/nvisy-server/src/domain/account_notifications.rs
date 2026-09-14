//! Account notification domain logic: list, unread count, mark read.
//!
//! Owns the read-status rules a handler would otherwise inline — marking one or
//! all notifications read (scoped to the owning account) and broadcasting the
//! resulting unread count so a watching badge updates live. The unread-count SSE
//! stream stays in the handler (it is transport), subscribing through the same
//! [`NotificationEmitter`].

use nvisy_postgres::PgClient;
use nvisy_postgres::model::AccountNotification;
use nvisy_postgres::query::{AccountNotificationRepository, NotificationCursor};
use nvisy_postgres::types::{CursorPage, CursorPagination};
use uuid::Uuid;

use crate::response::{Error, Result};
use crate::service::NotificationEmitter;

/// Tracing target for account notification domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::account";

/// Lists, counts, and marks read an account's notifications.
///
/// Holds the Postgres client (own-connection-per-call) and the notification
/// emitter (to broadcast the unread count after a mark-read). Resolved per request
/// from [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct AccountNotificationService {
    postgres: PgClient,
    emitter: NotificationEmitter,
}

impl AccountNotificationService {
    /// Creates an [`AccountNotificationService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, emitter: NotificationEmitter) -> Self {
        Self { postgres, emitter }
    }

    /// Lists the account's notifications, most recent first (a pure read; does not
    /// change read status).
    pub async fn list(
        &self,
        account_id: Uuid,
        pagination: CursorPagination<NotificationCursor>,
    ) -> Result<CursorPage<AccountNotification>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_account_notifications(account_id, pagination)
            .await?)
    }

    /// Returns the account's unread notification count.
    pub async fn unread_count(&self, account_id: Uuid) -> Result<i64> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn.count_unread_account_notifications(account_id).await?)
    }

    /// Marks every unread notification read, broadcasting the now-zero count when
    /// any were marked. Returns how many it marked.
    pub async fn mark_all_read(&self, account_id: Uuid) -> Result<i64> {
        let mut conn = self.postgres.get_connection().await?;
        let marked_read = i64::try_from(
            conn.mark_all_account_notifications_as_read(account_id)
                .await?,
        )
        .unwrap_or(i64::MAX);

        // Push the now-zero unread count so a watching badge clears live.
        if marked_read > 0 {
            self.emitter.broadcast_unread(&mut conn, account_id).await;
        }

        tracing::debug!(target: TRACING_TARGET, marked_read, "Notifications marked as read");
        Ok(marked_read)
    }

    /// Marks a single notification read, scoped to the owning account, broadcasting
    /// the decremented count. A notification owned by another account (or a missing
    /// id) is reported as not-found.
    pub async fn mark_read(&self, account_id: Uuid, notification_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let marked = conn
            .mark_account_notification_as_read(account_id, notification_id)
            .await?;
        if !marked {
            return Err(Error::not_found("notification"));
        }

        // Push the decremented unread count so a watching badge updates live.
        self.emitter.broadcast_unread(&mut conn, account_id).await;
        Ok(())
    }
}
