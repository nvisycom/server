//! Replacing a stored, encrypted provider or connection config.

use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::response::{ErrorKind, Result};
use crate::service::{ConnectionConfig, CryptoService, ProviderConfig};

/// A config stored encrypted on its row, keyed by the provider it targets.
pub(super) trait StoredConfig: Serialize + DeserializeOwned {
    /// The resource, for the error when a replacement changes provider.
    const RESOURCE: &'static str;

    /// The provider the config targets.
    fn provider_id(&self) -> &str;

    /// Carries state from the stored config that a replacement does not supply.
    fn carry_over(&mut self, _stored: &Self) {}
}

impl StoredConfig for ProviderConfig {
    const RESOURCE: &'static str = "provider";

    fn provider_id(&self) -> &str {
        Self::provider_id(self)
    }
}

impl StoredConfig for ConnectionConfig {
    const RESOURCE: &'static str = "connection";

    fn provider_id(&self) -> &str {
        Self::provider_id(self)
    }

    /// A file-service connection keeps its OAuth tokens: the client replaces
    /// the settings, never the grant.
    fn carry_over(&mut self, stored: &Self) {
        if let (Self::FileService(new), Self::FileService(existing)) = (self, stored) {
            new.set_tokens(existing.tokens().clone());
        }
    }
}

/// Replaces a stored config with `config`, returning the provider id and the
/// re-encrypted config to write. The provider cannot change: switching it is a
/// different resource, so it is deleted and recreated instead.
///
/// # Errors
///
/// - `BadRequest` if `config` targets a different provider than the stored one.
/// - A crypto error if decrypting the stored config or encrypting `config` fails.
pub(super) fn replace_config<C: StoredConfig>(
    crypto: &CryptoService,
    workspace_id: Uuid,
    stored: &[u8],
    mut config: C,
) -> Result<(String, Vec<u8>)> {
    let stored: C = crypto.decrypt_json(workspace_id, stored)?;
    if config.provider_id() != stored.provider_id() {
        return Err(ErrorKind::BadRequest.with_message(format!(
            "A {}'s provider cannot be changed; delete and recreate instead",
            C::RESOURCE,
        )));
    }
    config.carry_over(&stored);
    let encrypted = crypto.encrypt_json(workspace_id, &config)?;
    Ok((config.provider_id().to_owned(), encrypted))
}
