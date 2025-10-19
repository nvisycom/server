use std::any::Any;

use axum::response::{IntoResponse, Response};

use crate::handler::{Error, ErrorKind};

type Panic = Box<dyn Any + Send + 'static>;

/// Transforms any panic into the [`Error`] and then [`Response`].
pub fn catch_panic(err: Panic) -> Response {
    if let Some(panic) = err.downcast_ref::<String>() {
        tracing::error!(
            target: "server::otel",
            "service panic: {}", panic,
        );
    } else if let Some(panic) = err.downcast_ref::<&str>() {
        tracing::error!(
            target: "server::otel",
            "service panic: {}", panic,
        );
    } else if let Some(panic) = err.downcast_ref::<Error>() {
        tracing::error!(
            target: "server::otel",
            "service panic: {}", panic,
        );
    } else {
        tracing::error!(
            target: "server::otel",
            "service panic: unknown panic type",
        );
    }

    ErrorKind::InternalServerError.into_response()
}
