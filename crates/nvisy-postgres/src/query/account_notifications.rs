//! Account notifications repository for managing notification operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountNotification, NewAccountNotification, UpdateAccountNotification};
use crate::types::NotificationType;
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive account notification database operations.
///
/// Provides database operations for managing user notifications across the system.
/// Handles the complete lifecycle of notifications including creation, delivery
/// tracking, read status management, and cleanup operations. Notifications can
/// be associated with specific entities and have optional expiration times.
///
/// This repository supports various notification types and provides filtering
/// capabilities for building notification feeds, dashboards, and administrative
/// interfaces.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountNotificationRepository;

impl AccountNotificationRepository {
    /// Creates a new account notification repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `AccountNotificationRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new notification in the database.
    ///
    /// Generates a new notification record for the specified account with the
    /// provided content and metadata. The notification is immediately available
    /// for retrieval and will appear in the user's notification feed.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_notification` - Complete notification data including account ID, type, and content
    ///
    /// # Returns
    ///
    /// The created `AccountNotification` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
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

    /// Finds a specific notification by its unique identifier.
    ///
    /// Retrieves a notification using its UUID for direct access or administrative
    /// purposes. This method returns the notification regardless of its read status
    /// or expiration time, making it suitable for all notification management operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `notification_id` - UUID of the notification to retrieve
    ///
    /// # Returns
    ///
    /// The matching `AccountNotification` if found, `None` if not found,
    /// or a database error if the query fails.
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

    /// Finds all active notifications for an account with pagination support.
    ///
    /// Retrieves a paginated list of notifications for the specified account,
    /// automatically filtering out expired notifications. Results are ordered
    /// by creation date with newest notifications first, providing a chronological
    /// view of the user's notification history.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to retrieve notifications for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of active `AccountNotification` entries for the account,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    ///
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

    /// Finds unread notifications for an account with pagination support.
    ///
    /// Retrieves a paginated list of notifications that haven't been marked as
    /// read by the user. This is the primary method for building notification
    /// badges, alerts, and priority notification displays. Automatically excludes
    /// expired notifications.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to retrieve unread notifications for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of unread `AccountNotification` entries for the account,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Priority Use Cases
    ///
    /// - Notification badge counts and indicators
    /// - Priority notification displays
    /// - Mobile push notification queues
    /// - Email digest generation
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

    /// Finds notifications filtered by type for a specific account.
    ///
    /// Retrieves notifications of a specific type (e.g., security alerts, comments,
    /// system messages) for the given account. This enables type-specific notification
    /// feeds and specialized notification management interfaces.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to retrieve notifications for
    /// * `notification_type` - Specific notification type to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountNotification` entries of the specified type,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Specialized Use Cases
    ///
    /// - Security notification dashboards
    /// - Comment and mention feeds
    /// - System maintenance notification displays
    /// - Category-specific notification management
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

    /// Finds notifications related to a specific entity or resource.
    ///
    /// Retrieves notifications that are associated with a particular entity
    /// (such as a project, document, or user) identified by type and UUID.
    /// This enables entity-specific notification feeds and context-aware
    /// notification displays.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to retrieve notifications for
    /// * `related_type` - String identifier for the entity type (e.g., "project", "document")
    /// * `related_id` - UUID of the specific entity instance
    ///
    /// # Returns
    ///
    /// A vector of `AccountNotification` entries related to the specified entity,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Contextual Use Cases
    ///
    /// - Project-specific notification feeds
    /// - Document-related activity notifications
    /// - User-specific interaction alerts
    /// - Entity-focused notification management
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

    /// Marks a specific notification as read by the user.
    ///
    /// Updates the notification's read status and sets the read timestamp to
    /// the current time. This operation is critical for notification badge
    /// management and user experience, as it removes the notification from
    /// unread notification queries.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `notification_id` - UUID of the notification to mark as read
    ///
    /// # Returns
    ///
    /// The updated `AccountNotification` with read status and timestamp,
    /// or a database error if the operation fails.
    ///
    /// # User Experience Impact
    ///
    /// - Removes notification from unread badge counts
    /// - Updates notification appearance in feeds
    /// - Provides audit trail of user engagement
    /// - Enables personalized notification experiences
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

    /// Marks a specific notification as unread by resetting its read status.
    ///
    /// Resets the notification's read status to false and clears the read
    /// timestamp, effectively returning it to the unread state. This can be
    /// useful for user-requested re-notifications or administrative operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `notification_id` - UUID of the notification to mark as unread
    ///
    /// # Returns
    ///
    /// The updated `AccountNotification` with unread status,
    /// or a database error if the operation fails.
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

    /// Marks all unread notifications as read for a specific account.
    ///
    /// Bulk operation that updates all currently unread notifications for
    /// the account to read status with current timestamp. This is commonly
    /// used for "mark all as read" functionality in notification interfaces
    /// and bulk notification management operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account whose notifications should be marked as read
    ///
    /// # Returns
    ///
    /// The number of notifications that were marked as read,
    /// or a database error if the operation fails.
    ///
    /// # Bulk Operation Benefits
    ///
    /// - Efficient clearing of notification badges
    /// - Improved user experience for high-volume notifications
    /// - Reduced individual database operations
    /// - Convenient bulk notification management
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

