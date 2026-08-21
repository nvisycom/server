//! Shared helpers for file-download (attachment) responses.

use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue};

/// Builds the response headers for a downloadable attachment: a
/// `Content-Disposition: attachment` with `filename`, the `content_type`, and the
/// `content_length`.
///
/// `filename` goes into a quoted header value verbatim, so a caller passing a
/// user-supplied name must strip control characters, `"`, and `\` first; a
/// server-generated name (a UUID, ISO dates, a fixed stem) needs no sanitizing.
/// A name that still fails to parse falls back to a bare `attachment`.
pub fn attachment_headers(
    filename: &str,
    content_type: HeaderValue,
    content_length: u64,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let disposition = format!("attachment; filename=\"{filename}\"")
        .parse()
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    headers.insert(CONTENT_DISPOSITION, disposition);
    headers.insert(CONTENT_TYPE, content_type);
    headers.insert(CONTENT_LENGTH, HeaderValue::from(content_length));
    headers
}
