//! OpenAPI-documented wrapper around an [`Sse`] response.
//!
//! Axum's [`Sse`] does not implement aide's [`OperationOutput`], so a handler
//! returning it cannot be registered via `api_route`/`get_with` (no OpenAPI is
//! produced). [`SseResponse`] wraps it, delegating [`IntoResponse`] to the inner
//! stream while advertising a `200 text/event-stream` response to the schema.

use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::{MediaType, Operation, Response, StatusCode};
use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response as AxumResponse};

/// An [`Sse`] response that also produces OpenAPI documentation.
///
/// Wrap a handler's [`Sse`] in this so the route can be registered with
/// `api_route`/`get_with` and appear in the generated schema as a
/// `text/event-stream` endpoint.
#[must_use = "responses do nothing unless returned from a handler"]
pub struct SseResponse<S>(pub Sse<S>);

impl<S> IntoResponse for SseResponse<S>
where
    Sse<S>: IntoResponse,
{
    fn into_response(self) -> AxumResponse {
        self.0.into_response()
    }
}

impl<S> OperationOutput for SseResponse<S> {
    type Inner = Self;

    fn operation_response(_ctx: &mut GenContext, _operation: &mut Operation) -> Option<Response> {
        let mut response = Response {
            description: "Server-sent event stream".to_owned(),
            ..Default::default()
        };
        response
            .content
            .insert("text/event-stream".to_owned(), MediaType::default());
        Some(response)
    }

    fn inferred_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<StatusCode>, Response)> {
        match Self::operation_response(ctx, operation) {
            Some(response) => Vec::from([(Some(StatusCode::Code(200)), response)]),
            None => Vec::new(),
        }
    }
}
