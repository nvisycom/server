//! Account notifications repository for managing notification operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{AccountNotification, NewAccountNotification, UpdateAccountNotification};
use crate::types::{CursorPage, CursorPagination};
use crate::{Error, PgConnection, Result, schema};

/// Repository for account notification database operations.
///
/// Handles user notifications including creation, delivery tracking, read status
/// management, and cleanup operations.
pub trait AccountNotificationRepository {
    /// Creates a new account notification.
    fn create_account_notification(
        &mut self,
        new_notification: NewAccountNotification,
    ) -> impl Future<Output = Result<AccountNotification>> + Send;

    /// Creates many account notifications in one statement, returning the number
    /// inserted. Used to fan a broadcast out to its recipients in a single query.
    fn create_account_notifications(
        &mut self,
        new_notifications: Vec<NewAccountNotification>,
    ) -> impl Future<Output = Result<usize>> + Send;

    /// Lists account notifications with cursor pagination.
    ///
    /// Excludes expired notifications, ordered by creation date descending.
    fn cursor_list_account_notifications(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = Result<CursorPage<AccountNotification>>> + Send;

    /// Marks all unread account notifications as read.
    ///
    /// Returns the count of notifications marked as read.
    fn mark_all_account_notifications_as_read(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = Result<usize>> + Send;

    /// Marks a single notification as read, scoped to its owning account.
    ///
    /// Returns `true` if a matching notification was updated. The `account_id`
    /// filter is part of the `WHERE` clause, so a notification belonging to
    /// another account (or a missing id) updates nothing and returns `false` —
    /// there is no cross-account read or probe.
    fn mark_account_notification_as_read(
        &mut self,
        account_id: Uuid,
        notification_id: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Counts unread account notifications.
    fn count_unread_account_notifications(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = Result<i64>> + Send;
}

impl AccountNotificationRepository for PgConnection {
    async fn create_account_notification(
        &mut self,
        new_notification: NewAccountNotification,
    ) -> Result<AccountNotification> {
        use schema::account_notifications;

        diesel::insert_into(account_notifications::table)
            .values(&new_notification)
            .returning(AccountNotification::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn create_account_notifications(
        &mut self,
        new_notifications: Vec<NewAccountNotification>,
    ) -> Result<usize> {
        use schema::account_notifications;

        if new_notifications.is_empty() {
            return Ok(0);
        }

        diesel::insert_into(account_notifications::table)
            .values(&new_notifications)
            .execute(self)
            .await
            .map_err(Error::from)
    }

    async fn cursor_list_account_notifications(
        &mut self,
        acct_id: Uuid,
        pagination: CursorPagination,
    ) -> Result<CursorPage<AccountNotification>> {
        use diesel::dsl::{count_star, now};
        use schema::account_notifications::{self, dsl};

        let base_filter = dsl::account_id
            .eq(acct_id)
            .and(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)));

        let total = if pagination.include_count {
            Some(
                account_notifications::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            account_notifications::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(AccountNotification::as_select())
                .load(self)
                .await
                .map_err(Error::from)?
        } else {
            account_notifications::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(AccountNotification::as_select())
                .load(self)
                .await
                .map_err(Error::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |n| {
            (n.created_at.into(), n.id)
        }))
    }

    async fn mark_all_account_notifications_as_read(&mut self, account_id: Uuid) -> Result<usize> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            read_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
        };

        diesel::update(
            account_notifications::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::read_at.is_null()),
        )
        .set(&update_data)
        .execute(self)
        .await
        .map_err(Error::from)
    }

    async fn mark_account_notification_as_read(
        &mut self,
        account_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            read_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
        };

        // Scope by account in the WHERE clause: a notification owned by another
        // account (or a missing id) matches no row, so this returns `false`
        // without ever revealing whether the id exists.
        let updated = diesel::update(
            account_notifications::table
                .filter(dsl::id.eq(notification_id))
                .filter(dsl::account_id.eq(account_id)),
        )
        .set(&update_data)
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(updated > 0)
    }

    async fn count_unread_account_notifications(&mut self, account_id: Uuid) -> Result<i64> {
        use diesel::dsl::{count_star, now};
        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::read_at.is_null())
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .select(count_star())
            .get_result(self)
            .await
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use uuid::Uuid;

    use crate::PgConn;
    use crate::model::{AccountNotification, NewAccountNotification};
    use crate::query::AccountNotificationRepository;
    use crate::test_util::TestDatabase;
    use crate::types::CursorPagination;

    /// Creates a notification already past its expiry: `created_at` two hours ago,
    /// `expires_at` an hour after that (so it reads as expired against `now()`
    /// while satisfying the `expires_at > created_at` check).
    async fn expired_notification(
        conn: &mut PgConn,
        account_id: Uuid,
    ) -> anyhow::Result<AccountNotification> {
        let created = Timestamp::now() - Span::new().hours(2);
        let mut new = NewAccountNotification::test(account_id);
        new.created_at = Some(jiff_diesel::Timestamp::from(created));
        new.expires_at = Some(jiff_diesel::Timestamp::from(created + Span::new().hours(1)));
        Ok(conn.create_account_notification(new).await?)
    }

    /// Creates a notification whose `created_at` is an hour old, so two rows in one
    /// test have a strict, deterministic newest-first order.
    async fn old_notification(
        conn: &mut PgConn,
        account_id: Uuid,
    ) -> anyhow::Result<AccountNotification> {
        let mut new = NewAccountNotification::test(account_id);
        new.created_at = Some(jiff_diesel::Timestamp::from(
            Timestamp::now() - Span::new().hours(1),
        ));
        Ok(conn.create_account_notification(new).await?)
    }

    #[tokio::test]
    async fn cursor_list_excludes_expired_and_orders_newest_first() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // Two live notifications (the first inserted an hour old so the newest-first
        // order is deterministic) and one already expired.
        let first = old_notification(&mut conn, account_id).await?;
        let second = conn
            .create_account_notification(NewAccountNotification::test(account_id))
            .await?;
        let _expired = expired_notification(&mut conn, account_id).await?;

        let page = conn
            .cursor_list_account_notifications(account_id, CursorPagination::new(50))
            .await?;

        // The expired one is filtered out; the newest live one comes first.
        let ids: Vec<_> = page.items.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![second.id, first.id]);
        Ok(())
    }

    #[tokio::test]
    async fn create_many_inserts_each_and_counts_them() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let batch = vec![
            NewAccountNotification::test(account_id),
            NewAccountNotification::test(account_id),
            NewAccountNotification::test(account_id),
        ];
        assert_eq!(conn.create_account_notifications(batch).await?, 3);
        // An empty batch is a no-op, not an error.
        assert_eq!(conn.create_account_notifications(vec![]).await?, 0);

        assert_eq!(
            conn.count_unread_account_notifications(account_id).await?,
            3
        );
        Ok(())
    }

    #[tokio::test]
    async fn mark_as_read_is_scoped_to_the_owning_account() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let owner = db.seed_account().await;
        let other = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let notification = conn
            .create_account_notification(NewAccountNotification::test(owner))
            .await?;

        // Another account cannot mark it read; nothing is updated.
        assert!(
            !conn
                .mark_account_notification_as_read(other, notification.id)
                .await?
        );
        assert_eq!(conn.count_unread_account_notifications(owner).await?, 1);

        // The owner can, and the unread count drops to zero.
        assert!(
            conn.mark_account_notification_as_read(owner, notification.id)
                .await?
        );
        assert_eq!(conn.count_unread_account_notifications(owner).await?, 0);
        Ok(())
    }

    #[tokio::test]
    async fn mark_all_as_read_clears_only_the_accounts_unread() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let _ = conn
            .create_account_notifications(vec![
                NewAccountNotification::test(account_id),
                NewAccountNotification::test(account_id),
            ])
            .await?;

        assert_eq!(
            conn.mark_all_account_notifications_as_read(account_id)
                .await?,
            2
        );
        assert_eq!(
            conn.count_unread_account_notifications(account_id).await?,
            0
        );
        // A second call has nothing left to mark.
        assert_eq!(
            conn.mark_all_account_notifications_as_read(account_id)
                .await?,
            0
        );
        Ok(())
    }
}
