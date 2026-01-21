//! Type-safe embedding model references.

use serde::{Deserialize, Serialize};

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
#[serde(rename_all = "kebab-case")]
pub enum OpenAiEmbeddingModel {
    /// text-embedding-3-small (1536 dimensions)
    TextEmbedding3Small,
    /// text-embedding-3-large (3072 dimensions)
    TextEmbedding3Large,
    /// text-embedding-ada-002 (legacy, 1536 dimensions)
    TextEmbeddingAda002,
}

impl OpenAiEmbeddingModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextEmbedding3Small => "text-embedding-3-small",
            Self::TextEmbedding3Large => "text-embedding-3-large",
            Self::TextEmbeddingAda002 => "text-embedding-ada-002",
        }
    }

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
#[serde(rename_all = "kebab-case")]
pub enum CohereEmbeddingModel {
    /// embed-english-v3.0 (1024 dimensions)
    EmbedEnglishV3,
    /// embed-multilingual-v3.0 (1024 dimensions)
    EmbedMultilingualV3,
    /// embed-english-light-v3.0 (384 dimensions)
    EmbedEnglishLightV3,
    /// embed-multilingual-light-v3.0 (384 dimensions)
    EmbedMultilingualLightV3,
}

impl CohereEmbeddingModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmbedEnglishV3 => "embed-english-v3.0",
            Self::EmbedMultilingualV3 => "embed-multilingual-v3.0",
            Self::EmbedEnglishLightV3 => "embed-english-light-v3.0",
            Self::EmbedMultilingualLightV3 => "embed-multilingual-light-v3.0",
        }
    }

    pub fn dimensions(&self) -> usize {
        match self {
            Self::EmbedEnglishV3 | Self::EmbedMultilingualV3 => 1024,
            Self::EmbedEnglishLightV3 | Self::EmbedMultilingualLightV3 => 384,
        }
    }
}

/// Google Gemini embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiEmbeddingModel {
    /// text-embedding-004 (768 dimensions)
    TextEmbedding004,
}

impl GeminiEmbeddingModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextEmbedding004 => "text-embedding-004",
        }
    }

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
    pub fn new(name: impl Into<String>, dimensions: usize) -> Self {
        Self {
            name: name.into(),
            dimensions,
        }
    }

    pub fn nomic_embed_text() -> Self {
        Self::new("nomic-embed-text", 768)
    }

    pub fn mxbai_embed_large() -> Self {
        Self::new("mxbai-embed-large", 1024)
    }

    pub fn all_minilm() -> Self {
        Self::new("all-minilm", 384)
    }
}

impl EmbeddingModel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAi(m) => m.as_str(),
            Self::Cohere(m) => m.as_str(),
            Self::Gemini(m) => m.as_str(),
            #[cfg(feature = "ollama")]
            Self::Ollama(m) => &m.name,
        }
    }

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
