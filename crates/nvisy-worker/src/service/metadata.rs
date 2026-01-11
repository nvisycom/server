//! Document metadata extraction and validation service.
//!
//! Provides functionality to extract, validate, and fix document metadata
//! such as page count, dimensions, content type, and other properties.

use serde::{Deserialize, Serialize};

/// Extracted document metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// MIME content type.
    pub content_type: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Number of pages (for multi-page documents).
    pub page_count: Option<u32>,
    /// Document width in pixels (for images/PDFs).
    pub width: Option<u32>,
    /// Document height in pixels (for images/PDFs).
    pub height: Option<u32>,
    /// Document title if available.
    pub title: Option<String>,
    /// Document author if available.
    pub author: Option<String>,
    /// Creation date as ISO 8601 string.
    pub created_at: Option<String>,
    /// Last modification date as ISO 8601 string.
    pub modified_at: Option<String>,
    /// Whether the document appears to be scanned (needs OCR).
    pub is_scanned: bool,
    /// Whether the document has text content.
    pub has_text: bool,
    /// Language code if detected.
    pub language: Option<String>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            content_type: String::new(),
            size_bytes: 0,
            page_count: None,
            width: None,
            height: None,
            title: None,
            author: None,
            created_at: None,
            modified_at: None,
            is_scanned: false,
            has_text: false,
            language: None,
        }
    }
}

/// Validation result for document metadata.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the metadata is valid.
    pub is_valid: bool,
    /// List of validation issues found.
    pub issues: Vec<ValidationIssue>,
}

/// A specific validation issue.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// The field with the issue.
    pub field: String,
    /// Description of the issue.
    pub message: String,
    /// Severity of the issue.
    pub severity: IssueSeverity,
}

/// Severity levels for validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    /// Informational, no action needed.
    Info,
    /// Warning, may cause problems.
    Warning,
    /// Error, must be fixed.
    Error,
}

/// Service for extracting and validating document metadata.
#[derive(Clone, Default)]
pub struct MetadataService {
    _private: (),
}

impl MetadataService {
    /// Creates a new metadata service.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Extracts metadata from document bytes.
    ///
    /// Detects the content type and extracts relevant metadata based on
    /// the document format.
    pub async fn extract(&self, data: &[u8]) -> crate::Result<DocumentMetadata> {
        let content_type = self.detect_content_type(data);
        let size_bytes = data.len() as u64;

        // TODO: Implement format-specific metadata extraction
        // - PDF: page count, dimensions, title, author, dates
        // - Images: dimensions, format-specific metadata (EXIF, etc.)
        // - Office documents: title, author, dates

        Ok(DocumentMetadata {
            content_type,
            size_bytes,
            ..Default::default()
        })
    }

    /// Validates document metadata for consistency and completeness.
    pub fn validate(&self, metadata: &DocumentMetadata) -> ValidationResult {
        let mut issues = Vec::new();

        // Check content type
        if metadata.content_type.is_empty() {
            issues.push(ValidationIssue {
                field: "content_type".to_string(),
                message: "Content type is missing".to_string(),
                severity: IssueSeverity::Error,
            });
        }

        // Check for zero size
        if metadata.size_bytes == 0 {
            issues.push(ValidationIssue {
                field: "size_bytes".to_string(),
                message: "Document has zero size".to_string(),
                severity: IssueSeverity::Error,
            });
        }

        // Check for scanned documents without OCR flag
        if metadata.is_scanned && !metadata.has_text {
            issues.push(ValidationIssue {
                field: "is_scanned".to_string(),
                message: "Scanned document may need OCR processing".to_string(),
                severity: IssueSeverity::Info,
            });
        }

        let is_valid = !issues.iter().any(|i| i.severity == IssueSeverity::Error);

        ValidationResult { is_valid, issues }
    }

    /// Detects the content type from file magic bytes.
    fn detect_content_type(&self, data: &[u8]) -> String {
        if data.len() < 8 {
            return "application/octet-stream".to_string();
        }

        // PDF
        if data.starts_with(b"%PDF") {
            return "application/pdf".to_string();
        }

        // PNG
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return "image/png".to_string();
        }

        // JPEG
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return "image/jpeg".to_string();
        }

        // GIF
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return "image/gif".to_string();
        }

        // WebP
        if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
            return "image/webp".to_string();
        }

        // TIFF (little-endian)
        if data.starts_with(&[0x49, 0x49, 0x2A, 0x00]) {
            return "image/tiff".to_string();
        }

        // TIFF (big-endian)
        if data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
            return "image/tiff".to_string();
        }

        // ZIP-based formats (DOCX, XLSX, PPTX, ODT, etc.)
        if data.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
            // Would need to inspect zip contents to determine exact type
            return "application/zip".to_string();
        }

        "application/octet-stream".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_pdf() {
        let service = MetadataService::new();
        let data = b"%PDF-1.4 some content";
        assert_eq!(service.detect_content_type(data), "application/pdf");
    }

    #[test]
    fn test_detect_png() {
        let service = MetadataService::new();
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert_eq!(service.detect_content_type(&data), "image/png");
    }

    #[test]
    fn test_detect_jpeg() {
        let service = MetadataService::new();
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        assert_eq!(service.detect_content_type(&data), "image/jpeg");
    }

    #[test]
    fn test_validate_empty_content_type() {
        let service = MetadataService::new();
        let metadata = DocumentMetadata::default();
        let result = service.validate(&metadata);
        assert!(!result.is_valid);
    }
}
