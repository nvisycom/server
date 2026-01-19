//! Transformer node configuration types.

use serde::{Deserialize, Serialize};

use super::document::{
    LanguageDetectionConfig, SentimentAnalysisConfig, SummarizationConfig,
    TopicClassificationConfig, TranslationConfig,
};
use super::embedding::GenerateEmbeddingsConfig;
use super::extraction::{
    CitationParsingConfig, EntityRelationExtractionConfig, ExtractTextConfig,
    ImageDescriptionConfig, MetadataExtractionConfig, NamedEntityRecognitionConfig,
    TableDescriptionConfig, TableToHtmlConfig,
};
use super::processing::{
    ChunkContentConfig, ConvertFormatConfig, FilterConfig, LlmTransformConfig, MergeConfig,
    ValidateConfig,
};
use super::quality::{DataNormalizationConfig, DeduplicationConfig, TextCleaningConfig};
use super::routing::{
    ContentTypeRouterConfig, DurationRouterConfig, FileDateRouterConfig, FileNameRouterConfig,
    FileSizeRouterConfig, LanguageRouterConfig, PageCountRouterConfig,
};

/// Transformer node configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformerConfig {
    /// Route by detected content/mime type.
    ContentTypeRouter(ContentTypeRouterConfig),
    /// Route by file size threshold.
    FileSizeRouter(FileSizeRouterConfig),
    /// Route by document page count threshold.
    PageCountRouter(PageCountRouterConfig),
    /// Route by audio/video duration threshold.
    DurationRouter(DurationRouterConfig),
    /// Route by detected language.
    LanguageRouter(LanguageRouterConfig),
    /// Route by file date (created/modified).
    FileDateRouter(FileDateRouterConfig),
    /// Route by filename regex patterns.
    FileNameRouter(FileNameRouterConfig),

    /// Detect language of text content.
    LanguageDetection(LanguageDetectionConfig),
    /// Translate text to target language.
    Translation(TranslationConfig),
    /// Analyze sentiment of text content.
    SentimentAnalysis(SentimentAnalysisConfig),
    /// Classify content into topics.
    TopicClassification(TopicClassificationConfig),
    /// Generate summary of content.
    Summarization(SummarizationConfig),

    /// Extract text from documents (PDF, images via OCR).
    ExtractText(ExtractTextConfig),
    /// Extract metadata from documents.
    MetadataExtraction(MetadataExtractionConfig),
    /// Extract named entities (people, organizations, locations, dates).
    NamedEntityRecognition(NamedEntityRecognitionConfig),
    /// Extract relationships between entities.
    EntityRelationExtraction(EntityRelationExtractionConfig),
    /// Generate descriptions for images.
    ImageDescription(ImageDescriptionConfig),
    /// Generate descriptions for tables.
    TableDescription(TableDescriptionConfig),
    /// Convert tables to HTML.
    TableToHtml(TableToHtmlConfig),
    /// Parse and normalize citations and references.
    CitationParsing(CitationParsingConfig),

    /// Normalize data formats (dates, times, units).
    DataNormalization(DataNormalizationConfig),
    /// Detect and remove duplicate content.
    Deduplication(DeduplicationConfig),
    /// Clean and correct text (spelling, grammar, formatting, noise removal).
    TextCleaning(TextCleaningConfig),

    /// Split content into chunks.
    ChunkContent(ChunkContentConfig),
    /// Generate vector embeddings.
    GenerateEmbeddings(GenerateEmbeddingsConfig),
    /// Transform using an LLM.
    LlmTransform(LlmTransformConfig),
    /// Convert file format.
    ConvertFormat(ConvertFormatConfig),
    /// Validate content against schema.
    Validate(ValidateConfig),
    /// Filter data based on conditions.
    Filter(FilterConfig),
    /// Merge multiple inputs.
    Merge(MergeConfig),
}
