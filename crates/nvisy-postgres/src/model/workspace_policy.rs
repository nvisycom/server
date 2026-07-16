//! Workspace policy model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_policies;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt, Slug};

/// Workspace policy representing a structured redaction governance policy.
///
/// The `definition` holds a `nvisy_schema` Policy (rules, labels, fallback,
/// retention) that the redaction engine consumes.
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
    pub slug: Slug,
    /// Human-readable policy name.
    pub name: String,
    /// Policy description.
    pub description: Option<String>,
    /// Semver of the policy body.
    pub version: String,
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
pub struct NewWorkspacePolicy {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// URL-safe policy identifier, unique within the workspace.
    pub slug: Slug,
    /// Policy name.
    pub name: String,
    /// Policy description.
    pub description: Option<String>,
    /// Semver of the policy body.
    pub version: String,
    /// Encrypted Policy body (the engine's Policy type as JSON).
    pub definition: Vec<u8>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

/// Data for updating a workspace policy.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspacePolicy {
    /// Policy name.
    pub name: Option<String>,
    /// Policy description.
    pub description: Option<Option<String>>,
    /// Semver of the policy body.
    pub version: Option<String>,
    /// Encrypted Policy body (the engine's Policy type as JSON).
    pub definition: Option<Vec<u8>>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl WorkspacePolicy {
    /// Returns whether the policy is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

impl HasCreatedAt for WorkspacePolicy {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspacePolicy {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for WorkspacePolicy {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
