//! Review timeline event-kind enumeration.

use super::db_enum;

db_enum! {
    /// The kind of a non-message entry in a review's timeline.
    ///
    /// Corresponds to the `REVIEW_EVENT_KIND` `PostgreSQL` enum. A review's stream
    /// interleaves comments (messages) with these events: the lifecycle (opened,
    /// renamed, submitted, changes requested, approved, reopened), re-detection, and
    /// assignment changes.
    pub enum ReviewEventKind = "crate::schema::sql_types::ReviewEventKind" {
        /// The review was opened. Its target carries the initial detection id, when
        /// the review was opened against one.
        Opened = "review.opened",
        /// The review's display name was changed.
        Renamed = "review.renamed",
        /// The review was repointed at a newer detection (the document was
        /// re-detected). Its target carries the new detection id.
        Redetected = "review.redetected",
        /// The review was assigned to a reviewer.
        Assigned = "review.assigned",
        /// A reviewer was unassigned.
        Unassigned = "review.unassigned",
        /// The review was submitted (or resubmitted) for review.
        Submitted = "review.submitted",
        /// A reviewer requested changes.
        ChangesRequested = "review.changes_requested",
        /// A reviewer approved the review (quorum may require more approvals). Its
        /// target carries the approved output redaction id, when one was named.
        Approved = "review.approved",
        /// An approved review was reopened (back to `in_review`).
        Reopened = "review.reopened",
    }
}
