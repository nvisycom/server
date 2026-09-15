//! Review timeline event-kind enumeration.

use super::db_enum;

db_enum! {
    /// The kind of an entry in a document review's activity log.
    ///
    /// Corresponds to the `REVIEW_EVENT_KIND` `PostgreSQL` enum. A review's timeline
    /// records what was done to the review: artifacts referenced, the assignee
    /// changing, and the verification lifecycle. Distinct from a thread's discussion
    /// timeline ([`ThreadEventKind`](super::ThreadEventKind)).
    pub enum ReviewEventKind = "crate::schema::sql_types::ReviewEventKind" {
        /// A detection was referenced by the review.
        DetectionLinked = "detection.linked",
        /// A redaction was referenced by the review.
        RedactionLinked = "redaction.linked",
        /// The review was assigned to a reviewer.
        Assigned = "assigned",
        /// The review's assignee was cleared.
        Unassigned = "unassigned",
        /// The review was verified (resolved).
        Verified = "verified",
        /// A resolved review was reopened.
        Reopened = "reopened",
    }
}
