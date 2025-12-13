//! Models module for OLMo v2 OCR integration
//!
//! This module provides types and functionality for working with different OLMo v2 models
//! that can be used for optical character recognition tasks.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Represents an OLMo v2 model available for OCR tasks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OlmoModel {
    /// Unique identifier for the model
    pub id: String,
    /// Display name of the model
    pub name: String,
    /// Model version
    pub version: String,
    /// Model capabilities
    pub capabilities: ModelCapabilities,
    /// Supported languages
    pub supported_languages: Vec<SupportedLanguage>,
    /// Maximum input size in bytes
    pub max_input_size: usize,
    /// Whether the model is currently available
    pub is_available: bool,
    /// Model-specific configuration
    pub config: ModelConfig,
}

/// Configuration parameters for an OLMo v2 model
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Temperature for text generation (0.0 to 1.0)
    pub temperature: f64,
    /// Maximum number of tokens to generate
    pub max_tokens: Option<u32>,
    /// Top-p sampling parameter
    pub top_p: Option<f64>,
    /// Top-k sampling parameter
    pub top_k: Option<u32>,
    /// Repetition penalty
    pub repetition_penalty: Option<f64>,
    /// Whether to use beam search
    pub use_beam_search: bool,
    /// Number of beams for beam search
    pub beam_size: Option<u32>,
    /// Custom model parameters
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

/// Capabilities of an OLMo v2 model
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supports text extraction from images
    pub text_extraction: bool,
    /// Supports document layout analysis
    pub layout_analysis: bool,
    /// Supports table recognition
    pub table_recognition: bool,
    /// Supports handwriting recognition
    pub handwriting_recognition: bool,
    /// Supports mathematical formula recognition
    pub formula_recognition: bool,
    /// Supports multi-language documents
    pub multilingual: bool,
    /// Supports structured data extraction
    pub structured_extraction: bool,
    /// Maximum image resolution supported (width × height)
    pub max_resolution: (u32, u32),
    /// Supported image formats
    pub supported_formats: Vec<ImageFormat>,
}

/// Supported image formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    /// JPEG format
    #[serde(rename = "jpeg")]
    Jpeg,
    /// PNG format
    #[serde(rename = "png")]
    Png,
    /// WebP format
    #[serde(rename = "webp")]
    WebP,
    /// TIFF format
    #[serde(rename = "tiff")]
    Tiff,
    /// BMP format
    #[serde(rename = "bmp")]
    Bmp,
    /// PDF format
    #[serde(rename = "pdf")]
    Pdf,
}

/// Supported languages for OCR
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportedLanguage {
    /// ISO 639-1 language code
    pub code: String,
    /// Language name in English
    pub name: String,
    /// Native language name
    pub native_name: String,
    /// Confidence score for this language (0.0 to 1.0)
    pub confidence: f64,
}

/// Predefined OLMo v2 model variants
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OlmoModelVariant {
    /// Base OLMo v2 model for general OCR tasks
    #[serde(rename = "olmo-2-base")]
    Base,
    /// Large OLMo v2 model for high-accuracy OCR
    #[serde(rename = "olmo-2-large")]
    Large,
    /// Specialized model for document analysis
    #[serde(rename = "olmo-2-document")]
    Document,
    /// Specialized model for handwriting recognition
    #[serde(rename = "olmo-2-handwriting")]
    Handwriting,
    /// Specialized model for mathematical formulas
    #[serde(rename = "olmo-2-math")]
    Math,
    /// Custom model variant
    #[serde(rename = "custom")]
    Custom(String),
}

/// Model selection criteria
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSelector {
    /// Preferred model variant
    pub variant: Option<OlmoModelVariant>,
    /// Required capabilities
    pub required_capabilities: Vec<String>,
    /// Preferred languages
    pub preferred_languages: Vec<String>,
    /// Maximum acceptable processing time
    pub max_processing_time: Option<std::time::Duration>,
    /// Minimum confidence threshold
    pub min_confidence: f64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: None,
            top_p: Some(0.9),
            top_k: None,
            repetition_penalty: Some(1.1),
            use_beam_search: false,
            beam_size: None,
            custom_parameters: HashMap::new(),
        }
    }
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            text_extraction: true,
            layout_analysis: false,
            table_recognition: false,
            handwriting_recognition: false,
            formula_recognition: false,
            multilingual: true,
            structured_extraction: false,
            max_resolution: (4096, 4096),
            supported_formats: vec![
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::WebP,
                ImageFormat::Pdf,
            ],
        }
    }
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self {
            variant: Some(OlmoModelVariant::Base),
            required_capabilities: vec!["text_extraction".to_string()],
            preferred_languages: vec!["en".to_string()],
            max_processing_time: None,
            min_confidence: 0.8,
        }
    }
}

