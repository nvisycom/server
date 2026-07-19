//! Connection response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceConnection;
use nvisy_postgres::types::{Slug, Username};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Page;

/// Response type for a workspace connection.
///
/// Note: The encrypted connection data is never exposed in API responses.
/// Only metadata about the connection is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// URL slug of the connection, unique within its workspace.
    pub slug: Slug,
    /// Slug of the workspace this connection belongs to.
    pub workspace_slug: Slug,
    /// Handle of the account that created this connection.
    pub creator_username: Username,
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
    /// Creates a response from a database model and its creator's handle.
    pub fn from_model(
        connection: WorkspaceConnection,
        workspace_slug: Slug,
        creator_username: Username,
    ) -> Self {
        Self {
            slug: connection.slug,
            workspace_slug,
            creator_username,
            name: connection.name,
            provider: connection.provider,
            created_at: connection.created_at.into(),
            updated_at: connection.updated_at.into(),
        }
    }
}
