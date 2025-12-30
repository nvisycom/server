//! Workspace member response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceMember;
use nvisy_postgres::types::WorkspaceRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a workspace member.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Role of the member in the workspace.
    pub member_role: WorkspaceRole,
    /// Timestamp when the member joined the workspace.
    pub created_at: Timestamp,
    /// Timestamp when the member last accessed the workspace.
    pub last_accessed_at: Option<Timestamp>,
}

impl From<WorkspaceMember> for Member {
    fn from(member: WorkspaceMember) -> Self {
        Self {
            account_id: member.account_id,
            member_role: member.member_role,
            created_at: member.created_at.into(),
            last_accessed_at: member.last_accessed_at.map(Into::into),
        }
    }
}

/// Response for listing workspace members.
pub type Members = Vec<Member>;
