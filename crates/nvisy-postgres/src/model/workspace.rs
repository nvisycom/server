//! Main workspace model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspaces;
use crate::types::{Handle, Json, WorkspaceMetadata, WorkspaceSettings};

/// Main workspace model representing a workspace workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
#[must_use]
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

impl NewWorkspace {
    /// Creates a workspace with the required name, slug, and creator; the
    /// description, avatar, metadata, and settings default to `None`.
    pub fn new(display_name: impl Into<String>, slug: Handle, created_by: Uuid) -> Self {
        Self {
            display_name: display_name.into(),
            slug,
            description: None,
            avatar_url: None,
            metadata: None,
            settings: None,
            created_by,
        }
    }

    /// A workspace owned by `created_by`, for tests.
    ///
    /// Both the slug and the display name are unique per call, so one owner can
    /// hold several test workspaces without colliding on the per-owner
    /// display-name unique index.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(created_by: Uuid) -> Self {
        let suffix = &Uuid::now_v7().simple().to_string()[..12];
        Self::new(
            format!("Test Workspace {suffix}"),
            Handle::test(),
            created_by,
        )
    }
}

/// Data for updating a workspace.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspaces)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
