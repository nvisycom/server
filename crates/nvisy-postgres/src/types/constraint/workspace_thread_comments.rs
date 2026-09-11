//! Workspace thread-comments table constraint violations.

use strum::EnumString;

/// Workspace thread-comments table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceThreadCommentConstraints {
    /// The body is empty once trimmed, or longer than the maximum.
    #[strum(serialize = "workspace_thread_comments_body_length")]
    BodyLength,
}
