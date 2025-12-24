//! Project member response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

/// Represents a project member.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Whether the member is currently active.
    pub is_active: bool,
    /// Timestamp when the member joined the project.
    pub created_at: Timestamp,
    /// Timestamp when the member last accessed the project.
    pub last_accessed_at: Option<Timestamp>,
}

impl From<model::ProjectMember> for Member {
    fn from(member: model::ProjectMember) -> Self {
        Self {
            account_id: member.account_id,
            member_role: member.member_role,
            is_active: member.is_active,
            created_at: member.created_at.into(),
            last_accessed_at: member.last_accessed_at.map(Into::into),
        }
    }
}

/// Response for listing project members.
pub type Members = Vec<Member>;
