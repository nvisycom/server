//! Thread timeline event-kind enumeration.

use super::db_enum;

db_enum! {
    /// The kind of a non-message entry in a comment thread's timeline.
    ///
    /// Corresponds to the `THREAD_EVENT_KIND` PostgreSQL enum. A thread's stream
    /// interleaves comments (messages) with these events. A workspace thread uses
    /// only the discussion lifecycle (opened/closed/reopened/renamed); a file
    /// thread is the review of its file and also records its review transitions
    /// (a detection ran, a redaction was made, the review was verified or
    /// reopened, the assignee changed).
    pub enum ThreadEventKind = "crate::schema::sql_types::ThreadEventKind" {
        /// The thread was opened.
        Opened = "thread.opened",
        /// The thread was closed (workspace threads).
        Closed = "thread.closed",
        /// The thread was reopened (workspace threads).
        Reopened = "thread.reopened",
        /// The thread's display name was changed.
        Renamed = "thread.renamed",
        /// A detection ran on the file, so its review is needed.
        DetectionCreated = "review.detection_created",
        /// A redaction (review pass) was made on the file.
        RedactionCreated = "review.redaction_created",
        /// The file's review was verified (approved).
        Verified = "review.verified",
        /// A new detection reopened a previously verified review.
        ReviewReopened = "review.reopened",
        /// The review was assigned to a reviewer.
        Assigned = "review.assigned",
        /// The review's assignee was cleared.
        Unassigned = "review.unassigned",
    }
}
