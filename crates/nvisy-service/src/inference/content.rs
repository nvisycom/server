//! Unified content representation for nvisy-core.
//!
//! This module provides a unified `Content` type that can represent different
//! kinds of data including text, documents, and chat conversations. This type
//! is used across embedding requests, messages, and other content-based operations.

use derive_more::From;
use serde::{Deserialize, Serialize};

use super::{Chat, Document};

/// Unified content type for representing different kinds of data.
///
/// This enum provides a standardized way to represent various content types
/// across the nvisy ecosystem, including plain text, structured documents,
/// and chat conversations.
///
/// # Examples
///
/// Creating text content:
/// ```rust
/// use nvisy_service::inference::Content;
///
/// let content = Content::text("Hello, world!");
/// assert!(content.is_text());
/// ```
///
/// Creating document content:
/// ```rust
/// use nvisy_service::inference::{Content, Document};
/// use bytes::Bytes;
///
/// let doc = Document::new(Bytes::from("PDF content"));
/// let content = Content::document(doc);
/// assert!(content.is_document());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, From)]
#[serde(tag = "type", content = "content")]
pub enum Content {
    /// Plain text content.
    #[serde(rename = "text")]
    #[from]
    Text(String),

    /// Document content with metadata and binary data.
    #[serde(rename = "document")]
    #[from]
    Document(Document),

    /// Chat conversation content.
    #[serde(rename = "chat")]
    #[from]
    Chat(Chat),
}

impl Content {
    /// Creates text content.
    ///
    /// # Parameters
    ///
    /// * `text` - The text content
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::Content;
    ///
    /// let content = Content::text("Hello, world!");
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Creates document content.
    ///
    /// # Parameters
    ///
    /// * `document` - The document
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Document};
    /// use bytes::Bytes;
    ///
    /// let doc = Document::new(Bytes::from("content"));
    /// let content = Content::document(doc);
    /// ```
    pub fn document(document: Document) -> Self {
        Self::Document(document)
    }

    /// Creates chat content.
    ///
    /// # Parameters
    ///
    /// * `chat` - The chat conversation
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Chat};
    ///
    /// let chat = Chat::new();
    /// let content = Content::chat(chat);
    /// ```
    pub fn chat(chat: Chat) -> Self {
        Self::Chat(chat)
    }

    /// Returns `true` if this content is text.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::Content;
    ///
    /// let content = Content::text("hello");
    /// assert!(content.is_text());
    /// ```
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Returns `true` if this content is a document.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Document};
    /// use bytes::Bytes;
    ///
    /// let doc = Document::new(Bytes::from("content"));
    /// let content = Content::document(doc);
    /// assert!(content.is_document());
    /// ```
    pub fn is_document(&self) -> bool {
        matches!(self, Self::Document(_))
    }

    /// Returns `true` if this content is a chat.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Chat};
    ///
    /// let chat = Chat::new();
    /// let content = Content::chat(chat);
    /// assert!(content.is_chat());
    /// ```
    pub fn is_chat(&self) -> bool {
        matches!(self, Self::Chat(_))
    }

    /// Returns the text content if this is text content.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::Content;
    ///
    /// let content = Content::text("hello");
    /// assert_eq!(content.as_text(), Some("hello"));
    /// ```
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the document content if this is document content.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Document};
    /// use bytes::Bytes;
    ///
    /// let doc = Document::new(Bytes::from("content"));
    /// let content = Content::document(doc.clone());
    /// assert_eq!(content.as_document(), Some(&doc));
    /// ```
    pub fn as_document(&self) -> Option<&Document> {
        match self {
            Self::Document(document) => Some(document),
            _ => None,
        }
    }

    /// Returns the chat content if this is chat content.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Chat};
    ///
    /// let chat = Chat::new();
    /// let content = Content::chat(chat.clone());
    /// assert_eq!(content.as_chat(), Some(&chat));
    /// ```
    pub fn as_chat(&self) -> Option<&Chat> {
        match self {
            Self::Chat(chat) => Some(chat),
            _ => None,
        }
    }

    /// Converts this content into text content if it is text.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::Content;
    ///
    /// let content = Content::text("hello");
    /// assert_eq!(content.into_text(), Some("hello".to_string()));
    /// ```
    pub fn into_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Converts this content into document content if it is a document.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Document};
    /// use bytes::Bytes;
    ///
    /// let doc = Document::new(Bytes::from("content"));
    /// let content = Content::document(doc.clone());
    /// assert_eq!(content.into_document(), Some(doc));
    /// ```
    pub fn into_document(self) -> Option<Document> {
        match self {
            Self::Document(document) => Some(document),
            _ => None,
        }
    }

    /// Converts this content into chat content if it is a chat.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::{Content, Chat};
    ///
    /// let chat = Chat::new();
    /// let content = Content::chat(chat.clone());
    /// assert_eq!(content.into_chat(), Some(chat));
    /// ```
    pub fn into_chat(self) -> Option<Chat> {
        match self {
            Self::Chat(chat) => Some(chat),
            _ => None,
        }
    }

    /// Estimates the size of this content in bytes.
    ///
    /// This is useful for batching and memory management. The size
    /// estimation includes the content data and reasonable estimates
    /// for metadata overhead.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::Content;
    ///
    /// let content = Content::text("hello");
    /// assert!(content.estimated_size() > 0);
    /// ```
    pub fn estimated_size(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Document(document) => document.data().len() + document.estimated_metadata_size(),
            Self::Chat(chat) => chat.estimated_size(),
        }
    }

    /// Returns a string representation suitable for display or logging.
    ///
    /// This method provides a concise summary of the content without
    /// exposing sensitive data in logs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_service::inference::Content;
    ///
    /// let content = Content::text("hello world");
    /// let display = content.display_summary();
    /// assert!(display.contains("text"));
    /// ```
    pub fn display_summary(&self) -> String {
        match self {
            Self::Text(text) => {
                if text.len() <= 50 {
                    format!("text({})", text)
                } else {
                    format!("text({}...)", &text[..47])
                }
            }
            Self::Document(document) => {
                format!(
                    "document({}, {} bytes)",
                    document.content_type().unwrap_or("unknown"),
                    document.data().len()
                )
            }
            Self::Chat(chat) => {
                format!("chat({} messages)", chat.message_count())
            }
        }
    }
}

