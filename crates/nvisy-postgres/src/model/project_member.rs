//! Project member model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::project_members;
use crate::types::{HasCreatedAt, HasLastActivityAt, HasOwnership, HasUpdatedAt, ProjectRole};

/// Project member model representing a user's membership in a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectMember {
    /// Reference to the project.
    pub project_id: Uuid,
    /// Reference to the member's account.
    pub account_id: Uuid,
    /// Member's role in the project.
    pub member_role: ProjectRole,
    /// Custom permissions (JSON).
    pub custom_permissions: serde_json::Value,
    /// Display order for UI sorting.
    pub show_order: i32,
    /// Whether member has marked project as favorite.
    pub is_favorite: bool,
    /// Whether member has hidden the project.
    pub is_hidden: bool,
    /// Whether member receives update notifications.
    pub notify_updates: bool,
    /// Whether member receives comment notifications.
    pub notify_comments: bool,
    /// Whether member receives mention notifications.
    pub notify_mentions: bool,
    /// Whether the membership is active.
    pub is_active: bool,
    /// Last time member accessed the project.
    pub last_accessed_at: Option<Timestamp>,
    /// Account that created this membership.
    pub created_by: Uuid,
    /// Account that last updated this membership.
    pub updated_by: Uuid,
    /// Timestamp when membership was created.
    pub created_at: Timestamp,
    /// Timestamp when membership was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new project member.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = project_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectMember {
    /// Project ID.
    pub project_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Member role.
    pub member_role: ProjectRole,
    /// Custom permissions.
    pub custom_permissions: serde_json::Value,
    /// Show order.
    pub show_order: i32,
    /// Is favorite.
    pub is_favorite: bool,
    /// Is hidden.
    pub is_hidden: bool,
    /// Notify updates.
    pub notify_updates: bool,
    /// Notify comments.
    pub notify_comments: bool,
    /// Notify mentions.
    pub notify_mentions: bool,
    /// Is active.
    pub is_active: bool,
    /// Created by.
    pub created_by: Uuid,
    /// Updated by.
    pub updated_by: Uuid,
}

/// Data for updating a project member.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectMember {
    /// Member role.
    pub member_role: Option<ProjectRole>,
    /// Custom permissions.
    pub custom_permissions: Option<serde_json::Value>,
    /// Show order.
    pub show_order: Option<i32>,
    /// Is favorite.
    pub is_favorite: Option<bool>,
    /// Is hidden.
    pub is_hidden: Option<bool>,
    /// Notify updates.
    pub notify_updates: Option<bool>,
    /// Notify comments.
    pub notify_comments: Option<bool>,
    /// Notify mentions.
    pub notify_mentions: Option<bool>,
    /// Is active.
    pub is_active: Option<bool>,
    /// Last accessed at.
    pub last_accessed_at: Option<Timestamp>,
    /// Updated by.
    pub updated_by: Option<Uuid>,
}

impl ProjectMember {
    /// Returns whether the membership is currently active.
    pub fn is_active_member(&self) -> bool {
        self.is_active
    }

    /// Returns whether the member has admin privileges.
    pub fn is_admin(&self) -> bool {
        matches!(self.member_role, ProjectRole::Admin)
    }

    /// Returns whether the member can invite others.
    pub fn can_invite(&self) -> bool {
        matches!(self.member_role, ProjectRole::Admin)
    }

    /// Returns whether the member is an editor.
    pub fn is_editor(&self) -> bool {
        self.member_role == ProjectRole::Editor
    }

    /// Returns whether the member is a viewer.
    pub fn is_viewer(&self) -> bool {
        self.member_role == ProjectRole::Viewer
    }

    /// Returns whether the member can edit project content.
    pub fn can_edit(&self) -> bool {
        matches!(self.member_role, ProjectRole::Admin | ProjectRole::Editor)
    }

    /// Returns whether the member can manage other members.
    pub fn can_manage_members(&self) -> bool {
        matches!(self.member_role, ProjectRole::Admin)
    }

    /// Returns whether the member has notifications enabled for updates.
    pub fn has_update_notifications(&self) -> bool {
        self.notify_updates
    }

    /// Returns whether the member has notifications enabled for comments.
    pub fn has_comment_notifications(&self) -> bool {
        self.notify_comments
    }

    /// Returns whether the member has notifications enabled for mentions.
    pub fn has_mention_notifications(&self) -> bool {
        self.notify_mentions
    }

    /// Returns whether the member has marked this project as favorite.
    pub fn is_favorite(&self) -> bool {
        self.is_favorite
    }

    /// Returns whether the member has hidden this project.
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Returns whether the member has any notification preferences enabled.
    pub fn has_notifications_enabled(&self) -> bool {
        self.notify_updates || self.notify_comments || self.notify_mentions
    }

    /// Returns whether the member has all notifications enabled.
    pub fn has_all_notifications_enabled(&self) -> bool {
        self.notify_updates && self.notify_comments && self.notify_mentions
    }

    /// Returns whether the member has custom permissions.
    pub fn has_custom_permissions(&self) -> bool {
        !self
            .custom_permissions
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the member has never accessed the project.
    pub fn has_never_accessed(&self) -> bool {
        self.last_accessed_at.is_none()
    }

    /// Returns the time since last access.
    pub fn time_since_last_access(&self) -> Option<jiff::Span> {
        self.last_accessed_at
            .map(|last_access| jiff::Timestamp::now() - jiff::Timestamp::from(last_access))
    }

    /// Returns whether the member is inactive (no recent access).
    pub fn is_inactive(&self) -> bool {
        if let Some(duration) = self.time_since_last_access() {
            duration.get_days() > 30 // No access for 30+ days
        } else {
            true // Never accessed
        }
    }

    /// Returns whether the member can perform administrative actions.
    pub fn can_administrate(&self) -> bool {
        self.is_active && self.is_admin()
    }

    /// Returns whether the member can modify project settings.
    pub fn can_modify_settings(&self) -> bool {
        self.is_active && self.is_admin()
    }

    /// Returns whether the member can delete the project.
    pub fn can_delete_project(&self) -> bool {
        self.is_active && self.is_admin()
    }

    /// Returns whether the member can be promoted to the given role.
    pub fn can_be_promoted_to(&self, role: ProjectRole) -> bool {
        self.is_active && role > self.member_role
    }

    /// Returns whether the member can be demoted to the given role.
    pub fn can_be_demoted_to(&self, role: ProjectRole) -> bool {
        self.is_active && role < self.member_role
    }

    /// Returns whether the membership can be removed.
    pub fn can_be_removed(&self) -> bool {
        true // Any member can be removed by an admin
    }
}

impl HasCreatedAt for ProjectMember {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for ProjectMember {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasOwnership for ProjectMember {
    fn created_by(&self) -> Uuid {
        self.created_by
    }

    fn updated_by(&self) -> Option<Uuid> {
        Some(self.updated_by)
    }
}

impl HasLastActivityAt for ProjectMember {
    fn last_activity_at(&self) -> Option<jiff::Timestamp> {
        self.last_accessed_at.map(Into::into)
    }
}
