//! Review status enumeration: the derived state of a file thread's review.

use super::db_enum;

db_enum! {
    /// The review state of a file thread (a file thread is the review of its
    /// file). `None` on the thread means a workspace thread, which has no review.
    ///
    /// Corresponds to the `REVIEW_STATUS` PostgreSQL enum. It is derived from the
    /// review's events, never set by a user: a detection makes it `NeedsReview`, a
    /// redaction `InReview`, and verification `Resolved`; a later detection reopens
    /// it to `NeedsReview`.
    pub enum ReviewStatus: Default = NeedsReview, "crate::schema::sql_types::ReviewStatus" {
        /// A detection exists; no redaction has been reviewed yet.
        NeedsReview = "needs_review",
        /// A redaction (review pass) exists but has not been verified.
        InReview = "in_review",
        /// The review has been verified (approved).
        Resolved = "resolved",
    }
}

impl ReviewStatus {
    /// Whether the review has been verified.
    #[inline]
    pub fn is_resolved(self) -> bool {
        matches!(self, ReviewStatus::Resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::ReviewStatus::{self, InReview, NeedsReview, Resolved};

    #[test]
    fn default_is_needs_review() {
        assert_eq!(ReviewStatus::default(), NeedsReview);
    }

    #[test]
    fn is_resolved_only_for_resolved() {
        assert!(!NeedsReview.is_resolved());
        assert!(!InReview.is_resolved());
        assert!(Resolved.is_resolved());
    }
}
