//! Redaction request types.

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::redaction_categories::RedactionCategory;

/// A single data item that may need redaction.
///
/// This represents a piece of text that should be analyzed for sensitive information.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
pub struct RedactionItem {
    /// Unique identifier for this data item (UUID v4)
    #[builder(default = "Uuid::new_v4()")]
    pub id: Uuid,

    /// The text content that may contain sensitive data
    pub text: String,

    /// Optional entity this data belongs to (e.g., person name, organization)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub entity: Option<String>,

    /// Optional specific categories to look for in this item
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub categories: Option<Vec<RedactionCategory>>,
}

impl RedactionItem {
    /// Creates a new redaction item with a generated UUID.
    ///
    /// # Arguments
    ///
    /// * `text` - The text content to analyze
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionItem;
    ///
    /// let item = RedactionItem::new("123 Main St");
    /// ```
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            text: text.into(),
            entity: None,
            categories: None,
        }
    }

    /// Creates a new redaction item with a specific UUID.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID for this item
    /// * `text` - The text content to analyze
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionItem;
    /// use uuid::Uuid;
    ///
    /// let id = Uuid::new_v4();
    /// let item = RedactionItem::with_id(id, "123 Main St");
    /// ```
    pub fn with_id(id: Uuid, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            entity: None,
            categories: None,
        }
    }

    /// Sets the entity for this item.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity name
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionItem;
    ///
    /// let item = RedactionItem::new("123 Main St")
    ///     .with_entity("John Doe");
    /// ```
    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    /// Sets the categories to look for in this item.
    ///
    /// # Arguments
    ///
    /// * `categories` - Vector of categories to check
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::{RedactionItem, RedactionCategory};
    ///
    /// let item = RedactionItem::new("john@example.com, 555-1234")
    ///     .with_categories(vec![
    ///         RedactionCategory::EmailAddresses,
    ///         RedactionCategory::PhoneNumbers,
    ///     ]);
    /// ```
    pub fn with_categories(mut self, categories: Vec<RedactionCategory>) -> Self {
        self.categories = Some(categories);
        self
    }

    /// Returns a builder for creating a RedactionItem.
    pub fn builder() -> RedactionItemBuilder {
        RedactionItemBuilder::default()
    }
}

/// A complete redaction request containing data items and redaction criteria.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
pub struct RedactionRequest {
    /// List of data items to evaluate for redaction
    #[builder(default)]
    pub data: Vec<RedactionItem>,

    /// User prompt specifying what should be redacted
    pub prompt: String,

    /// Optional global categories to look for across all items
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub categories: Option<Vec<RedactionCategory>>,
}

impl RedactionRequest {
    /// Creates a new redaction request.
    ///
    /// # Arguments
    ///
    /// * `data` - List of items to analyze
    /// * `prompt` - Redaction criteria
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::{RedactionRequest, RedactionItem};
    ///
    /// let request = RedactionRequest::new(
    ///     vec![RedactionItem::new("123 Main St")],
    ///     "Redact all addresses"
    /// );
    /// ```
    pub fn new(data: Vec<RedactionItem>, prompt: impl Into<String>) -> Self {
        Self {
            data,
            prompt: prompt.into(),
            categories: None,
        }
    }

    /// Sets the global categories for this request.
    ///
    /// # Arguments
    ///
    /// * `categories` - Vector of categories to check across all items
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::{RedactionRequest, RedactionItem, RedactionCategory};
    ///
    /// let request = RedactionRequest::new(
    ///     vec![RedactionItem::new("john@example.com")],
    ///     "Find sensitive data"
    /// ).with_categories(vec![
    ///     RedactionCategory::EmailAddresses,
    ///     RedactionCategory::PhoneNumbers,
    /// ]);
    /// ```
    pub fn with_categories(mut self, categories: Vec<RedactionCategory>) -> Self {
        self.categories = Some(categories);
        self
    }

    /// Returns a builder for creating a RedactionRequest.
    pub fn builder() -> RedactionRequestBuilder {
        RedactionRequestBuilder::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction_item_new() {
        let item = RedactionItem::new("test text");
        assert_eq!(item.text, "test text");
        assert!(item.entity.is_none());
        assert!(item.categories.is_none());
    }

    #[test]
    fn test_redaction_item_with_entity() {
        let item = RedactionItem::new("test text").with_entity("John Doe");
        assert_eq!(item.text, "test text");
        assert_eq!(item.entity.as_deref(), Some("John Doe"));
    }

    #[test]
    fn test_redaction_item_with_categories() {
        let item = RedactionItem::new("john@example.com")
            .with_categories(vec![RedactionCategory::EmailAddresses]);
        assert_eq!(item.text, "john@example.com");
        assert_eq!(item.categories.as_ref().unwrap().len(), 1);
        assert_eq!(
            item.categories.as_ref().unwrap()[0],
            RedactionCategory::EmailAddresses
        );
    }

    #[test]
    fn test_redaction_request_new() {
        let items = vec![RedactionItem::new("text")];
        let request = RedactionRequest::new(items, "test prompt");

        assert_eq!(request.data.len(), 1);
        assert_eq!(request.prompt, "test prompt");
        assert!(request.categories.is_none());
    }

    #[test]
    fn test_redaction_request_with_categories() {
        let items = vec![RedactionItem::new("text")];
        let request = RedactionRequest::new(items, "test prompt")
            .with_categories(vec![RedactionCategory::EmailAddresses]);

        assert_eq!(request.categories.as_ref().unwrap().len(), 1);
    }
}
