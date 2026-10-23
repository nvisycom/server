//! Ollama provider, backed by rig's Ollama client.
//!
//! Ollama is typically self-hosted and unauthenticated, so it carries no
//! credentials — only a server base URL.

use rig::DynModel;
use rig::operation::Completion;
use rig::providers::ollama::{self, OllamaConfig};

use super::{Client, OllamaCredentials};
use crate::error::Result;

/// Ollama-backed inference client.
pub struct OllamaProvider(ollama::Ollama);

impl Client for OllamaProvider {
    type Credentials = OllamaCredentials;

    const ID: &str = "ollama";

    fn connect(_credentials: &Self::Credentials, base_url: Option<&str>) -> Self {
        let mut config = OllamaConfig::new();
        if let Some(base) = base_url {
            config = config.with_base_url(base);
        }
        Self(config.client())
    }

    /// Ollama is an unauthenticated local daemon, so there is no credential to
    /// check; verification is a no-op.
    fn verify(&self) -> impl Future<Output = Result<()>> + Send {
        std::future::ready(Ok(()))
    }

    fn completion_model(&self, model: &str) -> DynModel<Completion> {
        self.0.completion(model).into()
    }
}
