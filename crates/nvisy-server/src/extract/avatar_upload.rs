//! The [`AvatarUpload`] extractor: the image bytes of a multipart avatar upload.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::Operation;
use axum::extract::{FromRequest, Request};
use bytes::Bytes;

use crate::extract::Multipart;
use crate::response::{Error, ErrorKind};

/// The raw bytes of an uploaded avatar image, read from the first file field of a
/// multipart request.
///
/// Holds [`Bytes`] rather than `Vec<u8>` so the multipart field's buffer is
/// carried through to image processing without a copy.
#[must_use]
pub struct AvatarUpload(pub Bytes);

impl<S> FromRequest<S> for AvatarUpload
where
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Multipart(mut multipart) = Multipart::from_request(req, state).await?;

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
            return Ok(AvatarUpload(bytes));
        }

        Err(ErrorKind::BadRequest.with_message("No image file in upload"))
    }
}

impl OperationInput for AvatarUpload {
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Multipart::operation_input(ctx, operation);
    }
}
