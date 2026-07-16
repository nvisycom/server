//! Workspace context model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_contexts;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt, Slug};

/// Workspace context representing structured reference-data for redaction.
///
/// The `definition` holds a `nvisy_schema` Context (typed reference-data
/// entries) that the redaction engine consumes.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_contexts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceContext {
    /// Unique context identifier.
    pub id: Uuid,
    /// Reference to the workspace this context belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this context.
    pub account_id: Uuid,
    /// URL-safe context identifier, unique within the workspace.
    pub slug: Slug,
    /// Human-readable context name.
    pub name: String,
    /// Context description.
    pub description: Option<String>,
    /// Semver of the context body.
    pub version: String,
    /// Encrypted Context body (the engine's Context type as JSON).
    pub definition: Vec<u8>,
    /// Metadata for filtering/display.
    pub metadata: JsonValue,
    /// Timestamp when the context was created.
    pub created_at: Timestamp,
    /// Timestamp when the context was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the context was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace context.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_contexts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceContext {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// URL-safe context identifier, unique within the workspace.
    pub slug: Slug,
    /// Context name.
    pub name: String,
    /// Context description.
    pub description: Option<String>,
    /// Semver of the context body.
    pub version: String,
    /// Encrypted Context body (the engine's Context type as JSON).
    pub definition: Vec<u8>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

/// Data for updating a workspace context.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_contexts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceContext {
    /// Context name.
    pub name: Option<String>,
    /// Context description.
    pub description: Option<Option<String>>,
    /// Semver of the context body.
    pub version: Option<String>,
    /// Encrypted Context body (the engine's Context type as JSON).
    pub definition: Option<Vec<u8>>,
    /// Metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl WorkspaceContext {
    /// Returns whether the context is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

impl HasCreatedAt for WorkspaceContext {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceContext {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for WorkspaceContext {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
