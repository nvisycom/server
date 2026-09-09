//! Workspace connection model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_connections;
use crate::types::ConnectionType;

/// Workspace connection model: a generic encrypted provider connection.
///
/// A connection stores encrypted credentials for an external provider (object
/// store, LLM, ...); the provider and the shape of the encrypted config
/// distinguish them. Capabilities such as syncing live in satellite tables (see
/// `WorkspaceConnectionSchedule`).
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct WorkspaceConnection {
    /// Unique connection identifier.
    pub id: Uuid,
    /// Reference to the workspace this connection belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this connection.
    pub account_id: Uuid,
    /// Human-readable connection display name.
    pub display_name: String,
    /// Provider identifier (`s3`, `azure`, `gcs`, `google_drive`, `dropbox`, ...).
    pub provider: String,
    /// Capability category of the connection (object store, file service).
    pub connection_type: ConnectionType,
    /// Encrypted connection config (XChaCha20-Poly1305 encrypted JSON):
    /// provider tag, credentials, and any provider-specific settings.
    pub encrypted_data: Vec<u8>,
    /// Whether the connection is enabled.
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
#[must_use]
pub struct NewWorkspaceConnection {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// Connection display name.
    pub display_name: String,
    /// Provider identifier, for indexing and filtering.
    pub provider: String,
    /// Capability category of the connection.
    pub connection_type: ConnectionType,
    /// Encrypted connection config.
    pub encrypted_data: Vec<u8>,
    /// Whether the connection is enabled.
    pub is_active: Option<bool>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

impl NewWorkspaceConnection {
    /// A minimal active object-store connection for `workspace_id`, for tests.
    ///
    /// The display name is unique per call, so several test connections can be
    /// seeded into one workspace without colliding on the per-workspace
    /// display-name unique index.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid) -> Self {
        let suffix = &Uuid::now_v7().simple().to_string()[..12];
        Self {
            workspace_id,
            account_id,
            display_name: format!("Test Connection {suffix}"),
            provider: "s3".to_owned(),
            connection_type: ConnectionType::ObjectStore,
            encrypted_data: vec![1, 2, 3],
            is_active: Some(true),
            metadata: None,
        }
    }
}

/// Data for updating a workspace connection.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceConnection {
    /// Connection display name.
    pub display_name: Option<String>,
    /// Provider identifier.
    pub provider: Option<String>,
    /// Encrypted connection config.
    pub encrypted_data: Option<Vec<u8>>,
    /// Whether the connection is enabled.
    pub is_active: Option<bool>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}
