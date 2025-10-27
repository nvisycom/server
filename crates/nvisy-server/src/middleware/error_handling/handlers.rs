use std::future::ready;

use axum::response::{IntoResponse, Response};
use futures::future::{BoxFuture, FutureExt};

use crate::handler::{Error, ErrorKind};

type ResponseFut = BoxFuture<'static, Response>;

/// Transforms any known [`tower::BoxError`] into a custom [`Error`] response.
///
/// This function attempts to downcast known error types and provide appropriate
/// error responses with context. For unknown errors, it returns a generic
/// internal server error.
pub fn handle_error(err: tower::BoxError) -> ResponseFut {
    use axum_client_ip::Rejection as IpRejection;
    use tower::timeout::error::Elapsed;

    let error = if let Some(_elapsed) = err.downcast_ref::<Elapsed>() {
        tracing::error!(
            target: "server::middleware::error",
            error = %err,
            "Request timeout exceeded"
        );

        Error::new(ErrorKind::InternalServerError)
            .with_message("Request timeout")
            .with_context("The request took too long to process and was terminated")
    } else if let Some(_ip_rejection) = err.downcast_ref::<IpRejection>() {
        tracing::error!(
            target: "server::middleware::error",
            error = %err,
            "Failed to extract client IP address"
        );

        Error::new(ErrorKind::InternalServerError)
            .with_message("IP address extraction failed")
            .with_context("Could not determine client IP address")
    } else {
        tracing::error!(
            target: "server::middleware::error",
            error = %err,
            "Unknown middleware error"
        );

        Error::new(ErrorKind::InternalServerError)
            .with_message("An unexpected error occurred")
            .with_context(err.to_string())
    };

    ready(error.into_response()).boxed()
}
