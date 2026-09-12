//! Workspace policy model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_policies;
use crate::types::Handle;

/// Workspace policy: the logical identity of a redaction governance policy.
///
/// The policy is a stable identity (slug, display name); its content is
/// versioned. `current_version_id` names the live [`WorkspacePolicyVersion`]
/// whose encrypted `definition` the redaction engine consumes.
///
/// [`WorkspacePolicyVersion`]: crate::model::WorkspacePolicyVersion
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
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
    /// The live version whose definition the engine consumes.
    pub current_version_id: Option<Uuid>,
    /// Metadata for filtering/display.
    pub metadata: JsonValue,
    /// Timestamp when the policy was created.
    pub created_at: Timestamp,
    /// Timestamp when the policy was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the policy was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new logical policy row.
///
/// The policy's first version (carrying the definition) is inserted alongside
/// this row; see [`WorkspacePolicyRepository::create_workspace_policy`].
///
/// [`WorkspacePolicyRepository::create_workspace_policy`]: crate::query::WorkspacePolicyRepository::create_workspace_policy
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
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

impl NewWorkspacePolicy {
    /// A minimal logical policy for `workspace_id`, for tests.
    ///
    /// Both the slug and the display name are unique per call, so several test
    /// policies fit in one workspace without colliding on the per-workspace
    /// unique indexes.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid) -> Self {
        let hex = Uuid::now_v7().simple().to_string();
        let suffix = &hex[hex.len() - 12..];
        Self {
            workspace_id,
            account_id,
            slug: Handle::test(),
            display_name: format!("Test Policy {suffix}"),
            description: None,
            metadata: None,
        }
    }
}

/// Data for updating a logical policy's identity fields.
///
/// The definition is not here: editing a policy's definition mints a new version
/// (see [`WorkspacePolicyRepository::create_policy_version`]) rather than
/// mutating the logical row.
///
/// [`WorkspacePolicyRepository::create_policy_version`]: crate::query::WorkspacePolicyRepository::create_policy_version
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspacePolicy {
    /// Policy display name.
    pub display_name: Option<String>,
    /// Policy description.
    pub description: Option<Option<String>>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}
