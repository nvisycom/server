//! Transformer node types for processing and transforming data.

mod chunking;
mod config;
mod document;
mod embedding;
mod extraction;
mod processing;
mod quality;
mod routing;

pub use chunking::{
    CharacterChunkingConfig, ChunkingStrategy, ContextualChunkingConfig, PageChunkingConfig,
    ParagraphChunkingConfig, RecursiveChunkingConfig, SemanticChunkingConfig,
    SentenceChunkingConfig, TitleChunkingConfig,
};
pub use config::TransformerConfig;
pub use document::{
    LanguageDetectionConfig, SentimentAnalysisConfig, SentimentGranularity, SummarizationConfig,
    SummaryLength, SummaryStyle, TopicClassificationConfig, TranslationConfig,
};
pub use embedding::{
    CohereEmbeddingConfig, CohereEmbeddingModel, CohereInputType, EmbeddingProvider,
    GeminiEmbeddingConfig, GeminiEmbeddingModel, GenerateEmbeddingsConfig, OllamaEmbeddingConfig,
    OpenAiEmbeddingConfig, OpenAiEmbeddingModel,
};
pub use extraction::{
    CitationFormat, CitationParsingConfig, DetailLevel, EntityRelationExtractionConfig, EntityType,
    ExtractTextConfig, ImageDescriptionConfig, MetadataExtractionConfig,
    NamedEntityRecognitionConfig, TableDescriptionConfig, TableToHtmlConfig,
};
pub use processing::{
    ChunkContentConfig, ChunkContentConfigBuilder, ConvertFormatConfig, FilterConfig,
    LlmTransformConfig, LlmTransformConfigBuilder, MergeConfig, MergeStrategy, ValidateConfig,
};
pub use quality::{
    DataNormalizationConfig, DateTimeNormalization, DeduplicationConfig, DeduplicationStrategy,
    NormalizationType, TextCleaningConfig, TextCleaningOperation, UnitMapping, UnitNormalization,
    UnitSystem,
};
pub use routing::{
    ContentTypePort, ContentTypeRouterConfig, DateField, DurationRouterConfig,
    FileDateRouterConfig, FileNamePattern, FileNameRouterConfig, FileSizeRouterConfig,
    LanguageRouterConfig, MimeMapping, PageCountRouterConfig,
};
use serde::{Deserialize, Serialize};

/// A data transformer node that processes or transforms data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformerNode {
    /// Display name of the transformer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this transformer does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Transformer configuration.
    pub config: TransformerConfig,
}

impl TransformerNode {
    /// Creates a new transformer node.
    pub fn new(config: TransformerConfig) -> Self {
        Self {
            name: None,
            description: None,
            config,
        }
    }

    /// Sets the display name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl From<TransformerConfig> for TransformerNode {
    fn from(config: TransformerConfig) -> Self {
        Self::new(config)
    }
}
