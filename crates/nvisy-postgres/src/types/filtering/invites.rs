//! Filtering options for workspace invite queries.

use serde::{Deserialize, Serialize};

use crate::types::WorkspaceRole;

/// Filter options for workspace invites.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct InviteFilter {
    /// Filter by invited role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
}
