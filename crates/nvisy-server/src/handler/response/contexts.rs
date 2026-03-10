//! Context response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a workspace context.
///
/// Note: The encrypted content is stored in NATS and never exposed
/// in API responses. Only metadata is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Unique context identifier.
    pub id: Uuid,
    /// Workspace this context belongs to.
    pub workspace_id: Uuid,
    /// Account that created this context.
    pub account_id: Uuid,
    /// Human-readable context name.
    pub name: String,
    /// Context description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Content MIME type.
    pub mime_type: String,
    /// Size of the content in bytes.
    pub content_size: i64,
    /// When the context was created.
    pub created_at: Timestamp,
    /// When the context was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of contexts.
pub type ContextsPage = Page<Context>;

impl Context {
    /// Creates a response from a database model.
    pub fn from_model(context: WorkspaceContext) -> Self {
        Self {
            id: context.id,
            workspace_id: context.workspace_id,
            account_id: context.account_id,
            name: context.name,
            description: context.description,
            mime_type: context.mime_type,
            content_size: context.content_size,
            created_at: context.created_at.into(),
            updated_at: context.updated_at.into(),
        }
    }
}
