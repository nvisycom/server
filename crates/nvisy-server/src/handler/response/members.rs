//! Workspace member response types.

use jiff::Timestamp;
use nvisy_postgres::model::{Account, WorkspaceMember};
use nvisy_postgres::types::WorkspaceRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Represents a workspace member.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Email address of the member.
    pub email_address: String,
    /// Display name of the member.
    pub display_name: String,
    /// Role of the member in the workspace.
    pub member_role: WorkspaceRole,
    /// Whether the member has two-factor authentication enabled.
    pub has_2fa: bool,
    /// Timestamp when the member joined the workspace.
    pub created_at: Timestamp,
}

impl Member {
    /// Creates a Member response from database models.
    // TODO: Fetch actual 2FA status from account settings
    pub fn from_model(member: WorkspaceMember, account: Account) -> Self {
        Self {
            account_id: member.account_id,
            email_address: account.email_address,
            display_name: account.display_name,
            member_role: member.member_role,
            has_2fa: false,
            created_at: member.created_at.into(),
        }
    }

    /// Creates a list of Member responses from database models.
    pub fn from_models(models: Vec<(WorkspaceMember, Account)>) -> Vec<Self> {
        models
            .into_iter()
            .map(|(member, account)| Self::from_model(member, account))
            .collect()
    }
}

/// Paginated response for workspace members.
pub type MembersPage = Page<Member>;
