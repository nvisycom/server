//! Connection provider configuration spanning every capability.
//!
//! A connection's stored config is one of the capability configs — object
//! storage, LLM inference, or a cloud file service. The outer enum is untagged,
//! so on the wire it is flat: the inner config's own `provider` tag is the sole
//! discriminator (`{ "provider": "s3", ... }`, `{ "provider": "openai", ... }`,
//! `{ "provider": "google_drive", ... }`). Capability crates own their provider
//! configs; this type only composes them.

use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::provider::FileServiceConfig;
use nvisy_inference::LlmConfig;
use nvisy_object_store::provider::StorageConfig;
use nvisy_postgres::types::ProviderType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handler::{ErrorKind, Result};

/// A fully-typed connection configuration for any capability.
///
/// Untagged: the two inner enums have disjoint `provider` values, so serde
/// resolves the variant from the flat payload without an outer discriminator.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ConnectionConfig {
    /// An object-storage connection (s3, azure, gcs) — transfer-capable.
    ObjectStore(StorageConfig),
    /// A file-service connection (google_drive, dropbox, ...) — transfer-capable.
    FileService(FileServiceConfig),
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
            Self::FileService(config) => config.provider_id(),
            Self::Inference(config) => config.provider_id(),
        }
    }

    /// The capability category of this config, stored on the connection so it can
    /// be found by what it does without decrypting the config.
    #[must_use]
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Self::ObjectStore(_) => ProviderType::ObjectStore,
            Self::FileService(_) => ProviderType::FileService,
            Self::Inference(_) => ProviderType::LanguageModel,
        }
    }

    /// Whether this connection can transfer files (import from or export to it).
    /// Object stores and cloud file services can; an inference connection cannot.
    /// Determines whether sync configuration, syncs, and transfers apply.
    #[must_use]
    pub fn supports_transfer(&self) -> bool {
        matches!(self, Self::ObjectStore(_) | Self::FileService(_))
    }

    /// Whether this connection is a cloud file service. Only file services back
    /// the picker import (their listing is a provider-native picker, not a
    /// server-side enumeration).
    #[must_use]
    pub fn is_file_service(&self) -> bool {
        matches!(self, Self::FileService(_))
    }

    /// Validates any caller-supplied endpoint on this config under `policy`,
    /// rejecting one the deployment does not allow (plaintext http, a non-global
    /// host in strict mode, and so on) before it is stored or reached.
    ///
    /// Object-store `endpoint` and inference `base_url` are the attacker-
    /// influenced URLs the server would otherwise send requests to; a file-
    /// service connection has no such URL (OAuth endpoints are provider-owned).
    ///
    /// # Errors
    ///
    /// Returns a `BadRequest` if the endpoint is not permitted by `policy`.
    pub async fn validate_endpoints(&self, policy: EndpointPolicy) -> Result<()> {
        let endpoint = match self {
            Self::ObjectStore(config) => config.endpoint(),
            Self::Inference(config) => config.base_url(),
            Self::FileService(_) => None,
        };
        if let Some(endpoint) = endpoint {
            policy
                .validate_endpoint(endpoint)
                .await
                .map_err(|err| ErrorKind::BadRequest.with_message(err.to_string()))?;
        }
        Ok(())
    }
}
