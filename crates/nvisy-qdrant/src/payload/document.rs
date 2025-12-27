//! Document payload types and utilities.
//!
//! This module provides payload structures and utilities for document collections,
//! including point definitions, document types, status tracking, and document-specific metadata.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SearchResult;
use crate::error::{Error, Result};
use crate::types::{Payload, Point, PointId, Vector};

/// Create a payload with standard metadata fields
fn create_metadata_payload() -> Payload {
    let now = jiff::Timestamp::now().to_string();
    Payload::new()
        .with("created_at", now.clone())
        .with("updated_at", now)
        .with("version", 1)
}

/// Types of documents supported by the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum DocumentType {
    /// Plain text document
    Text,
    /// Markdown document
    Markdown,
    /// PDF document
    Pdf,
    /// Microsoft Word document
    Word,
    /// HTML document
    Html,
    /// Source code file
    Code(String), // programming language
    /// JSON data file
    Json,
    /// XML document
    Xml,
    /// CSV data file
    Csv,
    /// Research paper
    ResearchPaper,
    /// Technical documentation
    TechnicalDoc,
    /// Legal document
    Legal,
    /// Contract document
    Contract,
    /// Custom document type
    Custom(String),
}

impl DocumentType {
    /// Get the string representation of the document type
    pub fn as_str(&self) -> &str {
        match self {
            DocumentType::Text => "text",
            DocumentType::Markdown => "markdown",
            DocumentType::Pdf => "pdf",
            DocumentType::Word => "word",
            DocumentType::Html => "html",
            DocumentType::Code(_) => "code",
            DocumentType::Json => "json",
            DocumentType::Xml => "xml",
            DocumentType::Csv => "csv",
            DocumentType::ResearchPaper => "research_paper",
            DocumentType::TechnicalDoc => "technical_doc",
            DocumentType::Legal => "legal",
            DocumentType::Contract => "contract",
            DocumentType::Custom(name) => name,
        }
    }

    /// Get the MIME type for this document type
    pub fn mime_type(&self) -> Option<&'static str> {
        match self {
            DocumentType::Text => Some("text/plain"),
            DocumentType::Markdown => Some("text/markdown"),
            DocumentType::Pdf => Some("application/pdf"),
            DocumentType::Word => {
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
            }
            DocumentType::Html => Some("text/html"),
            DocumentType::Code(_) => Some("text/plain"),
            DocumentType::Json => Some("application/json"),
            DocumentType::Xml => Some("application/xml"),
            DocumentType::Csv => Some("text/csv"),
            DocumentType::ResearchPaper => Some("application/pdf"),
            DocumentType::TechnicalDoc => Some("text/plain"),
            DocumentType::Legal => Some("application/pdf"),
            DocumentType::Contract => Some("application/pdf"),
            DocumentType::Custom(_) => None,
        }
    }

    /// Get the programming language for code documents
    pub fn programming_language(&self) -> Option<&str> {
        match self {
            DocumentType::Code(lang) => Some(lang),
            _ => None,
        }
    }
}

impl std::fmt::Display for DocumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentType::Code(lang) => write!(f, "code:{}", lang),
            _ => write!(f, "{}", self.as_str()),
        }
    }
}

/// Document processing and publication status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum DocumentStatus {
    /// Draft document, not yet finalized
    Draft,
    /// Under review
    Review,
    /// Published and available
    Published,
    /// Archived document
    Archived,
    /// Deleted document
    Deleted,
    /// Processing document (e.g., being indexed)
    Processing,
    /// Failed to process
    Failed,
    /// Pending approval
    Pending,
}

impl DocumentStatus {
    /// Get the string representation of the document status
    pub fn as_str(&self) -> &str {
        match self {
            DocumentStatus::Draft => "draft",
            DocumentStatus::Review => "review",
            DocumentStatus::Published => "published",
            DocumentStatus::Archived => "archived",
            DocumentStatus::Deleted => "deleted",
            DocumentStatus::Processing => "processing",
            DocumentStatus::Failed => "failed",
            DocumentStatus::Pending => "pending",
        }
    }

    /// Check if the document is publicly accessible
    pub fn is_public(&self) -> bool {
        matches!(self, DocumentStatus::Published)
    }

    /// Check if the document is in a modifiable state
    pub fn is_modifiable(&self) -> bool {
        matches!(
            self,
            DocumentStatus::Draft | DocumentStatus::Review | DocumentStatus::Pending
        )
    }
}

