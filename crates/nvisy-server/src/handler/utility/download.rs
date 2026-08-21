//! Shared helpers for file-download (attachment) responses.

use aide::openapi::MediaType;
use aide::transform::TransformOperation;
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

/// Documents downloadable (raw-body) responses on an operation.
pub trait DownloadResponseExt<'a> {
    /// Documents a `200` download response that carries a body under
    /// `content_types`.
    ///
    /// A download handler returns raw bytes (`(StatusCode, HeaderMap, Body)`),
    /// which aide cannot introspect, so without this the generated spec records an
    /// empty `200` (`content: never`) and a client's low-level contract omits the
    /// downloadable body. This declares each media type the endpoint can return,
    /// so the body is present in the spec. The bodies are opaque (CSV, a zip, a
    /// stream), so the media types carry no schema.
    fn download_response(self, description: &str, content_types: &[&str]) -> Self;
}

impl<'a> DownloadResponseExt<'a> for TransformOperation<'a> {
    fn download_response(self, description: &str, content_types: &[&str]) -> Self {
        self.response_with::<200, (), _>(|mut res| {
            res.inner().description = description.to_owned();
            for content_type in content_types {
                res.inner()
                    .content
                    .insert((*content_type).to_owned(), MediaType::default());
            }
            res
        })
    }
}
