//! Notification event enumeration for user notifications.

use super::db_enum;

db_enum! {
    /// Defines the type of notification event sent to a user.
    ///
    /// Corresponds to the `NOTIFICATION_EVENT` PostgreSQL enum and is used for
    /// member, connection-sync, detection, redaction, and system notifications.
    /// The values mirror the [`WebhookEvent`](super::WebhookEvent) naming for the
    /// events the two channels share.
    pub enum NotificationEvent = "crate::schema::sql_types::NotificationEvent" {
        /// A new member joined a workspace.
        MemberJoined = "member.joined",
        /// A connection sync completed.
        ConnectionSyncCompleted = "connection.sync.completed",
        /// A connection sync failed.
        ConnectionSyncFailed = "connection.sync.failed",
        /// A detection finished analysis and is ready to redact.
        DetectionCompleted = "pipeline.detection.completed",
        /// A redaction was created (redacted output produced).
        RedactionCreated = "pipeline.redaction.created",
        /// A detection failed.
        DetectionFailed = "pipeline.detection.failed",
        /// A file's review was assigned to the reviewer.
        ReviewAssigned = "review.assigned",
        /// An account was mentioned in a comment.
        CommentMentioned = "comment.mentioned",
    }
}
