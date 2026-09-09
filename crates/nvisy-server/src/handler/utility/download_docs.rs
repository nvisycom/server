//! OpenAPI documentation helper for download (raw-body) responses.

use aide::openapi::MediaType;
use aide::transform::TransformOperation;

/// Documents downloadable (raw-body) responses on an operation.
pub trait DownloadDocs<'a> {
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

impl<'a> DownloadDocs<'a> for TransformOperation<'a> {
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
