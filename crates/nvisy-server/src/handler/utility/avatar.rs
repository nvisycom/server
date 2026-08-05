//! Shared avatar request/response helpers for the account and workspace handlers.

use axum::body::Body;
use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::handler::{ErrorKind, Result};
use crate::service::AVATAR_CONTENT_TYPE;

/// Reads the first file field of a multipart upload into memory as the raw image
/// bytes. Rejects a body with no file field.
pub async fn read_image_field(mut multipart: Multipart) -> Result<Vec<u8>> {
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
        return Ok(bytes.to_vec());
    }

    Err(ErrorKind::BadRequest.with_message("No image file in upload"))
}

/// Builds the serve response for stored avatar bytes: WebP content type plus an
/// immutable cache header.
///
/// The serve URL carries a content hash, so a given URL always maps to the same
/// bytes and may be cached indefinitely; a new upload changes the URL rather
/// than the contents at a URL.
pub fn avatar_response(bytes: Vec<u8>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, AVATAR_CONTENT_TYPE.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=31536000, immutable".parse().unwrap(),
    );
    (StatusCode::OK, headers, Body::from(bytes)).into_response()
}