impl std::fmt::Display for DocumentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A point representing a document in the vector database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct DocumentPoint {
    /// Unique identifier for the document
    pub id: PointId,

    /// Vector embedding of the document content
    pub embedding: Vector,

    /// Type of document
    pub document_type: DocumentType,

    /// Document status
    pub status: DocumentStatus,

    /// Document title
    pub title: String,

    /// Document content (full text or summary)
    pub content: String,

    /// Author/creator of the document
    pub author_id: String,

    /// Document tags for categorization
    pub tags: Vec<String>,

    /// Document language (ISO 639-1 code)
    pub language: Option<String>,

    /// Original filename
    pub filename: Option<String>,

    /// File size in bytes
    pub file_size: Option<u64>,

    /// MIME type of the document
    pub mime_type: Option<String>,

    /// URL or path to the original document
    pub source_url: Option<String>,

    /// Parent document ID (for chunks or sub-documents)
    pub parent_document_id: Option<String>,

    /// Chunk number (for document chunks)
    pub chunk_number: Option<u32>,

    /// Total number of chunks in the parent document
    pub total_chunks: Option<u32>,

    /// Document version number
    pub version: Option<String>,

    /// Previous version ID
    pub previous_version_id: Option<String>,

    /// Additional document metadata
    pub metadata: Payload,
}

impl DocumentPoint {
    /// Create a new document point
    pub fn new(
        id: impl Into<PointId>,
        embedding: Vector,
        document_type: DocumentType,
        title: String,
        content: String,
        author_id: String,
    ) -> Self {
        Self {
            id: id.into(),
            embedding,
            document_type: document_type.clone(),
            status: DocumentStatus::Draft,
            title,
            content,
            author_id,
            tags: Vec::new(),
            language: None,
            filename: None,
            file_size: None,
            mime_type: document_type.mime_type().map(String::from),
            source_url: None,
            parent_document_id: None,
            chunk_number: None,
            total_chunks: None,
            version: None,
            previous_version_id: None,
            metadata: create_metadata_payload(),
        }
    }

    /// Create a text document
    pub fn text_document(
        id: impl Into<PointId>,
        embedding: Vector,
        title: String,
        content: String,
        author_id: String,
    ) -> Self {
        Self::new(id, embedding, DocumentType::Text, title, content, author_id)
    }

    /// Create a code document with programming language
    pub fn code_document(
        id: impl Into<PointId>,
        embedding: Vector,
        title: String,
        content: String,
        author_id: String,
        language: String,
    ) -> Self {
        Self::new(
            id,
            embedding,
            DocumentType::Code(language),
            title,
            content,
            author_id,
        )
    }

    /// Create a PDF document
    pub fn pdf_document(
        id: impl Into<PointId>,
        embedding: Vector,
        title: String,
        content: String,
        author_id: String,
    ) -> Self {
        Self::new(id, embedding, DocumentType::Pdf, title, content, author_id)
    }

    /// Create a document chunk
    #[allow(clippy::too_many_arguments)]
    pub fn document_chunk(
        id: impl Into<PointId>,
        embedding: Vector,
        document_type: DocumentType,
        title: String,
        content: String,
        author_id: String,
        parent_document_id: String,
        chunk_number: u32,
        total_chunks: u32,
    ) -> Self {
        let mut doc = Self::new(id, embedding, document_type, title, content, author_id);
        doc.parent_document_id = Some(parent_document_id);
        doc.chunk_number = Some(chunk_number);
        doc.total_chunks = Some(total_chunks);
        doc
    }

    /// Set the document status
    pub fn with_status(mut self, status: DocumentStatus) -> Self {
        self.status = status;
        self
    }

