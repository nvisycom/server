//! Enhanced request extractors with improved error handling and validation.
//!
//! This module provides custom Axum extractors that enhance the default functionality
//! with better error messages, validation, and type safety. These extractors are
//! designed to be drop-in replacements for their standard Axum counterparts while
//! providing additional features like detailed error context and automatic validation.

mod form_with_rej;
mod json_with_rej;
mod mutlipart_with_rej;
mod path_with_rej;
mod query_with_rej;
mod validated_json;

pub use self::form_with_rej::Form;
pub use self::json_with_rej::Json;
pub use self::mutlipart_with_rej::Multipart;
pub use self::path_with_rej::Path;
pub use self::query_with_rej::Query;
pub use self::validated_json::ValidateJson;

/// Sanitizes a deserializer error message before it is surfaced or logged.
///
/// Deserializer errors can echo submitted values (e.g. `invalid value:
/// string "user@example.com"`), which may be personal data on credential
/// routes. Quoted and backtick-quoted spans are replaced with a redaction
/// marker, and the result is capped in length.
pub(super) fn sanitize_error_message(message: &str) -> String {
    let redacted = redact_quoted(message);
    redacted
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

/// Replaces `"..."` and `` `...` `` spans with `<redacted>`, stripping any
/// submitted values a deserializer embedded in its message.
fn redact_quoted(message: &str) -> String {
    let mut output = String::with_capacity(message.len());
    let mut chars = message.chars();
    while let Some(ch) = chars.next() {
        if ch == '"' || ch == '`' {
            output.push_str("<redacted>");
            // Consume through the matching closing delimiter, if any.
            for inner in chars.by_ref() {
                if inner == ch {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::sanitize_error_message;

    #[test]
    fn redacts_submitted_values() {
        let message = r#"invalid value: string "user@example.com", expected an integer"#;
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("user@example.com"));
        assert!(sanitized.contains("<redacted>"));
    }

    #[test]
    fn redacts_backtick_field_values() {
        let message = "unknown field `secret_token`, expected one of ...";
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("secret_token"));
        assert!(sanitized.contains("<redacted>"));
    }

    #[test]
    fn caps_length() {
        let message = "x".repeat(500);
        assert_eq!(sanitize_error_message(&message).chars().count(), 200);
    }
}
