//! Review timeline event-kind enumeration.

use super::db_enum;

db_enum! {
    /// The kind of a non-message entry in a review's timeline.
    ///
    /// Corresponds to the `REVIEW_EVENT_KIND` `PostgreSQL` enum. A review's stream
    /// interleaves comments (messages) with these events: the lifecycle (opened,
    /// renamed, verified, reopened) and the sign-off workflow (detections and
    /// redactions referenced, assignment changes).
    pub enum ReviewEventKind = "crate::schema::sql_types::ReviewEventKind" {
        /// The review was opened.
        Opened = "review.opened",
        /// The review's display name was changed.
        Renamed = "review.renamed",
        /// A detection was referenced by the review.
        DetectionLinked = "detection.linked",
        /// A redaction was referenced by the review.
        RedactionLinked = "redaction.linked",
        /// The review was assigned to a reviewer.
        Assigned = "assigned",
        /// A reviewer was unassigned.
        Unassigned = "unassigned",
        /// The review was verified (resolved).
        Verified = "verified",
        /// A resolved review was reopened (back to `needs_review`).
        Reopened = "reopened",
    }
}