    /// Add tags to the document
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add a single tag
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Set the document language
    pub fn with_language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    /// Set the filename
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }

    /// Set the file size
    pub fn with_file_size(mut self, size: u64) -> Self {
        self.file_size = Some(size);
        self
    }

    /// Set the source URL
    pub fn with_source_url(mut self, url: String) -> Self {
        self.source_url = Some(url);
        self
    }

    /// Set the document version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Set the previous version ID
    pub fn with_previous_version(mut self, previous_id: String) -> Self {
        self.previous_version_id = Some(previous_id);
        self
    }

    /// Add additional metadata
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if this is a document chunk
    pub fn is_chunk(&self) -> bool {
        self.parent_document_id.is_some()
    }

    /// Convert to a generic Point for storage
    pub fn to_point(self) -> Point {
        let mut payload = Payload::new()
            .with("document_type", self.document_type.to_string())
            .with("status", self.status.as_str())
            .with("title", self.title)
            .with("content", self.content)
            .with("author_id", self.author_id)
            .with("tags", self.tags);

        if let Some(language) = self.language {
            payload = payload.with("language", language);
        }

        if let Some(filename) = self.filename {
            payload = payload.with("filename", filename);
        }

        if let Some(file_size) = self.file_size {
            payload = payload.with("file_size", file_size as i64);
        }

        if let Some(mime_type) = self.mime_type {
            payload = payload.with("mime_type", mime_type);
        }

        if let Some(source_url) = self.source_url {
            payload = payload.with("source_url", source_url);
        }

        if let Some(parent_id) = self.parent_document_id {
            payload = payload.with("parent_document_id", parent_id);
        }

        if let Some(chunk_number) = self.chunk_number {
            payload = payload.with("chunk_number", chunk_number as i64);
        }

        if let Some(total_chunks) = self.total_chunks {
            payload = payload.with("total_chunks", total_chunks as i64);
        }

        if let Some(version) = self.version {
            payload = payload.with("version", version);
        }

        if let Some(previous_version_id) = self.previous_version_id {
            payload = payload.with("previous_version_id", previous_version_id);
        }

        // Merge additional metadata
        payload.merge(&self.metadata);

        Point::new(self.id, self.embedding, payload)
    }

    /// Create from a search result
    pub fn from_search_result(result: SearchResult) -> Result<Self> {
        let id = result.id.clone();
        let embedding = result.vector().unwrap_or_default();
        let payload = result.payload;

        let document_type = match payload.get_string("document_type") {
            Some(type_str) => {
                if type_str.starts_with("code:") {
                    let language = type_str.strip_prefix("code:").unwrap_or("").to_string();
                    DocumentType::Code(language)
                } else {
                    match type_str {
                        "text" => DocumentType::Text,
                        "markdown" => DocumentType::Markdown,
                        "pdf" => DocumentType::Pdf,
                        "word" => DocumentType::Word,
                        "html" => DocumentType::Html,
                        "json" => DocumentType::Json,
                        "xml" => DocumentType::Xml,
                        "csv" => DocumentType::Csv,
                        "research_paper" => DocumentType::ResearchPaper,
                        "technical_doc" => DocumentType::TechnicalDoc,
                        "legal" => DocumentType::Legal,
                        "contract" => DocumentType::Contract,
                        custom => DocumentType::Custom(custom.to_string()),
                    }
                }
            }
            None => {
                return Err(Error::invalid_input().with_message("Missing document_type"));
            }
        };

        let status = match payload.get_string("status") {
            Some(status_str) => match status_str {
                "draft" => DocumentStatus::Draft,
                "review" => DocumentStatus::Review,
                "published" => DocumentStatus::Published,
                "archived" => DocumentStatus::Archived,
                "deleted" => DocumentStatus::Deleted,
                "processing" => DocumentStatus::Processing,
                "failed" => DocumentStatus::Failed,
                "pending" => DocumentStatus::Pending,
                _ => DocumentStatus::Draft,
            },
            None => DocumentStatus::Draft,
        };

        let title = payload
            .get_string("title")
            .ok_or_else(|| Error::invalid_input().with_message("Missing title"))?
            .to_string();

        let content = payload
            .get_string("content")
            .ok_or_else(|| Error::invalid_input().with_message("Missing content"))?
            .to_string();

        let author_id = payload
            .get_string("author_id")
            .ok_or_else(|| Error::invalid_input().with_message("Missing author_id"))?
            .to_string();

        let tags = payload
            .get_array("tags")
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let language = payload.get_string("language").map(|s| s.to_string());
        let filename = payload.get_string("filename").map(|s| s.to_string());
        let file_size = payload.get_i64("file_size").map(|s| s as u64);
        let mime_type = payload.get_string("mime_type").map(|s| s.to_string());
        let source_url = payload.get_string("source_url").map(|s| s.to_string());
        let parent_document_id = payload
            .get_string("parent_document_id")
            .map(|s| s.to_string());
        let chunk_number = payload.get_i64("chunk_number").map(|n| n as u32);
        let total_chunks = payload.get_i64("total_chunks").map(|n| n as u32);
        let version = payload.get_string("version").map(|s| s.to_string());
        let previous_version_id = payload
            .get_string("previous_version_id")
            .map(|s| s.to_string());

        Ok(Self {
            id,
            embedding,
            document_type,
            status,
            title,
            content,
            author_id,
            tags,
            language,
            filename,
            file_size,
            mime_type,
            source_url,
            parent_document_id,
            chunk_number,
            total_chunks,
            version,
            previous_version_id,
            metadata: payload,
        })
    }
}

