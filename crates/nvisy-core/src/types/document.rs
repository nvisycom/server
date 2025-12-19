//! Document types and utilities.
//!
//! This module provides the core [`Document`] type used throughout the nvisy ecosystem
//! for representing various types of content in a uniform way. Documents are backed by
//! efficient byte storage and include comprehensive metadata support.

use std::collections::HashMap;
use std::fmt;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Result, TypeError, content_types};

/// A uniform document representation backed by efficient byte storage.
///
/// The [`Document`] type provides a standardized way to handle various types of content
/// throughout the nvisy ecosystem. It uses [`Bytes`] for efficient memory management
/// and includes rich metadata support for content classification and processing hints.
///
/// # Examples
///
/// Creating a simple text document:
///
/// ```rust,ignore
/// use nvisy_core::types::Document;
/// use bytes::Bytes;
///
/// let content = "Hello, world!";
/// let doc = Document::new(Bytes::from(content))
///     .with_content_type("text/plain")
///     .with_metadata("author", "nvisy");
/// ```
///
/// Creating a document from binary data:
///
/// ```rust,ignore
/// let binary_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
/// let doc = Document::new(Bytes::from(binary_data))
///     .with_content_type("image/png")
///     .with_filename("example.png");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier for this document.
    pub id: Uuid,

    /// The document content as bytes.
    pub content: Bytes,

    /// Document metadata including content type, filename, etc.
    pub metadata: DocumentMetadata,

    /// Timestamp when the document was created.
    pub created_at: jiff::Timestamp,

    /// Timestamp when the document was last modified.
    pub updated_at: jiff::Timestamp,
}

/// Metadata associated with a document.
///
/// This struct contains various metadata fields commonly used for document processing,
/// including content type information, filename, encoding, and custom attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// MIME type of the document content.
    pub content_type: Option<String>,

    /// Character encoding of the content (for text documents).
    pub encoding: Option<String>,

    /// Original filename of the document.
    pub filename: Option<String>,

    /// File extension derived from filename or content type.
    pub extension: Option<String>,

    /// Size of the document content in bytes.
    pub size: usize,

    /// Language of the content (ISO 639-1 code).
    pub language: Option<String>,

    /// Additional custom metadata as key-value pairs.
    pub attributes: HashMap<String, String>,

    /// Processing hints for downstream services.
    pub processing_hints: HashMap<String, serde_json::Value>,
}

impl Document {
    /// Creates a new document with the given content.
    ///
    /// The document is assigned a random UUID and initialized with basic metadata
    /// derived from the content size.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use bytes::Bytes;
    /// use nvisy_core::types::Document;
    ///
    /// let content = Bytes::from("Hello, world!");
    /// let doc = Document::new(content);
    /// assert_eq!(doc.size(), 13);
    /// ```
    pub fn new(content: Bytes) -> Self {
        let now = jiff::Timestamp::now();
        let size = content.len();

        Self {
            id: Uuid::new_v4(),
            content,
            metadata: DocumentMetadata {
                content_type: None,
                encoding: None,
                filename: None,
                extension: None,
                size,
                language: None,
                attributes: HashMap::new(),
                processing_hints: HashMap::new(),
            },
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a new document builder.
    pub fn builder() -> DocumentBuilder {
        DocumentBuilder::new()
    }

    /// Returns the size of the document content in bytes.
    pub fn size(&self) -> usize {
        self.content.len()
    }

    /// Returns true if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the content type of the document.
    pub fn content_type(&self) -> Option<&str> {
        self.metadata.content_type.as_deref()
    }

    /// Returns the filename of the document.
    pub fn filename(&self) -> Option<&str> {
        self.metadata.filename.as_deref()
    }

    /// Returns the file extension of the document.
    pub fn extension(&self) -> Option<&str> {
        self.metadata.extension.as_deref()
    }

    /// Returns true if this appears to be a text document.
    pub fn is_text(&self) -> bool {
        self.metadata
            .content_type
            .as_ref()
            .map(|ct| ct.starts_with("text/"))
            .unwrap_or(false)
    }

    /// Returns true if this appears to be an image document.
    pub fn is_image(&self) -> bool {
        self.metadata
            .content_type
            .as_ref()
            .map(|ct| ct.starts_with("image/"))
            .unwrap_or(false)
    }

    /// Returns true if this appears to be a PDF document.
    pub fn is_pdf(&self) -> bool {
        self.metadata
            .content_type
            .as_ref()
            .map(|ct| ct == content_types::APPLICATION_PDF)
            .unwrap_or(false)
    }

    /// Attempts to decode the content as UTF-8 text.
    ///
    /// Returns `None` if the content is not valid UTF-8.
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.content).ok()
    }

