//! Workspace member request types.

use garde::Validate;
use nvisy_postgres::model;
use nvisy_postgres::types::{
    Direction, MemberFilter, MemberSortBy, MemberSortField, WorkspaceRole,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspaceMember {
    /// New role for the member.
    pub role: WorkspaceRole,
}

impl UpdateWorkspaceMember {
    pub fn into_model(self) -> model::UpdateWorkspaceMember {
        model::UpdateWorkspaceMember {
            member_role: Some(self.role),
            ..Default::default()
        }
    }
}

/// Query parameters for listing workspace members.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkspaceMembers {
    /// Filter by workspace role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
    /// Sort by field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<MemberSortField>,
    /// Sort order (asc or desc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Direction>,
}

impl ListWorkspaceMembers {
    /// Converts to filter model.
    pub fn to_filter(&self) -> MemberFilter {
        MemberFilter { role: self.role }
    }

    /// Converts to sort model.
    pub fn to_sort(&self) -> MemberSortBy {
        let order = self.order.unwrap_or_default();
        let field = self.sort_by.unwrap_or_default();
        MemberSortBy::new(field, order)
    }
}
