//! Workspace member request types.

use nvisy_postgres::types::{MemberFilter, MemberSortBy, SortOrder, WorkspaceRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMember {
    /// New role for the member.
    pub role: WorkspaceRole,
}

impl UpdateMember {
    /// Converts to database model.
    pub fn into_model(self) -> nvisy_postgres::model::UpdateWorkspaceMember {
        nvisy_postgres::model::UpdateWorkspaceMember {
            member_role: Some(self.role),
            ..Default::default()
        }
    }
}

/// Query parameters for listing workspace members.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListMembersQuery {
    /// Filter by workspace role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,

    /// Filter by 2FA status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_2fa: Option<bool>,

    /// Sort by field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<MemberSortField>,

    /// Sort order (asc or desc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<SortOrder>,
}

/// Fields to sort members by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemberSortField {
    /// Sort by display name.
    Name,
    /// Sort by join date.
    Date,
}

impl ListMembersQuery {
    /// Converts to filter model.
    pub fn to_filter(&self) -> MemberFilter {
        MemberFilter {
            role: self.role,
            has_2fa: self.has_2fa,
        }
    }

    /// Converts to sort model.
    pub fn to_sort(&self) -> MemberSortBy {
        let order = self.order.unwrap_or_default();
        match self.sort_by {
            Some(MemberSortField::Name) => MemberSortBy::Name(order),
            Some(MemberSortField::Date) | None => MemberSortBy::Date(order),
        }
    }
}
