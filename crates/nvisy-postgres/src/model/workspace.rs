//! Main workspace model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspaces;
use crate::types::{HasCreatedAt, HasDeletedAt, HasOwnership, HasUpdatedAt, Tags};

/// Main workspace model representing a workspace workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Workspace {
    /// Unique workspace identifier.
    pub id: Uuid,
    /// Human-readable workspace name (2-100 characters).
    pub display_name: String,
    /// Detailed description of the workspace purpose and goals.
    pub description: Option<String>,
    /// URL to workspace avatar/logo image.
    pub avatar_url: Option<String>,
    /// Data retention period in seconds (NULL for indefinite retention).
    pub keep_for_sec: Option<i32>,
    /// Whether automatic cleanup is enabled.
    pub auto_cleanup: bool,
    /// Maximum storage in MB.
    pub max_storage: Option<i32>,
    /// Whether approval is required.
    pub require_approval: bool,
    /// Whether comments are enabled.
    pub enable_comments: bool,
    /// Workspace tags.
    pub tags: Vec<Option<String>>,
    /// Additional workspace metadata.
    pub metadata: serde_json::Value,
    /// Workspace-specific settings.
    pub settings: serde_json::Value,
    /// Account that created the workspace.
    pub created_by: Uuid,
    /// Timestamp when the workspace was created.
    pub created_at: Timestamp,
    /// Timestamp when the workspace was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the workspace was archived.
    pub archived_at: Option<Timestamp>,
    /// Timestamp when the workspace was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspace {
    /// Workspace display name.
    pub display_name: String,
    /// Workspace description.
    pub description: Option<String>,
    /// Optional avatar URL.
    pub avatar_url: Option<String>,
    /// Data retention period.
    pub keep_for_sec: Option<i32>,
    /// Auto cleanup enabled.
    pub auto_cleanup: Option<bool>,
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

/// Data for updating a workspace.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspace {
    /// Display name.
    pub display_name: Option<String>,
    /// Description.
    pub description: Option<Option<String>>,
    /// Avatar URL.
    pub avatar_url: Option<Option<String>>,
    /// Data retention period.
    pub keep_for_sec: Option<i32>,
    /// Auto cleanup enabled.
    pub auto_cleanup: Option<bool>,
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

impl Workspace {
    /// Returns whether the workspace is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the workspace has auto cleanup enabled.
    pub fn has_auto_cleanup(&self) -> bool {
        self.auto_cleanup
    }

    /// Returns the tags as a Tags helper.
    pub fn tags_helper(&self) -> Tags {
        Tags::from_optional_strings(self.tags.clone())
    }

    /// Returns the flattened tags (removing None values).
    pub fn get_tags(&self) -> Vec<String> {
        self.tags.iter().filter_map(|tag| tag.clone()).collect()
    }

    /// Returns whether the workspace has tags.
    pub fn has_tags(&self) -> bool {
        self.tags.iter().any(|tag| tag.is_some())
    }

    /// Returns whether the workspace contains a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags
            .iter()
            .any(|t| t.as_ref() == Some(&tag.to_string()))
    }

    /// Returns whether the workspace has a description.
    pub fn has_description(&self) -> bool {
        self.description
            .as_deref()
            .is_some_and(|desc| !desc.is_empty())
    }

    /// Returns whether the workspace has an avatar.
    pub fn has_avatar(&self) -> bool {
        self.avatar_url
            .as_deref()
            .is_some_and(|url| !url.is_empty())
    }

    /// Returns whether the workspace has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the workspace has custom settings.
    pub fn has_settings(&self) -> bool {
        !self.settings.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the workspace has storage limits.
    pub fn has_storage_limit(&self) -> bool {
        self.max_storage.is_some()
    }

    /// Returns whether the workspace is at or near storage capacity.
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

    /// Returns whether the workspace allows file uploads.
    pub fn allows_file_uploads(&self) -> bool {
        !self.is_deleted()
    }

    /// Returns whether the workspace allows collaboration.
    pub fn allows_collaboration(&self) -> bool {
        !self.is_deleted() && self.enable_comments
    }

    /// Returns the age of the workspace since creation.
    pub fn age(&self) -> jiff::Span {
        jiff::Timestamp::now() - jiff::Timestamp::from(self.created_at)
    }

    /// Returns the display name or a default.
    pub fn display_name_or_default(&self) -> &str {
        if self.display_name.is_empty() {
            "Untitled Workspace"
        } else {
            &self.display_name
        }
    }
}

impl HasCreatedAt for Workspace {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for Workspace {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for Workspace {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}

impl HasOwnership for Workspace {
    fn created_by(&self) -> Uuid {
        self.created_by
    }
}
