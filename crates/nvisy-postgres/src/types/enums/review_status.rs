//! Review status enumeration: the lifecycle state of a document review.

use super::db_enum;

db_enum! {
    /// The lifecycle state of a review (a reviewer-opened discussion thread on a
    /// document).
    ///
    /// Corresponds to the `REVIEW_STATUS` `PostgreSQL` enum. The state is driven by
    /// explicit reviewer decisions, not by assignment: a review opens at `Draft`;
    /// `submit` moves it to `InReview`; a reviewer can `request-changes`
    /// (`ChangesRequested`, from which `resubmit` returns it to `InReview`) or
    /// `approve`. An approval reaches `Approved` once the workspace's required number
    /// of distinct approvals is met, and rests at `PartiallyApproved` in between
    /// (only reachable when two or more approvals are required). Reopening an
    /// `Approved` review returns it to `InReview`.
    pub enum ReviewStatus: Default = Draft, "crate::schema::sql_types::ReviewStatus" {
        /// Opened, edits being assembled; `submit` sends it for review.
        Draft = "draft",
        /// Submitted and awaiting a reviewer decision.
        InReview = "in_review",
        /// A reviewer requested changes; `resubmit` returns it to `InReview`.
        ChangesRequested = "changes_requested",
        /// Some, but not all, required approvals are in. Only reachable when the
        /// workspace requires two or more approvals.
        PartiallyApproved = "partially_approved",
        /// Signed off: the required approvals are in and the approved redaction is
        /// the review's output.
        Approved = "approved",
    }
}

impl ReviewStatus {
    /// Whether the review is fully signed off.
    #[inline]
    #[must_use]
    pub fn is_approved(self) -> bool {
        matches!(self, ReviewStatus::Approved)
    }
}

#[cfg(test)]
mod tests {
    use super::ReviewStatus::{
        self, Approved, ChangesRequested, Draft, InReview, PartiallyApproved,
    };

    #[test]
    fn default_is_draft() {
        assert_eq!(ReviewStatus::default(), Draft);
    }

    #[test]
    fn is_approved_only_for_approved() {
        assert!(!Draft.is_approved());
        assert!(!InReview.is_approved());
        assert!(!ChangesRequested.is_approved());
        assert!(!PartiallyApproved.is_approved());
        assert!(Approved.is_approved());
    }
}
