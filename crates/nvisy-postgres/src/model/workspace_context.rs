//! Workspace context model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_contexts;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// Workspace context model representing metadata for encrypted context files.
///
/// The actual encrypted content is stored in NATS object storage.
/// This record holds the metadata and storage reference.
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
    /// Human-readable context name.
    pub name: String,
    /// Context description.
    pub description: Option<String>,
    /// Content MIME type.
    pub mime_type: String,
    /// NATS object store key for the encrypted content.
    pub storage_key: String,
    /// Size of the encrypted content in bytes.
    pub content_size: i64,
    /// SHA-256 hash of the encrypted content.
    pub content_hash: Vec<u8>,
    /// Non-encrypted metadata for filtering/display.
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
    /// Context name.
    pub name: String,
    /// Context description.
    pub description: Option<String>,
    /// Content MIME type.
    pub mime_type: String,
    /// NATS object store key.
    pub storage_key: String,
    /// Size of the encrypted content in bytes.
    pub content_size: i64,
    /// SHA-256 hash of the encrypted content.
    pub content_hash: Vec<u8>,
    /// Non-encrypted metadata for filtering/display.
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
    /// Content MIME type.
    pub mime_type: Option<String>,
    /// NATS object store key (updated on content replacement).
    pub storage_key: Option<String>,
    /// Size of the encrypted content in bytes.
    pub content_size: Option<i64>,
    /// SHA-256 hash of the encrypted content.
    pub content_hash: Option<Vec<u8>>,
    /// Non-encrypted metadata for filtering/display.
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
