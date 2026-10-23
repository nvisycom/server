//! `OpenAI` provider, backed by rig's `OpenAI` client.
//!
//! Works with `OpenAI` and any OpenAI-compatible endpoint (Azure `OpenAI`, a proxy).

use rig::DynModel;
use rig::operation::Completion;
use rig::providers::openai::{self, OpenAIConfig};

use super::{Client, map_verify};
use crate::error::Result;

/// OpenAI-backed inference client.
pub struct OpenAiProvider(openai::OpenAI);

impl Client for OpenAiProvider {
    /// The `OpenAI` API key.
    type Credentials = str;

    const ID: &str = "openai";

    fn connect(api_key: &Self::Credentials, base_url: Option<&str>) -> Self {
        let mut config = OpenAIConfig::new(api_key);
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
