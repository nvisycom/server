//! Filtering options for workspace member queries.

use serde::{Deserialize, Serialize};

use crate::types::WorkspaceRole;

/// Filter options for workspace members.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MemberFilter {
    /// Filter by workspace role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
}
