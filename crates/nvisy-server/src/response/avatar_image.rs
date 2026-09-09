//! The [`AvatarImage`] response: a served avatar image.

use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::{MediaType, Operation, Response as OpenApiResponse};
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::service::AVATAR_CONTENT_TYPE;

/// A stored avatar image, served with the WebP content type and an immutable
/// cache header.
///
/// The serve URL carries a content hash, so a given URL always maps to the same
/// bytes and may be cached indefinitely; a new upload changes the URL rather than
/// the contents at a URL. The bytes come straight from the blob store, so this
/// holds a `Vec<u8>` (which becomes the response body without a copy).
#[must_use]
pub struct AvatarImage(pub Vec<u8>);

impl IntoResponse for AvatarImage {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(AVATAR_CONTENT_TYPE),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ),
            ],
            Body::from(self.0),
        )
            .into_response()
    }
}

impl OperationOutput for AvatarImage {
    type Inner = Self;

    fn operation_response(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Option<OpenApiResponse> {
        let mut response = OpenApiResponse {
            description: "The owner's avatar image.".to_owned(),
            ..Default::default()
        };
        response
            .content
            .insert(AVATAR_CONTENT_TYPE.to_owned(), MediaType::default());
        Some(response)
    }
}
