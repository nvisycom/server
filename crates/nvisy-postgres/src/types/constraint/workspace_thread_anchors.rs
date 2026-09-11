//! Workspace thread-anchors table constraint violations.

use strum::EnumString;

/// Workspace thread-anchors table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceThreadAnchorConstraints {
    /// The anchor JSON exceeds the maximum stored size.
    #[strum(serialize = "workspace_thread_anchors_size")]
    Size,
}
