//! Embedding generation configurations.

use serde::{Deserialize, Serialize};

use super::chunking::ChunkingStrategy;

/// Configuration for embedding generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateEmbeddingsConfig {
    /// Embedding provider and model.
    pub provider: EmbeddingProvider,
    /// Chunking strategy (if content should be chunked before embedding).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking: Option<ChunkingStrategy>,
    /// Batch size for embedding requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,
}

impl GenerateEmbeddingsConfig {
    /// Creates a new embedding config with the given provider.
    pub fn new(provider: EmbeddingProvider) -> Self {
        Self {
            provider,
            chunking: None,
            batch_size: None,
        }
    }

    /// Sets the chunking strategy.
    pub fn with_chunking(mut self, chunking: ChunkingStrategy) -> Self {
        self.chunking = Some(chunking);
        self
    }

    /// Sets the batch size.
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = Some(batch_size);
        self
    }
}

/// Embedding provider configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmbeddingProvider {
    /// OpenAI embeddings.
    OpenAi(OpenAiEmbeddingConfig),
    /// Ollama local embeddings.
    Ollama(OllamaEmbeddingConfig),
    /// Cohere embeddings.
    Cohere(CohereEmbeddingConfig),
    /// Google Gemini embeddings.
    Gemini(GeminiEmbeddingConfig),
}

/// OpenAI embedding configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OpenAiEmbeddingConfig {
    /// Model to use.
    #[serde(default)]
    pub model: OpenAiEmbeddingModel,
    /// Embedding dimensions (for models that support it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl OpenAiEmbeddingConfig {
    /// Creates a new OpenAI embedding config with the given model.
    pub fn new(model: OpenAiEmbeddingModel) -> Self {
        Self {
            model,
            dimensions: None,
        }
    }

    /// Sets custom dimensions.
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

/// OpenAI embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenAiEmbeddingModel {
    /// text-embedding-3-small (1536 dimensions, cheapest).
    #[default]
    TextEmbedding3Small,
    /// text-embedding-3-large (3072 dimensions, best quality).
    TextEmbedding3Large,
    /// text-embedding-ada-002 (1536 dimensions, legacy).
    TextEmbeddingAda002,
}

impl OpenAiEmbeddingModel {
    /// Returns the model identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextEmbedding3Small => "text-embedding-3-small",
            Self::TextEmbedding3Large => "text-embedding-3-large",
            Self::TextEmbeddingAda002 => "text-embedding-ada-002",
        }
    }

    /// Returns the default dimensions for this model.
    pub fn default_dimensions(&self) -> usize {
        match self {
            Self::TextEmbedding3Small => 1536,
            Self::TextEmbedding3Large => 3072,
            Self::TextEmbeddingAda002 => 1536,
        }
    }
}

/// Ollama embedding configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaEmbeddingConfig {
    /// Model name.
    #[serde(default = "default_ollama_model")]
    pub model: String,
    /// Ollama server base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for OllamaEmbeddingConfig {
    fn default() -> Self {
        Self {
            model: default_ollama_model(),
            base_url: None,
        }
    }
}

impl OllamaEmbeddingConfig {
    /// Creates a new Ollama embedding config with the given model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: None,
        }
    }

    /// Sets the base URL.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }
}

/// Cohere embedding configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CohereEmbeddingConfig {
    /// Model to use.
    #[serde(default)]
    pub model: CohereEmbeddingModel,
    /// Input type for embeddings.
    #[serde(default)]
    pub input_type: CohereInputType,
}

impl CohereEmbeddingConfig {
    /// Creates a new Cohere embedding config with the given model.
    pub fn new(model: CohereEmbeddingModel) -> Self {
        Self {
            model,
            input_type: CohereInputType::default(),
        }
    }

    /// Sets the input type.
    pub fn with_input_type(mut self, input_type: CohereInputType) -> Self {
        self.input_type = input_type;
        self
    }
}

/// Cohere embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CohereEmbeddingModel {
    /// embed-english-v3.0 (1024 dimensions).
    #[default]
    EmbedEnglishV3,
    /// embed-multilingual-v3.0 (1024 dimensions).
    EmbedMultilingualV3,
    /// embed-english-light-v3.0 (384 dimensions).
    EmbedEnglishLightV3,
    /// embed-multilingual-light-v3.0 (384 dimensions).
    EmbedMultilingualLightV3,
}

impl CohereEmbeddingModel {
    /// Returns the model identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmbedEnglishV3 => "embed-english-v3.0",
            Self::EmbedMultilingualV3 => "embed-multilingual-v3.0",
            Self::EmbedEnglishLightV3 => "embed-english-light-v3.0",
            Self::EmbedMultilingualLightV3 => "embed-multilingual-light-v3.0",
        }
    }

    /// Returns the default dimensions for this model.
    pub fn default_dimensions(&self) -> usize {
        match self {
            Self::EmbedEnglishV3 | Self::EmbedMultilingualV3 => 1024,
            Self::EmbedEnglishLightV3 | Self::EmbedMultilingualLightV3 => 384,
        }
    }
}

/// Cohere input types.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CohereInputType {
    /// For search queries.
    SearchQuery,
    /// For documents to be searched.
    #[default]
    SearchDocument,
    /// For classification tasks.
    Classification,
    /// For clustering tasks.
    Clustering,
}

/// Google Gemini embedding configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GeminiEmbeddingConfig {
    /// Model to use.
    #[serde(default)]
    pub model: GeminiEmbeddingModel,
}

impl GeminiEmbeddingConfig {
    /// Creates a new Gemini embedding config with the given model.
    pub fn new(model: GeminiEmbeddingModel) -> Self {
        Self { model }
    }
}

/// Gemini embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiEmbeddingModel {
    /// text-embedding-004 (768 dimensions).
    #[default]
    TextEmbedding004,
}

impl GeminiEmbeddingModel {
    /// Returns the model identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextEmbedding004 => "text-embedding-004",
        }
    }

    /// Returns the default dimensions for this model.
    pub fn default_dimensions(&self) -> usize {
        768
    }
}

fn default_ollama_model() -> String {
    "nomic-embed-text".to_string()
}
