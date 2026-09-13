//! Provider request types.

use garde::Validate;
use nvisy_postgres::types::ProviderId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::input::{CreateProviderInput, UpdateProviderInput};
use crate::service::ProviderConfig;

/// Path parameters for provider operations.
///
/// The workspace is resolved separately from the `{workspaceSlug}` segment by the
/// [`WorkspaceContext`] extractor.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProviderPathParams {
    /// Opaque identifier of the provider.
    pub provider_id: ProviderId,
}

/// Request payload for creating a new workspace provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspaceProvider {
    /// Human-readable provider display name.
    #[garde(length(min = 1, max = 255, chars))]
    pub display_name: String,
    /// Whether the provider is enabled. Omit to default to active; set `false` to
    /// create it disabled.
    pub is_active: Option<bool>,
    /// Typed provider configuration (provider tag + its credentials), encrypted at
    /// rest. The `provider` tag selects which credential shape is required and
    /// which model type the provider has.
    pub config: ProviderConfig,
}

impl From<CreateWorkspaceProvider> for CreateProviderInput {
    fn from(request: CreateWorkspaceProvider) -> Self {
        CreateProviderInput {
            display_name: request.display_name,
            is_active: request.is_active,
            config: request.config,
        }
    }
}

/// Request payload for updating an existing workspace provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspaceProvider {
    /// Human-readable provider display name.
    #[garde(length(min = 1, max = 255, chars))]
    pub display_name: Option<String>,
    /// Whether the provider is enabled. Omit to leave unchanged.
    pub is_active: Option<bool>,
    /// Typed provider configuration. If provided, fully replaces the stored config
    /// (and, with it, the provider). Omit to leave it unchanged.
    pub config: Option<ProviderConfig>,
}

impl From<UpdateWorkspaceProvider> for UpdateProviderInput {
    fn from(request: UpdateWorkspaceProvider) -> Self {
        UpdateProviderInput {
            display_name: request.display_name,
            is_active: request.is_active,
            config: request.config,
        }
    }
}

/// Query parameters for listing providers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProvidersQuery {
    /// Filter by provider (`openai`, `ollama`, `anthropic`). Repeatable; a
    /// provider matches if it uses any of the given providers. Empty means no
    /// filter.
    #[serde(default)]
    pub provider: Vec<String>,
}
