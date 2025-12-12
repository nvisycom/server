//! Typed serialization and deserialization support for JSON format.
//!
//! This module provides parsing and serialization capabilities for JSON format,
//! optimized for LLM responses.
//!
//! # Features
//!
//! - JSON serialization and deserialization
//! - Robust parsing of LLM responses with markdown code blocks
//! - Extraction of structured data from embedded text
//!
//! # Examples
//!
//! ## Basic JSON Parsing
//!
//! ```rust
//! use nvisy_portkey::typed::parse_json_response;
//! use serde::{Deserialize, Serialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct User {
//!     id: u32,
//!     name: String,
//! }
//!
//! let json = r#"{"id": 123, "name": "Alice"}"#;
//! let user: User = parse_json_response(json).unwrap();
//! ```

// Module declarations
pub mod common;
pub mod deserialization;
pub mod deserialization_utils;
pub mod serialization;

// Re-export format types
pub use common::MarkdownLanguage;
// Re-export extraction types and functions
pub use deserialization::{
    MarkdownExtractResult, extract_json_object, extract_response_content, parse_json_response,
    parse_response_content,
};

#[cfg(test)]
mod integration_tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct TestUser {
        id: u32,
        name: String,
        active: bool,
    }

    #[test]
    fn test_exported_functions_work() {
        // Test JSON parsing
        let json = r#"{"id": 1, "name": "Test", "active": true}"#;
        let user: TestUser = parse_json_response(json).unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "Test");

        // Test response content parsing
        let response = r#"{"id": 3, "name": "Config", "active": true}"#;
        let user: TestUser = parse_response_content(response).unwrap();
        assert_eq!(user.id, 3);
        assert_eq!(user.name, "Config");
    }

    #[test]
    fn test_markdown_extraction_export() {
        let markdown = "```json\n{\"id\": 4, \"name\": \"Markdown\", \"active\": true}\n```";
        let result = MarkdownExtractResult::from_markdown(markdown).unwrap();
        assert!(result.content.contains("Markdown"));
    }
}
