//! Workspace member request types.

use nvisy_postgres::types::WorkspaceRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberRole {
    /// New role for the member.
    pub role: WorkspaceRole,
}

impl UpdateMemberRole {
    /// Converts to database model.
    pub fn into_model(self) -> nvisy_postgres::model::UpdateWorkspaceMember {
        nvisy_postgres::model::UpdateWorkspaceMember {
            member_role: Some(self.role),
            ..Default::default()
        }
    }
}