    /// Permanently deletes a specific notification from the database.
    ///
    /// Removes the notification record completely from the database. This is
    /// a hard delete operation that cannot be undone, so it should be used
    /// carefully and typically only for user-requested deletions or
    /// administrative cleanup operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `notification_id` - UUID of the notification to delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    ///
    /// # Important Considerations
    ///
    /// - This is a permanent operation that cannot be undone
    /// - Consider audit requirements before implementing
    /// - May affect notification analytics and reporting
    /// - Should be used judiciously for privacy or cleanup purposes
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

    /// Permanently deletes all notifications for a specific account.
    ///
    /// Removes all notification records for the specified account from the
    /// database. This is a bulk hard delete operation commonly used during
    /// account deletion, privacy compliance requests, or administrative
    /// cleanup operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account whose notifications should be deleted
    ///
    /// # Returns
    ///
    /// The number of notifications that were deleted,
    /// or a database error if the operation fails.
    ///
    /// # Critical Use Cases
    ///
    /// - Account deletion and cleanup procedures
    /// - Privacy compliance (GDPR, CCPA) requests
    /// - Administrative account management
    /// - Data retention policy enforcement
    ///
    /// # Warning
    ///
    /// This permanently deletes all notification history for the account
    /// and cannot be undone. Ensure compliance with audit and legal requirements.
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

    /// Performs system-wide cleanup of expired notifications.
    ///
    /// Permanently deletes all notifications that have passed their expiration
    /// time across all accounts. This maintenance operation should be run
    /// regularly to prevent accumulation of obsolete notifications and
    /// maintain optimal database performance.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    ///
    /// # Returns
    ///
    /// The number of expired notifications that were deleted,
    /// or a database error if the operation fails.
    ///
    /// # Maintenance Benefits
    ///
    /// - Improves database query performance
    /// - Reduces storage requirements
    /// - Maintains system hygiene and cleanliness
    /// - Should be automated via scheduled maintenance jobs
    ///
    /// # Scheduling Recommendation
    ///
    /// Run this operation daily to maintain optimal performance and
    /// prevent accumulation of expired notification data.
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

    /// Counts the total number of active notifications for an account.
    ///
    /// Returns the count of all non-expired notifications for the specified
    /// account, providing a quick way to assess notification volume without
    /// retrieving the full notification data. Useful for pagination calculations
    /// and user interface elements.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to count notifications for
    ///
    /// # Returns
    ///
    /// The total count of active notifications for the account,
    /// or a database error if the query fails.
    ///
    /// # Performance Use Cases
    ///
    /// - Pagination total count calculations
    /// - User interface badge and indicator displays
    /// - Account activity monitoring
    /// - Administrative notification volume analysis
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

    /// Counts the number of unread notifications for an account.
    ///
    /// Returns the count of unread, non-expired notifications for the specified
    /// account. This is the primary method for generating notification badge
    /// counts and determining if a user has pending notifications requiring
    /// attention.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to count unread notifications for
    ///
    /// # Returns
    ///
    /// The count of unread notifications for the account,
    /// or a database error if the query fails.
    ///
    /// # Critical UI Use Cases
    ///
    /// - Notification badge counts in headers and sidebars
    /// - Mobile app push notification scheduling
    /// - Email digest frequency determination
    /// - User attention and engagement indicators
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

    /// Retrieves recent notifications from the last 7 days for an account.
    ///
    /// Filters notifications to only include those created within the past
    /// week, providing a focused view of recent activity. This is useful for
    /// building "what's new" displays, recent activity feeds, and mobile
    /// notification summaries.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to retrieve recent notifications for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountNotification` entries from the last 7 days,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Focused Display Use Cases
    ///
    /// - "What's New" dashboard sections
    /// - Recent activity summaries
    /// - Mobile app notification previews
    /// - Weekly notification digests
    /// - Focused user engagement displays
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

/// Comprehensive statistical information about account notifications.
///
/// Provides aggregated counts and metrics for different notification states
/// and types for analytical and administrative purposes. This data structure
/// is used for building notification dashboards, monitoring user engagement,
/// and optimizing notification strategies.
///
/// The statistics include both raw counts and breakdowns by notification type
/// to enable detailed analysis of notification patterns and effectiveness.
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
    /// Calculates the percentage of notifications that have been read by the user.
    ///
    /// Returns the ratio of read notifications to total notifications as a percentage.
    /// A higher read rate indicates better user engagement with notifications
    /// and effective notification content and timing.
    ///
    /// # Returns
    ///
    /// A percentage value between 0.0 and 100.0. Returns 0.0 if there are
    /// no notifications for the account.
    ///
    /// # Engagement Analysis Use Cases
    ///
    /// - Measuring notification effectiveness
    /// - User engagement analytics
    /// - Notification strategy optimization
    /// - A/B testing notification approaches
    pub fn read_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            let read_count = self.total_count - self.unread_count;
            (read_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Checks if there are any unread notifications requiring user attention.
    ///
    /// Returns true if the unread count is greater than zero, providing a
    /// simple boolean check for determining if notification indicators
    /// should be displayed or if user attention is needed.
    ///
    /// # Returns
    ///
    /// `true` if there are unread notifications, `false` otherwise.
    ///
    /// # User Interface Use Cases
    ///
    /// - Showing/hiding notification badges
    /// - Conditional notification indicator displays
    /// - Determining if notification sounds should play
    /// - Controlling notification panel visibility
    pub fn has_unread(&self) -> bool {
        self.unread_count > 0
    }
}
