//! Main project model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::projects;
use crate::types::{ProjectStatus, ProjectVisibility};

/// Main project model representing a project workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Project {
    /// Unique project identifier
    pub id: Uuid,
    /// Human-readable project name (2-100 characters)
    pub display_name: String,
    /// Detailed description of the project purpose and goals
    pub description: String,
    /// Optional short code for easy project identification
    pub project_code: Option<String>,
    /// URL to project avatar/logo image
    pub avatar_url: Option<String>,
    /// Current status of the project (active, archived, etc.)
    pub status: ProjectStatus,
    /// Project visibility level (public, private, etc.)
    pub visibility: ProjectVisibility,
    /// Data retention period in seconds
    pub keep_for_sec: i32,
    /// Whether automatic cleanup is enabled
    pub auto_cleanup: bool,
    /// Maximum number of members allowed
    pub max_members: i32,
    /// Maximum number of documents allowed
    pub max_documents: Option<i32>,
    /// Maximum storage in MB
    pub max_storage_mb: Option<i32>,
    /// Whether public signup is allowed
    pub allow_public_signup: bool,
    /// Whether approval is required for membership
    pub require_approval: bool,
    /// Whether comments are enabled
    pub enable_comments: bool,
    /// Whether notifications are enabled
    pub enable_notifications: bool,
    /// Whether this project serves as a template
    pub is_template: bool,
    /// Reference to template project if created from template
    pub template_id: Option<Uuid>,
    /// Project category
    pub category: Option<String>,
    /// Project tags
    pub tags: Vec<Option<String>>,
    /// Additional project metadata
    pub metadata: serde_json::Value,
    /// Project-specific settings
    pub settings: serde_json::Value,
    /// Account that created the project
    pub created_by: Uuid,
    /// Timestamp when the project was created
    pub created_at: OffsetDateTime,
    /// Timestamp when the project was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the project was archived
    pub archived_at: Option<OffsetDateTime>,
    /// Timestamp when the project was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new project.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProject {
    /// Project display name
    pub display_name: String,
    /// Project description
    pub description: String,
    /// Optional project code
    pub project_code: Option<String>,
    /// Optional avatar URL
    pub avatar_url: Option<String>,
    /// Project status
    pub status: ProjectStatus,
    /// Project visibility
    pub visibility: ProjectVisibility,
    /// Data retention period
    pub keep_for_sec: i32,
    /// Auto cleanup enabled
    pub auto_cleanup: bool,
    /// Maximum members
    pub max_members: i32,
    /// Maximum documents
    pub max_documents: Option<i32>,
    /// Maximum storage
    pub max_storage_mb: Option<i32>,
    /// Allow public signup
    pub allow_public_signup: bool,
    /// Require approval
    pub require_approval: bool,
    /// Enable comments
    pub enable_comments: bool,
    /// Enable notifications
    pub enable_notifications: bool,
    /// Is template
    pub is_template: bool,
    /// Template ID
    pub template_id: Option<Uuid>,
    /// Category
    pub category: Option<String>,
    /// Tags
    pub tags: Vec<Option<String>>,
    /// Metadata
    pub metadata: serde_json::Value,
    /// Settings
    pub settings: serde_json::Value,
    /// Created by
    pub created_by: Uuid,
}

/// Data for updating a project.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProject {
    /// Display name
    pub display_name: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Project code
    pub project_code: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Status
    pub status: Option<ProjectStatus>,
    /// Visibility
    pub visibility: Option<ProjectVisibility>,
    /// Keep for seconds
    pub keep_for_sec: Option<i32>,
    /// Auto cleanup
    pub auto_cleanup: Option<bool>,
    /// Max members
    pub max_members: Option<i32>,
    /// Max documents
    pub max_documents: Option<i32>,
    /// Max storage MB
    pub max_storage_mb: Option<i32>,
    /// Allow public signup
    pub allow_public_signup: Option<bool>,
    /// Require approval
    pub require_approval: Option<bool>,
    /// Enable comments
    pub enable_comments: Option<bool>,
    /// Enable notifications
    pub enable_notifications: Option<bool>,
    /// Category
    pub category: Option<String>,
    /// Tags
    pub tags: Option<Vec<Option<String>>>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Settings
    pub settings: Option<serde_json::Value>,
    /// Archived at
    pub archived_at: Option<OffsetDateTime>,
}

impl Default for NewProject {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            description: String::new(),
            project_code: None,
            avatar_url: None,
            status: ProjectStatus::Active,
            visibility: ProjectVisibility::Private,
            keep_for_sec: 604800, // 7 days
            auto_cleanup: true,
            max_members: 50,
            max_documents: None,
            max_storage_mb: None,
            allow_public_signup: false,
            require_approval: true,
            enable_comments: true,
            enable_notifications: true,
            is_template: false,
            template_id: None,
            category: None,
            tags: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            settings: serde_json::Value::Object(serde_json::Map::new()),
            created_by: Uuid::new_v4(),
        }
    }
}

impl Project {
    /// Returns whether the project is currently active.
    pub fn is_active(&self) -> bool {
        self.deleted_at.is_none()
            && self.archived_at.is_none()
            && self.status == ProjectStatus::Active
    }

    /// Returns whether the project is archived.
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some() || self.status == ProjectStatus::Archived
    }

    /// Returns whether the project is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the project allows new members.
    pub fn accepts_members(&self) -> bool {
        self.is_active() && self.allow_public_signup
    }

    /// Returns whether the project is suspended.
    pub fn is_suspended(&self) -> bool {
        self.status == ProjectStatus::Suspended
    }

    /// Returns whether the project is a template.
    pub fn is_template(&self) -> bool {
        self.is_template
    }

    /// Returns whether the project is public.
    pub fn is_public(&self) -> bool {
        self.visibility == ProjectVisibility::Public
    }

    /// Returns whether the project is private.
    pub fn is_private(&self) -> bool {
        self.visibility == ProjectVisibility::Private
    }

    /// Returns whether the project allows read operations.
    pub fn allows_reads(&self) -> bool {
        self.status.allows_reads() && !self.is_deleted()
    }

    /// Returns whether the project allows write operations.
    pub fn allows_writes(&self) -> bool {
        self.status.allows_writes() && !self.is_deleted()
    }

    /// Returns whether the project can be archived.
    pub fn can_be_archived(&self) -> bool {
        self.is_active() && !self.is_template
    }

    /// Returns whether the project can be restored from archive.
    pub fn can_be_restored(&self) -> bool {
        self.is_archived() && !self.is_deleted()
    }

    /// Returns whether the project has auto cleanup enabled.
    pub fn has_auto_cleanup(&self) -> bool {
        self.auto_cleanup
    }

    /// Returns whether the project allows invitations.
    pub fn allows_invitations(&self) -> bool {
        self.is_active() || self.is_archived()
    }
}
