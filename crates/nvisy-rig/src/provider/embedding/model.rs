//! Type-safe embedding model references.

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

/// Reference to an embedding model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "model", rename_all = "snake_case")]
pub enum EmbeddingModel {
    /// OpenAI embedding models.
    OpenAi(OpenAiEmbeddingModel),
    /// Cohere embedding models.
    Cohere(CohereEmbeddingModel),
    /// Google Gemini embedding models.
    Gemini(GeminiEmbeddingModel),
    /// Ollama local models.
    #[cfg(feature = "ollama")]
    Ollama(OllamaEmbeddingModel),
}

/// OpenAI embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum OpenAiEmbeddingModel {
    /// text-embedding-3-small (1536 dimensions)
    #[strum(serialize = "text-embedding-3-small")]
    TextEmbedding3Small,
    /// text-embedding-3-large (3072 dimensions)
    #[strum(serialize = "text-embedding-3-large")]
    TextEmbedding3Large,
    /// text-embedding-ada-002 (legacy, 1536 dimensions)
    #[strum(serialize = "text-embedding-ada-002")]
    TextEmbeddingAda002,
}

impl OpenAiEmbeddingModel {
    /// Returns the embedding dimensions for this model.
    pub fn dimensions(&self) -> usize {
        match self {
            Self::TextEmbedding3Small => 1536,
            Self::TextEmbedding3Large => 3072,
            Self::TextEmbeddingAda002 => 1536,
        }
    }
}

/// Cohere embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum CohereEmbeddingModel {
    /// embed-english-v3.0 (1024 dimensions)
    #[strum(serialize = "embed-english-v3.0")]
    EmbedEnglishV3,
    /// embed-multilingual-v3.0 (1024 dimensions)
    #[strum(serialize = "embed-multilingual-v3.0")]
    EmbedMultilingualV3,
    /// embed-english-light-v3.0 (384 dimensions)
    #[strum(serialize = "embed-english-light-v3.0")]
    EmbedEnglishLightV3,
    /// embed-multilingual-light-v3.0 (384 dimensions)
    #[strum(serialize = "embed-multilingual-light-v3.0")]
    EmbedMultilingualLightV3,
}

impl CohereEmbeddingModel {
    /// Returns the embedding dimensions for this model.
    pub fn dimensions(&self) -> usize {
        match self {
            Self::EmbedEnglishV3 | Self::EmbedMultilingualV3 => 1024,
            Self::EmbedEnglishLightV3 | Self::EmbedMultilingualLightV3 => 384,
        }
    }
}

/// Google Gemini embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum GeminiEmbeddingModel {
    /// text-embedding-004 (768 dimensions)
    #[strum(serialize = "text-embedding-004")]
    TextEmbedding004,
}

impl GeminiEmbeddingModel {
    /// Returns the embedding dimensions for this model.
    pub fn dimensions(&self) -> usize {
        768
    }
}

/// Ollama embedding model configuration.
#[cfg(feature = "ollama")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaEmbeddingModel {
    /// Model name (e.g., "nomic-embed-text", "mxbai-embed-large").
    pub name: String,
    /// Embedding dimensions.
    pub dimensions: usize,
}

#[cfg(feature = "ollama")]
impl OllamaEmbeddingModel {
    /// Creates a new Ollama embedding model configuration.
    pub fn new(name: impl Into<String>, dimensions: usize) -> Self {
        Self {
            name: name.into(),
            dimensions,
        }
    }

    /// nomic-embed-text (768 dimensions)
    pub fn nomic_embed_text() -> Self {
        Self::new("nomic-embed-text", 768)
    }

    /// mxbai-embed-large (1024 dimensions)
    pub fn mxbai_embed_large() -> Self {
        Self::new("mxbai-embed-large", 1024)
    }

    /// all-minilm (384 dimensions)
    pub fn all_minilm() -> Self {
        Self::new("all-minilm", 384)
    }
}

impl EmbeddingModel {
    /// Returns the model identifier string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAi(m) => m.as_ref(),
            Self::Cohere(m) => m.as_ref(),
            Self::Gemini(m) => m.as_ref(),
            #[cfg(feature = "ollama")]
            Self::Ollama(m) => &m.name,
        }
    }

    /// Returns the embedding dimensions for this model.
    pub fn dimensions(&self) -> usize {
        match self {
            Self::OpenAi(m) => m.dimensions(),
            Self::Cohere(m) => m.dimensions(),
            Self::Gemini(m) => m.dimensions(),
            #[cfg(feature = "ollama")]
            Self::Ollama(m) => m.dimensions,
        }
    }
}
