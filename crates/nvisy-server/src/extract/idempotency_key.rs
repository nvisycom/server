//! Idempotency-Key header extractor.
//!
//! Parses and validates the optional `Idempotency-Key` request header so a
//! client can safely retry a mutating request: a repeat with the same key
//! returns the original outcome instead of performing the action again.

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::http::HeaderName;
use axum::http::request::Parts;

use crate::handler::{Error, ErrorKind};

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

impl IdempotencyKey {
    /// Returns the key as a string slice, if one was supplied.
    #[inline]
    #[must_use]
    pub fn as_deref(&self) -> Option<&str> {
        self.0.as_deref()
    }

    /// Consumes the extractor, returning the owned optional key.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> Option<String> {
        self.0
    }
}

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
