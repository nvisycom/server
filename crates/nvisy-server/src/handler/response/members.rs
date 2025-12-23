//! Project member response types.

use nvisy_postgres::model;
use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a project member.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Whether the member is currently active.
    pub is_active: bool,
    /// Timestamp when the member joined the project.
    pub created_at: OffsetDateTime,
    /// Timestamp when the member last accessed the project.
    pub last_accessed_at: Option<OffsetDateTime>,
}

impl From<model::ProjectMember> for Member {
    fn from(member: model::ProjectMember) -> Self {
        Self {
            account_id: member.account_id,
            member_role: member.member_role,
            is_active: member.is_active,
            created_at: member.created_at,
            last_accessed_at: member.last_accessed_at,
        }
    }
}

/// Response for listing project members.
pub type Members = Vec<Member>;
