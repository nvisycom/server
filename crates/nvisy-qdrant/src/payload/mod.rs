//! Payload types and utilities for Qdrant collections.
//!
//! This module provides specialized payload structures for different collection types,
//! each optimized for their specific use cases and data requirements.
//!
//! - **annotation**: Payload types for annotation collections
//! - **conversation**: Payload types for conversation collections
//! - **document**: Payload types for document collections
//!
//! Each payload module contains point definitions, specialized types, and utilities
//! for converting between domain objects and Qdrant points.

pub mod annotation;
pub mod conversation;
pub mod document;

// Re-export annotation types
pub use annotation::{AnnotationCoordinates, AnnotationPoint, AnnotationType};
// Re-export conversation types
pub use conversation::{ConversationPoint, ConversationStatus, MessageType};
// Re-export document types
pub use document::{DocumentPoint, DocumentStatus, DocumentType};

/// Common payload field names used across collections.
pub mod fields {
    /// Standard metadata fields
    pub mod metadata {
        pub const CREATED_AT: &str = "created_at";
        pub const UPDATED_AT: &str = "updated_at";
        pub const VERSION: &str = "version";
        pub const ENTITY_TYPE: &str = "entity_type";
        pub const SOURCE: &str = "source";
    }

    /// Common content fields
    pub mod content {
        pub const CONTENT: &str = "content";
        pub const TITLE: &str = "title";
        pub const DESCRIPTION: &str = "description";
        pub const LANGUAGE: &str = "language";
        pub const TAGS: &str = "tags";
    }

    /// User and author fields
    pub mod users {
        pub const USER_ID: &str = "user_id";
        pub const AUTHOR_ID: &str = "author_id";
        pub const PARTICIPANT_ID: &str = "participant_id";
        pub const PARTICIPANT_ROLE: &str = "participant_role";
    }

    /// Document-specific fields
    pub mod document {
        pub const DOCUMENT_TYPE: &str = "document_type";
        pub const STATUS: &str = "status";
        pub const FILENAME: &str = "filename";
        pub const FILE_SIZE: &str = "file_size";
        pub const MIME_TYPE: &str = "mime_type";
        pub const SOURCE_URL: &str = "source_url";
        pub const PARENT_DOCUMENT_ID: &str = "parent_document_id";
        pub const CHUNK_NUMBER: &str = "chunk_number";
        pub const TOTAL_CHUNKS: &str = "total_chunks";
        pub const VERSION: &str = "version";
        pub const PREVIOUS_VERSION_ID: &str = "previous_version_id";
    }

    /// Conversation-specific fields
    pub mod conversation {
        pub const CONVERSATION_ID: &str = "conversation_id";
        pub const MESSAGE_TYPE: &str = "message_type";
        pub const SEQUENCE_NUMBER: &str = "sequence_number";
        pub const REPLY_TO: &str = "reply_to";
    }

    /// Annotation-specific fields
    pub mod annotation {
        pub const ANNOTATION_TYPE: &str = "annotation_type";
        pub const SOURCE_ID: &str = "source_id";
        pub const X: &str = "x";
        pub const Y: &str = "y";
        pub const WIDTH: &str = "width";
        pub const HEIGHT: &str = "height";
        pub const POINTS: &str = "points";
    }
}

/// Validation utilities for payload data.
pub mod validation {
    use crate::error::{QdrantError, QdrantResult};
    use crate::types::Payload;

    /// Validate that required fields are present in payload
    pub fn validate_required_fields(
        payload: &Payload,
        required_fields: &[&str],
    ) -> QdrantResult<()> {
        for field in required_fields {
            if !payload.contains_key(field) {
                return Err(QdrantError::PayloadError(format!(
                    "Missing required field: {}",
                    field
                )));
            }
        }
        Ok(())
    }

    /// Validate that a string field is not empty
    pub fn validate_non_empty_string(payload: &Payload, field_name: &str) -> QdrantResult<String> {
        let value = payload
            .get_string(field_name)
            .ok_or_else(|| QdrantError::PayloadError(format!("Missing field: {}", field_name)))?;

        if value.trim().is_empty() {
            return Err(QdrantError::PayloadError(format!(
                "Field {} cannot be empty",
                field_name
            )));
        }

        Ok(value.to_string())
    }

