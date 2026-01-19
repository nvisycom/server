//! Data quality and normalization transformer configurations.

use serde::{Deserialize, Serialize};

/// Configuration for data normalization (dates, times, units).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DataNormalizationConfig {
    /// Types of normalization to apply.
    #[serde(default)]
    pub normalizations: Vec<NormalizationType>,
}

/// Types of data normalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NormalizationType {
    /// Normalize date and time formats.
    DateTime(DateTimeNormalization),
    /// Convert measurement units.
    Unit(UnitNormalization),
}

/// Date and time normalization settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DateTimeNormalization {
    /// Target format (ISO 8601 by default).
    #[serde(default = "default_datetime_format")]
    pub target_format: String,
    /// Target timezone (UTC by default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_timezone: Option<String>,
}

impl Default for DateTimeNormalization {
    fn default() -> Self {
        Self {
            target_format: default_datetime_format(),
            target_timezone: None,
        }
    }
}

/// Unit normalization settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct UnitNormalization {
    /// Target unit system.
    #[serde(default)]
    pub target_system: UnitSystem,
    /// Specific unit mappings (e.g., "miles" -> "kilometers").
    #[serde(default)]
    pub conversions: Vec<UnitMapping>,
}

/// Unit systems for conversion.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    /// International System of Units.
    #[default]
    Si,
    /// Imperial/US customary units.
    Imperial,
}

/// Mapping for unit conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitMapping {
    /// Source unit.
    pub from: String,
    /// Target unit.
    pub to: String,
}

/// Configuration for deduplication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    /// Similarity threshold for considering duplicates (0.0 to 1.0).
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,
    /// Deduplication strategy.
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: default_similarity_threshold(),
            strategy: DeduplicationStrategy::default(),
        }
    }
}

/// Deduplication strategies.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    /// Keep first occurrence.
    #[default]
    KeepFirst,
    /// Keep last occurrence.
    KeepLast,
    /// Keep longest version.
    KeepLongest,
    /// Merge duplicates.
    Merge,
}

/// Configuration for text cleaning and correction.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TextCleaningConfig {
    /// Language code for language-specific rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Cleaning operations to apply.
    #[serde(default)]
    pub operations: Vec<TextCleaningOperation>,
    /// Model to use for LLM-based cleaning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Text cleaning operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextCleaningOperation {
    /// Fix spelling errors.
    FixSpelling,
    /// Fix grammar errors.
    FixGrammar,
    /// Normalize whitespace (remove extra spaces, normalize line breaks).
    NormalizeWhitespace,
    /// Normalize unicode (NFC normalization).
    NormalizeUnicode,
    /// Remove HTML tags.
    StripHtml,
    /// Fix common OCR errors.
    FixOcrErrors,
}

fn default_datetime_format() -> String {
    "%Y-%m-%dT%H:%M:%S%.3fZ".to_string()
}

fn default_similarity_threshold() -> f32 {
    0.9
}
