//! Enhanced Multipart extractor with improved error handling.
//!
//! This module provides [`Multipart`], an enhanced version of [`axum::extract::Multipart`]
//! with better error messages and proper error responses.

use axum::extract::multipart::MultipartRejection;
use axum::extract::{FromRequest, Multipart as AxumMultipart, Request};
use derive_more::{Deref, DerefMut, From};

use crate::handler::{Error, ErrorKind};

/// Enhanced Multipart extractor with improved error handling.
///
/// This extractor wraps the default Axum Multipart extractor and provides
/// better error messages for multipart form parsing failures.
#[must_use]
#[derive(Debug, Deref, DerefMut, From)]
pub struct Multipart(pub AxumMultipart);

impl Multipart {
    /// Returns the inner Axum Multipart extractor.
    #[inline]
    pub fn into_inner(self) -> AxumMultipart {
        self.0
    }
}

impl<S> FromRequest<S> for Multipart
where
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        AxumMultipart::from_request(req, state)
            .await
            .map(Multipart)
            .map_err(Into::into)
    }
}

impl From<MultipartRejection> for Error<'static> {
    fn from(rejection: MultipartRejection) -> Self {
        match rejection {
            MultipartRejection::InvalidBoundary(_) => ErrorKind::BadRequest
                .with_message("Invalid multipart boundary")
                .with_context(
                    "The Content-Type header is missing or has an invalid boundary parameter. \
                     Ensure the request uses 'multipart/form-data' with a valid boundary.",
                ),
            _ => ErrorKind::BadRequest
                .with_message("Invalid multipart request")
                .with_context(format!("Multipart parsing failed: {}", rejection)),
        }
    }
}

impl aide::OperationInput for Multipart {
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        AxumMultipart::operation_input(ctx, operation);
    }
}
