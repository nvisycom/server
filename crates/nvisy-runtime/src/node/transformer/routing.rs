//! Routing transformer configurations.

use serde::{Deserialize, Serialize};

/// Configuration for content type routing.
///
/// Routes content based on detected mime type (magic bytes + extension fallback).
/// Output ports: `text`, `image`, `audio`, `video`, `document`, `default`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ContentTypeRouterConfig {
    /// Custom mime type to port mappings (overrides defaults).
    #[serde(default)]
    pub mappings: Vec<MimeMapping>,
}

/// Custom mime type to port mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MimeMapping {
    /// Mime type pattern (e.g., "application/pdf", "image/*").
    pub mime: String,
    /// Target port.
    pub port: ContentTypePort,
}

/// Output ports for content type routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTypePort {
    Text,
    Image,
    Audio,
    Video,
    Document,
    Default,
}

/// Configuration for file size routing.
///
/// Routes based on file size threshold.
/// Output ports: `true` (above threshold), `false` (below threshold), `default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSizeRouterConfig {
    /// Size threshold in bytes.
    pub threshold_bytes: u64,
}

/// Configuration for page count routing.
///
/// Routes documents based on page count threshold.
/// Output ports: `true` (above threshold), `false` (below threshold), `default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageCountRouterConfig {
    /// Page count threshold.
    pub threshold_pages: u32,
}

/// Configuration for duration routing.
///
/// Routes audio/video based on duration threshold.
/// Output ports: `true` (above threshold), `false` (below threshold), `default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationRouterConfig {
    /// Duration threshold in seconds.
    pub threshold_seconds: u64,
}

/// Configuration for language routing.
///
/// Routes based on detected content language.
/// Output ports: configured language codes + `multiple` + `default`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageRouterConfig {
    /// Language codes to route (e.g., "en", "es", "fr").
    #[serde(default)]
    pub languages: Vec<String>,
    /// Minimum confidence threshold (0.0 to 1.0) to consider a language detected.
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
    /// Minimum percentage of content (0.0 to 1.0) for a language to be considered present.
    #[serde(default = "default_min_percentage")]
    pub min_percentage: f32,
}

/// Configuration for file date routing.
///
/// Routes based on file date threshold.
/// Output ports: `true` (newer than threshold), `false` (older than threshold), `default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDateRouterConfig {
    /// Which date field to use.
    #[serde(default)]
    pub date_field: DateField,
    /// Threshold as ISO 8601 datetime or relative duration (e.g., "7d", "30d", "1y").
    pub threshold: String,
}

/// Date field to use for routing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateField {
    /// File creation date.
    #[default]
    Created,
    /// File modification date.
    Modified,
}

/// Configuration for filename routing.
///
/// Routes based on regex pattern matching on filename.
/// Output ports: user-defined ports from pattern mappings + `default`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FileNameRouterConfig {
    /// Regex pattern to port mappings (evaluated in order, first match wins).
    #[serde(default)]
    pub patterns: Vec<FileNamePattern>,
}

/// Filename pattern to port mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNamePattern {
    /// Regex pattern to match against filename.
    pub regex: String,
    /// Target port name.
    pub port: String,
}

fn default_confidence() -> f32 {
    0.8
}

fn default_min_percentage() -> f32 {
    0.1
}