impl From<&str> for Content {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_content_text() {
        let content = Content::text("hello world");
        assert!(content.is_text());
        assert!(!content.is_document());
        assert!(!content.is_chat());
        assert_eq!(content.as_text(), Some("hello world"));
    }

    #[test]
    fn test_content_document() {
        let doc = Document::new(Bytes::from("test content"));
        let content = Content::document(doc.clone());
        assert!(!content.is_text());
        assert!(content.is_document());
        assert!(!content.is_chat());
        assert_eq!(content.as_document(), Some(&doc));
    }

    #[test]
    fn test_content_chat() {
        let chat = Chat::new();
        let content = Content::chat(chat.clone());
        assert!(!content.is_text());
        assert!(!content.is_document());
        assert!(content.is_chat());
        assert_eq!(content.as_chat(), Some(&chat));
    }

    #[test]
    fn test_content_from_string() {
        let content: Content = "hello".into();
        assert!(content.is_text());
        assert_eq!(content.as_text(), Some("hello"));
    }

    #[test]
    fn test_content_from_document() {
        let doc = Document::new(Bytes::from("content"));
        let content: Content = doc.clone().into();
        assert!(content.is_document());
        assert_eq!(content.as_document(), Some(&doc));
    }

    #[test]
    fn test_content_estimated_size() {
        let content = Content::text("hello");
        assert_eq!(content.estimated_size(), 5);

        let doc = Document::new(Bytes::from("test content"));
        let content = Content::document(doc);
        assert!(content.estimated_size() >= 12); // At least the data size
    }

    #[test]
    fn test_content_display_summary() {
        let content = Content::text("hello world");
        let summary = content.display_summary();
        assert!(summary.contains("text"));
        assert!(summary.contains("hello world"));

        // Test truncation for long text
        let long_text = "a".repeat(100);
        let content = Content::text(long_text);
        let summary = content.display_summary();
        assert!(summary.len() < 60); // Should be truncated
        assert!(summary.contains("..."));
    }

    #[test]
    fn test_content_into_conversions() {
        let content = Content::text("hello");
        assert_eq!(content.into_text(), Some("hello".to_string()));

        let doc = Document::new(Bytes::from("content"));
        let content = Content::document(doc.clone());
        assert_eq!(content.into_document(), Some(doc));

        let chat = Chat::new();
        let content = Content::chat(chat.clone());
        assert_eq!(content.into_chat(), Some(chat));
    }
}
