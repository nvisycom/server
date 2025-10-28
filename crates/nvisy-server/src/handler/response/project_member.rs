//! Project member response types.

use nvisy_postgres::model::ProjectMember;
use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a project member in list responses.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListMembersResponseItem {
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

impl From<ProjectMember> for ListMembersResponseItem {
    fn from(member: ProjectMember) -> Self {
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
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListMembersResponse {
    /// ID of the project.
    pub project_id: Uuid,
    /// Whether the project is private.
    pub is_private: bool,
    /// List of project members.
    pub members: Vec<ListMembersResponseItem>,
}

/// Detailed information about a project member.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetMemberResponse {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Whether the member is currently active.
    pub is_active: bool,
    /// Whether the member receives update notifications.
    pub notify_updates: bool,
    /// Whether the member receives comment notifications.
    pub notify_comments: bool,
    /// Whether the member receives mention notifications.
    pub notify_mentions: bool,
    /// Whether the project is marked as favorite by this member.
    pub is_favorite: bool,
    /// Timestamp when the member joined the project.
    pub created_at: OffsetDateTime,
    /// Timestamp when the membership was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the member last accessed the project.
    pub last_accessed_at: Option<OffsetDateTime>,
}

impl From<ProjectMember> for GetMemberResponse {
    fn from(member: ProjectMember) -> Self {
        Self {
            account_id: member.account_id,
            member_role: member.member_role,
            is_active: member.is_active,
            notify_updates: member.notify_updates,
            notify_comments: member.notify_comments,
            notify_mentions: member.notify_mentions,
            is_favorite: member.is_favorite,
            created_at: member.created_at,
            updated_at: member.updated_at,
            last_accessed_at: member.last_accessed_at,
        }
    }
}

/// Response for member deletion operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMemberResponse {
    /// Account ID of the removed member.
    pub account_id: Uuid,
    /// Project ID from which the member was removed.
    pub project_id: Uuid,
}

/// Response after updating a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberRoleResponse {
    /// ID of the member
    pub account_id: Uuid,
    /// Project ID
    pub project_id: Uuid,
    /// New role
    pub role: ProjectRole,
    /// When the update occurred
    pub updated_at: OffsetDateTime,
}
