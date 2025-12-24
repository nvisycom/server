//! Main project model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::projects;
use crate::types::{
    HasCreatedAt, HasDeletedAt, HasOwnership, HasUpdatedAt, ProjectStatus, ProjectVisibility, Tags,
};

/// Main project model representing a project workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Project {
    /// Unique project identifier.
    pub id: Uuid,
    /// Human-readable project name (2-100 characters).
    pub display_name: String,
    /// Detailed description of the project purpose and goals.
    pub description: Option<String>,
    /// URL to project avatar/logo image.
    pub avatar_url: Option<String>,
    /// Current status of the project (active, archived, etc.).
    pub status: ProjectStatus,
    /// Project visibility level (public, private, etc.).
    pub visibility: ProjectVisibility,
    /// Data retention period in seconds (NULL for indefinite retention).
    pub keep_for_sec: Option<i32>,
    /// Whether automatic cleanup is enabled.
    pub auto_cleanup: bool,
    /// Maximum number of members allowed.
    pub max_members: Option<i32>,
    /// Maximum storage in MB.
    pub max_storage: Option<i32>,
    /// Whether approval is required.
    pub require_approval: bool,
    /// Whether comments are enabled.
    pub enable_comments: bool,
    /// Project tags.
    pub tags: Vec<Option<String>>,
    /// Additional project metadata.
    pub metadata: serde_json::Value,
    /// Project-specific settings.
    pub settings: serde_json::Value,
    /// Account that created the project.
    pub created_by: Uuid,
    /// Timestamp when the project was created.
    pub created_at: Timestamp,
    /// Timestamp when the project was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the project was archived.
    pub archived_at: Option<Timestamp>,
    /// Timestamp when the project was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new project.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProject {
    /// Project display name.
    pub display_name: String,
    /// Project description.
    pub description: Option<String>,
    /// Optional avatar URL.
    pub avatar_url: Option<String>,
    /// Project status.
    pub status: Option<ProjectStatus>,
    /// Project visibility.
    pub visibility: Option<ProjectVisibility>,
    /// Data retention period.
    pub keep_for_sec: Option<i32>,
    /// Auto cleanup enabled.
    pub auto_cleanup: Option<bool>,
    /// Maximum members.
    pub max_members: Option<i32>,
    /// Maximum storage.
    pub max_storage: Option<i32>,
    /// Require approval.
    pub require_approval: Option<bool>,
    /// Enable comments.
    pub enable_comments: Option<bool>,
    /// Tags.
    pub tags: Option<Vec<Option<String>>>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Settings.
    pub settings: Option<serde_json::Value>,
    /// Created by.
    pub created_by: Uuid,
}

/// Data for updating a project.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProject {
    /// Display name.
    pub display_name: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Avatar URL.
    pub avatar_url: Option<String>,
    /// Status.
    pub status: Option<ProjectStatus>,
    /// Visibility.
    pub visibility: Option<ProjectVisibility>,
    /// Data retention period.
    pub keep_for_sec: Option<i32>,
    /// Auto cleanup enabled.
    pub auto_cleanup: Option<bool>,
    /// Maximum members.
    pub max_members: Option<i32>,
    /// Maximum storage.
    pub max_storage: Option<i32>,
    /// Require approval.
    pub require_approval: Option<bool>,
    /// Enable comments.
    pub enable_comments: Option<bool>,
    /// Tags.
    pub tags: Option<Vec<Option<String>>>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Settings.
    pub settings: Option<serde_json::Value>,
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

    /// Returns the tags as a Tags helper.
    pub fn tags_helper(&self) -> Tags {
        Tags::from_optional_strings(self.tags.clone())
    }

    /// Returns the flattened tags (removing None values).
    pub fn get_tags(&self) -> Vec<String> {
        self.tags.iter().filter_map(|tag| tag.clone()).collect()
    }

    /// Returns whether the project has tags.
    pub fn has_tags(&self) -> bool {
        self.tags.iter().any(|tag| tag.is_some())
    }

    /// Returns whether the project contains a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags
            .iter()
            .any(|t| t.as_ref() == Some(&tag.to_string()))
    }

    /// Returns whether the project has a description.
    pub fn has_description(&self) -> bool {
        self.description
            .as_deref()
            .is_some_and(|desc| !desc.is_empty())
    }

    /// Returns whether the project has an avatar.
    pub fn has_avatar(&self) -> bool {
        self.avatar_url
            .as_deref()
            .is_some_and(|url| !url.is_empty())
    }

    /// Returns whether the project has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the project has custom settings.
    pub fn has_settings(&self) -> bool {
        !self.settings.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the project has member limits.
    pub fn has_member_limit(&self) -> bool {
        self.max_members.is_some()
    }

    /// Returns whether the project has storage limits.
    pub fn has_storage_limit(&self) -> bool {
        self.max_storage.is_some()
    }

    /// Returns whether the project is at or near member capacity.
    pub fn is_near_member_capacity(&self, current_members: i32) -> bool {
        if let Some(max_members) = self.max_members {
            let usage_percentage = (current_members as f64 / max_members as f64) * 100.0;
            usage_percentage >= 80.0 // 80% threshold
        } else {
            false
        }
    }

    /// Returns whether the project is at or near storage capacity.
    pub fn is_near_storage_capacity(&self, current_storage_mb: i32) -> bool {
        if let Some(max_storage) = self.max_storage {
            let usage_percentage = (current_storage_mb as f64 / max_storage as f64) * 100.0;
            usage_percentage >= 80.0 // 80% threshold
        } else {
            false
        }
    }

    /// Returns the data retention period in days.
    pub fn retention_days(&self) -> Option<i32> {
        self.keep_for_sec.map(|sec| sec / (24 * 60 * 60)) // Convert seconds to days
    }

    /// Returns whether the project allows file uploads.
    pub fn allows_file_uploads(&self) -> bool {
        self.is_active() && !self.is_deleted()
    }

    /// Returns whether the project allows collaboration.
    pub fn allows_collaboration(&self) -> bool {
        self.is_active() && self.enable_comments
    }

    /// Returns the age of the project since creation.
    pub fn age(&self) -> jiff::Span {
        jiff::Timestamp::now() - jiff::Timestamp::from(self.created_at)
    }

    /// Returns the display name or a default.
    pub fn display_name_or_default(&self) -> &str {
        if self.display_name.is_empty() {
            "Untitled Project"
        } else {
            &self.display_name
        }
    }
}

impl HasCreatedAt for Project {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for Project {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for Project {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}

impl HasOwnership for Project {
    fn created_by(&self) -> Uuid {
        self.created_by
    }
}
