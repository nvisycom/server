//! Idempotency-Key header extractor.
//!
//! Parses and validates the optional `Idempotency-Key` request header so a
//! client can safely retry a mutating request: a repeat with the same key
//! returns the original outcome instead of performing the action again.

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::http::HeaderName;
use axum::http::request::Parts;

use crate::response::{Error, ErrorKind};

/// The idempotency header name, lowercased to match `HeaderMap` lookup.
const IDEMPOTENCY_HEADER: HeaderName = HeaderName::from_static("idempotency-key");

/// Maximum accepted key length, mirroring the `idempotency_key` column bound.
const MAX_KEY_LENGTH: usize = 255;

/// The validated `Idempotency-Key` header, absent when the client omits it.
///
/// A present header must be a non-empty ASCII string of at most 255
/// characters; anything else rejects with `400 Bad Request` before the handler
/// runs.
#[must_use]
#[derive(Debug, Clone, Default)]
pub struct IdempotencyKey(pub Option<String>);

impl<S> FromRequestParts<S> for IdempotencyKey
where
    S: Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(value) = parts.headers.get(&IDEMPOTENCY_HEADER) else {
            return Ok(Self(None));
        };
        let key = value.to_str().map_err(|_| {
            ErrorKind::BadRequest.with_message("Idempotency-Key must be a valid ASCII string")
        })?;
        if key.is_empty() || key.len() > MAX_KEY_LENGTH {
            return Err(
                ErrorKind::BadRequest.with_message("Idempotency-Key must be 1 to 255 characters")
            );
        }
        Ok(Self(Some(key.to_owned())))
    }
}

impl OperationInput for IdempotencyKey {}

#[cfg(test)]
mod tests {
    use axum::extract::FromRequestParts;
    use axum::http::Request;

    use super::{IdempotencyKey, MAX_KEY_LENGTH};
    use crate::response::ErrorKind;

    /// Drives the extractor against a request carrying `header` (or none).
    async fn extract(header: Option<&str>) -> Result<Option<String>, ErrorKind> {
        let mut builder = Request::builder().uri("/");
        if let Some(value) = header {
            builder = builder.header("idempotency-key", value);
        }
        let (mut parts, ()) = builder.body(()).expect("request should build").into_parts();
        IdempotencyKey::from_request_parts(&mut parts, &())
            .await
            .map(|k| k.0)
            .map_err(|e| e.kind())
    }

    #[tokio::test]
    async fn an_absent_header_is_none() {
        assert_eq!(extract(None).await, Ok(None));
    }

    #[tokio::test]
    async fn a_present_header_is_carried_through() {
        assert_eq!(
            extract(Some("abc-123")).await,
            Ok(Some("abc-123".to_owned()))
        );
    }

    #[tokio::test]
    async fn an_empty_header_is_rejected() {
        assert_eq!(extract(Some("")).await, Err(ErrorKind::BadRequest));
    }

    #[tokio::test]
    async fn a_key_at_the_length_cap_is_accepted_but_one_over_is_rejected() {
        let at_cap = "k".repeat(MAX_KEY_LENGTH);
        assert_eq!(extract(Some(&at_cap)).await, Ok(Some(at_cap.clone())));

        let over_cap = "k".repeat(MAX_KEY_LENGTH + 1);
        assert_eq!(extract(Some(&over_cap)).await, Err(ErrorKind::BadRequest));
    }
}
