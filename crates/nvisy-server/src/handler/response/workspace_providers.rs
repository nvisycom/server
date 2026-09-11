//! Provider response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceProvider;
use nvisy_postgres::types::{Handle, ProviderId, ProviderType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};

/// Response type for a workspace inference provider.
///
/// Note: The encrypted provider data is never exposed in API responses. Only
/// metadata about the provider is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    /// Opaque identifier of the provider.
    pub id: ProviderId,
    /// Handle of the workspace this provider belongs to.
    pub workspace_slug: Handle,
    /// Account that created this provider.
    pub created_by: AccountRef,
    /// Human-readable provider display name.
    pub display_name: String,
    /// Provider identifier (`openai`, `ollama`, `anthropic`, ...).
    pub provider: String,
    /// Inference model type of the provider (llm, ner).
    pub provider_type: ProviderType,
    /// Whether the provider is enabled.
    pub is_active: bool,
    /// When the provider was created.
    pub created_at: Timestamp,
    /// When the provider was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of providers.
pub type ProvidersPage = Page<Provider>;

impl Provider {
    /// Creates a response from a database model and its creator.
    pub fn from_model(
        provider: WorkspaceProvider,
        workspace_slug: Handle,
        created_by: AccountRef,
    ) -> Self {
        Self {
            id: ProviderId::from_uuid(provider.id),
            workspace_slug,
            created_by,
            display_name: provider.display_name,
            provider: provider.provider,
            provider_type: provider.provider_type,
            is_active: provider.is_active,
            created_at: provider.created_at.into(),
            updated_at: provider.updated_at.into(),
        }
    }
}
