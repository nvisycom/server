//! Project member model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::project_members;
use crate::types::ProjectRole;

/// Project member model representing a user's membership in a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectMember {
    /// Reference to the project
    pub project_id: Uuid,
    /// Reference to the member's account
    pub account_id: Uuid,
    /// Member's role in the project
    pub member_role: ProjectRole,
    /// Custom permissions (JSON)
    pub custom_permissions: serde_json::Value,
    /// Display order for UI sorting
    pub show_order: i32,
    /// Whether member has marked project as favorite
    pub is_favorite: bool,
    /// Whether member has hidden the project
    pub is_hidden: bool,
    /// Whether member receives update notifications
    pub notify_updates: bool,
    /// Whether member receives comment notifications
    pub notify_comments: bool,
    /// Whether member receives mention notifications
    pub notify_mentions: bool,
    /// Whether the membership is active
    pub is_active: bool,
    /// Last time member accessed the project
    pub last_accessed_at: Option<OffsetDateTime>,
    /// Account that created this membership
    pub created_by: Uuid,
    /// Account that last updated this membership
    pub updated_by: Uuid,
    /// Timestamp when membership was created
    pub created_at: OffsetDateTime,
    /// Timestamp when membership was last updated
    pub updated_at: OffsetDateTime,
}

/// Data for creating a new project member.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = project_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectMember {
    /// Project ID
    pub project_id: Uuid,
    /// Account ID
    pub account_id: Uuid,
    /// Member role
    pub member_role: ProjectRole,
    /// Custom permissions
    pub custom_permissions: serde_json::Value,
    /// Show order
    pub show_order: i32,
    /// Is favorite
    pub is_favorite: bool,
    /// Is hidden
    pub is_hidden: bool,
    /// Notify updates
    pub notify_updates: bool,
    /// Notify comments
    pub notify_comments: bool,
    /// Notify mentions
    pub notify_mentions: bool,
    /// Is active
    pub is_active: bool,
    /// Created by
    pub created_by: Uuid,
    /// Updated by
    pub updated_by: Uuid,
}

/// Data for updating a project member.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectMember {
    /// Member role
    pub member_role: Option<ProjectRole>,
    /// Custom permissions
    pub custom_permissions: Option<serde_json::Value>,
    /// Show order
    pub show_order: Option<i32>,
    /// Is favorite
    pub is_favorite: Option<bool>,
    /// Is hidden
    pub is_hidden: Option<bool>,
    /// Notify updates
    pub notify_updates: Option<bool>,
    /// Notify comments
    pub notify_comments: Option<bool>,
    /// Notify mentions
    pub notify_mentions: Option<bool>,
    /// Is active
    pub is_active: Option<bool>,
    /// Last accessed at
    pub last_accessed_at: Option<OffsetDateTime>,
    /// Updated by
    pub updated_by: Option<Uuid>,
}

impl Default for NewProjectMember {
    fn default() -> Self {
        Self {
            project_id: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            member_role: ProjectRole::Viewer,
            custom_permissions: serde_json::Value::Object(serde_json::Map::new()),
            show_order: 0,
            is_favorite: false,
            is_hidden: false,
            notify_updates: true,
            notify_comments: true,
            notify_mentions: true,
            is_active: true,
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
        }
    }
}

impl ProjectMember {
    /// Returns whether the membership is currently active.
    pub fn is_active_member(&self) -> bool {
        self.is_active
    }

    /// Returns whether the member has admin privileges.
    pub fn is_admin(&self) -> bool {
        matches!(self.member_role, ProjectRole::Admin | ProjectRole::Owner)
    }

    /// Returns whether the member can invite others.
    pub fn can_invite(&self) -> bool {
        matches!(self.member_role, ProjectRole::Admin | ProjectRole::Owner)
    }

    /// Returns whether the member is the project owner.
    pub fn is_owner(&self) -> bool {
        self.member_role == ProjectRole::Owner
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
        matches!(
            self.member_role,
            ProjectRole::Owner | ProjectRole::Admin | ProjectRole::Editor
        )
    }

    /// Returns whether the member can manage other members.
    pub fn can_manage_members(&self) -> bool {
        matches!(self.member_role, ProjectRole::Owner | ProjectRole::Admin)
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

    /// Returns whether the member has recently accessed the project.
    pub fn has_recent_access(&self) -> bool {
        if let Some(last_access) = self.last_accessed_at {
            let now = time::OffsetDateTime::now_utc();
            let duration = now - last_access;
            duration.whole_days() < 7 // Within last week
        } else {
            false
        }
    }
}