    /// Returns the content as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.content
    }

    /// Returns a clone of the underlying bytes.
    pub fn to_bytes(&self) -> Bytes {
        self.content.clone()
    }

    /// Gets a metadata attribute by key.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.metadata.attributes.get(key).map(|s| s.as_str())
    }

    /// Gets a processing hint by key.
    pub fn get_processing_hint(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.processing_hints.get(key)
    }

    /// Sets the content type of the document.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        let content_type = content_type.into();

        // Try to derive extension from content type if not already set
        if self.metadata.extension.is_none() {
            self.metadata.extension = derive_extension_from_content_type(&content_type);
        }

        self.metadata.content_type = Some(content_type);
        self.metadata.size = self.content.len();
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Sets the filename of the document.
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        let filename = filename.into();

        // Try to derive extension from filename
        if let Some(ext) = extract_extension(&filename) {
            self.metadata.extension = Some(ext.to_string());

            // Try to derive content type from extension if not already set
            if self.metadata.content_type.is_none() {
                self.metadata.content_type = derive_content_type_from_extension(ext);
            }
        }

        self.metadata.filename = Some(filename);
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Sets the encoding of the document.
    pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.metadata.encoding = Some(encoding.into());
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Sets the language of the document.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.metadata.language = Some(language.into());
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Adds a metadata attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.attributes.insert(key.into(), value.into());
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Adds a processing hint.
    pub fn with_processing_hint(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.metadata.processing_hints.insert(key.into(), value);
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Updates the document content.
    pub fn with_content(mut self, content: Bytes) -> Self {
        self.metadata.size = content.len();
        self.content = content;
        self.updated_at = jiff::Timestamp::now();
        self
    }

    /// Validates the document structure and metadata.
    pub fn validate(&self) -> Result<()> {
        // Validate content type format if present
        if let Some(ref content_type) = self.metadata.content_type {
            if !content_type.contains('/') {
                return Err(TypeError::InvalidContentType(content_type.clone()));
            }
        }

        // Validate size consistency
        if self.metadata.size != self.content.len() {
            return Err(TypeError::ValidationFailed(
                "Metadata size does not match actual content size".to_string(),
            ));
        }

        // Validate timestamps
        if self.updated_at < self.created_at {
            return Err(TypeError::ValidationFailed(
                "Updated timestamp cannot be before created timestamp".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            content_type: None,
            encoding: None,
            filename: None,
            extension: None,
            size: 0,
            language: None,
            attributes: HashMap::new(),
            processing_hints: HashMap::new(),
        }
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Document(id={}, size={}", self.id, self.size())?;

        if let Some(ref content_type) = self.metadata.content_type {
            write!(f, ", type={}", content_type)?;
        }

        if let Some(ref filename) = self.metadata.filename {
            write!(f, ", filename={}", filename)?;
        }

        write!(f, ")")
    }
}

/// Builder for creating documents with fluent interface.
#[derive(Debug, Clone)]
pub struct DocumentBuilder {
    id: Option<Uuid>,
    content: Option<Bytes>,
    content_type: Option<String>,
    encoding: Option<String>,
    filename: Option<String>,
    extension: Option<String>,
    language: Option<String>,
    attributes: HashMap<String, String>,
    processing_hints: HashMap<String, serde_json::Value>,
}

impl DocumentBuilder {
    /// Creates a new document builder.
    pub fn new() -> Self {
        Self {
            id: None,
            content: None,
            content_type: None,
            encoding: None,
            filename: None,
            extension: None,
            language: None,
            attributes: HashMap::new(),
            processing_hints: HashMap::new(),
        }
    }

    /// Sets the document ID.
    pub fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the document content.
    pub fn content(mut self, content: Bytes) -> Self {
        self.content = Some(content);
        self
    }

    /// Sets the document content from a string.
    pub fn text_content(mut self, text: impl Into<String>) -> Self {
        self.content = Some(Bytes::from(text.into()));
        if self.content_type.is_none() {
            self.content_type = Some(content_types::TEXT_PLAIN.to_string());
        }
        self
    }

    /// Sets the document content from bytes.
    pub fn binary_content(mut self, data: Vec<u8>) -> Self {
        self.content = Some(Bytes::from(data));
        self
    }

    /// Sets the content type.
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Sets the encoding.
    pub fn encoding(mut self, encoding: impl Into<String>) -> Self {
        self.encoding = Some(encoding.into());
        self
    }

    /// Sets the filename.
    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Sets the file extension.
    pub fn extension(mut self, extension: impl Into<String>) -> Self {
        self.extension = Some(extension.into());
        self
    }

    /// Sets the language.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Adds an attribute.
    pub fn attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Adds a processing hint.
    pub fn processing_hint(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.processing_hints.insert(key.into(), value);
        self
    }

    /// Builds the document.
    pub fn build(self) -> Result<Document> {
        let content = self.content.ok_or_else(|| {
            TypeError::ValidationFailed("Document content is required".to_string())
        })?;

        let size = content.len();
        let now = jiff::Timestamp::now();

        // Auto-derive extension from filename if not provided
        let extension = self.extension.or_else(|| {
            self.filename
                .as_ref()
                .and_then(|f| extract_extension(f).map(|s| s.to_string()))
        });

        // Auto-derive content type from extension if not provided
        let content_type = self.content_type.or_else(|| {
            extension
                .as_ref()
                .and_then(|ext| derive_content_type_from_extension(ext))
        });

        let document = Document {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            content,
            metadata: DocumentMetadata {
                content_type,
                encoding: self.encoding,
                filename: self.filename,
                extension,
                size,
                language: self.language,
                attributes: self.attributes,
                processing_hints: self.processing_hints,
            },
            created_at: now,
            updated_at: now,
        };

        document.validate()?;
        Ok(document)
    }
}

impl Default for DocumentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts the file extension from a filename.
fn extract_extension(filename: &str) -> Option<&str> {
    filename.rfind('.').map(|pos| &filename[pos + 1..])
}

/// Derives a file extension from a content type.
fn derive_extension_from_content_type(content_type: &str) -> Option<String> {
    match content_type {
        content_types::TEXT_PLAIN => Some("txt".to_string()),
        content_types::TEXT_HTML => Some("html".to_string()),
        content_types::TEXT_MARKDOWN => Some("md".to_string()),
        content_types::APPLICATION_JSON => Some("json".to_string()),
        content_types::APPLICATION_PDF => Some("pdf".to_string()),
        content_types::IMAGE_JPEG => Some("jpg".to_string()),
        content_types::IMAGE_PNG => Some("png".to_string()),
        content_types::IMAGE_WEBP => Some("webp".to_string()),
        _ => None,
    }
}

/// Derives a content type from a file extension.
fn derive_content_type_from_extension(extension: &str) -> Option<String> {
    match extension.to_lowercase().as_str() {
        "txt" => Some(content_types::TEXT_PLAIN.to_string()),
        "html" | "htm" => Some(content_types::TEXT_HTML.to_string()),
        "md" | "markdown" => Some(content_types::TEXT_MARKDOWN.to_string()),
        "json" => Some(content_types::APPLICATION_JSON.to_string()),
        "pdf" => Some(content_types::APPLICATION_PDF.to_string()),
        "jpg" | "jpeg" => Some(content_types::IMAGE_JPEG.to_string()),
        "png" => Some(content_types::IMAGE_PNG.to_string()),
        "webp" => Some(content_types::IMAGE_WEBP.to_string()),
        _ => Some(content_types::APPLICATION_OCTET_STREAM.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_creation() {
        let content = Bytes::from("Hello, world!");
        let doc = Document::new(content.clone());

        assert_eq!(doc.content, content);
        assert_eq!(doc.size(), 13);
        assert!(!doc.is_empty());
    }

    #[test]
    fn test_document_with_metadata() {
        let doc = Document::new(Bytes::from("Hello, world!"))
            .with_content_type("text/plain")
            .with_filename("hello.txt")
            .with_attribute("author", "nvisy");

        assert_eq!(doc.content_type(), Some("text/plain"));
        assert_eq!(doc.filename(), Some("hello.txt"));
        assert_eq!(doc.extension(), Some("txt"));
        assert_eq!(doc.get_attribute("author"), Some("nvisy"));
        assert!(doc.is_text());
    }

    #[test]
    fn test_document_builder() {
        let doc = Document::builder()
            .text_content("Hello, world!")
            .filename("hello.txt")
            .attribute("author", "nvisy")
            .build()
            .unwrap();

        assert_eq!(doc.as_text(), Some("Hello, world!"));
        assert_eq!(doc.filename(), Some("hello.txt"));
        assert_eq!(doc.content_type(), Some("text/plain"));
        assert_eq!(doc.extension(), Some("txt"));
    }

    #[test]
    fn test_extension_derivation() {
        assert_eq!(extract_extension("test.txt"), Some("txt"));
        assert_eq!(extract_extension("test.tar.gz"), Some("gz"));
        assert_eq!(extract_extension("no_extension"), None);
    }

    #[test]
    fn test_content_type_derivation() {
        assert_eq!(
            derive_content_type_from_extension("txt"),
            Some("text/plain".to_string())
        );
        assert_eq!(
            derive_content_type_from_extension("json"),
            Some("application/json".to_string())
        );
    }
}
