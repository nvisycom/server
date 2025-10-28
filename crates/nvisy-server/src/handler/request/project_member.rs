//! Project member request types.

use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "role": "Admin"
}))]
pub struct UpdateMemberRoleRequest {
    /// New role for the member
    pub role: ProjectRole,
}
