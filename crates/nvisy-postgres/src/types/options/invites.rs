//! Query options for workspace invite queries.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SortOrder;
use crate::types::WorkspaceRole;

/// Sorting options for workspace invites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InviteSortBy {
    /// Sort by invitee email.
    Email(SortOrder),
    /// Sort by creation date.
    Date(SortOrder),
}

impl Default for InviteSortBy {
    fn default() -> Self {
        Self::Date(SortOrder::Desc)
    }
}

/// Filter options for workspace invites.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct InviteFilter {
    /// Filter by invited role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
}

impl InviteFilter {
    /// Creates a new empty filter.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters by role.
    #[inline]
    pub fn with_role(mut self, role: WorkspaceRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Returns whether any filter is active.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.role.is_none()
    }
}
