//! Workspace connection model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_connections;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

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
