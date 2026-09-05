//! Core authorization types and utilities.
//!
//! This module provides the fundamental types used for authorization throughout
//! the nvisy system, including permissions and results.

use std::borrow::Cow;

use nvisy_postgres::model::WorkspaceMember;
use nvisy_postgres::types::WorkspaceRole;
use strum::{EnumIter, EnumString, IntoEnumIterator};

use crate::handler::{ErrorKind, Result};

/// Granular workspace permissions for authorization checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(EnumIter, EnumString)]
pub enum Permission {
    // Workspace-level permissions
    /// Can view workspace basic information.
    ViewWorkspace,
    /// Can update workspace settings and metadata.
    UpdateWorkspace,
    /// Can delete the entire workspace.
    DeleteWorkspace,

    // File permissions
    /// Can list files and view their metadata.
    ViewFiles,
    /// Can upload new files to the workspace.
    UploadFiles,
    /// Can update file metadata and properties.
    UpdateFiles,
    /// Can download the bytes of original (source) files.
    DownloadOriginalFiles,
    /// Can download the bytes of redacted output files.
    DownloadRedactedFiles,
    /// Can download detection audit content (analyses and reviews).
    DownloadAudit,
    /// Can delete files from the workspace.
    DeleteFiles,

    // Pipeline permissions
    /// Can view pipelines in the workspace.
    ViewPipelines,
    /// Can create new pipelines.
    CreatePipelines,
    /// Can update existing pipelines.
    UpdatePipelines,
    /// Can delete pipelines.
    DeletePipelines,

    // Detection permissions
    /// Can view detections and their results (analyses, redactions, audits).
    ViewDetections,
    /// Can run detections (analyze a file for findings).
    RunDetections,
    /// Can run redactions (apply policies and produce a redacted file).
    RunRedactions,

    // Reporting permissions
    /// Can view workspace analytics.
    ViewAnalytics,
    /// Can view the workspace activity log.
    ViewActivity,

    // Chat permissions
    /// Can use workspace chat sessions.
    UseChat,

    // Member management permissions
    /// Can view workspace members and their roles.
    ViewMembers,
    /// Can invite new members to the workspace.
    InviteMembers,
    /// Can remove members from the workspace.
    RemoveMembers,
    /// Can change member roles and permissions.
    ManageRoles,

    // Connection permissions
    /// Can view workspace connections.
    ViewConnections,
    /// Can create, modify, and manage workspace connections.
    ManageConnections,
    /// Can trigger and cancel connection syncs.
    RunConnectionSyncs,

    // Policy permissions
    /// Can view workspace policies.
    ViewPolicies,
    /// Can create, modify, and manage workspace policies.
    ManagePolicies,

    // Webhook permissions
    /// Can view workspace webhooks.
    ViewWebhooks,
    /// Can create new webhooks in the workspace.
    CreateWebhooks,
    /// Can update existing webhooks.
    UpdateWebhooks,
    /// Can delete webhooks from the workspace.
    DeleteWebhooks,
    /// Can test webhooks by sending test payloads.
    TestWebhooks,
}

impl Permission {
    /// Checks if the given workspace role satisfies this permission requirement.
    ///
    /// This method leverages the role hierarchy to determine if the given role
    /// has sufficient permissions. A role is permitted if it has equal or higher
    /// permission level than the minimum required role for this permission.
    pub const fn is_permitted_by_role(self, role: WorkspaceRole) -> bool {
        role.has_permission_level_of(self.minimum_required_role())
    }

    /// Returns the minimum role required for this permission.
    #[must_use]
    pub const fn minimum_required_role(self) -> WorkspaceRole {
        match self {
            // Reviewer-level permissions (review access, no original files)
            Self::ViewWorkspace
            | Self::ViewFiles
            | Self::DownloadRedactedFiles
            | Self::DownloadAudit
            | Self::ViewPipelines
            | Self::ViewDetections
            | Self::ViewAnalytics
            | Self::ViewActivity
            | Self::ViewMembers
            | Self::ViewConnections
            | Self::ViewPolicies
            | Self::ViewWebhooks => WorkspaceRole::Reviewer,

            // Editor-level permissions (create and modify own resources)
            Self::UploadFiles
            | Self::UpdateFiles
            | Self::DownloadOriginalFiles
            | Self::DeleteFiles
            | Self::CreatePipelines
            | Self::UpdatePipelines
            | Self::DeletePipelines
            | Self::RunDetections
            | Self::RunRedactions
            | Self::UseChat
            | Self::RunConnectionSyncs => WorkspaceRole::Editor,

            // Admin-level permissions (manage workspace resources)
            Self::UpdateWorkspace
            | Self::InviteMembers
            | Self::RemoveMembers
            | Self::ManageConnections
            | Self::ManagePolicies
            | Self::CreateWebhooks
            | Self::UpdateWebhooks
            | Self::DeleteWebhooks
            | Self::TestWebhooks => WorkspaceRole::Admin,

            // Owner-only permissions (highest level)
            Self::DeleteWorkspace | Self::ManageRoles => WorkspaceRole::Owner,
        }
    }

    /// Returns all permissions available to the given role.
    pub fn permissions_for_role(role: WorkspaceRole) -> Vec<Self> {
        Self::iter()
            .filter(|perm| perm.is_permitted_by_role(role))
            .collect()
    }
}

/// Result of an authorization check with detailed information.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthResult {
    pub granted: bool,
    pub member: Option<WorkspaceMember>,
    pub reason: Option<Cow<'static, str>>,
}

impl AuthResult {
    /// Creates a granted authorization result without member information.
    pub const fn granted() -> Self {
        Self {
            granted: true,
            member: None,
            reason: None,
        }
    }

    /// Creates a granted authorization result with member information.
    pub const fn granted_with_member(member: WorkspaceMember) -> Self {
        Self {
            granted: true,
            member: Some(member),
            reason: None,
        }
    }

    /// Creates a denied authorization result with a reason.
    pub fn denied(reason: impl Into<Cow<'static, str>>) -> Self {
        Self {
            granted: false,
            member: None,
            reason: Some(reason.into()),
        }
    }

    /// Converts the result to a `Result` type, returning an error if access is denied.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::AuthResult;
    /// let result = AuthResult::granted();
    /// assert!(result.into_result().is_ok());
    ///
    /// let result = AuthResult::denied("Access denied");
    /// assert!(result.into_result().is_err());
    /// ```
    pub fn into_result(self) -> Result<Option<WorkspaceMember>> {
        if self.granted {
            Ok(self.member)
        } else {
            let error = match self.reason {
                Some(reason) => ErrorKind::Forbidden.with_context(reason),
                None => ErrorKind::Forbidden.into_error(),
            };
            Err(error)
        }
    }
}
