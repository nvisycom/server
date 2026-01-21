//! Embedding provider abstraction.

use super::credentials::EmbeddingCredentials;
use super::model::EmbeddingModel;
#[cfg(feature = "ollama")]
use super::model::OllamaEmbeddingModel;
use crate::{Error, Result};
#[cfg(feature = "ollama")]
use rig::client::Nothing;
use rig::embeddings::{Embedding, EmbeddingModel as RigEmbeddingModel};
use rig::prelude::EmbeddingsClient;
#[cfg(feature = "ollama")]
use rig::providers::ollama;
use rig::providers::{cohere, gemini, openai};

/// Embedding provider that wraps different rig embedding model implementations.
#[derive(Clone)]
pub enum EmbeddingProvider {
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

impl EmbeddingProvider {
    /// Creates a new embedding provider from credentials and model.
    pub fn new(credentials: &EmbeddingCredentials, model: &EmbeddingModel) -> Result<Self> {
        match (credentials, model) {
            (EmbeddingCredentials::OpenAi { api_key }, EmbeddingModel::OpenAi(m)) => {
                let client = openai::Client::new(api_key)
                    .map_err(|e| Error::provider("openai", e.to_string()))?;
                Ok(Self::OpenAi {
                    model: client.embedding_model_with_ndims(m.as_str(), m.dimensions()),
                    model_name: m.as_str().to_string(),
                })
            }
            (EmbeddingCredentials::Cohere { api_key }, EmbeddingModel::Cohere(m)) => {
                let client = cohere::Client::new(api_key)
                    .map_err(|e| Error::provider("cohere", e.to_string()))?;
                Ok(Self::Cohere {
                    model: client.embedding_model_with_ndims(
                        m.as_str(),
                        "search_document",
                        m.dimensions(),
                    ),
                    model_name: m.as_str().to_string(),
                })
            }
            (EmbeddingCredentials::Gemini { api_key }, EmbeddingModel::Gemini(m)) => {
                let client = gemini::Client::new(api_key)
                    .map_err(|e| Error::provider("gemini", e.to_string()))?;
                Ok(Self::Gemini {
                    model: client.embedding_model_with_ndims(m.as_str(), m.dimensions()),
                    model_name: m.as_str().to_string(),
                })
            }
            #[cfg(feature = "ollama")]
            (EmbeddingCredentials::Ollama { base_url }, EmbeddingModel::Ollama(m)) => {
                let client = ollama::Client::builder()
                    .api_key(Nothing)
                    .base_url(base_url)
                    .build()
                    .map_err(|e| Error::provider("ollama", e.to_string()))?;
                Ok(Self::Ollama {
                    client,
                    model_name: m.name.clone(),
                    ndims: m.dimensions,
                })
            }
            #[allow(unreachable_patterns)]
            _ => Err(Error::config("mismatched credentials and model provider")),
        }
    }

    /// Creates an Ollama embedding provider (convenience for local development).
    #[cfg(feature = "ollama")]
    pub fn ollama(base_url: &str, model: OllamaEmbeddingModel) -> Result<Self> {
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(base_url)
            .build()
            .map_err(|e| Error::provider("ollama", e.to_string()))?;
        Ok(Self::Ollama {
            client,
            model_name: model.name,
            ndims: model.dimensions,
        })
    }

    /// Returns the model name.
    pub fn model_name(&self) -> &str {
        match self {
            Self::OpenAi { model_name, .. } => model_name,
            Self::Cohere { model_name, .. } => model_name,
            Self::Gemini { model_name, .. } => model_name,
            #[cfg(feature = "ollama")]
            Self::Ollama { model_name, .. } => model_name,
        }
    }

    /// Returns the number of dimensions.
    pub fn ndims(&self) -> usize {
        match self {
            Self::OpenAi { model, .. } => model.ndims(),
            Self::Cohere { model, .. } => model.ndims(),
            Self::Gemini { model, .. } => model.ndims(),
            #[cfg(feature = "ollama")]
            Self::Ollama { ndims, .. } => *ndims,
        }
    }

    /// Embed a single text document.
    pub async fn embed_text(&self, text: &str) -> Result<Embedding> {
        match self {
            Self::OpenAi { model, .. } => Ok(model.embed_text(text).await?),
            Self::Cohere { model, .. } => Ok(model.embed_text(text).await?),
            Self::Gemini { model, .. } => Ok(model.embed_text(text).await?),
            #[cfg(feature = "ollama")]
            Self::Ollama {
                client,
                model_name,
                ndims,
            } => {
                let model = ollama::EmbeddingModel::new(client.clone(), model_name, *ndims);
                Ok(model.embed_text(text).await?)
            }
        }
    }

    /// Embed multiple text documents.
    pub async fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> Result<Vec<Embedding>> {
        match self {
            Self::OpenAi { model, .. } => Ok(model.embed_texts(texts).await?),
            Self::Cohere { model, .. } => Ok(model.embed_texts(texts).await?),
            Self::Gemini { model, .. } => Ok(model.embed_texts(texts).await?),
            #[cfg(feature = "ollama")]
            Self::Ollama {
                client,
                model_name,
                ndims,
            } => {
                let model = ollama::EmbeddingModel::new(client.clone(), model_name, *ndims);
                Ok(model.embed_texts(texts).await?)
            }
        }
    }
}

impl std::fmt::Debug for EmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenAi { model, model_name } => f
                .debug_struct("EmbeddingProvider::OpenAi")
                .field("model", model_name)
                .field("ndims", &model.ndims())
                .finish(),
            Self::Cohere { model, model_name } => f
                .debug_struct("EmbeddingProvider::Cohere")
                .field("model", model_name)
                .field("ndims", &model.ndims())
                .finish(),
            Self::Gemini { model, model_name } => f
                .debug_struct("EmbeddingProvider::Gemini")
                .field("model", model_name)
                .field("ndims", &model.ndims())
                .finish(),
            #[cfg(feature = "ollama")]
            Self::Ollama {
                model_name, ndims, ..
            } => f
                .debug_struct("EmbeddingProvider::Ollama")
                .field("model", model_name)
                .field("ndims", ndims)
                .finish(),
        }
    }
}
