//! Anthropic (Claude) provider, backed by rig's Anthropic client.

use derive_more::Deref;
use rig::providers::anthropic;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Client;
use crate::error::Error;

/// Anthropic API credentials.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AnthropicCredentials {
    /// Anthropic API key.
    pub api_key: String,
}

/// Anthropic-backed inference client.
#[derive(Deref)]
pub struct AnthropicProvider(anthropic::Client);

impl Client for AnthropicProvider {
    type Credentials = AnthropicCredentials;

    const ID: &str = "anthropic";

    fn connect(credentials: &Self::Credentials, base_url: Option<&str>) -> Result<Self, Error> {
        let mut builder = anthropic::Client::builder().api_key(&credentials.api_key);
        if let Some(base) = base_url {
            builder = builder.base_url(base);
        }
        let client = builder
            .build()
            .map_err(|err| Error::Build(err.to_string()))?;
        Ok(Self(client))
    }
}
