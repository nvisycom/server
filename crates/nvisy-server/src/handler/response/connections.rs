//! Connection response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceConnection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a workspace connection.
///
/// Note: The encrypted connection data is never exposed in API responses.
/// Only metadata about the connection is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// Unique connection identifier.
    pub id: Uuid,
    /// Workspace this connection belongs to.
    pub workspace_id: Uuid,
    /// Account that created this connection.
    pub account_id: Uuid,
    /// Human-readable connection name.
    pub name: String,
    /// Provider type (e.g., "openai", "postgres", "s3").
    pub provider: String,
    /// When the connection was created.
    pub created_at: Timestamp,
    /// When the connection was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of connections.
pub type ConnectionsPage = Page<Connection>;

impl Connection {
    /// Creates a response from a database model.
    pub fn from_model(connection: WorkspaceConnection) -> Self {
        Self {
            id: connection.id,
            workspace_id: connection.workspace_id,
            account_id: connection.account_id,
            name: connection.name,
            provider: connection.provider,
            created_at: connection.created_at.into(),
            updated_at: connection.updated_at.into(),
        }
    }
}
