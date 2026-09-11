//! Assignment status enumeration indicating one reviewer's review-workflow state.

use super::db_enum;

db_enum! {
    /// The review-workflow status of one reviewer's assignment on a file.
    ///
    /// Corresponds to the `ASSIGNMENT_STATUS` PostgreSQL enum. A file may be
    /// assigned to several reviewers at once (like GitHub assignees); each
    /// reviewer's assignment carries its own status. This is the human
    /// review-workflow axis and is independent of a detection's execution status,
    /// which is driven by the analysis worker.
    pub enum AssignmentStatus: Default = Assigned, "crate::schema::sql_types::AssignmentStatus" {
        /// Assigned to the reviewer; not yet started.
        Assigned = "assigned",
        /// The reviewer has started reviewing.
        InReview = "in_review",
        /// The reviewer has finished their review.
        Done = "done",
    }
}

impl AssignmentStatus {
    /// Returns whether the reviewer has finished their review.
    #[inline]
    pub fn is_done(self) -> bool {
        matches!(self, AssignmentStatus::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::AssignmentStatus::{self, Assigned, Done, InReview};

    #[test]
    fn default_is_assigned() {
        assert_eq!(AssignmentStatus::default(), Assigned);
    }

    #[test]
    fn is_done_only_for_done() {
        assert!(!Assigned.is_done());
        assert!(!InReview.is_done());
        assert!(Done.is_done());
    }
}
