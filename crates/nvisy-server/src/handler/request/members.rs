//! Project member request types.

use nvisy_postgres::types::ProjectRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberRole {
    /// New role for the member
    pub role: ProjectRole,

    /// Optional reason for role change
    #[validate(length(max = 300))]
    pub reason: Option<String>,
}

/// Request to remove a member from the project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoveMember {
    /// Reason for removing the member
    #[validate(length(min = 1, max = 300))]
    pub reason: String,

    /// Whether to notify the member about removal
    pub notify_member: Option<bool>,

    /// Transfer ownership to another member (if removing owner)
    pub transfer_ownership: Option<Uuid>,
}
