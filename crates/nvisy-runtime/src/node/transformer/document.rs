//! Document understanding transformer configurations.

use serde::{Deserialize, Serialize};

/// Configuration for language detection.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageDetectionConfig {
    /// Minimum confidence threshold (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f32>,
}

/// Configuration for translation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationConfig {
    /// Target language code (e.g., "en", "es", "fr").
    pub target_language: String,
    /// Source language code (auto-detect if not specified).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    /// Model to use for translation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Configuration for sentiment analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SentimentAnalysisConfig {
    /// Granularity of analysis.
    #[serde(default)]
    pub granularity: SentimentGranularity,
    /// Model to use for analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Granularity for sentiment analysis.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SentimentGranularity {
    /// Analyze entire document.
    #[default]
    Document,
    /// Analyze each paragraph.
    Paragraph,
    /// Analyze each sentence.
    Sentence,
}

/// Configuration for topic classification.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TopicClassificationConfig {
    /// Predefined topics to classify into (empty for auto-discovery).
    #[serde(default)]
    pub topics: Vec<String>,
    /// Maximum number of topics to assign.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_topics: Option<usize>,
    /// Model to use for classification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Configuration for summarization.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SummarizationConfig {
    /// Target summary length.
    #[serde(default)]
    pub length: SummaryLength,
    /// Summary style.
    #[serde(default)]
    pub style: SummaryStyle,
    /// Model to use for summarization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Target length for summaries.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryLength {
    /// Brief summary (1-2 sentences).
    Brief,
    /// Standard summary.
    #[default]
    Standard,
    /// Detailed summary.
    Detailed,
    /// Custom max tokens.
    Custom(usize),
}

/// Style for summaries.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStyle {
    /// Extractive summary (key sentences).
    Extractive,
    /// Abstractive summary (rewritten).
    #[default]
    Abstractive,
    /// Bullet points.
    BulletPoints,
}
