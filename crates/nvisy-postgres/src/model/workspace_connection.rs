//! Workspace connection model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_connections;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt, SyncDeletionPolicy, SyncMode};

/// Workspace connection model representing encrypted provider connections.
///
/// Connections store encrypted credentials for an external object store
/// provider (s3, azure, gcs).
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
    /// Human-readable connection display name.
    pub display_name: String,
    /// Object store provider (`s3`, `azure`, `gcs`).
    pub provider: String,
    /// Whether the connection imports data in or exports data out.
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; `None` means manual-only.
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    pub deletion_policy: SyncDeletionPolicy,
    /// Encrypted connection data (XChaCha20-Poly1305 encrypted JSON).
    /// Contains the provider credentials and optional root path.
    pub encrypted_data: Vec<u8>,
    /// Whether the connection is enabled for syncing.
    pub is_active: bool,
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
    /// Connection display name.
    pub display_name: String,
    /// Provider type for indexing.
    pub provider: String,
    /// Sync direction (defaults to import).
    pub sync_mode: Option<SyncMode>,
    /// Cron expression for scheduled imports.
    pub schedule_cron: Option<String>,
    /// Deletion reconciliation policy (defaults to ignore).
    pub deletion_policy: Option<SyncDeletionPolicy>,
    /// Encrypted connection data.
    pub encrypted_data: Vec<u8>,
    /// Whether the connection is enabled for syncing.
    pub is_active: Option<bool>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

/// Data for updating a workspace connection.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceConnection {
    /// Connection display name.
    pub display_name: Option<String>,
    /// Provider type.
    pub provider: Option<String>,
    /// Sync direction.
    pub sync_mode: Option<SyncMode>,
    /// Cron expression for scheduled imports (`Some(None)` clears it).
    pub schedule_cron: Option<Option<String>>,
    /// Deletion reconciliation policy.
    pub deletion_policy: Option<SyncDeletionPolicy>,
    /// Encrypted connection data.
    pub encrypted_data: Option<Vec<u8>>,
    /// Whether the connection is enabled for syncing.
    pub is_active: Option<bool>,
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
