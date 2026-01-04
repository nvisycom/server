//! Account notifications repository for managing notification operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{AccountNotification, NewAccountNotification, UpdateAccountNotification};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for account notification database operations.
///
/// Handles user notifications including creation, delivery tracking, read status
/// management, and cleanup operations.
pub trait AccountNotificationRepository {
    /// Creates a new account notification.
    fn create_account_notification(
        &mut self,
        new_notification: NewAccountNotification,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Finds an account notification by its unique identifier.
    fn find_account_notification_by_id(
        &mut self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<AccountNotification>>> + Send;

    /// Lists account notifications with offset pagination.
    ///
    /// Excludes expired notifications, ordered by creation date.
    fn offset_list_account_notifications(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountNotification>>> + Send;

    /// Lists account notifications with cursor pagination.
    ///
    /// Excludes expired notifications, ordered by creation date descending.
    fn cursor_list_account_notifications(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<AccountNotification>>> + Send;

    /// Marks all unread account notifications as read.
    ///
    /// Returns the count of notifications marked as read.
    fn mark_all_account_notifications_as_read(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Deletes all expired account notifications system-wide.
    ///
    /// Returns the count of deleted notifications.
    fn delete_expired_account_notifications(
        &mut self,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Counts unread account notifications.
    fn count_unread_account_notifications(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl AccountNotificationRepository for PgConnection {
    async fn create_account_notification(
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

    async fn find_account_notification_by_id(
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

    async fn offset_list_account_notifications(
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

    async fn cursor_list_account_notifications(
        &mut self,
        acct_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<AccountNotification>> {
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
                    .map_err(PgError::from)?,
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

    async fn mark_all_account_notifications_as_read(
        &mut self,
        account_id: Uuid,
    ) -> PgResult<usize> {
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

    async fn delete_expired_account_notifications(&mut self) -> PgResult<usize> {
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

    async fn count_unread_account_notifications(&mut self, account_id: Uuid) -> PgResult<i64> {
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
