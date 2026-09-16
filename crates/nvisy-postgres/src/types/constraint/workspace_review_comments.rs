//! Workspace review-comments table constraint violations.

use strum::EnumString;

/// Workspace review-comments table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceReviewCommentConstraints {
    /// The body is empty once trimmed, or longer than the maximum.
    #[strum(serialize = "workspace_review_comments_body_length")]
    BodyLength,
}
