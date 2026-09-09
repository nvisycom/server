//! Response headers for file-download (attachment) responses.

use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue};

/// Builds the response headers for a downloadable attachment: a
/// `Content-Disposition: attachment` naming `filename`, the `content_type`, and
/// the `content_length`.
///
/// The name is handled safely regardless of its contents — the caller does not
/// need to pre-sanitize it:
/// - A plain-ASCII name is quoted with `"` and `\` escaped, so it cannot inject
///   extra `Content-Disposition` parameters.
/// - A name with non-ASCII characters is additionally emitted as an RFC 6266
///   `filename*=UTF-8''…` parameter (percent-encoded), so the name survives
///   rather than being dropped (a raw non-ASCII byte cannot go in a header value).
///   A plain `filename=` fallback (non-ASCII stripped) is kept for old clients.
pub fn attachment_headers(
    filename: &str,
    content_type: HeaderValue,
    content_length: u64,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let disposition = HeaderValue::from_str(&content_disposition(filename))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    headers.insert(CONTENT_DISPOSITION, disposition);
    headers.insert(CONTENT_TYPE, content_type);
    headers.insert(CONTENT_LENGTH, HeaderValue::from(content_length));
    headers
}

/// Renders the `Content-Disposition` value for an attachment named `filename`.
///
/// Always includes a quoted, escaped `filename=` (ASCII-only, for every client);
/// adds an RFC 6266 `filename*=UTF-8''…` when the name has non-ASCII characters,
/// so a modern client recovers the original name.
fn content_disposition(filename: &str) -> String {
    // Quoted ASCII form: escape `"` and `\`, and drop control chars and any
    // non-ASCII (the latter is carried by `filename*` below when present).
    let ascii: String = filename
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '"' => "\\\"".to_owned(),
            '\\' => "\\\\".to_owned(),
            c if c.is_ascii() => c.to_string(),
            _ => String::new(),
        })
        .collect();

    if filename.is_ascii() {
        format!("attachment; filename=\"{ascii}\"")
    } else {
        format!(
            "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
            percent_encode_rfc5987(filename)
        )
    }
}

/// Percent-encodes `value` for an RFC 5987 `ext-value` (used by RFC 6266
/// `filename*`): unreserved characters pass through, everything else is
/// `%HH`-encoded from its UTF-8 bytes.
fn percent_encode_rfc5987(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        // RFC 5987 `attr-char`: ALPHA / DIGIT and a fixed set of symbols.
        let unreserved = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            );
        if unreserved {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::content_disposition;

    #[test]
    fn ascii_name_is_quoted() {
        assert_eq!(
            content_disposition("report.csv"),
            r#"attachment; filename="report.csv""#
        );
    }

    #[test]
    fn quotes_and_backslashes_are_escaped() {
        // A name that tries to inject a second parameter is neutralized: the `"`
        // and `\` are escaped, so it stays inside the quoted value.
        assert_eq!(
            content_disposition(r#"a".pdf"#),
            r#"attachment; filename="a\".pdf""#
        );
        assert_eq!(
            content_disposition(r"a\b.pdf"),
            r#"attachment; filename="a\\b.pdf""#
        );
    }

    #[test]
    fn non_ascii_name_gets_a_filename_star() {
        // The name survives via `filename*` (percent-encoded UTF-8); the quoted
        // `filename=` keeps the ASCII remainder for old clients.
        let out = content_disposition("résumé.pdf");
        assert!(out.contains("filename=\"rsum.pdf\""), "{out}");
        assert!(
            out.contains("filename*=UTF-8''r%C3%A9sum%C3%A9.pdf"),
            "{out}"
        );
    }
}
