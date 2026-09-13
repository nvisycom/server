//! Provider service inputs.

use crate::service::ProviderConfig;

/// Input for creating a provider.
pub struct CreateProviderInput {
    /// Human-readable provider display name.
    pub display_name: String,
    /// Whether the provider is enabled; `None` defaults to active.
    pub is_active: Option<bool>,
    /// Typed provider configuration (provider tag + credentials).
    pub config: ProviderConfig,
}

/// Input for updating a provider. A present `config` fully replaces the stored
/// one; omitted fields are left unchanged.
pub struct UpdateProviderInput {
    /// New display name.
    pub display_name: Option<String>,
    /// New active state.
    pub is_active: Option<bool>,
    /// Replacement configuration.
    pub config: Option<ProviderConfig>,
}
