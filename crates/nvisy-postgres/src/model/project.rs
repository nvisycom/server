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
    pub max_members: Option<i32>,
    /// Maximum storage in MB
    pub max_storage: Option<i32>,
    /// Whether approval is required
    pub require_approval: bool,
    /// Whether comments are enabled
    pub enable_comments: bool,
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
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProject {
    /// Project display name
    pub display_name: String,
    /// Project description
    pub description: Option<String>,
    /// Optional avatar URL
    pub avatar_url: Option<String>,
    /// Project status
    pub status: Option<ProjectStatus>,
    /// Project visibility
    pub visibility: Option<ProjectVisibility>,
    /// Data retention period
    pub keep_for_sec: Option<i32>,
    /// Auto cleanup enabled
    pub auto_cleanup: Option<bool>,
    /// Maximum members
    pub max_members: Option<i32>,
    /// Maximum storage
    pub max_storage: Option<i32>,
    /// Require approval
    pub require_approval: Option<bool>,
    /// Enable comments
    pub enable_comments: Option<bool>,
    /// Tags
    pub tags: Option<Vec<Option<String>>>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Settings
    pub settings: Option<serde_json::Value>,
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
    /// Max storage MB
    pub max_storage: Option<i32>,
    /// Require approval
    pub require_approval: Option<bool>,
    /// Enable comments
    pub enable_comments: Option<bool>,
    /// Tags
    pub tags: Vec<Option<String>>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Settings
    pub settings: Option<serde_json::Value>,
    /// Archived at
    pub archived_at: Option<OffsetDateTime>,
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

    /// Returns whether the project is suspended.
    pub fn is_suspended(&self) -> bool {
        matches!(self.status, ProjectStatus::Suspended)
    }

    /// Returns whether the project is a template.
    pub fn is_template(&self) -> bool {
        matches!(self.status, ProjectStatus::Template)
    }

    /// Returns whether the project is public.
    pub fn is_public(&self) -> bool {
        matches!(self.visibility, ProjectVisibility::Public)
    }

    /// Returns whether the project is private.
    pub fn is_private(&self) -> bool {
        matches!(self.visibility, ProjectVisibility::Private)
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
        self.is_active() && !self.is_template()
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
