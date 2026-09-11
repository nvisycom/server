//! Workspace comments table constraint violations.

use strum::EnumString;

/// Workspace comments table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceCommentConstraints {
    /// The body is empty once trimmed, or longer than the maximum.
    #[strum(serialize = "workspace_comments_body_length")]
    BodyLength,
    /// The anchor JSON exceeds the maximum stored size.
    #[strum(serialize = "workspace_comments_anchor_size")]
    AnchorSize,
}
