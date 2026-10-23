//! Anthropic (Claude) provider, backed by rig's Anthropic client.

use rig::DynModel;
use rig::operation::Completion;
use rig::providers::anthropic::{self, AnthropicConfig};

use super::{Client, map_verify};
use crate::error::Result;

/// Anthropic-backed inference client.
pub struct AnthropicProvider(anthropic::Anthropic);

impl Client for AnthropicProvider {
    /// The Anthropic API key.
    type Credentials = str;

    const ID: &str = "anthropic";

    fn connect(api_key: &Self::Credentials, base_url: Option<&str>) -> Self {
        let mut config = AnthropicConfig::new(api_key);
        if let Some(base) = base_url {
            config = config.with_base_url(base);
        }
        Self(config.client())
    }

    async fn verify(&self) -> Result<()> {
        map_verify(self.0.verify().await)
    }

    fn completion_model(&self, model: &str) -> DynModel<Completion> {
        self.0.completion(model).into()
    }
}
