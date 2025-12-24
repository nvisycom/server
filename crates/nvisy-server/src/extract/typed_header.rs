//! Typed header extractor with aide OpenAPI compatibility.
//!
//! This module provides [`TypedHeader`], a wrapper around [`axum_extra::TypedHeader`]
//! that implements [`aide::OperationInput`] for OpenAPI documentation generation.

use axum::extract::FromRequestParts;
use derive_more::{Deref, DerefMut, From};

/// Typed header extractor with OpenAPI support.
///
/// This is a thin wrapper around [`axum_extra::TypedHeader`] that adds
/// [`aide::OperationInput`] implementation for OpenAPI schema generation.
/// It provides type-safe access to HTTP headers with automatic parsing.
///
/// # Extractable Headers
///
/// Any type implementing [`axum_extra::headers::Header`] can be extracted,
/// including standard headers like `Authorization`, `ContentType`, `Accept`, etc.
#[derive(Debug, Clone, Deref, DerefMut, From)]
pub struct TypedHeader<T>(pub T);

impl<S, T> FromRequestParts<S> for TypedHeader<T>
where
    S: Send + Sync,
    T: axum_extra::headers::Header,
{
    type Rejection = <axum_extra::TypedHeader<T> as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let axum_extra::TypedHeader(header) =
            axum_extra::TypedHeader::<T>::from_request_parts(parts, state).await?;
        Ok(Self(header))
    }
}

impl<T> aide::OperationInput for TypedHeader<T> {}
