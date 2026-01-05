//! Filtering options for workspace member queries.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::WorkspaceRole;

/// Filter options for workspace members.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct MemberFilter {
    /// Filter by workspace role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
    /// Filter by 2FA status (true = has 2FA enabled, false = no 2FA).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_2fa: Option<bool>,
}

impl MemberFilter {
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

    /// Filters by 2FA status.
    #[inline]
    pub fn with_2fa(mut self, has_2fa: bool) -> Self {
        self.has_2fa = Some(has_2fa);
        self
    }

    /// Returns whether any filter is active.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.role.is_none() && self.has_2fa.is_none()
    }
}
