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

pub use self::form_with_rej::Form;
pub use self::json_with_rej::Json;
pub use self::mutlipart_with_rej::Multipart;
pub use self::path_with_rej::Path;
pub use self::query_with_rej::Query;

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
            // Consume through the matching closing delimiter, treating a
            // backslash as escaping the next character so an escaped delimiter
            // (`\"`) inside the value does not end the span early and leak the
            // suffix that follows it.
            while let Some(inner) = chars.next() {
                match inner {
                    '\\' => {
                        // Skip the escaped character, whatever it is.
                        chars.next();
                    }
                    _ if inner == ch => break,
                    _ => {}
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

    #[test]
    fn an_escaped_delimiter_does_not_end_redaction_early() {
        // A submitted value containing an escaped quote must stay fully redacted:
        // the `\"` must not be treated as the closing delimiter, which would leak
        // the `secret` suffix that follows it.
        let message = r#"invalid value: string "a\"secret", expected an integer"#;
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("secret"), "leaked: {sanitized}");
        assert!(sanitized.contains("<redacted>"));
    }
}
