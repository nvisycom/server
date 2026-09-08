//! Inference-provider configuration.
//!
//! A provider's stored config is an [`InferenceConfig`] — the inference family
//! (a language model today, other model kinds later). The outer enum is untagged,
//! so on the wire it is flat: the inner config's own `provider` tag is the sole
//! discriminator (`{ "provider": "openai", ... }`). This is the inference analog
//! of [`ConnectionConfig`](super::ConnectionConfig): a provider is a service the
//! platform calls, not a data connection.

use nvisy_core::net::EndpointPolicy;
use nvisy_inference::InferenceConfig;
use nvisy_postgres::types::ProviderType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handler::{ErrorKind, Result};

/// A fully-typed inference-provider configuration.
///
/// Untagged: the inner config's `provider` tag is the sole discriminator, so the
/// stored/wire form is flat. A new inference kind is added as a variant of
/// [`InferenceConfig`] with no change here.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ProviderConfig {
    /// An inference connection (an LLM today; other model kinds later).
    Inference(InferenceConfig),
}

impl ProviderConfig {
    /// The provider identifier for this config, used for the stored `provider`
    /// column and for filtering.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        match self {
            Self::Inference(config) => config.provider_id(),
        }
    }

    /// The inference model type of this config, stored on the provider so it can
    /// be found by kind without decrypting the config.
    #[must_use]
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Self::Inference(config) => match config {
                InferenceConfig::Llm(_) => ProviderType::Llm,
            },
        }
    }

    /// Validates any caller-supplied endpoint on this config under `policy`,
    /// rejecting one the deployment does not allow (plaintext http, a non-global
    /// host in strict mode, and so on) before it is stored or reached.
    ///
    /// The inference `base_url` is the attacker-influenced URL the server would
    /// otherwise send requests to.
    ///
    /// # Errors
    ///
    /// Returns a `BadRequest` if the endpoint is not permitted by `policy`.
    pub async fn validate_endpoints(&self, policy: EndpointPolicy) -> Result<()> {
        let endpoint = match self {
            Self::Inference(config) => config.base_url(),
        };
        if let Some(endpoint) = endpoint {
            policy
                .validate_endpoint(endpoint)
                .await
                .map_err(|err| ErrorKind::BadRequest.with_message(err.to_string()))?;
        }
        Ok(())
    }

    /// Validates this config by building its provider client and verifying the
    /// credentials against the provider.
    ///
    /// # Errors
    ///
    /// Returns the inference error describing the build or verification failure.
    pub async fn validate(&self) -> nvisy_inference::Result<()> {
        match self {
            Self::Inference(config) => config.validate().await,
        }
    }
}
