//! Wrapper around [`axum_extra::TypedHeader`] for aide compatibility.
//!
//! This module provides a wrapper type that implements [`aide::OperationInput`]
//! to enable usage with aide's OpenAPI generation.

use axum::extract::FromRequestParts;
use derive_more::{Deref, DerefMut, From};

/// Wrapper around [`axum_extra::TypedHeader`] that implements [`aide::OperationInput`].
///
/// This allows the extractor to be used with aide's OpenAPI generation.
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
