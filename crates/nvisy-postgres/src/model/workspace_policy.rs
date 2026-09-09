//! Workspace policy model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_policies;
use crate::types::Handle;

/// Workspace policy representing a structured redaction governance policy.
///
/// The `definition` holds an `elide-governance` `PolicyDefinition` (rules,
/// labels, fallback, retention) that the redaction engine consumes.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct WorkspacePolicy {
    /// Unique policy identifier.
    pub id: Uuid,
    /// Reference to the workspace this policy belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this policy.
    pub account_id: Uuid,
    /// URL-safe policy identifier, unique within the workspace.
    pub slug: Handle,
    /// Human-readable policy display name.
    pub display_name: String,
    /// Policy description.
    pub description: Option<String>,
    /// Encrypted Policy body (the engine's Policy type as JSON).
    pub definition: Vec<u8>,
    /// Metadata for filtering/display.
    pub metadata: JsonValue,
    /// Timestamp when the policy was created.
    pub created_at: Timestamp,
    /// Timestamp when the policy was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the policy was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace policy.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspacePolicy {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// URL-safe policy identifier, unique within the workspace.
    pub slug: Handle,
    /// Policy display name.
    pub display_name: String,
    /// Policy description.
    pub description: Option<String>,
    /// Encrypted Policy body (the engine's Policy type as JSON).
    pub definition: Vec<u8>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Creation timestamp override, for tests only.
    #[cfg(any(feature = "test_util", test))]
    pub created_at: Option<Timestamp>,
}

impl NewWorkspacePolicy {
    /// A minimal policy for `workspace_id`, for tests.
    ///
    /// Both the slug and the display name are unique per call, so several test
    /// policies fit in one workspace without colliding on the per-workspace
    /// unique indexes. The definition is a non-empty placeholder blob.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid) -> Self {
        let suffix = &Uuid::now_v7().simple().to_string()[..12];
        Self {
            workspace_id,
            account_id,
            slug: Handle::test(),
            display_name: format!("Test Policy {suffix}"),
            description: None,
            definition: vec![1, 2, 3],
            metadata: None,
            created_at: None,
        }
    }
}

/// Data for updating a workspace policy.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspacePolicy {
    /// Policy display name.
    pub display_name: Option<String>,
    /// Policy description.
    pub description: Option<Option<String>>,
    /// Encrypted Policy body (the engine's Policy type as JSON).
    pub definition: Option<Vec<u8>>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}
