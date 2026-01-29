//! Workspace connection model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_connections;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt, SyncStatus};

/// Workspace connection model representing encrypted provider connections.
///
/// Connections store both credentials and context (resumption state) for
/// external providers like databases, cloud storage, and AI services.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceConnection {
    /// Unique connection identifier.
    pub id: Uuid,
    /// Reference to the workspace this connection belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this connection.
    pub account_id: Uuid,
    /// Human-readable connection name.
    pub name: String,
    /// Provider type for indexing (e.g., "openai", "postgres", "s3").
    pub provider: String,
    /// Encrypted connection data (XChaCha20-Poly1305 encrypted JSON).
    /// Contains credentials and context for resumption.
    pub encrypted_data: Vec<u8>,
    /// Whether the connection is active for syncing.
    pub is_active: bool,
    /// Timestamp of the last successful sync.
    pub last_sync_at: Option<Timestamp>,
    /// Current sync status.
    pub sync_status: Option<SyncStatus>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: JsonValue,
    /// Timestamp when the connection was created.
    pub created_at: Timestamp,
    /// Timestamp when the connection was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the connection was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace connection.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceConnection {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// Connection name.
    pub name: String,
    /// Provider type for indexing.
    pub provider: String,
    /// Encrypted connection data.
    pub encrypted_data: Vec<u8>,
    /// Whether the connection is active for syncing.
    pub is_active: Option<bool>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

/// Data for updating a workspace connection.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceConnection {
    /// Connection name.
    pub name: Option<String>,
    /// Provider type.
    pub provider: Option<String>,
    /// Encrypted connection data.
    pub encrypted_data: Option<Vec<u8>>,
    /// Whether the connection is active for syncing.
    pub is_active: Option<bool>,
    /// Timestamp of the last successful sync.
    pub last_sync_at: Option<Option<Timestamp>>,
    /// Current sync status.
    pub sync_status: Option<Option<SyncStatus>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl WorkspaceConnection {
    /// Returns whether the connection is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the connection is currently syncing.
    pub fn is_syncing(&self) -> bool {
        matches!(self.sync_status, Some(SyncStatus::Running))
    }

    /// Returns whether the connection has a pending sync.
    pub fn has_pending_sync(&self) -> bool {
        matches!(self.sync_status, Some(SyncStatus::Pending))
    }
}

impl HasCreatedAt for WorkspaceConnection {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceConnection {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for WorkspaceConnection {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
