//! Account notifications repository for managing notification operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{AccountNotification, NewAccountNotification, UpdateAccountNotification};
use crate::types::{CursorPage, CursorPagination, NotificationEvent, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for account notification database operations.
///
/// Handles user notifications including creation, delivery tracking, read status
/// management, and cleanup operations.
pub trait AccountNotificationRepository {
    /// Creates a new notification for an account.
    fn create_notification(
        &mut self,
        new_notification: NewAccountNotification,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Finds a notification by its unique identifier.
    fn find_notification_by_id(
        &mut self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<AccountNotification>>> + Send;

    /// Lists active notifications for an account with offset pagination.
    ///
    /// Excludes expired notifications, ordered by creation date.
    fn offset_list_notifications(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountNotification>>> + Send;

    /// Lists notifications for an account with cursor-based pagination.
    ///
    /// Excludes expired notifications, ordered by creation date descending.
    /// Returns a page with total count and next cursor for pagination.
    fn cursor_list_notifications(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<AccountNotification>>> + Send;

    /// Finds notifications filtered by type for an account.
    fn find_notifications_by_type(
        &mut self,
        account_id: Uuid,
        notification_type: NotificationEvent,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountNotification>>> + Send;

    /// Marks a notification as read with current timestamp.
    fn mark_as_read(
        &mut self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Marks a notification as unread by clearing read status.
    fn mark_as_unread(
        &mut self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Marks all unread notifications as read for an account.
    ///
    /// Returns the count of notifications marked as read.
    fn mark_all_as_read(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Permanently deletes a notification.
    fn delete_notification(
        &mut self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Permanently deletes all notifications for an account.
    ///
    /// Returns the count of deleted notifications.
    fn delete_all_notifications(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Deletes all expired notifications system-wide.
    ///
    /// Returns the count of deleted notifications.
    fn delete_expired_notifications(&mut self) -> impl Future<Output = PgResult<usize>> + Send;

    /// Counts unread notifications for an account.
    fn count_unread_notifications(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl AccountNotificationRepository for PgConnection {
    async fn create_notification(
        &mut self,
        new_notification: NewAccountNotification,
    ) -> PgResult<AccountNotification> {
        use schema::account_notifications;

        diesel::insert_into(account_notifications::table)
            .values(&new_notification)
            .returning(AccountNotification::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_notification_by_id(
        &mut self,
        notification_id: Uuid,
    ) -> PgResult<Option<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::id.eq(notification_id))
            .select(AccountNotification::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn offset_list_notifications(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<AccountNotification>> {
        use diesel::dsl::now;
        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn cursor_list_notifications(
        &mut self,
        acct_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<AccountNotification>> {
        use diesel::dsl::{count_star, now};
        use schema::account_notifications::{self, dsl};

        // Build base filter for non-expired notifications
        let base_filter = dsl::account_id
            .eq(acct_id)
            .and(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)));

        // Get total count only if requested
        let total = if pagination.include_count {
            Some(
                account_notifications::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Build query with cursor if provided
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
                .map_err(PgError::from)?
        } else {
            account_notifications::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(AccountNotification::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |n| {
            (n.created_at.into(), n.id)
        }))
    }

    async fn find_notifications_by_type(
        &mut self,
        account_id: Uuid,
        notification_type: NotificationEvent,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<AccountNotification>> {
        use diesel::dsl::now;
        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::notify_type.eq(notification_type))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn mark_as_read(&mut self, notification_id: Uuid) -> PgResult<AccountNotification> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(true),
            read_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
        };

        diesel::update(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .set(&update_data)
            .returning(AccountNotification::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn mark_as_unread(&mut self, notification_id: Uuid) -> PgResult<AccountNotification> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(false),
            read_at: Some(None),
        };

        diesel::update(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .set(&update_data)
            .returning(AccountNotification::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn mark_all_as_read(&mut self, account_id: Uuid) -> PgResult<usize> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(true),
            read_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
        };

        diesel::update(
            account_notifications::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::is_read.eq(false)),
        )
        .set(&update_data)
        .execute(self)
        .await
        .map_err(PgError::from)
    }

    async fn delete_notification(&mut self, notification_id: Uuid) -> PgResult<()> {
        use schema::account_notifications::{self, dsl};

        diesel::delete(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn delete_all_notifications(&mut self, account_id: Uuid) -> PgResult<usize> {
        use schema::account_notifications::{self, dsl};

        diesel::delete(account_notifications::table.filter(dsl::account_id.eq(account_id)))
            .execute(self)
            .await
            .map_err(PgError::from)
    }

    async fn delete_expired_notifications(&mut self) -> PgResult<usize> {
        use diesel::dsl::now;
        use schema::account_notifications::{self, dsl};

        diesel::delete(
            account_notifications::table
                .filter(dsl::expires_at.is_not_null())
                .filter(dsl::expires_at.lt(now)),
        )
        .execute(self)
        .await
        .map_err(PgError::from)
    }

    async fn count_unread_notifications(&mut self, account_id: Uuid) -> PgResult<i64> {
        use diesel::dsl::{count_star, now};
        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::is_read.eq(false))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .select(count_star())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }
}
