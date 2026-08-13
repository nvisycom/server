//! The [`Avatar`] type used by both ends of the avatar routes: extracted from an
//! upload request and returned as the served image.

use aide::generate::GenContext;
use aide::openapi::{MediaType, Operation, Response as OpenApiResponse};
use aide::{OperationInput, OperationOutput};
use axum::body::Body;
use axum::extract::{FromRequest, Request};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::extract::Multipart;
use crate::handler::{Error, ErrorKind};
use crate::service::AVATAR_CONTENT_TYPE;

/// Raw avatar image bytes, shared by the upload and serve directions.
///
/// As a request extractor it reads the first file field of a multipart upload
/// into memory. As a response it serves the bytes with the WebP content type and
/// an immutable cache header: the serve URL carries a content hash, so a given
/// URL always maps to the same bytes and may be cached indefinitely; a new
/// upload changes the URL rather than the contents at a URL.
#[must_use]
pub struct Avatar(pub Vec<u8>);

impl<S> FromRequest<S> for Avatar
where
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut multipart = Multipart::from_request(req, state).await?.into_inner();

        while let Some(field) = multipart.next_field().await.map_err(|err| {
            ErrorKind::BadRequest
                .with_message("Invalid multipart data")
                .with_context(err.to_string())
        })? {
            if field.file_name().is_none() {
                continue;
            }
            let bytes = field.bytes().await.map_err(|err| {
                ErrorKind::BadRequest
                    .with_message("Failed to read uploaded image")
                    .with_context(err.to_string())
            })?;
            return Ok(Avatar(bytes.to_vec()));
        }

        Err(ErrorKind::BadRequest.with_message("No image file in upload"))
    }
}

impl OperationInput for Avatar {
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Multipart::operation_input(ctx, operation);
    }
}

impl IntoResponse for Avatar {
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

impl OperationOutput for Avatar {
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
