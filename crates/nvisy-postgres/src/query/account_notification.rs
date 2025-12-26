//! Account notifications repository for managing notification operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountNotification, NewAccountNotification, UpdateAccountNotification};
use crate::types::NotificationType;
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for account notification database operations.
///
/// Handles user notifications including creation, delivery tracking, read status
/// management, and cleanup operations.
pub trait AccountNotificationRepository {
    /// Creates a new notification for an account.
    fn create_notification(
        &self,
        new_notification: NewAccountNotification,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Finds a notification by its unique identifier.
    fn find_notification_by_id(
        &self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<AccountNotification>>> + Send;

    /// Finds active notifications for an account.
    ///
    /// Excludes expired notifications, ordered by creation date.
    fn find_notifications_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<AccountNotification>>> + Send;

    /// Finds notifications filtered by type for an account.
    fn find_notifications_by_type(
        &self,
        account_id: Uuid,
        notification_type: NotificationType,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<AccountNotification>>> + Send;

    /// Marks a notification as read with current timestamp.
    fn mark_as_read(
        &self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Marks a notification as unread by clearing read status.
    fn mark_as_unread(
        &self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountNotification>> + Send;

    /// Marks all unread notifications as read for an account.
    ///
    /// Returns the count of notifications marked as read.
    fn mark_all_as_read(&self, account_id: Uuid) -> impl Future<Output = PgResult<usize>> + Send;

    /// Permanently deletes a notification.
    fn delete_notification(
        &self,
        notification_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Permanently deletes all notifications for an account.
    ///
    /// Returns the count of deleted notifications.
    fn delete_all_notifications(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Deletes all expired notifications system-wide.
    ///
    /// Returns the count of deleted notifications.
    fn delete_expired_notifications(&self) -> impl Future<Output = PgResult<usize>> + Send;
}

impl AccountNotificationRepository for PgClient {
    async fn create_notification(
        &self,
        new_notification: NewAccountNotification,
    ) -> PgResult<AccountNotification> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications;

        diesel::insert_into(account_notifications::table)
            .values(&new_notification)
            .returning(AccountNotification::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_notification_by_id(
        &self,
        notification_id: Uuid,
    ) -> PgResult<Option<AccountNotification>> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::id.eq(notification_id))
            .select(AccountNotification::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_notifications_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountNotification>> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_notifications_by_type(
        &self,
        account_id: Uuid,
        notification_type: NotificationType,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountNotification>> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::notify_type.eq(notification_type))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn mark_as_read(&self, notification_id: Uuid) -> PgResult<AccountNotification> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(true),
            read_at: Some(jiff_diesel::Timestamp::from(Timestamp::now())),
        };

        diesel::update(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .set(&update_data)
            .returning(AccountNotification::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn mark_as_unread(&self, notification_id: Uuid) -> PgResult<AccountNotification> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(false),
            read_at: None,
        };

        diesel::update(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .set(&update_data)
            .returning(AccountNotification::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn mark_all_as_read(&self, account_id: Uuid) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(true),
            read_at: Some(jiff_diesel::Timestamp::from(Timestamp::now())),
        };

        diesel::update(
            account_notifications::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::is_read.eq(false)),
        )
        .set(&update_data)
        .execute(&mut conn)
        .await
        .map_err(PgError::from)
    }

    async fn delete_notification(&self, notification_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        diesel::delete(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn delete_all_notifications(&self, account_id: Uuid) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        diesel::delete(account_notifications::table.filter(dsl::account_id.eq(account_id)))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn delete_expired_notifications(&self) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::account_notifications::{self, dsl};

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        diesel::delete(
            account_notifications::table
                .filter(dsl::expires_at.is_not_null())
                .filter(dsl::expires_at.lt(now)),
        )
        .execute(&mut conn)
        .await
        .map_err(PgError::from)
    }
}