impl From<DocumentPoint> for Point {
    fn from(document: DocumentPoint) -> Self {
        document.to_point()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_type_conversion() {
        assert_eq!(DocumentType::Text.as_str(), "text");
        assert_eq!(DocumentType::Pdf.as_str(), "pdf");
        assert_eq!(DocumentType::Code("rust".to_string()).as_str(), "code");
        assert_eq!(DocumentType::Custom("test".to_string()).as_str(), "test");
    }

    #[test]
    fn test_document_type_mime_type() {
        assert_eq!(DocumentType::Text.mime_type(), Some("text/plain"));
        assert_eq!(DocumentType::Pdf.mime_type(), Some("application/pdf"));
        assert_eq!(DocumentType::Json.mime_type(), Some("application/json"));
    }

    #[test]
    fn test_document_status_conversion() {
        assert_eq!(DocumentStatus::Draft.as_str(), "draft");
        assert_eq!(DocumentStatus::Published.as_str(), "published");
        assert!(DocumentStatus::Published.is_public());
        assert!(DocumentStatus::Draft.is_modifiable());
        assert!(!DocumentStatus::Published.is_modifiable());
    }

    #[test]
    fn test_document_point_creation() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let point = DocumentPoint::text_document(
            "doc-123",
            vector,
            "Test Document".to_string(),
            "This is test content".to_string(),
            "author-456".to_string(),
        );

        assert_eq!(point.document_type, DocumentType::Text);
        assert_eq!(point.title, "Test Document");
        assert_eq!(point.content, "This is test content");
        assert_eq!(point.author_id, "author-456");
        assert_eq!(point.status, DocumentStatus::Draft);
    }

    #[test]
    fn test_document_chunk_creation() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let chunk = DocumentPoint::document_chunk(
            "chunk-123",
            vector,
            DocumentType::Pdf,
            "Document Chunk 1".to_string(),
            "First chunk content".to_string(),
            "author-456".to_string(),
            "parent-doc-789".to_string(),
            1,
            5,
        );

        assert!(chunk.is_chunk());
        assert_eq!(chunk.parent_document_id, Some("parent-doc-789".to_string()));
        assert_eq!(chunk.chunk_number, Some(1));
        assert_eq!(chunk.total_chunks, Some(5));
    }

    #[test]
    fn test_document_point_with_metadata() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let point = DocumentPoint::code_document(
            "code-123",
            vector,
            "main.rs".to_string(),
            "fn main() { println!(\"Hello\"); }".to_string(),
            "dev-456".to_string(),
            "rust".to_string(),
        )
        .with_status(DocumentStatus::Published)
        .with_filename("main.rs".to_string())
        .with_language("en".to_string())
        .with_tags(vec!["rust".to_string(), "code".to_string()]);

        assert_eq!(point.status, DocumentStatus::Published);
        assert_eq!(point.filename, Some("main.rs".to_string()));
        assert_eq!(point.language, Some("en".to_string()));
        assert_eq!(point.tags, vec!["rust".to_string(), "code".to_string()]);

        if let DocumentType::Code(lang) = &point.document_type {
            assert_eq!(lang, "rust");
        } else {
            panic!("Expected Code document type");
        }
    }

    #[test]
    fn test_document_point_to_point_conversion() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let document_point = DocumentPoint::pdf_document(
            "doc-123",
            vector,
            "Research Paper".to_string(),
            "Abstract of the research...".to_string(),
            "researcher-456".to_string(),
        )
        .with_status(DocumentStatus::Published)
        .with_tags(vec!["research".to_string(), "ai".to_string()]);

        let point = document_point.to_point();

        assert_eq!(point.payload.get_string("document_type"), Some("pdf"));
        assert_eq!(point.payload.get_string("status"), Some("published"));
        assert_eq!(point.payload.get_string("title"), Some("Research Paper"));

        let tags = point.payload.get_array("tags").unwrap();
        assert_eq!(tags.len(), 2);
    }
}
