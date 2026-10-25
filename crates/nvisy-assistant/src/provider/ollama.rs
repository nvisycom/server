//! Ollama provider, backed by rig's Ollama client.
//!
//! Ollama is typically self-hosted and unauthenticated, so it carries no
//! credentials — only a server base URL.

use rig::DynModel;
use rig::operation::Completion;
use rig::providers::ollama::{self, OllamaConfig};

use super::{Client, OllamaCredentials, map_verify};
use crate::error::Result;

/// Ollama-backed assistant client.
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

    /// Ollama is unauthenticated, so there is no credential to check, but the
    /// daemon still has to be reachable at the configured base URL. Probe it by
    /// listing models (`GET /api/tags`); an unreachable server or a wrong URL
    /// fails here rather than at the first assistant request.
    async fn verify(&self) -> Result<()> {
        map_verify(self.0.list_models().await.map(|_| ()))
    }

    fn completion_model(&self, model: &str) -> DynModel<Completion> {
        self.0.completion(model).into()
    }
}
