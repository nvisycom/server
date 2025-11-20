//! Project member request types.

use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

use super::validation::validation_error;

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "role": "Admin",
    "reason": "Promotion due to excellent performance"
}))]
pub struct UpdateMemberRole {
    /// New role for the member
    #[validate(custom(function = "validate_project_role_wrapper"))]
    pub role: ProjectRole,

    /// Optional reason for role change
    #[validate(length(max = 300))]
    pub reason: Option<String>,
}

/// Request to remove a member from the project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "reason": "Member requested to leave project",
    "notifyMember": true,
    "transferOwnership": "550e8400-e29b-41d4-a716-446655440000"
}))]
pub struct RemoveMember {
    /// Reason for removing the member
    #[validate(length(min = 1, max = 300))]
    #[validate(custom(function = "validate_reason_content"))]
    pub reason: String,

    /// Whether to notify the member about removal
    pub notify_member: Option<bool>,

    /// Transfer ownership to another member (if removing owner)
    pub transfer_ownership: Option<Uuid>,
}

/// Validates project role wrapper.
fn validate_project_role_wrapper(_role: &ProjectRole) -> Result<(), ValidationError> {
    // ProjectRole is an enum, so it's already constrained
    // We can add additional business logic here if needed
    Ok(())
}

/// Validates reason content for safety and appropriateness.
fn validate_reason_content(reason: &str) -> Result<(), ValidationError> {
    let trimmed = reason.trim();

    if trimmed.is_empty() {
        return Err(validation_error("reason_empty", "Reason cannot be empty"));
    }

    Ok(())
}