    /// Validate that a numeric field is within a valid range
    pub fn validate_numeric_range(
        payload: &Payload,
        field_name: &str,
        min: Option<i64>,
        max: Option<i64>,
    ) -> QdrantResult<i64> {
        let value = payload
            .get_i64(field_name)
            .ok_or_else(|| QdrantError::PayloadError(format!("Missing field: {}", field_name)))?;

        if let Some(min_val) = min {
            if value < min_val {
                return Err(QdrantError::PayloadError(format!(
                    "Field {} value {} is below minimum {}",
                    field_name, value, min_val
                )));
            }
        }

        if let Some(max_val) = max {
            if value > max_val {
                return Err(QdrantError::PayloadError(format!(
                    "Field {} value {} exceeds maximum {}",
                    field_name, value, max_val
                )));
            }
        }

        Ok(value)
    }

    /// Validate that an array field has a minimum number of elements
    pub fn validate_array_min_length(
        payload: &Payload,
        field_name: &str,
        min_length: usize,
    ) -> QdrantResult<Vec<serde_json::Value>> {
        let array = payload
            .get_array(field_name)
            .ok_or_else(|| QdrantError::PayloadError(format!("Missing field: {}", field_name)))?;

        if array.len() < min_length {
            return Err(QdrantError::PayloadError(format!(
                "Field {} must have at least {} elements, got {}",
                field_name,
                min_length,
                array.len()
            )));
        }

        Ok(array.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Payload;

    #[test]
    fn test_field_constants() {
        // Test that field constants are accessible
        assert_eq!(fields::metadata::CREATED_AT, "created_at");
        assert_eq!(fields::content::CONTENT, "content");
        assert_eq!(fields::users::USER_ID, "user_id");
        assert_eq!(fields::document::DOCUMENT_TYPE, "document_type");
        assert_eq!(fields::conversation::CONVERSATION_ID, "conversation_id");
        assert_eq!(fields::annotation::ANNOTATION_TYPE, "annotation_type");
    }

    #[test]
    fn test_validation_required_fields() {
        let payload = Payload::new()
            .with("field1", "value1")
            .with("field2", "value2");

        // Should pass with all required fields present
        assert!(validation::validate_required_fields(&payload, &["field1", "field2"]).is_ok());

        // Should fail with missing required field
        assert!(validation::validate_required_fields(&payload, &["field1", "missing"]).is_err());
    }

    #[test]
    fn test_validation_non_empty_string() {
        let payload = Payload::new()
            .with("valid", "non-empty")
            .with("empty", "")
            .with("whitespace", "   ");

        assert!(validation::validate_non_empty_string(&payload, "valid").is_ok());
        assert!(validation::validate_non_empty_string(&payload, "empty").is_err());
        assert!(validation::validate_non_empty_string(&payload, "whitespace").is_err());
        assert!(validation::validate_non_empty_string(&payload, "missing").is_err());
    }

    #[test]
    fn test_validation_numeric_range() {
        let payload = Payload::new().with("number", 50).with("negative", -10);

        // Valid range
        assert!(validation::validate_numeric_range(&payload, "number", Some(0), Some(100)).is_ok());

        // Below minimum
        assert!(validation::validate_numeric_range(&payload, "negative", Some(0), None).is_err());

        // Above maximum
        assert!(validation::validate_numeric_range(&payload, "number", None, Some(10)).is_err());
    }

    #[test]
    fn test_validation_array_min_length() {
        let payload = Payload::new()
            .with("tags", vec!["tag1", "tag2", "tag3"])
            .with("empty_array", Vec::<String>::new());

        // Valid length
        assert!(validation::validate_array_min_length(&payload, "tags", 2).is_ok());

        // Too short
        assert!(validation::validate_array_min_length(&payload, "empty_array", 1).is_err());

        // Missing field
        assert!(validation::validate_array_min_length(&payload, "missing", 0).is_err());
    }
}
