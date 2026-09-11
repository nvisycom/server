//! Core authorization types.
//!
//! Defines [`Permission`] and its mapping to the minimum [`WorkspaceRole`] that
//! satisfies it.

use nvisy_postgres::types::WorkspaceRole;
use strum::{EnumIter, EnumString, IntoEnumIterator};

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

    // Assignment permissions
    /// Can view file review assignments (who is reviewing what).
    ViewAssignments,
    /// Can assign files to reviewers and unassign them.
    AssignTasks,

    // Comment permissions
    /// Can view comments on files.
    ViewComments,
    /// Can write comments and replies (and edit or delete one's own).
    Comment,
    /// Can resolve and reopen comment threads.
    ResolveComments,

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

    // Provider permissions
    /// Can view workspace inference providers.
    ViewProviders,
    /// Can create, modify, and manage workspace inference providers.
    ManageProviders,

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
            | Self::ViewAssignments
            | Self::ViewComments
            | Self::Comment
            | Self::ResolveComments
            | Self::ViewAnalytics
            | Self::ViewActivity
            | Self::ViewMembers
            | Self::ViewConnections
            | Self::ViewProviders
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
            | Self::AssignTasks
            | Self::UseChat
            | Self::RunConnectionSyncs => WorkspaceRole::Editor,

            // Admin-level permissions (manage workspace resources)
            Self::UpdateWorkspace
            | Self::InviteMembers
            | Self::RemoveMembers
            | Self::ManageConnections
            | Self::ManageProviders
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use nvisy_postgres::types::WorkspaceRole;

    use super::Permission;

    #[test]
    fn security_boundaries_map_to_the_right_minimum_role() {
        // The review-vs-original split is a real boundary: a Reviewer may see and
        // download redacted output and audit, but never the original bytes.
        assert_eq!(
            Permission::ViewFiles.minimum_required_role(),
            WorkspaceRole::Reviewer
        );
        assert_eq!(
            Permission::DownloadRedactedFiles.minimum_required_role(),
            WorkspaceRole::Reviewer
        );
        assert_eq!(
            Permission::DownloadOriginalFiles.minimum_required_role(),
            WorkspaceRole::Editor
        );

        // Managing the workspace is Admin; destroying it or changing roles is
        // Owner-only.
        assert_eq!(
            Permission::InviteMembers.minimum_required_role(),
            WorkspaceRole::Admin
        );
        assert_eq!(
            Permission::DeleteWorkspace.minimum_required_role(),
            WorkspaceRole::Owner
        );
        assert_eq!(
            Permission::ManageRoles.minimum_required_role(),
            WorkspaceRole::Owner
        );
    }

    #[test]
    fn is_permitted_follows_the_role_hierarchy() {
        // A Reviewer-tier permission is granted to everyone at Reviewer or above.
        for role in [
            WorkspaceRole::Reviewer,
            WorkspaceRole::Editor,
            WorkspaceRole::Admin,
            WorkspaceRole::Owner,
        ] {
            assert!(Permission::ViewFiles.is_permitted_by_role(role));
        }
        // An Owner-only permission is denied to everyone below Owner.
        assert!(!Permission::ManageRoles.is_permitted_by_role(WorkspaceRole::Admin));
        assert!(!Permission::ManageRoles.is_permitted_by_role(WorkspaceRole::Editor));
        assert!(Permission::ManageRoles.is_permitted_by_role(WorkspaceRole::Owner));
    }

    #[test]
    fn permissions_are_monotonic_up_the_hierarchy() {
        // A higher role must hold every permission a lower role does (roles are a
        // strict hierarchy, so permission sets nest). This catches any permission
        // that was mis-tiered such that it regressed for a higher role.
        let set = |role| -> HashSet<Permission> {
            Permission::permissions_for_role(role).into_iter().collect()
        };
        let reviewer = set(WorkspaceRole::Reviewer);
        let editor = set(WorkspaceRole::Editor);
        let admin = set(WorkspaceRole::Admin);
        let owner = set(WorkspaceRole::Owner);

        assert!(reviewer.is_subset(&editor));
        assert!(editor.is_subset(&admin));
        assert!(admin.is_subset(&owner));

        // Owner holds every permission; each step up strictly adds at least one.
        assert!(reviewer.len() < editor.len());
        assert!(editor.len() < admin.len());
        assert!(admin.len() < owner.len());
    }
}
