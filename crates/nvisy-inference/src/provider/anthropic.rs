//! Anthropic (Claude) provider, backed by rig's Anthropic client.

use derive_more::Deref;
use rig::providers::anthropic;

use super::Client;
use crate::error::{Error, Result};

/// Anthropic-backed inference client.
#[derive(Deref)]
pub struct AnthropicProvider(anthropic::Client);

impl Client for AnthropicProvider {
    /// The Anthropic API key.
    type Credentials = str;

    const ID: &str = "anthropic";

    fn connect(api_key: &Self::Credentials, base_url: Option<&str>) -> Result<Self> {
        let mut builder = anthropic::Client::builder().api_key(api_key);
        if let Some(base) = base_url {
            builder = builder.base_url(base);
        }
        let client = builder
            .build()
            .map_err(|err| Error::Build(err.to_string()))?;
        Ok(Self(client))
    }
}