impl fmt::Display for OlmoModelVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base => write!(f, "olmo-2-base"),
            Self::Large => write!(f, "olmo-2-large"),
            Self::Document => write!(f, "olmo-2-document"),
            Self::Handwriting => write!(f, "olmo-2-handwriting"),
            Self::Math => write!(f, "olmo-2-math"),
            Self::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jpeg => write!(f, "JPEG"),
            Self::Png => write!(f, "PNG"),
            Self::WebP => write!(f, "WebP"),
            Self::Tiff => write!(f, "TIFF"),
            Self::Bmp => write!(f, "BMP"),
            Self::Pdf => write!(f, "PDF"),
        }
    }
}

impl ImageFormat {
    /// Get the MIME type for this image format
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::WebP => "image/webp",
            Self::Tiff => "image/tiff",
            Self::Bmp => "image/bmp",
            Self::Pdf => "application/pdf",
        }
    }

    /// Get file extensions for this format
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Jpeg => &["jpg", "jpeg"],
            Self::Png => &["png"],
            Self::WebP => &["webp"],
            Self::Tiff => &["tiff", "tif"],
            Self::Bmp => &["bmp"],
            Self::Pdf => &["pdf"],
        }
    }

    /// Detect format from MIME type
    pub fn from_mime_type(mime: &str) -> Option<Self> {
        match mime {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/webp" => Some(Self::WebP),
            "image/tiff" => Some(Self::Tiff),
            "image/bmp" => Some(Self::Bmp),
            "application/pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.to_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::WebP),
            "tiff" | "tif" => Some(Self::Tiff),
            "bmp" => Some(Self::Bmp),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}

impl OlmoModel {
    /// Create a new OLMo model instance
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            capabilities: ModelCapabilities::default(),
            supported_languages: vec![SupportedLanguage {
                code: "en".to_string(),
                name: "English".to_string(),
                native_name: "English".to_string(),
                confidence: 0.95,
            }],
            max_input_size: 10 * 1024 * 1024, // 10 MB default
            is_available: true,
            config: ModelConfig::default(),
        }
    }

    /// Check if the model supports a specific capability
    pub fn supports_capability(&self, capability: &str) -> bool {
        match capability {
            "text_extraction" => self.capabilities.text_extraction,
            "layout_analysis" => self.capabilities.layout_analysis,
            "table_recognition" => self.capabilities.table_recognition,
            "handwriting_recognition" => self.capabilities.handwriting_recognition,
            "formula_recognition" => self.capabilities.formula_recognition,
            "multilingual" => self.capabilities.multilingual,
            "structured_extraction" => self.capabilities.structured_extraction,
            _ => false,
        }
    }

    /// Check if the model supports a specific language
    pub fn supports_language(&self, language_code: &str) -> bool {
        self.supported_languages
            .iter()
            .any(|lang| lang.code == language_code)
    }

    /// Check if the model supports a specific image format
    pub fn supports_format(&self, format: &ImageFormat) -> bool {
        self.capabilities.supported_formats.contains(format)
    }

    /// Validate that input size is within limits
    pub fn validate_input_size(&self, size: usize) -> Result<()> {
        if size > self.max_input_size {
            return Err(Error::document_too_large(size, self.max_input_size));
        }
        Ok(())
    }
}

impl ModelSelector {
    /// Create a new model selector
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the preferred model variant
    pub fn variant(mut self, variant: OlmoModelVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// Add a required capability
    pub fn require_capability(mut self, capability: impl Into<String>) -> Self {
        self.required_capabilities.push(capability.into());
        self
    }

    /// Add a preferred language
    pub fn prefer_language(mut self, language: impl Into<String>) -> Self {
        self.preferred_languages.push(language.into());
        self
    }

    /// Set maximum processing time
    pub fn max_processing_time(mut self, duration: std::time::Duration) -> Self {
        self.max_processing_time = Some(duration);
        self
    }

    /// Set minimum confidence threshold
    pub fn min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence;
        self
    }

