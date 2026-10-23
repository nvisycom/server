//! Review status enumeration: the lifecycle state of a document review.

use super::db_enum;

db_enum! {
    /// The lifecycle state of a review (a reviewer-opened discussion thread on a
    /// document).
    ///
    /// Corresponds to the `REVIEW_STATUS` `PostgreSQL` enum. The state is driven by
    /// assignment and explicit sign-off, not derived from linked artifacts: a review
    /// opens at `NeedsReview`; gaining its first assignee moves it to `InReview`, and
    /// removing the last assignee returns it to `NeedsReview`; verifying it sets
    /// `Resolved`, and reopening a resolved review returns it to `NeedsReview`.
    /// Assigning or unassigning a resolved review leaves it `Resolved`.
    pub enum ReviewStatus: Default = NeedsReview, "crate::schema::sql_types::ReviewStatus" {
        /// Open and not yet signed off: either awaiting a reviewer to pick it up,
        /// or reopened after being resolved (an assignee from before may remain).
        NeedsReview = "needs_review",
        /// At least one reviewer is assigned and the review is in progress.
        InReview = "in_review",
        /// A reviewer has verified (signed off on) the review.
        Resolved = "resolved",
    }
}

impl ReviewStatus {
    /// Whether the review has been verified.
    #[inline]
    #[must_use]
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
