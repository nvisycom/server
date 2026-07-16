//! Context response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceContext;
use nvisy_postgres::types::Slug;
use nvisy_schema::context::Context as SchemaContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;
use crate::service::CryptoService;

/// Response type for a workspace context.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// URL slug of the context, unique within its workspace.
    pub slug: Slug,
    /// Slug of the workspace this context belongs to.
    pub workspace_slug: Slug,
    /// Account that created this context.
    pub account_id: Uuid,
    /// Human-readable context name.
    pub name: String,
    /// Context description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semver of the context body.
    pub version: String,
    /// The structured context body consumed by the engine.
    pub definition: SchemaContext,
    /// When the context was created.
    pub created_at: Timestamp,
    /// When the context was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of contexts.
pub type ContextsPage = Page<Context>;

impl Context {
    /// Creates a response from a database model, decrypting the definition.
    pub fn from_model(
        context: WorkspaceContext,
        workspace_slug: Slug,
        crypto: &CryptoService,
    ) -> crate::handler::Result<Self> {
        let definition =
            crypto.decrypt_json::<SchemaContext>(context.workspace_id, &context.definition)?;

        Ok(Self {
            slug: context.slug,
            workspace_slug,
            account_id: context.account_id,
            name: context.name,
            description: context.description,
            version: context.version,
            definition,
            created_at: context.created_at.into(),
            updated_at: context.updated_at.into(),
        })
    }
}
