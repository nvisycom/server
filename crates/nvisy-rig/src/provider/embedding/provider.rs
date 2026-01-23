//! Embedding provider abstraction.

use std::sync::Arc;

use nvisy_core::IntoProvider;
#[cfg(feature = "ollama")]
use rig::client::Nothing;
use rig::embeddings::{Embedding, EmbeddingModel as RigEmbeddingModel};
use rig::prelude::EmbeddingsClient;
#[cfg(feature = "ollama")]
use rig::providers::ollama;
use rig::providers::{cohere, gemini, openai};

use super::credentials::EmbeddingCredentials;
use super::model::EmbeddingModel;
#[cfg(feature = "ollama")]
use super::model::OllamaEmbeddingModel;
use crate::Error;

/// Default maximum documents per embedding request.
///
/// This is a conservative default; individual providers may support more.
pub(crate) const DEFAULT_MAX_DOCUMENTS: usize = 96;

/// Embedding provider that wraps different rig embedding model implementations.
///
/// This is a cheaply cloneable wrapper around an `Arc<EmbeddingService>`.
#[derive(Clone)]
pub struct EmbeddingProvider(Arc<EmbeddingService>);

pub(crate) enum EmbeddingService {
    OpenAi {
        model: openai::EmbeddingModel,
        model_name: String,
    },
    Cohere {
        model: cohere::EmbeddingModel,
        model_name: String,
    },
    Gemini {
        model: gemini::embedding::EmbeddingModel,
        model_name: String,
    },
    #[cfg(feature = "ollama")]
    Ollama {
        client: ollama::Client,
        model_name: String,
        ndims: usize,
    },
}

#[async_trait::async_trait]
impl IntoProvider for EmbeddingProvider {
    type Credentials = EmbeddingCredentials;
    type Params = EmbeddingModel;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = match (credentials, params) {
            (EmbeddingCredentials::OpenAi { api_key }, EmbeddingModel::OpenAi(m)) => {
                let client = openai::Client::new(&api_key)
                    .map_err(|e| Error::provider("openai", e.to_string()))?;
                EmbeddingService::OpenAi {
                    model: client.embedding_model_with_ndims(m.as_ref(), m.dimensions()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (EmbeddingCredentials::Cohere { api_key }, EmbeddingModel::Cohere(m)) => {
                let client = cohere::Client::new(&api_key)
                    .map_err(|e| Error::provider("cohere", e.to_string()))?;
                EmbeddingService::Cohere {
                    model: client.embedding_model_with_ndims(
                        m.as_ref(),
                        "search_document",
                        m.dimensions(),
                    ),
                    model_name: m.as_ref().to_string(),
                }
            }
            (EmbeddingCredentials::Gemini { api_key }, EmbeddingModel::Gemini(m)) => {
                let client = gemini::Client::new(&api_key)
                    .map_err(|e| Error::provider("gemini", e.to_string()))?;
                EmbeddingService::Gemini {
                    model: client.embedding_model_with_ndims(m.as_ref(), m.dimensions()),
                    model_name: m.as_ref().to_string(),
                }
            }
            #[cfg(feature = "ollama")]
            (EmbeddingCredentials::Ollama { base_url }, EmbeddingModel::Ollama(m)) => {
                let client = ollama::Client::builder()
                    .api_key(Nothing)
                    .base_url(&base_url)
                    .build()
                    .map_err(|e| Error::provider("ollama", e.to_string()))?;
                EmbeddingService::Ollama {
                    client,
                    model_name: m.name.clone(),
                    ndims: m.dimensions,
                }
            }
            #[allow(unreachable_patterns)]
            _ => return Err(Error::config("mismatched credentials and model provider").into()),
        };
        Ok(Self(Arc::new(inner)))
    }
}

impl EmbeddingProvider {
    /// Returns a reference to the inner provider.
    pub(crate) fn inner(&self) -> &EmbeddingService {
        &self.0
    }

    /// Creates an Ollama embedding provider (convenience for local development).
    #[cfg(feature = "ollama")]
    pub fn ollama(base_url: &str, model: OllamaEmbeddingModel) -> nvisy_core::Result<Self> {
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(base_url)
            .build()
            .map_err(|e| Error::provider("ollama", e.to_string()))?;
        Ok(Self(Arc::new(EmbeddingService::Ollama {
            client,
            model_name: model.name,
            ndims: model.dimensions,
        })))
    }

    /// Returns the model name.
    pub fn model_name(&self) -> &str {
        match self.0.as_ref() {
            EmbeddingService::OpenAi { model_name, .. } => model_name,
            EmbeddingService::Cohere { model_name, .. } => model_name,
            EmbeddingService::Gemini { model_name, .. } => model_name,
            #[cfg(feature = "ollama")]
            EmbeddingService::Ollama { model_name, .. } => model_name,
        }
    }

    /// Returns the provider name.
    pub fn provider_name(&self) -> &'static str {
        match self.0.as_ref() {
            EmbeddingService::OpenAi { .. } => "openai",
            EmbeddingService::Cohere { .. } => "cohere",
            EmbeddingService::Gemini { .. } => "gemini",
            #[cfg(feature = "ollama")]
            EmbeddingService::Ollama { .. } => "ollama",
        }
    }

    /// Embed a single text document.
    ///
    /// This is a convenience method that delegates to the trait implementation.
    pub async fn embed_text(&self, text: &str) -> nvisy_core::Result<Embedding> {
        RigEmbeddingModel::embed_text(self, text)
            .await
            .map_err(|e| Error::provider(self.provider_name(), e.to_string()).into())
    }

    /// Embed multiple text documents.
    ///
    /// This is a convenience method that delegates to the trait implementation.
    pub async fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> nvisy_core::Result<Vec<Embedding>> {
        RigEmbeddingModel::embed_texts(self, texts)
            .await
            .map_err(|e| Error::provider(self.provider_name(), e.to_string()).into())
    }
}

impl std::fmt::Debug for EmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.as_ref() {
            EmbeddingService::OpenAi { model, model_name } => f
                .debug_struct("EmbeddingProvider::OpenAi")
                .field("model", model_name)
                .field("ndims", &model.ndims())
                .finish(),
            EmbeddingService::Cohere { model, model_name } => f
                .debug_struct("EmbeddingProvider::Cohere")
                .field("model", model_name)
                .field("ndims", &model.ndims())
                .finish(),
            EmbeddingService::Gemini { model, model_name } => f
                .debug_struct("EmbeddingProvider::Gemini")
                .field("model", model_name)
                .field("ndims", &model.ndims())
                .finish(),
            #[cfg(feature = "ollama")]
            EmbeddingService::Ollama {
                model_name, ndims, ..
            } => f
                .debug_struct("EmbeddingProvider::Ollama")
                .field("model", model_name)
                .field("ndims", ndims)
                .finish(),
        }
    }
}
