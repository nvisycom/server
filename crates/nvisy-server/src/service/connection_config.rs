//! Connection provider configuration spanning every capability.
//!
//! A connection's stored config is one of the capability configs — object
//! storage or LLM inference. The outer enum is untagged, so on the wire it is
//! flat: the inner config's own `provider` tag is the sole discriminator
//! (`{ "provider": "s3", ... }`, `{ "provider": "openai", ... }`). Capability
//! crates own their provider configs; this type only composes them.

use nvisy_inference::providers::LlmConfig;
use nvisy_object::providers::StorageConfig;
use nvisy_postgres::types::ProviderType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A fully-typed connection configuration for any capability.
///
/// Untagged: the two inner enums have disjoint `provider` values, so serde
/// resolves the variant from the flat payload without an outer discriminator.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ConnectionConfig {
    /// An object-storage connection (s3, azure, gcs) — sync-capable.
    ObjectStore(StorageConfig),
    /// An LLM inference connection (openai, ollama, anthropic).
    Inference(LlmConfig),
}

impl ConnectionConfig {
    /// The provider identifier for this config, used for the stored `provider`
    /// column and for filtering.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        match self {
            Self::ObjectStore(config) => config.provider_id(),
            Self::Inference(config) => config.provider_id(),
        }
    }

    /// The capability category of this config, stored on the connection so it can
    /// be found by what it does without decrypting the config.
    #[must_use]
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Self::ObjectStore(_) => ProviderType::ObjectStore,
            Self::Inference(_) => ProviderType::LanguageModel,
        }
    }

    /// Whether this connection has the sync capability (object stores do; LLM
    /// connections do not). Determines whether sync configuration and syncs
    /// apply.
    #[must_use]
    pub fn supports_sync(&self) -> bool {
        matches!(self, Self::ObjectStore(_))
    }
}
