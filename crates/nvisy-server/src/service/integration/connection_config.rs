//! Transfer-connection configuration.
//!
//! A connection's stored config is one of the transfer-capability configs —
//! object storage or a cloud file service. The outer enum is untagged, so on the
//! wire it is flat: the inner config's own `provider` tag is the sole
//! discriminator (`{ "provider": "s3", ... }`, `{ "provider": "google_drive",
//! ... }`). Capability crates own their provider configs; this type only composes
//! them. Inference services are a separate resource (see `ProviderConfig`).

use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::provider::FileServiceConfig;
use nvisy_object_store::provider::StorageConfig;
use nvisy_postgres::types::ConnectionType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::response::{ErrorKind, Result};

/// A fully-typed transfer-connection configuration.
///
/// Untagged: the two inner enums have disjoint `provider` values, so serde
/// resolves the variant from the flat payload without an outer discriminator.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ConnectionConfig {
    /// An object-storage connection (s3, azure, gcs).
    ObjectStore(StorageConfig),
    /// A file-service connection (google_drive, dropbox, ...).
    FileService(FileServiceConfig),
}

impl ConnectionConfig {
    /// The provider identifier for this config, used for the stored `provider`
    /// column and for filtering.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        match self {
            Self::ObjectStore(config) => config.provider_id(),
            Self::FileService(config) => config.provider_id(),
        }
    }

    /// The capability category of this config, stored on the connection so it can
    /// be found by what it does without decrypting the config.
    #[must_use]
    pub fn connection_type(&self) -> ConnectionType {
        match self {
            Self::ObjectStore(_) => ConnectionType::ObjectStore,
            Self::FileService(_) => ConnectionType::FileService,
        }
    }

    /// Whether this connection can be given a scheduled-sync configuration (a
    /// cron that runs the transfer on a timer).
    ///
    /// Delegates to [`ConnectionType::supports_schedule`] on the config's category,
    /// so the rule lives in one place whether the caller has the typed config or
    /// only the stored `connection_type`.
    #[must_use]
    pub fn supports_schedule(&self) -> bool {
        self.connection_type().supports_schedule()
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
    /// An object store's `endpoint` is the attacker-influenced URL the server
    /// would otherwise send requests to; a file-service connection has no such URL
    /// (OAuth endpoints are provider-owned).
    ///
    /// # Errors
    ///
    /// Returns a `BadRequest` if the endpoint is not permitted by `policy`.
    pub async fn validate_endpoints(&self, policy: EndpointPolicy) -> Result<()> {
        let endpoint = match self {
            Self::ObjectStore(config) => config.endpoint(),
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
