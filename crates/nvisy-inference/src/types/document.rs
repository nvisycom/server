//! Document types and utilities.
//!
//! This module provides the core [`Document`] type used throughout the nvisy ecosystem
//! for representing various types of content in a uniform way. Documents are backed by
//! efficient byte storage and include comprehensive metadata support.

use std::collections::HashMap;
use std::fmt;

use bytes::Bytes;
use derive_more::{From, Into};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Annotation;
use crate::{Error, Result};

/// Unique identifier for documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, From, Into)]
pub struct DocumentId(
    #[from]
    #[into]
    pub Uuid,
);

impl DocumentId {
    /// Creates a new random document ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

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
/// ```rust
/// use nvisy_inference::types::Document;
/// use bytes::Bytes;
///
/// let content = "Hello, world!";
/// let doc = Document::new(Bytes::from(content))
///     .with_content_type("text/plain")
///     .with_attribute("author", "nvisy");
/// ```
///
/// Creating a document from binary data:
///
/// ```rust
/// use nvisy_inference::types::Document;
/// use bytes::Bytes;
///
/// let binary_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
/// let doc = Document::new(Bytes::from(binary_data))
///     .with_content_type("image/png")
///     .with_filename("example.png");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier for this document.
    pub id: DocumentId,

    /// The document content as bytes.
    pub content: Bytes,

    /// Rich metadata about the document.
    pub metadata: DocumentMetadata,

    /// Optional annotations associated with this document.
    pub annotations: Option<Vec<Annotation>>,

    /// When the document was created.
    pub created_at: Timestamp,

    /// When the document was last updated.
    pub updated_at: Timestamp,
}

/// Metadata associated with a document.
///
/// This struct contains various metadata fields commonly used for document processing,
/// including content type information, filename, encoding, and custom attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// ```rust
    /// use bytes::Bytes;
    /// use nvisy_inference::types::Document;
    ///
    /// let content = Bytes::from("Hello, world!");
    /// let doc = Document::new(content);
    /// assert_eq!(doc.size(), 13);
    /// ```
    /// Creates a new document with the given content.
    pub fn new(content: Bytes) -> Self {
        let now = Timestamp::now();
        let size = content.len();
        Self {
            id: DocumentId::new(),
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
            annotations: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Sets the document ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = DocumentId(id);
        self
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.metadata.content_type = Some(content_type.into());
        self
    }

    /// Sets the encoding.
    pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.metadata.encoding = Some(encoding.into());
        self
    }

    /// Sets the filename.
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        let filename = filename.into();

        // Extract extension from filename
        if let Some(ext) = extract_extension(&filename) {
            self.metadata.extension = Some(ext.to_string());
        }

        self.metadata.filename = Some(filename);
        self
    }

    /// Sets the file extension.
    pub fn with_extension(mut self, extension: impl Into<String>) -> Self {
        self.metadata.extension = Some(extension.into());
        self
    }

    /// Sets the language.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.metadata.language = Some(language.into());
        self
    }

    /// Adds an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.attributes.insert(key.into(), value.into());
        self
    }

    /// Adds multiple attributes.
    pub fn with_attributes(mut self, attributes: HashMap<String, String>) -> Self {
        self.metadata.attributes.extend(attributes);
        self
    }

    /// Adds a processing hint.
    pub fn with_processing_hint(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.metadata.processing_hints.insert(key.into(), value);
        self
    }

    /// Sets the annotations.
    pub fn with_annotations(mut self, annotations: Vec<Annotation>) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Adds an annotation.
    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        match self.annotations {
            Some(mut annotations) => {
                annotations.push(annotation);
                self.annotations = Some(annotations);
            }
            None => {
                self.annotations = Some(vec![annotation]);
            }
        }
        self
    }

    /// Returns the size of the document content in bytes.
    pub fn size(&self) -> usize {
        self.content.len()
    }

    /// Returns true if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Estimates the size of the document metadata in bytes.
    pub fn estimated_metadata_size(&self) -> usize {
        self.metadata.content_type.as_ref().map(|s| s.len()).unwrap_or(0)
            + self.metadata.filename.as_ref().map(|s| s.len()).unwrap_or(0)
            + self.metadata.extension.as_ref().map(|s| s.len()).unwrap_or(0)
            + self.metadata.language.as_ref().map(|s| s.len()).unwrap_or(0)
            + self.metadata.attributes.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>()
            + self.metadata.processing_hints.len() * 50 // Rough estimate for JSON values
            + 100 // Base overhead for structure and timestamps
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

    /// Returns a reference to the document data.
    pub fn data(&self) -> &Bytes {
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
        if let Some(ref content_type) = self.metadata.content_type
            && !content_type.contains('/')
        {
            return Err(Error::invalid_input()
                .with_message(format!("Invalid content type format: {}", content_type)));
        }

        // Validate size consistency
        if self.metadata.size != self.content.len() {
            return Err(Error::invalid_input()
                .with_message("Metadata size does not match actual content size"));
        }

        // Validate timestamps
        if self.updated_at < self.created_at {
            return Err(Error::invalid_input()
                .with_message("Updated timestamp cannot be before created timestamp"));
        }

        Ok(())
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

/// Extracts the file extension from a filename.
fn extract_extension(filename: &str) -> Option<&str> {
    filename.rfind('.').map(|pos| &filename[pos + 1..])
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
        let doc = Document::new(Bytes::from("Hello, world!"))
            .with_content_type("text/plain")
            .with_filename("hello.txt")
            .with_attribute("author", "nvisy");

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
}
