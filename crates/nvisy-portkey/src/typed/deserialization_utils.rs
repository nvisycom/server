//! Utility functions for deserialization error handling.

use crate::Error;

/// Creates a standardized parsing error with context.
///
/// # Examples
///
/// ```rust
/// # use nvisy_portkey::typed::deserialization_utils::create_parsing_error;
/// let error = create_parsing_error("Invalid JSON structure", r#"{"invalid"#);
/// assert!(error.to_string().contains("Invalid JSON"));
/// ```
pub fn create_parsing_error(message: &str, content_preview: &str) -> Error {
    let preview = if content_preview.len() > 100 {
        format!("{}...", &content_preview[..100])
    } else {
        content_preview.to_string()
    };

    Error::invalid_response(format!("{message}. Content preview: {preview}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_parsing_error() {
        let error = create_parsing_error("Test error", "short content");
        assert!(error.to_string().contains("Test error"));
        assert!(error.to_string().contains("short content"));
    }

    #[test]
    fn test_create_parsing_error_truncates() {
        let long_content = "a".repeat(200);
        let error = create_parsing_error("Test error", &long_content);
        let error_str = error.to_string();
        assert!(error_str.len() < long_content.len());
        assert!(error_str.contains("..."));
    }
}
