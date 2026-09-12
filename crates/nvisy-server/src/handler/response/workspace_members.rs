//! Workspace member response types.

use jiff::Timestamp;
use nvisy_postgres::model::{Account, WorkspaceMember as WorkspaceMemberModel};
use nvisy_postgres::types::{Handle, WorkspaceRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Page;

/// Represents a workspace member.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMember {
    /// Handle of the member's account.
    pub username: Handle,
    /// Email address of the member.
    pub email_address: String,
    /// Display name of the member, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Serve path of the member's avatar, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    /// Role of the member in the workspace.
    pub member_role: WorkspaceRole,
    /// Timestamp when the member joined the workspace.
    pub created_at: Timestamp,
}

impl WorkspaceMember {
    /// Creates a Member response from database models.
    pub fn from_model(member: WorkspaceMemberModel, account: Account) -> Self {
        Self {
            username: account.username,
            email_address: account.email_address,
            display_name: account.display_name,
            avatar_url: account.avatar_url,
            member_role: member.member_role,
            created_at: member.created_at.into(),
        }
    }

    /// Creates a list of Member responses from database models.
    pub fn from_models(models: Vec<(WorkspaceMemberModel, Account)>) -> Vec<Self> {
        models
            .into_iter()
            .map(|(member, account)| Self::from_model(member, account))
            .collect()
    }
}

/// Paginated response for workspace members.
pub type WorkspaceMembersPage = Page<WorkspaceMember>;