    /// Select the best model from available models
    pub fn select_model<'a>(&self, available_models: &'a [OlmoModel]) -> Option<&'a OlmoModel> {
        let mut candidates: Vec<_> = available_models
            .iter()
            .filter(|model| {
                // Check availability
                if !model.is_available {
                    return false;
                }

                // Check required capabilities
                for capability in &self.required_capabilities {
                    if !model.supports_capability(capability) {
                        return false;
                    }
                }

                // Check preferred languages
                if !self.preferred_languages.is_empty() {
                    let has_preferred_lang = self
                        .preferred_languages
                        .iter()
                        .any(|lang| model.supports_language(lang));
                    if !has_preferred_lang {
                        return false;
                    }
                }

                true
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Sort by preference (variant match, then capabilities, then languages)
        candidates.sort_by(|a, b| {
            // Prefer exact variant match
            let a_variant_match = self
                .variant
                .as_ref()
                .map(|v| a.id.contains(&v.to_string()))
                .unwrap_or(false);
            let b_variant_match = self
                .variant
                .as_ref()
                .map(|v| b.id.contains(&v.to_string()))
                .unwrap_or(false);

            match (a_variant_match, b_variant_match) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }

            // Prefer more capabilities
            let a_caps = self
                .required_capabilities
                .iter()
                .filter(|cap| a.supports_capability(cap))
                .count();
            let b_caps = self
                .required_capabilities
                .iter()
                .filter(|cap| b.supports_capability(cap))
                .count();

            b_caps.cmp(&a_caps)
        });

        candidates.into_iter().next()
    }
}

/// Common OLMo v2 models for OCR tasks
pub mod presets {
    use super::*;

    /// Create the base OLMo v2 model configuration
    pub fn base_model() -> OlmoModel {
        OlmoModel {
            id: "olmo-2-base".to_string(),
            name: "OLMo v2 Base".to_string(),
            version: "2.0.0".to_string(),
            capabilities: ModelCapabilities {
                text_extraction: true,
                layout_analysis: true,
                table_recognition: false,
                handwriting_recognition: false,
                formula_recognition: false,
                multilingual: true,
                structured_extraction: true,
                max_resolution: (2048, 2048),
                supported_formats: vec![
                    ImageFormat::Jpeg,
                    ImageFormat::Png,
                    ImageFormat::WebP,
                    ImageFormat::Pdf,
                ],
            },
            supported_languages: vec![
                SupportedLanguage {
                    code: "en".to_string(),
                    name: "English".to_string(),
                    native_name: "English".to_string(),
                    confidence: 0.95,
                },
                SupportedLanguage {
                    code: "es".to_string(),
                    name: "Spanish".to_string(),
                    native_name: "Español".to_string(),
                    confidence: 0.90,
                },
                SupportedLanguage {
                    code: "fr".to_string(),
                    name: "French".to_string(),
                    native_name: "Français".to_string(),
                    confidence: 0.90,
                },
            ],
            max_input_size: 5 * 1024 * 1024, // 5 MB
            is_available: true,
            config: ModelConfig::default(),
        }
    }

    /// Create the large OLMo v2 model configuration
    pub fn large_model() -> OlmoModel {
        let mut model = base_model();
        model.id = "olmo-2-large".to_string();
        model.name = "OLMo v2 Large".to_string();
        model.capabilities.max_resolution = (4096, 4096);
        model.capabilities.table_recognition = true;
        model.capabilities.handwriting_recognition = true;
        model.max_input_size = 20 * 1024 * 1024; // 20 MB
        model
    }

    /// Create the document-specialized OLMo v2 model configuration
    pub fn document_model() -> OlmoModel {
        let mut model = large_model();
        model.id = "olmo-2-document".to_string();
        model.name = "OLMo v2 Document".to_string();
        model.capabilities.layout_analysis = true;
        model.capabilities.table_recognition = true;
        model.capabilities.structured_extraction = true;
        model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_detection() {
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(
            ImageFormat::from_mime_type("image/jpeg"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_mime_type("application/pdf"),
            Some(ImageFormat::Pdf)
        );
    }

    #[test]
    fn test_model_capabilities() {
        let model = presets::base_model();
        assert!(model.supports_capability("text_extraction"));
        assert!(model.supports_language("en"));
        assert!(model.supports_format(&ImageFormat::Png));
    }

    #[test]
    fn test_model_selector() {
        let models = vec![presets::base_model(), presets::large_model()];
        let selector = ModelSelector::new()
            .variant(OlmoModelVariant::Large)
            .require_capability("text_extraction");

        let selected = selector.select_model(&models);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().id, "olmo-2-large");
    }
}
