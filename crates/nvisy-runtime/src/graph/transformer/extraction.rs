//! Content extraction transformer configurations.

use serde::{Deserialize, Serialize};

/// Configuration for text extraction.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ExtractTextConfig {
    /// Enable OCR for images.
    #[serde(default)]
    pub ocr_enabled: bool,
    /// OCR language codes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_languages: Option<Vec<String>>,
}

/// Configuration for metadata extraction.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MetadataExtractionConfig {
    /// Specific fields to extract (empty for all available).
    #[serde(default)]
    pub fields: Vec<String>,
}

/// Configuration for named entity recognition.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NamedEntityRecognitionConfig {
    /// Entity types to extract (empty for all).
    #[serde(default)]
    pub entity_types: Vec<EntityType>,
    /// Model to use for NER.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Types of named entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Person,
    Organization,
    Location,
    Date,
    Time,
    Money,
    Percent,
    Product,
    Event,
    WorkOfArt,
    Law,
    Language,
}

/// Configuration for entity relation extraction.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EntityRelationExtractionConfig {
    /// Relation types to extract (empty for all).
    #[serde(default)]
    pub relation_types: Vec<String>,
    /// Model to use for extraction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Include confidence scores.
    #[serde(default)]
    pub include_confidence: bool,
}

/// Configuration for image description.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImageDescriptionConfig {
    /// Detail level of description.
    #[serde(default)]
    pub detail_level: DetailLevel,
    /// Model to use for description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Detail level for descriptions.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    /// Brief, concise description.
    Brief,
    /// Standard level of detail.
    #[default]
    Standard,
    /// Comprehensive, detailed description.
    Detailed,
}

/// Configuration for table description.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TableDescriptionConfig {
    /// Include column statistics.
    #[serde(default)]
    pub include_statistics: bool,
    /// Model to use for description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Configuration for table to HTML conversion.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TableToHtmlConfig {
    /// Include CSS styling.
    #[serde(default)]
    pub include_styles: bool,
    /// Preserve cell formatting.
    #[serde(default = "default_true")]
    pub preserve_formatting: bool,
}

/// Configuration for citation parsing.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CitationParsingConfig {
    /// Output format for normalized citations.
    #[serde(default)]
    pub output_format: CitationFormat,
}

/// Citation output formats.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationFormat {
    /// BibTeX format.
    #[default]
    Bibtex,
    /// CSL-JSON format.
    CslJson,
    /// RIS format.
    Ris,
}

fn default_true() -> bool {
    true
}
