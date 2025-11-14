//! Account notifications repository for managing notification operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountNotification, NewAccountNotification, UpdateAccountNotification};
use crate::types::NotificationType;
use crate::{PgError, PgResult, schema};

/// Repository for account notification table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountNotificationRepository;

impl AccountNotificationRepository {
    /// Creates a new account notification repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new notification in the database.
    pub async fn create_notification(
        conn: &mut AsyncPgConnection,
        new_notification: NewAccountNotification,
    ) -> PgResult<AccountNotification> {
        use schema::account_notifications;

        diesel::insert_into(account_notifications::table)
            .values(&new_notification)
            .returning(AccountNotification::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds a notification by its ID.
    pub async fn find_notification_by_id(
        conn: &mut AsyncPgConnection,
        notification_id: Uuid,
    ) -> PgResult<Option<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        account_notifications::table
            .filter(dsl::id.eq(notification_id))
            .select(AccountNotification::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds all notifications for an account.
    pub async fn find_notifications_by_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds unread notifications for an account.
    pub async fn find_unread_notifications(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::is_read.eq(false))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds notifications by type for an account.
    pub async fn find_notifications_by_type(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        notification_type: NotificationType,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::notify_type.eq(notification_type))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds notifications related to a specific entity.
    pub async fn find_notifications_by_related_entity(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        related_type: &str,
        related_id: Uuid,
    ) -> PgResult<Vec<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::related_type.eq(related_type))
            .filter(dsl::related_id.eq(related_id))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .select(AccountNotification::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Marks a notification as read.
    pub async fn mark_as_read(
        conn: &mut AsyncPgConnection,
        notification_id: Uuid,
    ) -> PgResult<AccountNotification> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(true),
            read_at: Some(OffsetDateTime::now_utc()),
        };

        diesel::update(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .set(&update_data)
            .returning(AccountNotification::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Marks a notification as unread.
    pub async fn mark_as_unread(
        conn: &mut AsyncPgConnection,
        notification_id: Uuid,
    ) -> PgResult<AccountNotification> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(false),
            read_at: None,
        };

        diesel::update(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .set(&update_data)
            .returning(AccountNotification::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Marks all notifications as read for an account.
    pub async fn mark_all_as_read(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<usize> {
        use schema::account_notifications::{self, dsl};

        let update_data = UpdateAccountNotification {
            is_read: Some(true),
            read_at: Some(OffsetDateTime::now_utc()),
        };

        diesel::update(
            account_notifications::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::is_read.eq(false)),
        )
        .set(&update_data)
        .execute(conn)
        .await
        .map_err(PgError::from)
    }

    /// Deletes a notification by ID.
    pub async fn delete_notification(
        conn: &mut AsyncPgConnection,
        notification_id: Uuid,
    ) -> PgResult<()> {
        use schema::account_notifications::{self, dsl};

        diesel::delete(account_notifications::table.filter(dsl::id.eq(notification_id)))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Deletes all notifications for an account.
    pub async fn delete_all_notifications(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<usize> {
        use schema::account_notifications::{self, dsl};

        diesel::delete(account_notifications::table.filter(dsl::account_id.eq(account_id)))
            .execute(conn)
            .await
            .map_err(PgError::from)
    }

    /// Deletes expired notifications.
    pub async fn delete_expired_notifications(conn: &mut AsyncPgConnection) -> PgResult<usize> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        diesel::delete(
            account_notifications::table
                .filter(dsl::expires_at.is_not_null())
                .filter(dsl::expires_at.lt(now)),
        )
        .execute(conn)
        .await
        .map_err(PgError::from)
    }

    /// Counts total notifications for an account.
    pub async fn count_notifications(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<i64> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Counts unread notifications for an account.
    pub async fn count_unread_notifications(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<i64> {
        use schema::account_notifications::{self, dsl};

        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::is_read.eq(false))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Gets recent notifications (last 7 days) for an account.
    pub async fn find_recent_notifications(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountNotification>> {
        use schema::account_notifications::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);
        let now = OffsetDateTime::now_utc();

        account_notifications::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::expires_at.is_null().or(dsl::expires_at.gt(now)))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountNotification::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }
}

/// Statistics for account notifications.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationStats {
    /// Total number of notifications
    pub total_count: i64,
    /// Number of unread notifications
    pub unread_count: i64,
    /// Number of notifications by type
    pub by_type: Vec<(NotificationType, i64)>,
}

impl NotificationStats {
    /// Returns the read rate as a percentage (0-100).
    pub fn read_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            let read_count = self.total_count - self.unread_count;
            (read_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns whether there are unread notifications.
    pub fn has_unread(&self) -> bool {
        self.unread_count > 0
    }
}
