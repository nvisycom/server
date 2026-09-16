//! Workspace reviews table constraint violations.

use strum::EnumString;

/// Workspace reviews table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceReviewConstraints {
    /// The title is empty once trimmed, or longer than the maximum.
    #[strum(serialize = "workspace_reviews_display_name_length")]
    DisplayNameLength,
}
