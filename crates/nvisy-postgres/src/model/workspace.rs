//! Main workspace model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspaces;
use crate::types::{
    Handle, HasCreatedAt, HasDeletedAt, HasOwnership, HasUpdatedAt, Json, WorkspaceMetadata,
    WorkspaceSettings,
};

/// Main workspace model representing a workspace workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Workspace {
    /// Unique workspace identifier.
    pub id: Uuid,
    /// Human-readable workspace name (3-32 characters).
    pub display_name: String,
    /// URL-safe workspace identifier, unique across the platform.
    pub slug: Handle,
    /// Detailed description of the workspace purpose and goals.
    pub description: Option<String>,
    /// URL to workspace avatar/logo image.
    pub avatar_url: Option<String>,
    /// Additional workspace metadata.
    pub metadata: Json<WorkspaceMetadata>,
    /// Workspace-specific settings.
    pub settings: Json<WorkspaceSettings>,
    /// Account that created the workspace.
    pub created_by: Uuid,
    /// Timestamp when the workspace was created.
    pub created_at: Timestamp,
    /// Timestamp when the workspace was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the workspace was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspace {
    /// Workspace display name.
    pub display_name: String,
    /// URL-safe workspace identifier, unique across the platform.
    pub slug: Handle,
    /// Workspace description.
    pub description: Option<String>,
    /// Optional avatar URL.
    pub avatar_url: Option<String>,
    /// Metadata.
    pub metadata: Option<Json<WorkspaceMetadata>>,
    /// Settings.
    pub settings: Option<Json<WorkspaceSettings>>,
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
    /// Metadata.
    pub metadata: Option<Json<WorkspaceMetadata>>,
    /// Settings.
    pub settings: Option<Json<WorkspaceSettings>>,
}

impl Workspace {
    /// Returns whether the workspace is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
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
        !self.metadata.is_empty()
    }

    /// Returns whether the workspace has custom settings.
    pub fn has_settings(&self) -> bool {
        !self.settings.is_empty()
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
