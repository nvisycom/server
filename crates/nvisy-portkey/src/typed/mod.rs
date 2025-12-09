//! Typed serialization and deserialization support for JSON and TOON formats.
//!
//! This module provides unified parsing and serialization capabilities for both
//! JSON and TOON (Token-Oriented Object Notation) formats, optimized for LLM responses.
//!
//! # Features
//!
//! - Support for both JSON and TOON serialization formats
//! - Automatic format detection and fallback
//! - Robust parsing of LLM responses with markdown code blocks
//! - Extraction of structured data from embedded text
//! - Configurable parsing behavior
//!
//! # Examples
//!
//! ## Basic JSON Parsing
//!
//! ```rust
//! use nvisy_openrouter::typed::parse_json_response;
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
//!
//! ## TOON Format Parsing
//!
//! ```rust
//! use nvisy_openrouter::typed::parse_toon_response;
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct User {
//! #     id: u32,
//! #     name: String,
//! # }
//!
//! let toon = "id: 123\nname: Alice";
//! let user: User = parse_toon_response(toon).unwrap();
//! ```
//!
//! ## Configurable Parsing with Fallback
//!
//! ```rust
//! use nvisy_openrouter::typed::{parse_response_content, ParseConfig, SerializationFormat};
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct User {
//! #     id: u32,
//! #     name: String,
//! # }
//!
//! let config = ParseConfig::new()
//!     .with_preferred_format(SerializationFormat::Toon)
//!     .with_fallback(true);
//!
//! // This will try TOON first, then fall back to JSON if that fails
//! let response = r#"{"id": 123, "name": "Alice"}"#;
//! let user: User = parse_response_content(response, &config).unwrap();
//! ```

// Module declarations
pub mod common;
pub mod deserialization;
pub mod deserialization_utils;
pub mod serialization;

// Re-export main configuration types
pub use common::ParseConfig;
// Re-export format types
pub use common::{MarkdownLanguage, SerializationFormat};
// Re-export extraction types and functions
pub use deserialization::{
    MarkdownExtractResult, extract_json_object, extract_response_content,
    extract_structured_content, extract_toon_data, parse_json_response,
    parse_json_with_toon_fallback, parse_response_content, parse_toon_response,
    parse_toon_with_json_fallback,
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

        // Test TOON parsing
        let toon = "id: 2\nname: Toon\nactive: false";
        let user: TestUser = parse_toon_response(toon).unwrap();
        assert_eq!(user.id, 2);
        assert_eq!(user.name, "Toon");

        // Test configurable parsing
        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Json)
            .with_fallback(true);
        let response = "id: 3\nname: Config\nactive: true";
        let user: TestUser = parse_response_content(response, &config).unwrap();
        assert_eq!(user.id, 3);
        assert_eq!(user.name, "Config");
    }

    #[test]
    fn test_markdown_extraction_export() {
        let markdown = "```json\n{\"id\": 4, \"name\": \"Markdown\", \"active\": true}\n```";
        let result = MarkdownExtractResult::from_markdown(markdown).unwrap();
        assert_eq!(result.detected_format, Some(SerializationFormat::Json));
        assert!(result.content.contains("Markdown"));
    }
}
